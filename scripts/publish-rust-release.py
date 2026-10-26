"""Upload a fully verified bundle without replacing existing release assets."""
import argparse
import json
import os
import pathlib
import re
import subprocess
import tempfile


def gh(*args, missing_ok=False):
    result = subprocess.run(['gh', *map(str, args)], capture_output=True, text=True, check=False)
    if result.returncode:
        if missing_ok and ('HTTP 404' in result.stderr or 'release not found' in result.stderr.lower()):
            return None
        # Do not echo headers, tokens, environment or release body in errors.
        raise RuntimeError('GitHub release operation failed: ' + ' '.join(map(str, args[:2])))
    return result.stdout


def validate_target(repository, tag, commit):
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository) or not re.fullmatch(r'v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', tag) or not re.fullmatch('[a-f0-9]{40}', commit):
        raise ValueError('Invalid release target')


def verification_module():
    # Load the verifier, not an artifact-supplied script or executable.
    import importlib.util
    spec = importlib.util.spec_from_file_location('release_packages', pathlib.Path(__file__).with_name('release-packages.py'))
    checks = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checks)
    return checks


def validate_manifest(manifest, tag, commit, fingerprint=None):
    if manifest.get('version') != tag[1:] or manifest.get('commit') != commit or len(manifest.get('packages', [])) != 5:
        raise ValueError('Release bundle identity mismatch')
    suffixes = ['windows-x64.zip', 'windows-x64-setup.exe', 'linux-amd64.deb', 'osx-arm64.dmg', 'osx-x64.dmg']
    expected_packages = {f'HandyMeeting-{tag[1:]}-{suffix}' for suffix in suffixes}
    expected_files = {name + extension for name in expected_packages for extension in ['', '.sha256', '.verification.json' if name.endswith(('.zip', '.exe')) else '.manifest.json']}
    if {item['name'] for item in manifest['packages']} != expected_packages or len(manifest['files']) != len(expected_files) or {item['name'] for item in manifest['files']} != expected_files:
        raise ValueError('Unexpected release payload; only the five platform packages and proofs may be uploaded')
    if fingerprint is not None and manifest.get('sourceFingerprintSHA256') != fingerprint:
        raise ValueError('Existing release has a different source fingerprint')
    if manifest.get('actualNativeCredentialChecks') is not True or manifest.get('actualWindowsInstallUninstallChecked') is not True:
        raise ValueError('Release manifest lacks executed distribution checks')


def asset_matches(repository, tag, checks, asset, item):
    digest = asset.get('digest')
    if digest is None:
        with tempfile.TemporaryDirectory(prefix='handymeeting-release-verify-') as temp:
            gh('release', 'download', tag, '-R', repository, '--pattern', item['name'], '--dir', temp)
            digest = 'sha256:' + checks.sha(pathlib.Path(temp) / item['name'])
    return asset['size'] == item['bytes'] and digest == 'sha256:' + item['sha256']


def existing_complete(repository, tag, commit, fingerprint):
    validate_target(repository, tag, commit)
    raw = gh('api', f'repos/{repository}/releases/tags/{tag}', missing_ok=True)
    if raw is None:
        return False
    release = json.loads(raw)
    assets = {item['name']: item for item in release['assets']}
    if 'RELEASE_MANIFEST.json' not in assets:
        return False
    checks = verification_module()
    with tempfile.TemporaryDirectory(prefix='handymeeting-manifest-verify-') as temp:
        gh('release', 'download', tag, '-R', repository, '--pattern', 'RELEASE_MANIFEST.json', '--dir', temp)
        file = pathlib.Path(temp) / 'RELEASE_MANIFEST.json'
        manifest = checks.read(file)
        validate_manifest(manifest, tag, commit, fingerprint)
        rows = manifest['files'] + [{'name': 'RELEASE_MANIFEST.json', 'sha256': checks.sha(file), 'bytes': file.stat().st_size}]
        sums = ''.join(f'{item["sha256"]}  {item["name"]}\n' for item in manifest['packages']).encode()
        import hashlib
        rows.append({'name': 'SHA256SUMS.txt', 'sha256': hashlib.sha256(sums).hexdigest(), 'bytes': len(sums)})
        if any(item['name'] not in assets or not asset_matches(repository, tag, checks, assets[item['name']], item) for item in rows):
            raise ValueError('Existing release is incomplete or differs; published assets were preserved')
    return True


