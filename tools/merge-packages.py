#!/usr/bin/env python3
"""Merge native release artifacts; emit a catalog only for files in this release."""
import argparse
import base64
import datetime
import hashlib
import json
from urllib.parse import quote

MAX_LIBRARY_BYTES = 64 * 1024 * 1024
MAX_RUNTIME_BYTES = 256 * 1024 * 1024
MAX_TARGET_RUNTIME_BYTES = 384 * 1024 * 1024
MAX_PACKAGE_BYTES = 768 * 1024 * 1024
TARGETS = {'macos-aarch64', 'macos-x86_64', 'linux-aarch64', 'linux-x86_64', 'windows-aarch64', 'windows-x86_64'}

def validate_runtime_name(name):
    if (not isinstance(name, str) or not 1 <= len(name) <= 128 or name.endswith('.')
            or not name.isascii() or any(not (c.isalnum() or c in '._-') for c in name)
            or name in ('.', '..')):
        raise ValueError('Invalid runtime filename')
    lower = name.lower()
    stem = lower.split('.')[0]
    if (lower in ('plugin.dll', 'plugin.so', 'plugin.dylib') or stem in ('con', 'prn', 'aux', 'nul')
            or (len(stem) == 4 and stem[:3] in ('com', 'lpt') and stem[3] in '123456789')):
        raise ValueError('Reserved runtime filename')

def verify_payload(payload, maximum):
    encoded = payload['data']
    if not isinstance(encoded, str) or len(encoded) > ((maximum + 2) // 3) * 4:
        raise ValueError('Artifact exceeds size limit')
    data = base64.b64decode(encoded, validate=True)
    if not data or len(data) > maximum or hashlib.sha256(data).hexdigest() != payload['sha256']:
        raise ValueError('Corrupt or oversized native artifact')
    return len(data)

def verify_artifact(target, artifact):
    if target not in TARGETS:
        raise ValueError('Unsupported native target')
    verify_payload(artifact, MAX_LIBRARY_BYTES)
    files = artifact.get('files', {})
    if not isinstance(files, dict):
        raise ValueError('Runtime files must be an object')
    names = set()
    total = 0
    for name, payload in files.items():
        validate_runtime_name(name)
        if name.lower() in names:
            raise ValueError('Case-colliding runtime filenames')
        names.add(name.lower())
        if not isinstance(payload.get('executable'), bool):
            raise ValueError('Runtime executable must be a boolean')
        total += verify_payload(payload, MAX_RUNTIME_BYTES)
        if total > MAX_TARGET_RUNTIME_BYTES:
            raise ValueError('Target runtimes exceed 384 MiB')

def write_package(file, package):
    encoded = json.dumps(package, separators=(',', ':')) + '\n'
    if len(encoded.encode()) > MAX_PACKAGE_BYTES:
        raise ValueError(f'Package exceeds 768 MiB: {file.name}')
    file.write_text(encoded)
from pathlib import Path


def catalog_image(manifest, repository, tag, sources):
    image = manifest.get('image')
    if image is None or image.startswith(('https://', 'http://')):
        return image
    # Packed metadata embeds local assets for offline installation. Catalogs link
    # directly to the source file at the immutable release tag instead.
    candidates = [*sources.glob('*/plugin.json'), *sources.glob('examples/*/plugin.json')]
    for file in candidates:
        source = json.loads(file.read_text())
        if source.get('id') != manifest['id']:
            continue
        local = source.get('image', '')
        if not local or ':' in local:
            break
        path = (file.parent / local).resolve()
        relative = path.relative_to(sources.resolve())
        if not path.is_file():
            raise ValueError(f'Missing catalog image file: {path}')
        return f'https://raw.githubusercontent.com/{repository}/{quote(tag, safe="")}/{quote(relative.as_posix())}'
    raise ValueError(f'Catalog image must link to an image file: {manifest["id"]}')


def merge(inputs, output, repository, tag, date=None, sources=None):
    sources = sources or Path(__file__).resolve().parents[1]
    output.mkdir(parents=True, exist_ok=True)
    groups = {}
    for file in sorted(inputs.rglob('*.player-plugin')):
        if file.stat().st_size > MAX_PACKAGE_BYTES:
            raise ValueError(f'Package exceeds 768 MiB: {file}')
        package = json.loads(file.read_text())
        manifest = package['manifest']
        if manifest['apiVersion'] != 2 or package['native']['kind'] != 'library':
            raise ValueError(f'Not a native API 2 package: {file}')
        group = groups.setdefault(manifest['id'], {'manifest': manifest, 'native': {'kind': 'library', 'artifacts': {}}})
        if group['manifest'] != manifest:
            raise ValueError(f'Manifest mismatch: {file}')
        for target, artifact in package['native']['artifacts'].items():
            if target.split('-')[0] not in manifest.get('platforms', []):
                raise ValueError('Artifact OS is absent from manifest platforms')
            if target in group['native']['artifacts']:
                raise ValueError(f'Duplicate target: {target}')
            verify_artifact(target, artifact)
            group['native']['artifacts'][target] = artifact
    if not groups:
        raise ValueError('No native packages to publish')
    added = date or datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    entries = []
    for plugin_id, package in groups.items():
        artifacts = package['native']['artifacts']
        split = any(artifact.get('files') for artifact in artifacts.values())
        urls = {}
        if split:
            for target, artifact in artifacts.items():
                file = output / f'{plugin_id}-{target}.player-plugin'
                write_package(file, {'manifest': package['manifest'], 'native': {'kind': 'library', 'artifacts': {target: artifact}}})
                urls[target] = f'https://github.com/{repository}/releases/download/{tag}/{file.name}'
        else:
            file = output / f'{plugin_id}.player-plugin'
            write_package(file, package)
        manifest = package['manifest']
        entry = {key: manifest[key] for key in ('id', 'name', 'description', 'repository', 'version', 'author', 'image', 'platforms') if key in manifest}
        if 'image' in entry:
            entry['image'] = catalog_image(manifest, repository, tag, sources)
        entry.update(addedAt=added, updatedAt=added,
                     tags=['native', 'music'] if plugin_id in ('player.soulseek', 'player.youtube') else ['native', 'presence'])
        if split:
            entry['installUrls'] = urls
        else:
            entry['installUrl'] = f'https://github.com/{repository}/releases/download/{tag}/{file.name}'
        apk_files = list(inputs.rglob(f'{plugin_id}.apk'))
        if apk_files:
            if len(apk_files) != 1:
                raise ValueError(f'Duplicate APK: {plugin_id}')
            apk = apk_files[0]
            (output / apk.name).write_bytes(apk.read_bytes())
            entry['androidApkUrl'] = f'https://github.com/{repository}/releases/download/{tag}/{apk.name}'
            entry['platforms'] = [*entry['platforms'], 'android']
        entries.append(entry)
    return {'schemaVersion': 2, 'updatedAt': added, 'plugins': entries}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('inputs', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--repository', default='NoName-Audio/player-plugins')
    parser.add_argument('--tag', required=True)
    parser.add_argument('--catalog', type=Path, default=Path('catalog.json'))
    args = parser.parse_args()
    catalog = merge(args.inputs, args.output, args.repository, args.tag)
    if args.catalog.exists():
        previous = {p['id']: p for p in json.loads(args.catalog.read_text())['plugins']}
        for entry in catalog['plugins']:
            if entry['id'] in previous:
                entry['addedAt'] = previous[entry['id']]['addedAt']
    args.catalog.write_text(json.dumps(catalog, indent=2) + '\n')
