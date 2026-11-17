import base64
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('merge_packages', Path(__file__).with_name('merge-packages.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PublishingTests(unittest.TestCase):
    def fixture(self, directory, target, version='1.0.0'):
        directory.mkdir(parents=True, exist_ok=True)
        data = b'native library fixture'
        package = {'manifest': {'id': 'player.soulseek', 'name': 'Soulseek', 'description': 'Music',
                               'version': version, 'apiVersion': 2, 'repository': 'https://github.com/NoName-Audio/player-plugins',
                               'platforms': ['macos', 'windows', 'linux'],
                               'image': 'https://raw.githubusercontent.com/NoName-Audio/player-plugins/main/soulseek/assets/soulseek.png'},
                   'native': {'kind': 'library', 'artifacts': {target: {'data': base64.b64encode(data).decode(),
                                                                     'sha256': hashlib.sha256(data).hexdigest()}}}}
        file = directory / f'{target}.player-plugin'
        file.write_text(json.dumps(package))
        return file

    def test_merges_verified_targets_and_lists_separate_apk(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root / 'input', 'macos-aarch64')
            self.fixture(root / 'input', 'windows-x86_64')
            (root / 'input/player.soulseek.apk').write_bytes(b'apk fixture')
            catalog = module.merge(root / 'input', root / 'dist', 'NoName-Audio/player-plugins', 'native-v1.0.0', '2026-10-31')
            self.assertEqual(catalog['schemaVersion'], 2)
            self.assertTrue(catalog['plugins'][0]['installUrl'].endswith('/player.soulseek.player-plugin'))
            self.assertTrue(catalog['plugins'][0]['androidApkUrl'].endswith('/player.soulseek.apk'))
            self.assertIn('android', catalog['plugins'][0]['platforms'])
            self.assertEqual(catalog['plugins'][0]['image'], 'https://raw.githubusercontent.com/NoName-Audio/player-plugins/main/soulseek/assets/soulseek.png')
            package = json.loads((root / 'dist/player.soulseek.player-plugin').read_text())
            self.assertEqual(len(package['native']['artifacts']), 2)

    def test_catalog_rejects_embedded_images(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            file = self.fixture(root / 'input', 'macos-aarch64')
            package = json.loads(file.read_text())
            package['manifest']['image'] = 'data:image/png;base64,aWNvbg=='
            file.write_text(json.dumps(package))
            with self.assertRaisesRegex(ValueError, 'Catalog image must link to an image file'):
                module.merge(root / 'input', root / 'dist', 'repo', 'tag', sources=root / 'sources')

    def test_catalog_links_source_asset_instead_of_embedded_package_image(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            file = self.fixture(root / 'input', 'macos-aarch64')
            package = json.loads(file.read_text())
            package['manifest']['image'] = 'data:image/png;base64,aWNvbg=='
            file.write_text(json.dumps(package))
            folder = root / 'sources/soulseek'
            (folder / 'assets').mkdir(parents=True)
            (folder / 'assets/icon.png').write_bytes(b'icon')
            (folder / 'plugin.json').write_text(json.dumps({'id':'player.soulseek', 'image':'assets/icon.png'}))
            catalog = module.merge(root / 'input', root / 'dist', 'owner/repo', 'native-v2', sources=root / 'sources')
            self.assertEqual(catalog['plugins'][0]['image'], 'https://raw.githubusercontent.com/owner/repo/native-v2/soulseek/assets/icon.png')
            packed = json.loads((root / 'dist/player.soulseek.player-plugin').read_text())
            self.assertTrue(packed['manifest']['image'].startswith('data:'))

    def test_rejects_mixed_versions_and_tampered_binaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root / 'input', 'macos-aarch64')
            second = self.fixture(root / 'input', 'windows-x86_64', '2.0.0')
            with self.assertRaises(ValueError):
                module.merge(root / 'input', root / 'dist', 'repo', 'tag')
            second.unlink()
            package = json.loads((root / 'input/macos-aarch64.player-plugin').read_text())
            package['native']['artifacts']['macos-aarch64']['sha256'] = '0' * 64
            (root / 'input/macos-aarch64.player-plugin').write_text(json.dumps(package))
            with self.assertRaises(ValueError):
                module.merge(root / 'input', root / 'dist', 'repo', 'tag')



    def test_runtime_targets_publish_separately_without_dropping_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for target in ('macos-aarch64', 'windows-x86_64'):
                file = self.fixture(root / 'input', target)
                package = json.loads(file.read_text())
                data = b'private runtime'
                package['native']['artifacts'][target]['files'] = {'deno': {
                    'data': base64.b64encode(data).decode(),
                    'sha256': hashlib.sha256(data).hexdigest(), 'executable': True}}
                file.write_text(json.dumps(package))
            catalog = module.merge(root / 'input', root / 'dist', 'repo', 'tag')
            entry = catalog['plugins'][0]
            self.assertNotIn('installUrl', entry)
            self.assertEqual(set(entry['installUrls']), {'macos-aarch64', 'windows-x86_64'})
            for target in entry['installUrls']:
                packed = json.loads((root / 'dist' / f'player.soulseek-{target}.player-plugin').read_text())
                self.assertEqual(list(packed['native']['artifacts']), [target])
                self.assertTrue(packed['native']['artifacts'][target]['files']['deno']['executable'])

    def test_runtime_hash_names_and_bounds_are_validated(self):
        for name in ('../deno', 'DENO.', 'CON.txt', 'plugin.dll', '', 'é', 'a' * 129):
            with self.subTest(name=name), self.assertRaises(ValueError):
                module.validate_runtime_name(name)
        data = b'valid'
        payload = {'data': base64.b64encode(data).decode(), 'sha256': hashlib.sha256(data).hexdigest(), 'executable': True}
        artifact = {**payload, 'files': {'deno': payload}}
        module.verify_artifact('linux-x86_64', artifact)
        with self.assertRaises(ValueError):
            module.verify_artifact('linux-x86_64', {**artifact, 'files': {'deno': payload, 'DENO': payload}})
        with self.assertRaises(ValueError):
            module.verify_artifact('linux-x86_64', {**artifact, 'files': {'deno': {**payload, 'sha256': '0' * 64}}})
        with self.assertRaises(ValueError):
            module.verify_payload(payload, 2)
        original = module.MAX_TARGET_RUNTIME_BYTES
        try:
            module.MAX_TARGET_RUNTIME_BYTES = len(data) - 1
            with self.assertRaisesRegex(ValueError, '384 MiB'):
                module.verify_artifact('linux-x86_64', artifact)
        finally:
            module.MAX_TARGET_RUNTIME_BYTES = original
if __name__ == '__main__':
    unittest.main()
