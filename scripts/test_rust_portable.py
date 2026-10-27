"""Synthetic filesystem guards only; no native execution, devices, or downloads."""
import contextlib
import hashlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock


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

    def proof_fixture(self):
        work = self.root / ".build/rust-portable-package/osx-arm64-synthetic"
        self.put(".build/portable-source-before.json", {"synthetic": True})
        self.put(str(work.relative_to(self.root) / "package-manifest.json"), {"synthetic": "known manifest"})
        self.put(str(work.relative_to(self.root) / "relocated-checks/release-checks.json"), {"synthetic": "known checks"})
        self.put(str(work.relative_to(self.root) / "source-after-package.json"), {"synthetic": "known source"})
        return work

    def test_final_package_binding_hashes_the_archive_without_changing_payload_proof(self):
        for rid, suffix in [('linux-x64', 'linux-amd64.deb'), ('osx-arm64', 'osx-arm64.dmg'), ('osx-x64', 'osx-x64.dmg')]:
            with self.subTest(rid=rid):
                work = self.root / ('.build/rust-portable-package/' + rid + '-synthetic')
                proof = self.put(str(work.relative_to(self.root) / 'package-manifest.json'), {'rid': rid, 'applicationVersion': '1.0.0', 'sourceSHA256': 'a' * 64, 'files': [{'path': 'HandyMeeting', 'sha256': 'b' * 64}]})
                package = self.put('packages/HandyMeeting-1.0.0-' + suffix, b'synthetic final archive, not an installer')
                before = proof.read_bytes()
                with contextlib.redirect_stdout(io.StringIO()):
                    artifacts.bind_package(work, package)
                bound = artifacts.read(package.with_name(package.name + '.manifest.json'))
                self.assertEqual(bound['package'], package.name)
                self.assertEqual(bound['sha256'], artifacts.sha(package))
                self.assertEqual(bound['bytes'], package.stat().st_size)
                self.assertEqual(bound['sourceSHA256'], 'a' * 64)
                self.assertEqual(proof.read_bytes(), before)
                self.assertNotIn(str(self.root), json.dumps(bound))
                with self.assertRaises(FileExistsError):
                    artifacts.bind_package(work, package)
                self.assertEqual(proof.read_bytes(), before)

    def test_final_package_binding_rejects_mismatched_filename_and_external_directory(self):
        work = self.root / '.build/rust-portable-package/linux-x64-synthetic'
        self.put(str(work.relative_to(self.root) / 'package-manifest.json'), {'rid': 'linux-x64', 'applicationVersion': '1.0.0'})
        for name in ['packages/HandyMeeting-1.0.0-osx-arm64.dmg', 'other/HandyMeeting-1.0.0-linux-amd64.deb']:
            package = self.put(name, b'synthetic final archive')
            with self.assertRaisesRegex(RuntimeError, 'filename or directory mismatch'):
                artifacts.bind_package(work, package)
            self.assertFalse(package.with_name(package.name + '.manifest.json').exists())

    def assert_flat_proof_scope(self, work):
        observed = []
        original_read = artifacts.read

        def observe(file):
            observed.append(file)
            return original_read(file)

        # A future recursive search is a regression even when its current result
        # happens to contain only safe files. The collector must use exact names.
        with mock.patch.object(pathlib.Path, "rglob", side_effect=AssertionError("Never search an application/image tree")), mock.patch.object(artifacts, "read", side_effect=observe), contextlib.redirect_stdout(io.StringIO()):
            artifacts.collect_proofs("osx-arm64", work)
        output = self.root / ".build/rust-portable-proofs/osx-arm64"
        expected = {"source-before.json", "package-manifest.json", "relocated-release-checks.json", "source-after-package.json"}
        self.assertEqual({file.name for file in output.iterdir()}, expected)
        self.assertTrue(all(file.is_file() and not file.is_symlink() for file in output.iterdir()))
        self.assertEqual(len(observed), len(expected))
        self.assertTrue(all(file.is_relative_to(self.root) for file in observed))

    def test_proof_collector_uses_only_explicit_files_and_never_recurses(self):
        work = self.proof_fixture()
        self.put(str(work.relative_to(self.root) / "image/Applications/unrelated-proof.json"), {"never": "upload this"})
        self.assert_flat_proof_scope(work)

    def test_isolated_credential_proof_is_an_explicit_regular_file_only(self):
        work = self.proof_fixture()
        expected = {"actualNativeStore": True, "simulated": False, "usesUserEntries": False}
        self.put(".build/portable-credential-checks/credential-checks.json", expected)
        artifacts.collect_proofs("osx-arm64", work)
        output = self.root / ".build/rust-portable-proofs/osx-arm64/credential-checks.json"
        self.assertEqual(json.loads(output.read_text()), expected)

    def test_applications_symlink_cannot_expand_proof_scope(self):
        work = self.proof_fixture()
        image = work / "image"
        image.mkdir()
        with tempfile.TemporaryDirectory() as external:
            outside = pathlib.Path(external)
            (outside / "package-manifest.json").write_text('{"never":"read or upload this external file"}')
            try:
                (image / "Applications").symlink_to(outside, target_is_directory=True)
            except OSError as error:
                self.skipTest("This test account cannot create directory symlinks: " + str(error))
            self.assert_flat_proof_scope(work)

    def test_debian_control_recommends_desktop_helpers_and_keeps_dependencies(self):
        files, links = artifacts.debian_files("1.0.3-beta.1")
        control = files["DEBIAN/control"]
        fields = dict(line.split(": ", 1) for line in control.splitlines() if line and not line.startswith(" "))
        self.assertEqual(fields["Package"], "handymeeting")
        self.assertEqual(fields["Version"], "1.0.3~beta1")
        self.assertIn("pulseaudio-utils", fields["Depends"])
        recommends = [item.strip() for item in fields["Recommends"].split(",")]
        self.assertIn("xdg-utils", recommends)
        self.assertIn("gnome-keyring", recommends)
        self.assertNotIn("gnome-keyring", fields["Depends"])  # Never a hard requirement.
        self.assertTrue(control.endswith("\n"))
        self.assertEqual(links, {"usr/bin/handymeeting": "/opt/handymeeting/1.0.3-beta.1/HandyMeeting"})
        self.assertIn("Exec=/opt/handymeeting/1.0.3-beta.1/HandyMeeting\n", files["usr/share/applications/handymeeting.desktop"])
        self.assertEqual(artifacts.debian_files("1.0.3")[0]["DEBIAN/control"].splitlines()[1], "Version: 1.0.3")

    def deb_fixture(self, rid="linux-x64"):
        work = self.root / f".build/rust-portable-package/{rid}-synthetic"
        self.put(str(work.relative_to(self.root) / "stage-receipt.json"), {"rid": rid, "applicationVersion": "1.0.3"})
        return work

    def test_deb_metadata_links_the_command_to_the_staged_executable(self):
        work = self.deb_fixture()
        executable = self.put(str(work.relative_to(self.root) / "deb/opt/handymeeting/1.0.3/HandyMeeting"), b"synthetic, never executed")
        with contextlib.redirect_stdout(io.StringIO()):
            artifacts.deb_metadata(work)
        link = work / "deb/usr/bin/handymeeting"
        self.assertTrue(link.is_symlink())
        self.assertEqual(link.readlink().as_posix(), "/opt/handymeeting/1.0.3/HandyMeeting")
        self.assertEqual((work / "deb" / link.readlink().as_posix().lstrip("/")).resolve(), executable.resolve())
        control = (work / "deb/DEBIAN/control").read_text(encoding="utf-8")
        self.assertIn("Recommends: xdg-utils, gnome-keyring\n", control)
        self.assertTrue((work / "deb/usr/share/applications/handymeeting.desktop").is_file())
        # Never overwrites an existing staged package file.
        with self.assertRaises(FileExistsError):
            artifacts.deb_metadata(work)

    def test_deb_metadata_requires_the_staged_linux_executable_and_work_directory(self):
        work = self.deb_fixture()
        with self.assertRaisesRegex(RuntimeError, "not part of the staged package"), contextlib.redirect_stdout(io.StringIO()):
            artifacts.deb_metadata(work)
        self.assertFalse((work / "deb/usr/bin/handymeeting").exists())
        with self.assertRaisesRegex(RuntimeError, "Unexpected Debian staging directory"):
            artifacts.deb_metadata(self.deb_fixture("osx-arm64"))
        outside = self.root / "elsewhere/linux-x64-synthetic"
        self.put(str(outside.relative_to(self.root) / "stage-receipt.json"), {"rid": "linux-x64", "applicationVersion": "1.0.3"})
        with self.assertRaisesRegex(RuntimeError, "Unexpected Debian staging directory"):
            artifacts.deb_metadata(outside)

    def test_proof_output_rejects_unexpected_directories(self):
        work = self.proof_fixture()
        output = self.root / ".build/rust-portable-proofs/osx-arm64"
        (output / "Applications").mkdir(parents=True)
        with self.assertRaisesRegex(RuntimeError, "unexpected"):
            artifacts.collect_proofs("osx-arm64", work)


if __name__ == "__main__":
    unittest.main()
