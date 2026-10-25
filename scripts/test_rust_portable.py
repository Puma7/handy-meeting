"""Synthetic filesystem guards only; no native execution, devices, or downloads."""
import contextlib
import hashlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).parent / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


artifacts = load("portable_artifacts", "rust-portable-artifacts.py")
prepare = load("portable_sdk", "prepare-rust-portable.py")


class PortableArtifactGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.previous = artifacts.ROOT
        artifacts.ROOT = self.root

    def tearDown(self):
        artifacts.ROOT = self.previous
        self.temp.cleanup()

    def put(self, name, data):
        file = self.root / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(data if isinstance(data, bytes) else json.dumps(data).encode())
        return file

    def fake_sdk(self):
        prefix = ".build/rust-native-beta2-0.4.1-linux-x64/"
        library = self.put(prefix + "lib/libtranscribe.so", b"synthetic-library-not-executable")
        self.put(prefix + "lib/transcribe-link.json", {})
        self.put(prefix + "SDK_PROVENANCE.json", {"Version": "0.4.1", "Target": artifacts.TARGETS["linux-x64"], "ArchiveSHA256": "fixture-pin", "Files": [{"Name": library.name, "SHA256": artifacts.sha(library)}]})
        self.put(prefix + "ABI_REPORT.json", {"synthetic": True})
        self.put("scripts/native-assets.json", {"linux-x64": {"sha256": "fixture-pin"}})
        return library

    def test_existing_sdk_file_is_never_overwritten_with_different_bytes(self):
        file = self.put("owned-sdk/lib.so", b"original")
        prepare.put(file, b"original")
        with self.assertRaisesRegex(RuntimeError, "differs"):
            prepare.put(file, b"replacement")
        self.assertEqual(file.read_bytes(), b"original")

    def test_sdk_requires_exact_file_set_and_hashes(self):
        library = self.fake_sdk()
        artifacts.sdk("linux-x64")
        unexpected = self.put(str(library.parent.relative_to(self.root) / "unexpected.so"), b"extra")
        with self.assertRaisesRegex(RuntimeError, "additional"):
            artifacts.sdk("linux-x64")
        unexpected.unlink()  # Only this test's own synthetic temporary file.
        library.write_bytes(b"mutated")
        with self.assertRaisesRegex(RuntimeError, "changed"):
            artifacts.sdk("linux-x64")

    def test_source_receipt_detects_changed_bytes_and_unsafe_paths(self):
        file = self.put("crates/example.rs", b"original")
        entry = {"path": "crates/example.rs", "sha256": artifacts.sha(file), "bytes": file.stat().st_size}
        row = f"{entry['path']}\t{entry['sha256']}\t{entry['bytes']}"
        receipt = self.put("source.json", {"sha256": hashlib.sha256(row.encode()).hexdigest(), "files": [entry]})
        artifacts.source(receipt)
        file.write_bytes(b"modified")
        with self.assertRaisesRegex(RuntimeError, "Source changed"):
            artifacts.source(receipt)
        entry["path"] = "../outside.rs"
        self.put("source.json", {"sha256": "unused", "files": [entry]})
        with self.assertRaisesRegex(RuntimeError, "Unsafe"):
            artifacts.source(receipt)

    def test_check_receipt_rejects_device_or_model_queries(self):
        self.put("apps/desktop/src-tauri/tauri.conf.json", {"version": "1.0.0-beta.2"})
        folder = self.root / "checks"
        value = {"success": True, "applicationVersion": "1.0.0-beta.2", "nativeRuntimeChecks": 1, "syntheticChecks": 7, "installedMetadataQueried": False, "realCaptureOpened": False}
        self.put("checks/release-checks.json", value)
        artifacts.checks(folder)
        for field in ["success", "installedMetadataQueried", "realCaptureOpened"]:
            changed = dict(value)
            changed[field] = not changed[field]
            self.put("checks/release-checks.json", changed)
            with self.assertRaisesRegex(RuntimeError, "did not pass"):
                artifacts.checks(folder)

    def test_finalize_leaves_sealed_application_bytes_untouched_and_excludes_local_paths(self):
        self.put("apps/desktop/src-tauri/tauri.conf.json", {"version": "1.0.0-beta.2"})
        value = {"success": True, "applicationVersion": "1.0.0-beta.2", "nativeRuntimeChecks": 1, "syntheticChecks": 7, "installedMetadataQueried": False, "realCaptureOpened": False}
        self.put("checks/release-checks.json", value)
        empty_hash = hashlib.sha256(b"").hexdigest()
        source = self.put("source.json", {"sha256": empty_hash, "files": []})
        program = self.put("work/app/HandyMeeting", b"synthetic-sealed-executable")
        work = self.root / "work"
        self.put("work/stage-receipt.json", {"app": str(program.parent), "program": str(program.parent), "resources": str(program.parent), "rid": "linux-x64", "applicationVersion": "1.0.0-beta.2", "sourceSHA256": empty_hash})
        before = artifacts.sha(program)
        with contextlib.redirect_stdout(io.StringIO()):
            artifacts.finalize(work, source, self.root / "checks")
        self.assertEqual(artifacts.sha(program), before)
        self.assertEqual(len(list(program.parent.iterdir())), 1)
        manifest = artifacts.read(work / "package-manifest.json")
        self.assertNotIn("app", manifest)
        self.assertNotIn(str(self.root), json.dumps(manifest))


if __name__ == "__main__":
    unittest.main()
