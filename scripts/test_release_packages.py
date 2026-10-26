"""Synthetic package/proof fixtures only: never install software or publish a release."""
import importlib.util
import json
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
            proof = {'success': True, 'version': VERSION, 'applicationVersion': VERSION, 'sourceFingerprintSHA256': FINGERPRINT, 'sourceSHA256': FINGERPRINT, 'package': name, 'sha256': packages.sha(file), 'bytes': file.stat().st_size, 'userRecordingsIncluded': False, 'rid': rid, 'releaseChecks': {'success': True}, 'relocatedReleaseChecks': {'success': True}, 'actualInstallUninstallChecked': True, 'freshMissingParentsChecked': True, 'existingDestinationRejected': True, 'duplicateVersionRejected': True, 'unknownUserFilePreserved': True, 'recursiveDeletion': False}
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

    def test_installer_requires_actual_duplicate_version_rejection(self):
        file = next((self.input / 'windows').glob('*setup.exe.verification.json'))
        self.change(file, 'duplicateVersionRejected', False)
        with self.assertRaisesRegex(ValueError, 'Actual installer'):
            self.assemble()

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
        old = {'assets': [{'name': result['files'][0]['name'], 'size': 7, 'digest': 'sha256:' + 'c' * 64}], 'html_url': 'https://example.test/release'}
        with mock.patch.object(publisher, 'gh', side_effect=[json.dumps({'sha': COMMIT}), json.dumps(old), json.dumps(old)]) as cli:
            with self.assertRaisesRegex(ValueError, 'existing release asset differs'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', False)
            self.assertTrue(all(call.args[0] == 'api' for call in cli.call_args_list))

    def test_complete_existing_release_is_verified_without_rebuilding_or_uploading(self):
        result = self.assemble()
        rows = result['files'] + [{'name': name, 'sha256': packages.sha(self.output / name), 'bytes': (self.output / name).stat().st_size} for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        release = {'assets': [{'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']} for row in rows]}
        def cli(*args, **_):
            if args[0] == 'api':
                return json.dumps(release)
            self.assertEqual(args[:2], ('release', 'download'))
            target = pathlib.Path(args[args.index('--dir') + 1]) / 'RELEASE_MANIFEST.json'
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
        release = {'assets': [{'name': 'private.wav', 'size': 1}], 'html_url': 'https://example.test/release'}
        with mock.patch.object(publisher, 'gh', side_effect=[json.dumps({'sha': COMMIT}), json.dumps(release), json.dumps(release)]) as cli:
            with self.assertRaisesRegex(ValueError, 'Unexpected existing release assets'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', True)
            self.assertTrue(all(call.args[0] == 'api' for call in cli.call_args_list))
            self.assertEqual(release['assets'][0]['name'], 'private.wav')

    def test_new_remote_asset_after_verification_prevents_final_promotion(self):
        result = self.assemble()
        rows = result['files'] + [{'name': name, 'sha256': packages.sha(self.output / name), 'bytes': (self.output / name).stat().st_size} for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        release = {'assets': [{'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']} for row in rows], 'html_url': 'https://example.test/release'}
        changed = dict(release, assets=release['assets'] + [{'name': 'private.wav', 'size': 1}])
        with mock.patch.object(publisher, 'gh', side_effect=[json.dumps({'sha': COMMIT}), json.dumps(release), json.dumps(release), json.dumps(changed)]) as cli:
            with self.assertRaisesRegex(ValueError, 'Unexpected existing release assets'):
                publisher.publish(self.output, 'owner/repo', 'v1.0.0', COMMIT, self.root / 'notes.md', True)
            self.assertTrue(all(call.args[0] == 'api' for call in cli.call_args_list))

    def test_existing_complete_rejects_extra_or_duplicate_remote_assets(self):
        result = self.assemble()
        rows = result['files'] + [{'name': name, 'sha256': packages.sha(self.output / name), 'bytes': (self.output / name).stat().st_size} for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        assets = [{'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']} for row in rows]
        for extra in [{'name': 'private.wav', 'size': 1}, assets[0]]:
            release = {'assets': assets + [extra]}
            def cli(*args, **_):
                if args[0] == 'api':
                    return json.dumps(release)
                target = pathlib.Path(args[args.index('--dir') + 1]) / 'RELEASE_MANIFEST.json'
                target.write_bytes((self.output / 'RELEASE_MANIFEST.json').read_bytes())
                return ''
            with mock.patch.object(publisher, 'gh', side_effect=cli):
                with self.assertRaisesRegex(ValueError, 'Unexpected existing release assets'):
                    publisher.existing_complete('owner/repo', 'v1.0.0', COMMIT, FINGERPRINT)


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