def publish(folder, repository, tag, commit, notes, make_public):
    validate_target(repository, tag, commit)
    checks = verification_module()
    manifest = checks.read(folder / 'RELEASE_MANIFEST.json')
    validate_manifest(manifest, tag, commit)
    tag_commit = json.loads(gh('api', f'repos/{repository}/commits/{tag}'))['sha']
    if tag_commit != commit:
        raise ValueError('Release tag moved or refers to another source')
    rows = manifest['files'] + [{'name': name, 'sha256': checks.sha(folder / name), 'bytes': (folder / name).stat().st_size} for name in ['SHA256SUMS.txt', 'RELEASE_MANIFEST.json']]
    for item in rows:
        if pathlib.PurePath(item['name']).name != item['name'] or re.search(r'[\\/:]', item['name']):
            raise ValueError('Unsafe release asset name')
        file = folder / item['name']
        if file.is_symlink() or not file.is_file() or checks.sha(file) != item['sha256'] or file.stat().st_size != item['bytes']:
            raise ValueError('Release bundle changed before upload')
    existing = gh('api', f'repos/{repository}/releases/tags/{tag}', missing_ok=True)
    if existing is None:
        gh('release', 'create', tag, '-R', repository, '--verify-tag', '--target', commit, '--draft', '--title', 'HandyMeeting ' + tag[1:], '--notes-file', notes)
    release = json.loads(gh('api', f'repos/{repository}/releases/tags/{tag}'))
    assets = {asset['name']: asset for asset in release['assets']}
    for item in rows:
        asset = assets.get(item['name'])
        if asset:
            if not asset_matches(repository, tag, checks, asset, item):
                raise ValueError('An existing release asset differs; it was preserved')
        else:
            gh('release', 'upload', tag, '-R', repository, folder / item['name'])
    final = json.loads(gh('api', f'repos/{repository}/releases/tags/{tag}'))
    uploaded = {asset['name']: asset for asset in final['assets']}
    if any(item['name'] not in uploaded or not asset_matches(repository, tag, checks, uploaded[item['name']], item) for item in rows):
        raise ValueError('Final GitHub assets do not match the verified bundle')
    if make_public:
        args = ['release', 'edit', tag, '-R', repository, '--draft=false']
        args += ['--prerelease', '--latest=false'] if '-' in tag else ['--prerelease=false', '--latest']
        gh(*args)
    print(json.dumps({'tag': tag, 'commit': commit, 'url': final['html_url'], 'assetsVerified': len(rows), 'publishedByThisRun': make_public, 'existingAssetsOverwritten': False}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--folder', type=pathlib.Path)
    parser.add_argument('--repository', default=os.environ.get('GITHUB_REPOSITORY'), required=False)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--notes', type=pathlib.Path)
    parser.add_argument('--check-existing', action='store_true')
    parser.add_argument('--source', type=pathlib.Path)
    parser.add_argument('--publish', action='store_true')
    args = parser.parse_args()
    if args.check_existing:
        if args.source is None:
            parser.error('--source is required for existing-release verification')
        print(json.dumps({'complete': existing_complete(args.repository, args.tag, args.commit, verification_module().read(args.source)['sha256'])}))
    else:
        if args.folder is None or args.notes is None:
            parser.error('--folder and --notes are required for publication')
        publish(args.folder, args.repository, args.tag, args.commit, args.notes, args.publish)
