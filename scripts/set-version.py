"""Set or check the application version in every file that carries it.

The release tag is the only place a version is chosen. Release builds call
`set-version.py v1.2.3` right after checkout, so nobody has to bump files by hand:

    python3 scripts/set-version.py v1.2.3     # write 1.2.3 everywhere
    python3 scripts/set-version.py --check    # all files agree (exit 1 otherwise)

Only version fields change. The lock files keep every dependency; the recorded lock
hashes in the license manifests are refreshed so the notice check still matches.
"""
import hashlib
import json
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
SEMVER = re.compile(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?')
PACKAGE = 'apps/desktop/package.json'
PACKAGE_LOCK = 'apps/desktop/package-lock.json'
TAURI = 'apps/desktop/src-tauri/tauri.conf.json'
CARGO = 'Cargo.toml'
CARGO_LOCK = 'Cargo.lock'
RUST_LICENSES = 'licenses/rust'
FRONTEND_LICENSES = 'licenses/frontend/manifest.json'


def text(relative):
    # Bytes as stored: some files use CRLF and their hashes are checked byte for byte.
    return (ROOT / relative).read_bytes().decode('utf-8')


def write(relative, value):
    # Keep the existing line endings and encoding byte for byte apart from the edit.
    (ROOT / relative).write_bytes(value.encode('utf-8'))


def sha(relative):
    return hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()


def workspace_packages():
    members = tomllib.loads(text(CARGO))['workspace']['members']
    return {tomllib.loads(text(member + '/Cargo.toml'))['package']['name'] for member in members}


def replace_once(value, pattern, replacement, what):
    result, count = re.subn(pattern, replacement, value, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f'Version field not found: {what}')
    return result


def current():
    """Every version the repository carries, by file."""
    found = {
        PACKAGE: json.loads(text(PACKAGE))['version'],
        TAURI: json.loads(text(TAURI))['version'],
        CARGO: tomllib.loads(text(CARGO))['workspace']['package']['version'],
    }
    lock = json.loads(text(PACKAGE_LOCK))
    found[PACKAGE_LOCK] = lock['version']
    found[PACKAGE_LOCK + ' (root package)'] = lock['packages']['']['version']
    names = workspace_packages()
    for package in tomllib.loads(text(CARGO_LOCK))['package']:
        if package['name'] in names and 'source' not in package:
            found[CARGO_LOCK + ' (' + package['name'] + ')'] = package['version']
    return found


def set_version(version):
    value = text(PACKAGE)
    write(PACKAGE, replace_once(value, r'^(  "version": )"[^"]*"', rf'\g<1>"{version}"', PACKAGE))
    value = text(TAURI)
    write(TAURI, replace_once(value, r'^(  "version": )"[^"]*"', rf'\g<1>"{version}"', TAURI))
    value = text(PACKAGE_LOCK)
    value = replace_once(value, r'^(  "version": )"[^"]*"', rf'\g<1>"{version}"', PACKAGE_LOCK)
    value = replace_once(value, r'^(    "": \{\r?\n(?:      .*\n)*?      "version": )"[^"]*"', rf'\g<1>"{version}"', PACKAGE_LOCK + ' root package')
    write(PACKAGE_LOCK, value)
    value = text(CARGO)
    write(CARGO, replace_once(value, r'^(\[workspace\.package\]\r?\n(?:.*\n)*?version = )"[^"]*"', rf'\g<1>"{version}"', CARGO))
    value = text(CARGO_LOCK)
    for name in sorted(workspace_packages()):
        value = replace_once(value, rf'^(\[\[package\]\]\r?\nname = "{re.escape(name)}"\r?\nversion = )"[^"]*"(?=\r?\n(?!source))', rf'\g<1>"{version}"', CARGO_LOCK + ' ' + name)
    write(CARGO_LOCK, value)
    # The notice check binds the license records to the exact lock files.
    cargo_hash, package_hash = sha(CARGO_LOCK), sha(PACKAGE_LOCK)
    for manifest in sorted((ROOT / RUST_LICENSES).glob('*/manifest.json')):
        relative = str(manifest.relative_to(ROOT))
        write(relative, replace_once(text(relative), r'("CargoLockSHA256": )"[0-9a-f]{64}"', rf'\g<1>"{cargo_hash}"', relative))
    write(FRONTEND_LICENSES, replace_once(text(FRONTEND_LICENSES), r'("PackageLockSHA256": )"[0-9a-f]{64}"', rf'\g<1>"{package_hash}"', FRONTEND_LICENSES))


def main(argv):
    if argv == ['--check']:
        found = current()
        if len(set(found.values())) != 1:
            for name, version in found.items():
                print(f'{version}\t{name}')
            raise SystemExit('The version differs between files; run scripts/set-version.py <version>.')
        print(next(iter(found.values())))
        return
    if len(argv) != 1:
        raise SystemExit(__doc__)
    version = argv[0][1:] if argv[0].startswith('v') else argv[0]
    if not SEMVER.fullmatch(version):
        raise SystemExit(f'Not a version: {argv[0]} (expected v1.2.3 or v1.2.3-beta.1)')
    set_version(version)
    found = current()
    if set(found.values()) != {version}:
        raise SystemExit('Version update incomplete: ' + json.dumps(found))
    print(version)


if __name__ == '__main__':
    main(sys.argv[1:])
