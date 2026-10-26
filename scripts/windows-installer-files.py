"""Generate literal NSIS file lists from a checked payload; never execute/delete files."""
import argparse
import hashlib
import json
import pathlib
import re


def sha(file):
    with file.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def literal(text):
    if any(ord(char) < 32 for char in text):
        raise ValueError('Control character in installer path')
    return text.replace('$', '$$').replace('"', '$\\"')


def entries(program, manifest):
    result = []
    for item in manifest['files']:
        name = item['path']
        path = pathlib.PurePosixPath(name)
        if not name or path.is_absolute() or '..' in path.parts or path.as_posix() != name or re.search(r'[\\:*?<>|"\x00-\x1f]', name):
            raise ValueError('Unsafe installer payload path')
        file = program / name
        current = program
        for part in path.parts:
            current /= part
            if current.is_symlink():
                raise ValueError('Linked installer payload')
        if not file.is_file() or sha(file) != item['sha256']:
            raise ValueError('Installer payload hash mismatch')
        result.append((name, file))
    if len({name.casefold() for name, _ in result}) != len(result) or not any(name == 'HandyMeeting.exe' for name, _ in result):
        raise ValueError('Duplicate payload or missing executable')
    return sorted(result)


def generate(program, manifest, output):
    if output.exists():
        raise ValueError('Installer list output must be new')
    files = entries(program, manifest)
    # BUILD_MANIFEST describes its payload rather than hashing itself.
    if (program / 'BUILD_MANIFEST.json').is_file():
        files.append(('BUILD_MANIFEST.json', program / 'BUILD_MANIFEST.json'))
    output.mkdir(parents=True)
    install, uninstall, folders = [], [], set()
    for name, file in files:
        parts = pathlib.PurePosixPath(name)
        parent = '' if parts.parent == pathlib.PurePosixPath('.') else '\\' + str(parts.parent).replace('/', '\\')
        install += [f'SetOutPath "$INSTDIR{literal(parent)}"', f'File "/oname={literal(parts.name)}" "{literal(str(file.resolve()))}"']
        uninstall.append(f'Delete "$INSTDIR\\{literal(name.replace("/", chr(92)))}"')
        for parent_dir in parts.parents:
            if str(parent_dir) != '.':
                folders.add(str(parent_dir).replace('/', '\\'))
    uninstall += [f'RMDir "$INSTDIR\\{literal(folder)}"' for folder in sorted(folders, key=lambda x: (-x.count('\\'), x))]
    (output / 'install.nsh').write_text('\n'.join(install) + '\n', encoding='utf-8-sig')
    (output / 'uninstall.nsh').write_text('\n'.join(uninstall) + '\n', encoding='utf-8-sig')
    return {'files': len(files), 'recursiveDeletion': False, 'userDataIncluded': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('program', type=pathlib.Path)
    parser.add_argument('output', type=pathlib.Path)
    args = parser.parse_args()
    manifest = json.loads((args.program / 'BUILD_MANIFEST.json').read_text(encoding='utf-8-sig'))
    print(json.dumps(generate(args.program, manifest, args.output)))
