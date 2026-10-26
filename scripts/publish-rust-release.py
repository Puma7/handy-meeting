"""Upload a fully verified bundle without replacing existing release assets."""
import argparse
import json
import os
import pathlib
import re
import subprocess
import tempfile
from urllib.parse import quote


def gh(*args, missing_ok=False, output_path=None):
    command = ['gh', *map(str, args)]
    if output_path is None:
        result = subprocess.run(command, capture_output=True, text=True, check=False)
    else:
        with output_path.open('xb') as output:
            result = subprocess.run(command, stdout=output, stderr=subprocess.PIPE, check=False)
    if result.returncode:
        error = result.stderr if isinstance(result.stderr, str) else result.stderr.decode(errors='replace')
        if missing_ok and ('HTTP 404' in error or 'release not found' in error.lower()):
            return None
        # Do not echo headers, tokens, environment or release body in errors.
        raise RuntimeError('GitHub release operation failed: ' + ' '.join(map(str, args[:2])))
    return result.stdout


def release_identity(release, tag, expected_id=None):
    if (not isinstance(release, dict) or type(release.get('id')) is not int
            or release['id'] <= 0 or release.get('tag_name') != tag
            or (expected_id is not None and release['id'] != expected_id)):
        raise ValueError('Release identity mismatch; existing releases were preserved')
    return release


def get_release(repository, tag, release_id):
    if type(release_id) is not int or release_id <= 0:
        raise ValueError('Invalid release ID')
    return release_identity(json.loads(gh('api', f'repos/{repository}/releases/{release_id}')), tag, release_id)


def find_release(repository, tag, release_id=None):
    if release_id is not None:
        return get_release(repository, tag, release_id)
    # GitHub's tag endpoint returns published releases only. A 404 does not mean
    # there is no draft; inspect every page before deciding to create one.
    raw = gh('api', f'repos/{repository}/releases/tags/{tag}', missing_ok=True)
    if raw is not None:
        return release_identity(json.loads(raw), tag)
    matches = []
    for page in range(1, 1001):
        rows = json.loads(gh('api', f'repos/{repository}/releases?per_page=100&page={page}'))
        if not isinstance(rows, list):
            raise ValueError('Invalid release listing')
        matches.extend(release_identity(row, tag) for row in rows if row.get('tag_name') == tag)
        if len(rows) < 100:
            break
    else:
        raise ValueError('Release listing limit reached; no release was created')
    if len(matches) > 1:
        raise ValueError('Multiple releases use this tag; select an explicit --release-id. All drafts were preserved.')
    return matches[0] if matches else None


def download_asset(repository, asset, output):
    if type(asset.get('id')) is not int or asset['id'] <= 0:
        raise ValueError('Invalid release asset ID')
    gh('api', f'repos/{repository}/releases/assets/{asset["id"]}',
       '-H', 'Accept: application/octet-stream', output_path=output)


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
            output = pathlib.Path(temp) / item['name']
            download_asset(repository, asset, output)
            digest = 'sha256:' + checks.sha(output)
    return asset['size'] == item['bytes'] and digest == 'sha256:' + item['sha256']


def asset_index(release, rows, complete=False):
    assets = release['assets']
    names = [item['name'] for item in assets]
    expected = {item['name'] for item in rows}
    if len(set(names)) != len(names) or not set(names).issubset(expected):
        raise ValueError('Unexpected existing release assets; all files were preserved and publication was refused')
    if complete and set(names) != expected:
        raise ValueError('Existing release is incomplete or differs; all files were preserved')
    return {item['name']: item for item in assets}


def existing_complete(repository, tag, commit, fingerprint, release_id=None):
    validate_target(repository, tag, commit)
    release = find_release(repository, tag, release_id)
    if release is None:
        return False
    assets = {item['name']: item for item in release['assets']}
    if 'RELEASE_MANIFEST.json' not in assets:
        return False
    checks = verification_module()
    with tempfile.TemporaryDirectory(prefix='handymeeting-manifest-verify-') as temp:
        file = pathlib.Path(temp) / 'RELEASE_MANIFEST.json'
        download_asset(repository, assets['RELEASE_MANIFEST.json'], file)
        manifest = checks.read(file)
        validate_manifest(manifest, tag, commit, fingerprint)
        rows = manifest['files'] + [{'name': 'RELEASE_MANIFEST.json', 'sha256': checks.sha(file), 'bytes': file.stat().st_size}]
        sums = ''.join(f'{item["sha256"]}  {item["name"]}\n' for item in manifest['packages']).encode()
        import hashlib
        rows.append({'name': 'SHA256SUMS.txt', 'sha256': hashlib.sha256(sums).hexdigest(), 'bytes': len(sums)})
        assets = asset_index(release, rows, complete=True)
        if any(item['name'] not in assets or not asset_matches(repository, tag, checks, assets[item['name']], item) for item in rows):
            raise ValueError('Existing release is incomplete or differs; published assets were preserved')
    return True


def publish(folder, repository, tag, commit, notes, make_public, release_id=None):
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
    existing = find_release(repository, tag, release_id)
    if existing is None:
        gh('release', 'create', tag, '-R', repository, '--verify-tag', '--target', commit, '--draft', '--title', 'HandyMeeting ' + tag[1:], '--notes-file', notes)
        existing = find_release(repository, tag)
        if existing is None:
            raise ValueError('Created draft could not be located; inspect before retrying')
    release_id = existing['id']
    release = get_release(repository, tag, release_id)
    assets = asset_index(release, rows)
    for item in rows:
        asset = assets.get(item['name'])
        if asset:
            if not asset_matches(repository, tag, checks, asset, item):
                raise ValueError('An existing release asset differs; it was preserved')
        else:
            endpoint = f'https://uploads.github.com/repos/{repository}/releases/{release_id}/assets?name={quote(item["name"], safe="")}'
            gh('api', '--method', 'POST', endpoint, '-H', 'Content-Type: application/octet-stream', '--input', folder / item['name'])
    final = get_release(repository, tag, release_id)
    uploaded = asset_index(final, rows, complete=True)
    if any(item['name'] not in uploaded or not asset_matches(repository, tag, checks, uploaded[item['name']], item) for item in rows):
        raise ValueError('Final GitHub assets do not match the verified bundle')
    if make_public:
        args = ['api', '--method', 'PATCH', f'repos/{repository}/releases/{release_id}', '-F', 'draft=false']
        args += ['-F', 'prerelease=true', '-f', 'make_latest=false'] if '-' in tag else ['-F', 'prerelease=false', '-f', 'make_latest=true']
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
    parser.add_argument('--release-id', type=int, help='Explicit release identity for recovery when multiple drafts share a tag')
    args = parser.parse_args()
    if args.check_existing:
        if args.source is None:
            parser.error('--source is required for existing-release verification')
        print(json.dumps({'complete': existing_complete(args.repository, args.tag, args.commit, verification_module().read(args.source)['sha256'], args.release_id)}))
    else:
        if args.folder is None or args.notes is None:
            parser.error('--folder and --notes are required for publication')
        publish(args.folder, args.repository, args.tag, args.commit, args.notes, args.publish, args.release_id)
