#!/usr/bin/env python3
"""Merge native release artifacts; emit a catalog only for files in this release."""
import argparse
import base64
import datetime
import hashlib
import json
from pathlib import Path


def merge(inputs, output, repository, tag, date=None):
    output.mkdir(parents=True, exist_ok=True)
    groups = {}
    for file in sorted(inputs.rglob('*.player-plugin')):
        package = json.loads(file.read_text())
        manifest = package['manifest']
        if manifest['apiVersion'] != 2 or package['native']['kind'] != 'library':
            raise ValueError(f'Not a native API 2 package: {file}')
        group = groups.setdefault(manifest['id'], {'manifest': manifest, 'native': {'kind': 'library', 'artifacts': {}}})
        if group['manifest'] != manifest:
            raise ValueError(f'Manifest mismatch: {file}')
        for target, artifact in package['native']['artifacts'].items():
            if target in group['native']['artifacts']:
                raise ValueError(f'Duplicate target: {target}')
            data = base64.b64decode(artifact['data'], validate=True)
            if hashlib.sha256(data).hexdigest() != artifact['sha256']:
                raise ValueError(f'Corrupt native artifact: {file}')
            group['native']['artifacts'][target] = artifact
    if not groups:
        raise ValueError('No native packages to publish')
    added = date or datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    entries = []
    for plugin_id, package in groups.items():
        file = output / f'{plugin_id}.player-plugin'
        file.write_text(json.dumps(package, separators=(',', ':')) + '\n')
        if file.stat().st_size > 96 * 1024 * 1024:
            raise ValueError(f'Package exceeds 96 MiB: {plugin_id}')
        manifest = package['manifest']
        entry = {key: manifest[key] for key in ('id', 'name', 'description', 'repository', 'version', 'author', 'image', 'platforms') if key in manifest}
        entry.update(installUrl=f'https://github.com/{repository}/releases/download/{tag}/{file.name}', addedAt=added, updatedAt=added,
                     tags=['native', 'music'] if plugin_id == 'player.soulseek' else ['native', 'presence'])
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
    parser.add_argument('--repository', default='verncat/player-plugins')
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
