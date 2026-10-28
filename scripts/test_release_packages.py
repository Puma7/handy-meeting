"""Synthetic package/proof fixtures only: never install software or publish a release."""
import importlib.util
import hashlib
import shutil
import subprocess
import zipfile
import json
import os
import pathlib
import tempfile
import unittest
from unittest import mock


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).with_name(file))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packages = load('release_packages', 'release-packages.py')
installer = load('installer_files', 'windows-installer-files.py')
publisher = load('release_publisher', 'publish-rust-release.py')
VERSION, COMMIT, FINGERPRINT = '1.0.0', 'a' * 40, 'b' * 64


class ReleaseGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.input = self.root / 'artifacts'
        self.output = self.root / 'bundle'
        self.package_files = []
        self.put('windows-checks/rust-verification.json', {'success': True, 'version': VERSION, 'sourceFingerprintSHA256': FINGERPRINT, 'checks': {name: {'reportedExitCode': 0} for name in ['legacy', 'artifact-guards', 'distribution-guards', 'cargo-format', 'frontend-tests', 'frontend-build', 'rust-tests', 'rust-clippy']}})
        for scope, provider in [('windows-checks', 'windows-credential-manager'), ('linux-checks', 'linux-secret-service'), ('mac-arm64-checks', 'macos-keychain'), ('mac-x64-checks', 'macos-keychain')]:
            self.put(f'{scope}/credential-checks.json', {'applicationVersion': VERSION, 'provider': provider, 'success': True, 'actualNativeStore': True, 'simulated': False, 'usesUserEntries': False, 'checks': 5})
        for scope, suffix, rid in [('windows', 'windows-x64.zip', 'windows-x64'), ('windows', 'windows-x64-setup.exe', 'windows-x64'), ('linux', 'linux-amd64.deb', 'linux-x64'), ('mac-arm64', 'osx-arm64.dmg', 'osx-arm64'), ('mac-x64', 'osx-x64.dmg', 'osx-x64')]:
            name = f'HandyMeeting-{VERSION}-{suffix}'
            file = self.put(f'{scope}/{name}', b'not-executable-synthetic-' + suffix.encode())
            self.package_files.append(file)
            self.put(f'{scope}/{name}.sha256', (packages.sha(file) + '  ' + name + '\n').encode())
            proof = {'success': True, 'version': VERSION, 'applicationVersion': VERSION, 'sourceFingerprintSHA256': FINGERPRINT, 'sourceSHA256': FINGERPRINT, 'package': name, 'sha256': packages.sha(file), 'bytes': file.stat().st_size, 'userRecordingsIncluded': False, 'rid': rid, 'releaseChecks': {'success': True}, 'relocatedReleaseChecks': {'success': True}, 'actualInstallUninstallChecked': True, 'freshMissingParentsChecked': True, 'existingDestinationRejected': True, 'reinstallRepaired': True, 'startMenuShortcutChecked': True, 'legacyVersionsMigrated': True, 'runningProgramPreserved': True, 'unknownUserFilePreserved': True, 'recursiveDeletion': False}
            self.put(f'{scope}/{name}.' + ('verification' if scope == 'windows' else 'manifest') + '.json', proof)

    def put(self, name, value):
        file = self.input / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(value if isinstance(value, bytes) else json.dumps(value).encode())
        return file

    def change(self, file, key, value):
        data = packages.read(file)
        data[key] = value
        file.write_text(json.dumps(data))

    def assemble(self):
        return packages.assemble(self.input, VERSION, COMMIT, FINGERPRINT, self.output)

    def test_full_five_target_bundle_has_checksums_and_copies_no_unlisted_recording(self):
        self.put('unexpected/private.wav', b'not-real-audio')
        result = self.assemble()
        self.assertEqual(len(result['packages']), 5)
        self.assertEqual(len(list(self.output.iterdir())), 17)
        self.assertFalse((self.output / 'private.wav').exists())
        self.assertFalse(result['physicalAudioDevicesTestedInCI'])
        self.assertFalse(result['macNotarized'])

    def test_missing_target_or_duplicate_artifact_fails_before_copying(self):
        self.put('mac-arm64/duplicate/' + self.package_files[3].name, b'second')
        with self.assertRaisesRegex(ValueError, 'exactly one'):
            self.assemble()
        self.assertFalse(self.output.exists())

    def test_altered_package_and_foreign_source_are_rejected(self):
        self.package_files[0].write_bytes(b'altered')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.assemble()
        self.assertFalse(self.output.exists())

    def test_rehashed_portable_package_cannot_reuse_a_different_payload_proof(self):
        for package in self.package_files[2:]:
            with self.subTest(package=package.name):
                self.output = self.root / ('bundle-' + package.name)
                original = package.read_bytes()
                checksum = package.with_name(package.name + '.sha256')
                old_checksum = checksum.read_bytes()
                proof = package.with_name(package.name + '.manifest.json')
                old_proof = proof.read_bytes()
                package.write_bytes(b'different synthetic package with a consistent checksum')
                checksum.write_text(packages.sha(package) + '  ' + package.name + '\n', encoding='utf-8')
                try:
                    with self.assertRaisesRegex(ValueError, 'Portable package proof mismatch'):
                        self.assemble()
                    self.assertFalse(self.output.exists())
                    self.assertEqual(proof.read_bytes(), old_proof)
                finally:
                    package.write_bytes(original)
                    checksum.write_bytes(old_checksum)

    def test_older_source_proof_is_not_mixed_into_release(self):
        file = next((self.input / 'mac-x64').glob('*.manifest.json'))
        self.change(file, 'sourceFingerprintSHA256', 'c' * 64)
        with self.assertRaisesRegex(ValueError, 'different source'):
            self.assemble()

    def test_failed_check_and_simulated_vault_never_pass(self):
        file = self.input / 'linux-checks/credential-checks.json'
        self.change(file, 'simulated', True)
        with self.assertRaisesRegex(ValueError, 'Actual isolated'):
            self.assemble()

    def test_failed_mandatory_rust_check_prevents_the_entire_bundle(self):
        file = self.input / 'windows-checks/rust-verification.json'
        value = packages.read(file)
        value['checks']['rust-tests']['reportedExitCode'] = 101
        file.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'mandatory source check'):
            self.assemble()
        self.assertFalse(self.output.exists())

    def test_installer_without_executed_uninstall_preservation_check_is_rejected(self):
        file = next((self.input / 'windows').glob('*setup.exe.verification.json'))
        self.change(file, 'actualInstallUninstallChecked', False)
        with self.assertRaisesRegex(ValueError, 'Actual installer'):
            self.assemble()

    def test_installer_requires_fresh_missing_parent_directory_check(self):
        file = next((self.input / 'windows').glob('*setup.exe.verification.json'))
        for value in [False, None]:
            with self.subTest(value=value):
                self.change(file, 'freshMissingParentsChecked', value)
                with self.assertRaisesRegex(ValueError, 'Actual installer'):
                    self.assemble()
                self.assertFalse(self.output.exists())

    def test_existing_bundle_is_preserved(self):
        self.output.mkdir()
        sentinel = self.output / 'keep.txt'
        sentinel.write_text('original')
        with self.assertRaisesRegex(ValueError, 'must be new'):
            self.assemble()
        self.assertEqual(sentinel.read_text(), 'original')

    def test_installer_requires_actual_repair_shortcut_migration_and_running_app_checks(self):
        # 1.0.5 setups refused a second run and left no Start menu link under the default policy.
        file = next((self.input / 'windows').glob('*setup.exe.verification.json'))
        for key in ['reinstallRepaired', 'startMenuShortcutChecked', 'legacyVersionsMigrated', 'runningProgramPreserved']:
            for value in [False, None]:
                with self.subTest(key=key, value=value):
                    original = packages.read(file)
                    self.change(file, key, value)
                    with self.assertRaisesRegex(ValueError, 'Actual installer'):
                        self.assemble()
                    self.assertFalse(self.output.exists())
                    file.write_text(json.dumps(original))

    def test_installer_requires_actual_existing_destination_preservation(self):
        file = next((self.input / 'windows').glob('*setup.exe.verification.json'))
        self.change(file, 'existingDestinationRejected', False)
        with self.assertRaisesRegex(ValueError, 'Actual installer'):
            self.assemble()

    def test_publisher_checks_tag_identity_before_any_upload(self):
        self.assemble()
        with mock.patch.object(publisher, 'gh', return_value=json.dumps({'sha': 'c' * 40})) as cli:
            with self.assertRaisesRegex(ValueError, 'tag moved'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', False)
            self.assertEqual(cli.call_count, 1)

    def test_publisher_rejects_extra_payload_without_contacting_github(self):
        self.assemble()
        manifest = packages.read(self.output / 'RELEASE_MANIFEST.json')
        manifest['files'].append({'name': 'private.wav', 'sha256': 'f' * 64, 'bytes': 1})
        (self.output / 'RELEASE_MANIFEST.json').write_text(json.dumps(manifest))
        with mock.patch.object(publisher, 'gh') as cli:
            with self.assertRaisesRegex(ValueError, 'Unexpected release payload'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', False)
            cli.assert_not_called()

    def test_publisher_preserves_mismatching_existing_asset(self):
        result = self.assemble()
        old = {'id': 41, 'tag_name': 'v1.0.0', 'assets': [{'name': result['files'][0]['name'], 'size': 7, 'digest': 'sha256:' + 'c' * 64}], 'html_url': 'https://example.test/release'}
        with mock.patch.object(publisher, 'gh', side_effect=[json.dumps({'sha': COMMIT}), json.dumps(old), json.dumps(old)]) as cli:
            with self.assertRaisesRegex(ValueError, 'existing release asset differs'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', False)
            self.assertTrue(all(call.args[0] == 'api' for call in cli.call_args_list))
            self.assertFalse(any('POST' in call.args or 'PATCH' in call.args for call in cli.call_args_list))

    def test_complete_existing_release_is_verified_without_rebuilding_or_uploading(self):
        result = self.assemble()
        rows = result['files'] + [{'name': name, 'sha256': packages.sha(self.output / name), 'bytes': (self.output / name).stat().st_size} for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        release = {'id': 41, 'tag_name': 'v1.0.0', 'assets': [{'id': index + 100, 'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']} for index, row in enumerate(rows)]}
        def cli(*args, **_):
            if args[0] == 'api' and 'Accept: application/octet-stream' not in args:
                return json.dumps(release)
            self.assertIn('/releases/assets/', args[1])
            target = _['output_path']
            target.write_bytes((self.output / 'RELEASE_MANIFEST.json').read_bytes())
            return ''
        with mock.patch.object(publisher, 'gh', side_effect=cli) as calls:
            self.assertTrue(publisher.existing_complete('owner/repo', 'v1.0.0', COMMIT, FINGERPRINT))
            self.assertEqual(calls.call_count, 2)
            release['assets'][0]['digest'] = 'sha256:' + 'd' * 64
            with self.assertRaisesRegex(ValueError, 'incomplete or differs'):
                publisher.existing_complete('owner/repo', 'v1.0.0', COMMIT, FINGERPRINT)

    def test_extra_remote_asset_prevents_upload_and_publication_without_deleting_it(self):
        self.assemble()
        release = {'id': 41, 'tag_name': 'v1.0.0', 'assets': [{'name': 'private.wav', 'size': 1}], 'html_url': 'https://example.test/release'}
        with mock.patch.object(publisher, 'gh', side_effect=[json.dumps({'sha': COMMIT}), json.dumps(release), json.dumps(release)]) as cli:
            with self.assertRaisesRegex(ValueError, 'Unexpected existing release assets'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', True)
            self.assertTrue(all(call.args[0] == 'api' for call in cli.call_args_list))
            self.assertFalse(any('POST' in call.args or 'PATCH' in call.args for call in cli.call_args_list))
            self.assertEqual(release['assets'][0]['name'], 'private.wav')

    def test_new_remote_asset_after_verification_prevents_final_promotion(self):
        result = self.assemble()
        rows = result['files'] + [{'name': name, 'sha256': packages.sha(self.output / name), 'bytes': (self.output / name).stat().st_size} for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        release = {'id': 41, 'tag_name': 'v1.0.0', 'assets': [{'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']} for row in rows], 'html_url': 'https://example.test/release'}
        changed = dict(release, assets=release['assets'] + [{'name': 'private.wav', 'size': 1}])
        with mock.patch.object(publisher, 'gh', side_effect=[json.dumps({'sha': COMMIT}), json.dumps(release), json.dumps(release), json.dumps(changed)]) as cli:
            with self.assertRaisesRegex(ValueError, 'Unexpected existing release assets'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', True)
            self.assertTrue(all(call.args[0] == 'api' for call in cli.call_args_list))
            self.assertFalse(any('POST' in call.args or 'PATCH' in call.args for call in cli.call_args_list))

    def test_existing_complete_rejects_extra_or_duplicate_remote_assets(self):
        result = self.assemble()
        rows = result['files'] + [{'name': name, 'sha256': packages.sha(self.output / name), 'bytes': (self.output / name).stat().st_size} for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        assets = [{'id': index + 100, 'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']} for index, row in enumerate(rows)]
        for extra in [{'name': 'private.wav', 'size': 1}, assets[0]]:
            release = {'id': 41, 'tag_name': 'v1.0.0', 'assets': assets + [extra]}
            def cli(*args, **_):
                if args[0] == 'api' and 'Accept: application/octet-stream' not in args:
                    return json.dumps(release)
                self.assertIn('/releases/assets/', args[1])
                target = _['output_path']
                target.write_bytes((self.output / 'RELEASE_MANIFEST.json').read_bytes())
                return ''
            with mock.patch.object(publisher, 'gh', side_effect=cli):
                with self.assertRaisesRegex(ValueError, 'Unexpected existing release assets'):
                    publisher.existing_complete('owner/repo', 'v1.0.0', COMMIT, FINGERPRINT)


class SourceExportContract(unittest.TestCase):
    @unittest.skipUnless(shutil.which('pwsh'), 'PowerShell required for the real source exporter')
    def test_allowlisted_links_cannot_archive_private_outside_bytes_or_remove_earlier_proofs(self):
        """Exercise real filesystem links with a private sentinel outside the source root."""
        for kind in ['top-hardlink', 'nested-symlink', 'directory-link', 'allowlist-root-link']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory(prefix='meeting-source-link-') as temp:
                base = pathlib.Path(temp).resolve()
                root = base / 'source'
                script = root / 'scripts/export-source.ps1'
                script.parent.mkdir(parents=True)
                shutil.copyfile(pathlib.Path(__file__).with_name(script.name), script)
                config = root / 'apps/desktop/package.json'
                config.parent.mkdir(parents=True)
                config.write_text(json.dumps({'version': '0.0.0-test'}), encoding='utf-8')
                earlier = root / '.build/source-export-earlier/retained.json'
                earlier.parent.mkdir(parents=True)
                earlier.write_bytes(b'earlier source evidence must remain byte-identical')
                outside = base / 'private'
                outside.mkdir()
                private = b'PRIVATE-OUTSIDE-SENTINEL-never-archive-731921'
                sentinel = outside / 'private.md'
                sentinel.write_bytes(private)
                if kind == 'top-hardlink':
                    os.link(sentinel, root / 'README.md')
                elif kind == 'nested-symlink':
                    link = root / 'apps/desktop/private.md'
                    try:
                        link.symlink_to(sentinel)
                    except OSError:
                        # File hardlinks do not require Windows symbolic-link privileges.
                        os.link(sentinel, link)
                else:
                    link = root / ('src' if kind == 'allowlist-root-link' else 'apps/desktop/private')
                    try:
                        link.symlink_to(outside, target_is_directory=True)
                    except OSError:
                        if os.name != 'nt':
                            raise
                        env = dict(os.environ, SOURCE_LINK_FIXTURE=str(link), SOURCE_TARGET_FIXTURE=str(outside))
                        creation = subprocess.run([shutil.which('pwsh'), '-NoProfile', '-NonInteractive', '-Command',
                            "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:SOURCE_LINK_FIXTURE -Target $env:SOURCE_TARGET_FIXTURE | Out-Null"],
                            capture_output=True, text=True, timeout=30, env=env)
                        self.assertEqual(creation.returncode, 0, creation.stdout + creation.stderr)
                result = subprocess.run([shutil.which('pwsh'), '-NoProfile', '-NonInteractive', '-File', str(script)],
                    cwd=root, capture_output=True, text=True, timeout=60)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn('link', result.stderr.lower())
                self.assertEqual(sentinel.read_bytes(), private)
                self.assertEqual(earlier.read_bytes(), b'earlier source evidence must remain byte-identical')
                for archive in (root / '.build').glob('*.zip'):
                    with zipfile.ZipFile(archive) as bundle:
                        self.assertFalse(any(private in bundle.read(name) for name in bundle.namelist()))

    @unittest.skipUnless(shutil.which('pwsh'), 'PowerShell required for the real source exporter')
    def test_nested_private_build_and_browser_proofs_never_enter_the_source_bundle(self):
        """Run the real exporter: retain legitimate sources and exclude private nested proofs."""
        with tempfile.TemporaryDirectory(prefix='meeting-source-fixture-') as temp:
            # An isolated checkout may itself live below a parent's .build directory.
            root = (pathlib.Path(temp) / '.build' / 'isolated-source').resolve()
            root.mkdir(parents=True)
            script = root / 'scripts/export-source.ps1'
            script.parent.mkdir()
            shutil.copyfile(pathlib.Path(__file__).with_name(script.name), script)
            kept = {
                'apps/desktop/package.json': json.dumps({'version': '0.0.0-test'}).encode(),
                'apps/desktop/src/App.tsx': b'export const syntheticPublicSource = true;\n',
            }
            excluded = [
                'apps/desktop/.build/probe/private.json',
                'scripts/.build/probe/private.md',
                'apps/desktop/.vite/cache.json',
                'apps/desktop/test-results/receipt.json',
                'apps/desktop/playwright-report/report.html',
                '.build/earlier-proof/retained.json',
            ]
            private = b'{"syntheticPrivateProbe":"never-export"}\n'
            for name, data in list(kept.items()) + [(name, private) for name in excluded]:
                file = root / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(data)
            result = subprocess.run([shutil.which('pwsh'), '-NoProfile', '-NonInteractive', '-File', str(script)], cwd=root, capture_output=True, text=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            receipt = json.loads(result.stdout.lstrip('\ufeff'))
            with zipfile.ZipFile(receipt['Archive']) as bundle:
                names = {name.replace('\\', '/') for name in bundle.namelist()}
                manifest = json.loads(bundle.read('SOURCE_MANIFEST.json').decode('utf-8-sig'))
                rows = {row['Path'].replace('\\', '/'): row for row in manifest}
                for name in excluded:
                    self.assertNotIn(name, names)
                    self.assertNotIn(name, rows)
                    self.assertEqual((root / name).read_bytes(), private)
                for name, data in kept.items():
                    self.assertIn(name, names)
                    self.assertEqual(rows[name]['Bytes'], len(data))
                    self.assertEqual(rows[name]['SHA256'].lower(), hashlib.sha256(data).hexdigest())
                self.assertEqual(receipt['Files'], len(rows))
                self.assertEqual(names, set(rows) | {'SOURCE_MANIFEST.json'})
            self.assertFalse(receipt['UserRecordingsIncluded'])
            self.assertFalse(receipt['Published'])


class WindowsZipContract(unittest.TestCase):
    @unittest.skipUnless(shutil.which('pwsh'), 'PowerShell required for the Windows-only ZIP packager')
    def test_branded_launcher_is_packaged_and_instructions_name_the_existing_file(self):
        """Inspect a synthetic ZIP and verify launcher naming plus no-overwrite behavior."""
        # Synthetic file-only receipt, isolated from real build proofs. Never executes a payload.
        with tempfile.TemporaryDirectory(prefix='meeting-zip-fixture-') as temp:
            root = pathlib.Path(temp).resolve()
            scripts = root / 'scripts'
            scripts.mkdir()
            packager = scripts / 'package-rust-windows.ps1'
            shutil.copyfile(pathlib.Path(__file__).with_name(packager.name), packager)
            version = '0.0.0-test'
            config = root / 'apps/desktop/src-tauri/tauri.conf.json'
            config.parent.mkdir(parents=True)
            config.write_text(json.dumps({'version': version}), encoding='utf-8')
            dist = root / ('dist-rust-' + version)
            dist.mkdir()
            executable = dist / 'HandyMeeting.exe'
            executable.write_bytes(b'non-executable synthetic packaging fixture')
            launcher = root / ('Meeting for Handy ' + version + ' starten.cmd')
            launcher.write_bytes(b'synthetic launcher fixture; never execute')
            def invoke(*args):
                """Run only the copied filesystem packager; fixture program bytes are never executed."""
                return subprocess.run([shutil.which('pwsh'), '-NoProfile', '-NonInteractive', '-File', str(packager), *args], cwd=root, capture_output=True, text=True, timeout=60)
            fingerprint = invoke('-SourceFingerprintOnly')
            self.assertEqual(fingerprint.returncode, 0, fingerprint.stderr)
            digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            receipt = {'success': True, 'version': version, 'releaseBinaryChecksPassed': True, 'legacyChecksExecuted': True, 'sourceFingerprintSHA256': fingerprint.stdout.strip(), 'programDirectory': dist.name, 'executableSHA256': digest(executable), 'programFiles': [{'path': executable.name, 'sha256': digest(executable)}], 'launcherFile': launcher.name, 'launcherSHA256': digest(launcher)}
            proof = root / '.build'
            proof.mkdir()
            (proof / 'rust-release-verification.json').write_text(json.dumps(receipt), encoding='utf-8')
            (proof / 'rust-verification.json').write_text(json.dumps({'success': True, 'sourceFingerprintSHA256': receipt['sourceFingerprintSHA256']}), encoding='utf-8')
            output = root / 'packages'
            result = invoke('-OutputDirectory', str(output))
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            archive = output / ('HandyMeeting-' + version + '-windows-x64.zip')
            with zipfile.ZipFile(archive) as package:
                names = package.namelist()
                self.assertIn('HandyMeeting/' + launcher.name, names)
                self.assertEqual(package.read('HandyMeeting/' + launcher.name), launcher.read_bytes())
                instructions = package.read('HandyMeeting/START-HERE.txt').decode('utf-8')
                self.assertIn('open ' + launcher.name + '.', instructions)
                self.assertIn('Meeting for Handy ' + version, instructions)
                self.assertIn('HandyMeeting/RustBeta', instructions)
                self.assertEqual(package.read('HandyMeeting/' + dist.name + '/HandyMeeting.exe'), executable.read_bytes())
            original = archive.read_bytes()
            repeated = invoke('-OutputDirectory', str(output))
            self.assertNotEqual(repeated.returncode, 0)
            self.assertEqual(archive.read_bytes(), original)


class InstallerGuards(unittest.TestCase):
    def test_lists_are_literal_and_uninstall_never_recurses_or_names_user_storage(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            program = root / 'program'
            program.mkdir()
            (program / 'HandyMeeting.exe').write_bytes(b'not-an-executable')
            (program / 'licenses').mkdir()
            (program / 'licenses/a $b.txt').write_text('retained license')
            manifest = {'files': [{'path': file.relative_to(program).as_posix(), 'sha256': packages.sha(file)} for file in program.rglob('*') if file.is_file()]}
            installer.generate(program, manifest, root / 'lists')
            script = (root / 'lists/uninstall.nsh').read_text(encoding='utf-8-sig')
            self.assertNotIn('/r', script.lower())
            self.assertNotIn('AppData', script)
            self.assertIn('a $$b.txt', script)
            self.assertIn('"/oname=a $$b.txt"', (root / 'lists/install.nsh').read_text(encoding='utf-8-sig'))
            for name in ['../private.wav', 'C:/private.wav', '*/private.wav', 'a/../private.wav']:
                with self.assertRaisesRegex(ValueError, 'Unsafe installer'):
                    installer.entries(program, {'files': [{'path': name, 'sha256': 'a' * 64}]})


if __name__ == '__main__':
    unittest.main()
