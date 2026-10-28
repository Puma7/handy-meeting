"""Prepare only reviewed transcribe.cpp SDK files; no model or user-data access."""
import argparse
import hashlib
import http.client
import json
import pathlib
import re
import ssl
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
TARGETS = {"linux-x64": "x86_64-unknown-linux-gnu", "osx-arm64": "aarch64-apple-darwin", "osx-x64": "x86_64-apple-darwin"}
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024


def digest(data):
    return hashlib.sha256(data).hexdigest()


def put(file, data):
    """Existing SDK files must already be identical; never overwrite a different SDK."""
    if file.exists():
        if file.is_symlink() or not file.is_file() or digest(file.read_bytes()) != digest(data):
            raise RuntimeError(f"Existing SDK file differs: {file.name}; choose a new SDK directory")
        return
    file.parent.mkdir(parents=True, exist_ok=True)
    with file.open("xb") as output:
        output.write(data)


def download(asset):
    """Retry only transient transport failures; validate complete bytes before caching."""
    request = urllib.request.Request("https://github.com/handy-computer/transcribe.cpp/releases/download/v0.4.1/" + asset["file"], headers={"User-Agent": "HandyMeeting-SDK-check"})
    for attempt in range(1, 4):
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                length = response.getheader("Content-Length") if hasattr(response, "getheader") and not getattr(response, "chunked", False) else None
                expected = int(length) if length is not None else None
                if expected is not None and (expected < 0 or expected > MAX_ARCHIVE_BYTES):
                    raise RuntimeError("Official native archive exceeds its limit or has an invalid content length")
                data = response.read(MAX_ARCHIVE_BYTES + 1)
                # A bounded stdlib read can return early at EOF without raising.
                if expected is not None and len(data) < expected:
                    raise http.client.IncompleteRead(data, expected - len(data))
        except (urllib.error.URLError, TimeoutError, ConnectionError, http.client.IncompleteRead) as error:
            if isinstance(error, urllib.error.HTTPError) and error.code not in {408, 429, 500, 502, 503, 504}:
                raise
            if isinstance(getattr(error, "reason", error), ssl.SSLCertVerificationError):
                raise
            if attempt == 3:
                raise RuntimeError("Native SDK download failed after 3 attempts; retry the job when the network is available. Existing files are preserved.") from error
            print(f"Native SDK transport failed ({type(error).__name__}); retry {attempt + 1}/3 in {attempt}s.", file=sys.stderr)
            time.sleep(attempt)
            continue
        if len(data) > MAX_ARCHIVE_BYTES or digest(data) != asset["sha256"]:
            raise RuntimeError("Official native archive exceeds its limit or differs from its fixed checksum")
        return data
    raise AssertionError("Unreachable download attempt")


def prepare(rid):
    asset = json.loads((ROOT / "scripts/native-assets.json").read_text())[rid]
    archive = ROOT / ".build/native-download" / asset["file"]
    archive.parent.mkdir(parents=True, exist_ok=True)
    if not archive.exists():
        put(archive, download(asset))
    if archive.is_symlink() or digest(archive.read_bytes()) != asset["sha256"]:
        raise RuntimeError("Cached native archive differs from its fixed checksum")
    sdk = ROOT / f".build/rust-native-beta2-0.4.1-{rid}"
    libraries = []
    contract = None
    seen = set()
    with tarfile.open(archive, "r:gz") as source:
        for member in source.getmembers():
            path = pathlib.PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or "\\" in member.name:
                raise RuntimeError("Unsafe native archive path")
            if member.isdir():
                continue
            if not member.isfile():
                # Release archives contain regular files today; unexpected links need review.
                raise RuntimeError("Unexpected non-regular native archive entry")
            relative = pathlib.PurePosixPath(*path.parts[1:])
            if not relative.parts or str(relative) in seen or member.size > 128 * 1024 * 1024:
                raise RuntimeError("Unexpected native archive entry")
            seen.add(str(relative))
            data = source.extractfile(member).read()
            if relative.name == "contract.json":
                contract = json.loads(data)
                put(sdk / "contract.json", data)
            elif len(relative.parts) == 1 and (".so" in relative.name or relative.suffix == ".dylib"):
                put(sdk / "lib" / relative.name, data)
                libraries.append({"Name": relative.name, "SHA256": digest(data)})
            elif relative.parts[0] == "licenses":
                put(sdk / relative, data)
            else:
                raise RuntimeError(f"Unreviewed native archive payload: {relative}")
    if not contract or contract.get("version") != "0.4.1" or contract.get("header_hash") != "f99ad4169374b8f3" or not libraries:
        raise RuntimeError("Native SDK version or ABI contract differs")
    expected = "libtranscribe.so" if rid == "linux-x64" else "libtranscribe.dylib"
    if expected not in {item["Name"] for item in libraries}:
        raise RuntimeError("Missing native recognition library")
    actual = {file.name for file in (sdk / "lib").iterdir() if file.name != "transcribe-link.json"}
    if actual != {item["Name"] for item in libraries}:
        raise RuntimeError("SDK contains unreviewed library files")
    (sdk / "include").mkdir(exist_ok=True)
    link = {"shared": True, "include_dir": "include", "lib_dir": "lib", "module_dir": "lib", "libraries": ["transcribe"], "system_libs": [], "library_paths": [], "frameworks": [], "link_flags": []}
    put(sdk / "lib/transcribe-link.json", (json.dumps(link, indent=2) + "\n").encode())
    provenance = {"Version": "0.4.1", "Target": TARGETS[rid], "RID": rid, "ArchiveSHA256": asset["sha256"], "Files": sorted(libraries, key=lambda item: item["Name"]), "Source": "Official hash-pinned transcribe.cpp release"}
    put(sdk / "SDK_PROVENANCE.json", (json.dumps(provenance, indent=2) + "\n").encode())
    print(sdk)


def check_abi(rid):
    """Run the platform's standard binary inspector, preserving its actual output."""
    sdk = ROOT / f".build/rust-native-beta2-0.4.1-{rid}"
    report = {}
    for file in sorted((sdk / "lib").iterdir()):
        if file.suffix == ".json":
            continue
        if rid == "linux-x64":
            output = subprocess.run(["readelf", "--version-info", str(file)], check=True, text=True, capture_output=True).stdout
            values = {}
            for kind, limit in [("GLIBC", (2, 35)), ("GLIBCXX", (3, 4, 30)), ("CXXABI", (1, 3, 13))]:
                versions = [tuple(map(int, value.split("."))) for value in re.findall(kind + r"_([0-9]+(?:\.[0-9]+)+)", output)]
                maximum = max(versions, default=(0,))
                if maximum > limit:
                    raise RuntimeError(f"{file.name} exceeds the Ubuntu22/Debian12 ABI baseline: {kind}_{maximum}")
                values[kind] = ".".join(map(str, maximum))
            report[file.name] = values
        else:
            output = subprocess.run(["otool", "-l", str(file)], check=True, text=True, capture_output=True).stdout
            versions = re.findall(r"\bminos\s+([0-9.]+)", output)
            if not versions or any(tuple(map(int, value.split("."))) > (13, 0, 0) for value in versions):
                raise RuntimeError(f"{file.name} exceeds macOS13 deployment target or has no build-version record")
            report[file.name] = {"macOSMinimum": versions}
    (sdk / "ABI_REPORT.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid", choices=TARGETS)
    parser.add_argument("--check-abi", action="store_true")
    args = parser.parse_args()
    check_abi(args.rid) if args.check_abi else prepare(args.rid)
