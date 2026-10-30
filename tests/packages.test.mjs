import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import test from 'node:test';

const root = fileURLToPath(new URL('../', import.meta.url));
const folders = ['discord-rpc', 'soulseek', 'examples/hello', 'examples/android-native'];
const read = file => JSON.parse(readFileSync(path.join(root, file), 'utf8'));

test('all plugin packages can be packed independently of Player', () => {
  const temporary = mkdtempSync(path.join(tmpdir(), 'player-plugins-'));
  try {
    const ids = new Set();
    for (const folder of folders) {
      const manifest = read(`${folder}/plugin.json`);
      assert.equal(ids.has(manifest.id), false); ids.add(manifest.id);
      assert.equal(manifest.apiVersion, 1);
      assert.equal(manifest.repository, 'https://github.com/verncat/player-plugins');
      const output = path.join(temporary, `${manifest.id}.player-plugin`);
      // Run from another directory and use paths with spaces to check standalone usage.
      execFileSync(process.execPath, [path.join(root, 'scripts/package-plugin.mjs'), path.join(root, folder), output], { cwd: temporary });
      const packed = JSON.parse(readFileSync(output, 'utf8'));
      assert.deepEqual(packed.manifest, manifest);
      assert.equal(packed.script, readFileSync(path.join(root, folder, 'main.js'), 'utf8'));
      assert.ok(Buffer.byteLength(JSON.stringify(packed)) <= 4 * 1024 * 1024);
      new vm.Script(packed.script, { filename: `${folder}/main.js` });
    }
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});

test('catalog install URLs point at real packages with consistent metadata', () => {
  const catalog = read('catalog.json');
  assert.equal(catalog.schemaVersion, 1);
  assert.ok(Buffer.byteLength(JSON.stringify(catalog)) <= 512 * 1024);
  const ids = new Set();
  for (const entry of catalog.plugins) {
    assert.equal(ids.has(entry.id), false); ids.add(entry.id);
    assert.equal(entry.id.startsWith('player.'), false);
    const url = new URL(entry.installUrl);
    assert.equal(url.origin, 'https://github.com');
    const prefix = '/verncat/player-plugins/tree/main/';
    assert.ok(url.pathname.startsWith(prefix));
    const folder = decodeURIComponent(url.pathname.slice(prefix.length));
    assert.ok(folders.includes(folder));
    const manifest = read(`${folder}/plugin.json`);
    for (const key of ['id', 'name', 'version', 'repository', 'platforms']) assert.deepEqual(entry[key], manifest[key]);
    assert.ok(entry.addedAt <= (entry.updatedAt ?? catalog.updatedAt));
    assert.ok((entry.updatedAt ?? entry.addedAt) <= catalog.updatedAt);
  }
});
