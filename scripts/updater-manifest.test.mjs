import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { prepareUpdaterArtifacts } from './updater-manifest.mjs';

function fixture({ version = '1.6.0', configVersion = version, files = {} } = {}) {
  const base = path.resolve('artifacts/updater-packaging/test-fixtures'); fs.mkdirSync(base, { recursive: true });
  const root = fs.mkdtempSync(path.join(base, 'case-')), bundleDirectory = path.join(root, 'nsis'), outputDirectory = path.join(root, 'updater');
  fs.mkdirSync(bundleDirectory); fs.mkdirSync(path.join(root, 'src-tauri'));
  fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify({ version }));
  fs.writeFileSync(path.join(root, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: configVersion, productName: 'Arcvera-Studio' }));
  for (const [name, bytes] of Object.entries(files)) fs.writeFileSync(path.join(bundleDirectory, name), bytes);
  return { projectRoot: root, bundleDirectory, outputDirectory };
}
const installer = 'Arcvera-Studio_1.6.0_x64-setup.exe';
const bytes = Buffer.from('MZ synthetic NSIS bytes; not executable'), signature = 'fixture-public-signature\r\n';
const valid = () => ({ [installer]: bytes, [installer + '.sig']: signature });

test('UP03 real CLI emits exact fixed-repo manifest and preserves installer/signature bytes', () => {
  const f = fixture({ files: valid() });
  const script = fileURLToPath(new URL('./updater-manifest.mjs', import.meta.url));
  const result = spawnSync(process.execPath, [script, f.bundleDirectory, f.outputDirectory], { cwd: f.projectRoot, encoding: 'utf8', windowsHide: true });
  assert.equal(result.status, 0, result.stderr);
  const name = 'Arcvera-Studio-1.6.0-windows-x64-setup.exe';
  assert.deepEqual(fs.readFileSync(path.join(f.outputDirectory, name)), bytes);
  assert.equal(fs.readFileSync(path.join(f.outputDirectory, name + '.sig'), 'utf8'), signature);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(f.outputDirectory, 'arcvera-latest.json'))), {
    version: '1.6.0', platforms: { 'windows-x86_64': {
      url: 'https://github.com/jaeseongs95/Arcvera-Studio/releases/download/v1.6.0/Arcvera-Studio-1.6.0-windows-x64-setup.exe', signature: 'fixture-public-signature',
    } },
  });
  const legacy = JSON.parse(fs.readFileSync(path.join(f.outputDirectory, 'latest.json')));
  assert.equal(legacy.platforms['windows-x86_64'].url, 'https://github.com/jaeseongs95/opendiy-desktop/releases/download/v1.6.0/Arcvera-Studio-1.6.0-windows-x64-setup.exe');
  assert.equal(legacy.platforms['windows-x86_64'].signature, 'fixture-public-signature');
  assert.equal(legacy.version, '1.6.0');
  const sums = fs.readFileSync(path.join(f.outputDirectory, 'SHA256SUMS.txt'), 'utf8').trim().split('\n');
  assert.equal(sums.length, 4);
  for (const line of sums) { const [digest, file] = line.split('  '); assert.equal(digest, crypto.createHash('sha256').update(fs.readFileSync(path.join(f.outputDirectory, file))).digest('hex')); }
  assert.throws(() => prepareUpdaterArtifacts(f), /output already exists/);
});

for (const [name, files] of [
  ['absent NSIS', {}], ['multiple NSIS', { ...valid(), 'other.exe': bytes }],
  ['wrong version', { 'Arcvera-Studio_1.5.0_x64-setup.exe': bytes }], ['missing pair', { [installer]: bytes }],
  ['empty installer', { ...valid(), [installer]: '' }], ['empty signature', { ...valid(), [installer + '.sig']: ' \r\n' }],
  ['oversized signature', { ...valid(), [installer + '.sig']: 'x'.repeat(16 * 1024 + 1) }],
]) test(`UP04 rejects ${name} without creating updater output`, () => {
  const f = fixture({ files }); assert.throws(() => prepareUpdaterArtifacts(f)); assert.equal(fs.existsSync(f.outputDirectory), false);
});

for (const options of [{ version: '1.6.0', configVersion: '1.7.0' }, { version: '1.6.0-beta.1' }, { version: '../1.6.0' }]) {
  test(`UP05 rejects unsafe or inconsistent version ${options.version}/${options.configVersion ?? options.version}`, () => {
    const f = fixture({ ...options, files: valid() }); assert.throws(() => prepareUpdaterArtifacts(f), /matching stable/); assert.equal(fs.existsSync(f.outputDirectory), false);
  });
}
