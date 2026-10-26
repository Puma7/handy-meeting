"""Assemble only exact-version, hash-bound packages from successful build jobs."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil


def sha(file):
    with file.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(file):
    if not file.is_file() or file.is_symlink() or file.stat().st_size > 32 * 1024 * 1024:
        raise ValueError('Missing, linked or oversized release proof')
    return json.loads(file.read_text(encoding='utf-8-sig'))


def unique(folder, name):
    found = list(folder.rglob(name))
    if len(found) != 1:
        raise ValueError(f'Expected exactly one {name}, got {len(found)}')
    return found[0]


def validate_credentials(file, version, provider):
    value = read(file)
    if value.get('applicationVersion') != version or value.get('provider') != provider or value.get('success') is not True or value.get('actualNativeStore') is not True or value.get('simulated') is not False or value.get('usesUserEntries') is not False or value.get('checks', 0) < 5:
        raise ValueError('Actual isolated native credential checks required')


def assemble(artifacts, version, commit, fingerprint, output):
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', version) or not re.fullmatch(r'[a-f0-9]{40}', commit) or not re.fullmatch(r'[a-f0-9]{64}', fingerprint):
        raise ValueError('Invalid release identity')
    if output.exists():
        raise ValueError('Release output must be new; prior bundles remain unchanged')
    if artifacts.is_symlink():
        raise ValueError('Linked release artifact root')
    for file in artifacts.rglob('*'):
        if file.is_symlink():
            raise ValueError('Linked release artifact')
    verification = read(unique(artifacts / 'windows-checks', 'rust-verification.json'))
    if verification.get('success') is not True or verification.get('version') != version or verification.get('sourceFingerprintSHA256') != fingerprint:
        raise ValueError('Windows source verification mismatch')
    required = {'legacy', 'artifact-guards', 'distribution-guards', 'cargo-format', 'frontend-tests', 'frontend-build', 'rust-tests', 'rust-clippy'}
    if not required.issubset(verification.get('checks', {})) or any(verification['checks'][name].get('reportedExitCode') != 0 for name in required):
        raise ValueError('Missing or failed mandatory source check')
    for scope, provider in [('windows-checks', 'windows-credential-manager'), ('linux-checks', 'linux-secret-service'), ('mac-arm64-checks', 'macos-keychain'), ('mac-x64-checks', 'macos-keychain')]:
        validate_credentials(unique(artifacts / scope, 'credential-checks.json'), version, provider)
    definitions = [('windows', 'windows-x64.zip', 'verification'), ('windows', 'windows-x64-setup.exe', 'verification'), ('linux', 'linux-amd64.deb', 'manifest'), ('mac-arm64', 'osx-arm64.dmg', 'manifest'), ('mac-x64', 'osx-x64.dmg', 'manifest')]
    checked = []
    for scope, suffix, proof_kind in definitions:
        name = f'HandyMeeting-{version}-{suffix}'
        package = unique(artifacts / scope, name)
        if not package.is_file() or package.is_symlink():
            raise ValueError('Invalid package file')
        actual = sha(package)
        checksum = package.with_name(name + '.sha256')
        if not checksum.is_file() or checksum.read_text(encoding='utf-8-sig').strip().split() != [actual, name]:
            raise ValueError('Package checksum or filename mismatch')
        proof_file = package.with_name(name + '.' + proof_kind + '.json')
        proof = read(proof_file)
        if proof.get('sourceFingerprintSHA256', proof.get('sourceSHA256')) != fingerprint or proof.get('version', proof.get('applicationVersion')) != version:
            raise ValueError('Package belongs to a different source/version')
        if proof_kind == 'verification':
            if proof.get('success') is not True or proof.get('package') != name or proof.get('sha256') != actual or proof.get('bytes') != package.stat().st_size or proof.get('userRecordingsIncluded') is not False:
                raise ValueError('Windows package proof mismatch')
            if suffix.endswith('setup.exe') and (proof.get('actualInstallUninstallChecked') is not True or proof.get('existingDestinationRejected') is not True or proof.get('duplicateVersionRejected') is not True or proof.get('unknownUserFilePreserved') is not True or proof.get('recursiveDeletion') is not False):
                raise ValueError('Actual installer/uninstaller preservation check required')
        else:
            expected_rid = {'linux': 'linux-x64', 'mac-arm64': 'osx-arm64', 'mac-x64': 'osx-x64'}[scope]
            if proof.get('rid') != expected_rid or proof.get('releaseChecks', {}).get('success') is not True or proof.get('relocatedReleaseChecks', {}).get('success') is not True:
                raise ValueError('Native and relocated package checks required')
        checked.append((package, checksum, proof_file))
    # Nothing is published or copied until the entire matrix is verified.
    output.mkdir(parents=True)
    files = []
    for group in checked:
        for source in group:
            target = output / source.name
            with source.open('rb') as src, target.open('xb') as dst:
                shutil.copyfileobj(src, dst)
            if sha(target) != sha(source):
                raise ValueError('Release copy changed bytes')
            files.append({'name': source.name, 'sha256': sha(target), 'bytes': target.stat().st_size})
    packages = [{'name': p.name, 'sha256': sha(p), 'bytes': p.stat().st_size} for p, _, _ in checked]
    (output / 'SHA256SUMS.txt').write_text(''.join(f'{p["sha256"]}  {p["name"]}\n' for p in packages), encoding='utf-8', newline='\n')
    manifest = {'schemaVersion': 1, 'version': version, 'commit': commit, 'sourceFingerprintSHA256': fingerprint, 'packages': packages, 'files': files, 'actualNativeCredentialChecks': True, 'actualWindowsInstallUninstallChecked': True, 'physicalAudioDevicesTestedInCI': False, 'macNotarized': False, 'windowsSigned': False}
    (output / 'RELEASE_MANIFEST.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifacts', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--source', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    source = read(args.source)
    print(json.dumps(assemble(args.artifacts, args.version, args.commit, source['sha256'], args.output), indent=2))
