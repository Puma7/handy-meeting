"""Set or check the application version in every file that carries it.

The release tag is the only place a version is chosen. Release builds call
`set-version.py v1.2.3` right after checkout, so nobody has to bump files by hand:

    python3 scripts/set-version.py v1.2.3     # write 1.2.3 everywhere
    python3 scripts/set-version.py --check    # all files agree (exit 1 otherwise)

Only version fields change. The lock files keep every dependency; the recorded lock
hashes in the license manifests are refreshed so the notice check still matches. A
license record that no longer matches its lock file is refused rather than refreshed.

The Linux release runner (ubuntu-22.04) has Python 3.10, so the TOML files are read
with fixed patterns instead of `tomllib` (Python 3.11+).
"""
import hashlib
import json
import pathlib
import re
import sys

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


def section(value, name):
    """The body of one TOML table, up to the next table header."""
    match = re.search(rf'^\[{re.escape(name)}\]\r?\n(.*?)(?=^\[|\Z)', value, re.MULTILINE | re.DOTALL)
    if match is None:
        raise SystemExit(f'Section [{name}] not found')
    return match.group(1)


def field(body, key, what):
    match = re.search(rf'^{key} = "([^"]*)"', body, re.MULTILINE)
    if match is None:
        raise SystemExit(f'Field not found: {what}')
    return match.group(1)


def workspace_packages():
    members = re.search(r'^members = \[([^\]]*)\]', section(text(CARGO), 'workspace'), re.MULTILINE)
    if members is None:
        raise SystemExit('Workspace members not found')
    paths = re.findall(r'"([^"]+)"', members.group(1))
    return {field(section(text(path + '/Cargo.toml'), 'package'), 'name', path) for path in paths}


def lock_packages():
    """(name, version, has source) of every Cargo.lock entry."""
    blocks = re.split(r'^\[\[package\]\]\r?\n', text(CARGO_LOCK), flags=re.MULTILINE)[1:]
    return [(field(block, 'name', CARGO_LOCK), field(block, 'version', CARGO_LOCK), re.search(r'^source = ', block, re.MULTILINE) is not None) for block in blocks]


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
        CARGO: field(section(text(CARGO), 'workspace.package'), 'version', CARGO),
    }
    lock = json.loads(text(PACKAGE_LOCK))
    found[PACKAGE_LOCK] = lock['version']
    found[PACKAGE_LOCK + ' (root package)'] = lock['packages']['']['version']
    names = workspace_packages()
    for name, version, registry in lock_packages():
        if name in names and not registry:
            found[CARGO_LOCK + ' (' + name + ')'] = version
    return found


def license_records():
    """Each license manifest with the lock file whose exact bytes it records."""
    records = [(str(path.relative_to(ROOT)), 'CargoLockSHA256', CARGO_LOCK) for path in sorted((ROOT / RUST_LICENSES).glob('*/manifest.json'))]
    return records + [(FRONTEND_LICENSES, 'PackageLockSHA256', PACKAGE_LOCK)]


def set_version(version):
    # Only a record that matches today's lock may follow the version-only edit; a stale
    # record means changed dependencies whose notices were never regenerated.
    for manifest, key, lock in license_records():
        if json.loads(text(manifest)).get(key) != sha(lock):
            raise SystemExit(f'{manifest} does not match {lock}; regenerate the license notices before releasing.')
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
    for manifest, key, lock in license_records():
        write(manifest, replace_once(text(manifest), rf'("{key}": )"[0-9a-f]{{64}}"', rf'\g<1>"{sha(lock)}"', manifest))


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
