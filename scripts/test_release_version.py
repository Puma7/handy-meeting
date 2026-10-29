"""The release tag sets the version; synthetic copies of the repository files only."""
import ast
import hashlib
import importlib.util
import json
import pathlib
import re
import shutil
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
FILES = ['Cargo.toml', 'Cargo.lock', 'apps/desktop/package.json', 'apps/desktop/package-lock.json',
         'apps/desktop/src-tauri/tauri.conf.json', 'licenses/frontend/manifest.json',
         'crates/handy-bridge/Cargo.toml', 'crates/meeting-core/Cargo.toml',
         'crates/meeting-notes/Cargo.toml', 'apps/desktop/src-tauri/Cargo.toml']


def load():
    spec = importlib.util.spec_from_file_location('set_version', ROOT / 'scripts/set-version.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def registry(lock):
    """Name and version of every Cargo.lock entry that comes from a registry or git source."""
    return re.findall(rb'^\[\[package\]\]\r?\nname = "([^"]+)"\r?\nversion = "([^"]+)"\r?\nsource = ', lock, re.M)


class VersionFromTag(unittest.TestCase):
    def setUp(self):
        self.temp = pathlib.Path(tempfile.mkdtemp(prefix='handymeeting-version-'))
        self.addCleanup(shutil.rmtree, self.temp)
        names = FILES + [str(p.relative_to(ROOT)) for p in (ROOT / 'licenses/rust').glob('*/manifest.json')]
        for name in names:
            (self.temp / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, self.temp / name)
        self.names = names
        self.module = load()
        self.module.ROOT = self.temp

    def test_tag_version_reaches_every_file_and_only_version_fields_change(self):
        before = {name: (self.temp / name).read_bytes() for name in self.names}
        self.module.main(['v9.8.7-beta.1'])
        self.assertEqual(set(self.module.current().values()), {'9.8.7-beta.1'})
        for name in self.names:
            old, new = before[name].split(b'\n'), (self.temp / name).read_bytes().split(b'\n')
            self.assertEqual(len(old), len(new), name)
            changed = [(a, b) for a, b in zip(old, new) if a != b]
            for a, b in changed:
                # Only a version value or a recorded lock hash may differ; line endings stay.
                self.assertTrue(b'9.8.7-beta.1' in b or b'LockSHA256' in b, (name, a, b))
                self.assertEqual(a.endswith(b'\r'), b.endswith(b'\r'), name)
        lock = hashlib.sha256((self.temp / 'Cargo.lock').read_bytes()).hexdigest()
        for manifest in (self.temp / 'licenses/rust').glob('*/manifest.json'):
            self.assertEqual(json.loads(manifest.read_text())['CargoLockSHA256'], lock)
        package_lock = hashlib.sha256((self.temp / 'apps/desktop/package-lock.json').read_bytes()).hexdigest()
        self.assertEqual(json.loads((self.temp / 'licenses/frontend/manifest.json').read_text())['PackageLockSHA256'], package_lock)
        # Registry crates keep their versions, including any that share the old number.
        self.assertEqual(registry(before['Cargo.lock']), registry((self.temp / 'Cargo.lock').read_bytes()))

    def test_invalid_tags_and_disagreeing_files_are_refused(self):
        for tag in ['1.2', 'release-1.2.3', 'v1.2.3;rm', '']:
            with self.assertRaises(SystemExit):
                self.module.main([tag])
        self.module.main(['--check'])
        package = self.temp / 'apps/desktop/package.json'
        package.write_text(re.sub(r'"version": "[^"]*"', '"version": "0.0.1"', package.read_text(), count=1))
        with self.assertRaises(SystemExit):
            self.module.main(['--check'])

    def test_a_stale_license_record_is_refused_instead_of_refreshed(self):
        # A changed dependency without regenerated notices must not look current after stamping.
        lock = self.temp / 'Cargo.lock'
        changed, count = re.subn(rb'(\[\[package\]\]\r?\nname = "[^"]+"\r?\nversion = ")([^"]+)("\r?\nsource = )', rb'\g<1>\g<2>-changed\g<3>', lock.read_bytes(), count=1)
        self.assertEqual(count, 1)
        lock.write_bytes(changed)
        before = {name: (self.temp / name).read_bytes() for name in self.names}
        with self.assertRaises(SystemExit) as refused:
            self.module.main(['v9.8.7'])
        self.assertIn('regenerate the license notices', str(refused.exception))
        self.assertEqual(before, {name: (self.temp / name).read_bytes() for name in self.names})

    def test_the_script_runs_on_the_linux_release_runner_python(self):
        # ubuntu-22.04 ships Python 3.10: tomllib (3.11+) stopped the Linux release job.
        tree = ast.parse((ROOT / 'scripts/set-version.py').read_text(encoding='utf-8'))
        imported = {alias.name.split('.')[0] for node in ast.walk(tree) if isinstance(node, ast.Import) for alias in node.names}
        imported |= {node.module.split('.')[0] for node in ast.walk(tree) if isinstance(node, ast.ImportFrom) and node.module}
        self.assertEqual(imported, {'hashlib', 'json', 'pathlib', 're', 'sys'})


if __name__ == '__main__':
    unittest.main()
