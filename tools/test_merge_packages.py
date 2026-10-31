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
                               'version': version, 'apiVersion': 2, 'repository': 'https://github.com/verncat/player-plugins',
                               'platforms': ['macos', 'windows', 'linux'],
                               'image': 'https://raw.githubusercontent.com/verncat/player-plugins/main/soulseek/assets/soulseek.png'},
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
            catalog = module.merge(root / 'input', root / 'dist', 'verncat/player-plugins', 'native-v1.0.0', '2026-10-31')
            self.assertEqual(catalog['schemaVersion'], 2)
            self.assertTrue(catalog['plugins'][0]['installUrl'].endswith('/player.soulseek.player-plugin'))
            self.assertTrue(catalog['plugins'][0]['androidApkUrl'].endswith('/player.soulseek.apk'))
            self.assertIn('android', catalog['plugins'][0]['platforms'])
            self.assertEqual(catalog['plugins'][0]['image'], 'https://raw.githubusercontent.com/verncat/player-plugins/main/soulseek/assets/soulseek.png')
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
                module.merge(root / 'input', root / 'dist', 'repo', 'tag')

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


if __name__ == '__main__':
    unittest.main()
