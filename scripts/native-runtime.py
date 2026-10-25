"""Pinned native runtime fetcher. Materializes only archive-local regular files/safe links."""
import hashlib
import io
import json
import pathlib
import sys
import tarfile
import urllib.request

root = pathlib.Path(__file__).resolve().parent.parent
rid, destination = sys.argv[1:]
asset = json.loads((root / "scripts/native-assets.json").read_text())[rid]
archive = root / ".build/native-download" / asset["file"]
archive.parent.mkdir(parents=True, exist_ok=True)
if not archive.exists():
    with urllib.request.urlopen("https://github.com/handy-computer/transcribe.cpp/releases/download/v0.4.1/" + asset["file"], timeout=120) as response:
        archive.write_bytes(response.read())
data = archive.read_bytes()
if hashlib.sha256(data).hexdigest() != asset["sha256"]:
    raise RuntimeError("Native runtime checksum mismatch")
target = pathlib.Path(destination).resolve()
target.mkdir(parents=True, exist_ok=True)
with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as source:
    for member in source.getmembers():
        parts = pathlib.PurePosixPath(member.name).parts
        if not parts or member.name.startswith("/") or ".." in parts or "\\" in member.name:
            raise RuntimeError("Unsafe archive path")
        if len(parts) < 2 or member.isdir():
            continue
        if not (member.isfile() or member.issym() or member.islnk()):
            raise RuntimeError("Unexpected archive entry")
        if (member.issym() or member.islnk()) and (member.linkname.startswith("/") or ".." in pathlib.PurePosixPath(member.linkname).parts):
            raise RuntimeError("Unsafe archive link")
        output = target.joinpath(*parts[1:])
        if not output.resolve().is_relative_to(target):
            raise RuntimeError("Archive escapes destination")
        output.parent.mkdir(parents=True, exist_ok=True)
        handle = source.extractfile(member)
        if handle is None:
            raise RuntimeError("Missing archive content")
        output.write_bytes(handle.read())
        output.chmod(member.mode & 0o777)
print(target)
