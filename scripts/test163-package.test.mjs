import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import vm from 'node:vm';
import { prepareUpdaterArtifacts } from './updater-manifest.mjs';

const BASE = 'd03a00e54e5c72947d2d20705932fb17110f5bd3';
const TITLE = 'OpenDIY Desktop 163 TEST', ID = 'io.agentgrid.desktop.test163';
const read = file => fs.readFileSync(file), sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const productionConfigSHA = 'b4f2fac8af938b4cdfa414438e9a5510e879ce58fec9083cb0d0051dedcbffba';
const preserved = [
  {
    "path": ".github/workflows/windows.yml",
    "sha256": "95f80e833fb9258dbb77a323f1cbaddf6c797923b437e5e4e577859637e69ceb"
  },
  {
    "path": "package-lock.json",
    "sha256": "52b297ab6049a50e9c171d56d47fd91fca1c24cee0f32c4ff1c672afa9617bed"
  },
  {
    "path": "package.json",
    "sha256": "36bc36f4973c241e1f394b5a347bccf68a2057a2a0795f0dd5c3c4c7c049d960"
  },
  {
    "path": "scripts/notices.mjs",
    "sha256": "871e74fb43739f5d770765b70dee1db20081a6b13d6dc94d9478b0318ac32682"
  },
  {
    "path": "scripts/updater-manifest.mjs",
    "sha256": "8266d749dde13d25a6242f4bf6e585365311b483cafcb671fae827eead28739c"
  },
  {
    "path": "src-tauri/Cargo.lock",
    "sha256": "24115a347c0393725fb8a1b009cbcc077d530b6f5451892447841bbdaf7613f8"
  },
  {
    "path": "src-tauri/Cargo.toml",
    "sha256": "22e821df968512246fb08b65aebda5774bc3a433f557d4993a41ccb8e93f1da6"
  },
  {
    "path": "src-tauri/src/desktop_updates.rs",
    "sha256": "a78bfbf89b6d6a0796c71e58a68baa96e97f2bd52263aabece741dbe60642e50"
  },
  {
    "path": "src-tauri/src/lib.rs",
    "sha256": "d2b4b5cce43340d6fc254f5d8ad1a507529a8193590622425405acebf98a7aed"
  },
  {
    "path": "src-tauri/src/process_job.rs",
    "sha256": "1b01ae9aeee9b46fe94658e1fdb78ff973226f447a332621cdeb124dbebce834"
  },
  {
    "path": "src-tauri/tauri.bundle.conf.json",
    "sha256": "b6bbbefa97941668ba87bdafef87dc8e31c146512915f615b2ed374ccb18a5ce"
  },
  {
    "path": "src-tauri/vendor/portable-pty/src/lib.rs",
    "sha256": "e218b8a450d5f70784d97582088346ae378212caa924b422a213235d07c63516"
  },
  {
    "path": "src-tauri/vendor/tauri-plugin-updater/src/updater.rs",
    "sha256": "f716457bb3ab298f21305fc1e4f1e309e27e8b78171c008cbbcbf2f3bc31c94e"
  }
];
function exactIdentity(bytes) {
  const text = bytes.toString('utf8'), config = JSON.parse(text);
  assert.equal(config.productName, TITLE); assert.equal(config.identifier, ID);
  assert.equal(config.app.windows[0].title, TITLE); assert.equal(config.version, '1.6.3');
  const restored = text.replace('"productName": "' + TITLE + '"', '"productName": "OpenDIY Desktop"')
    .replace('"identifier": "' + ID + '"', '"identifier": "io.agentgrid.desktop"')
    .replace('"title": "' + TITLE + '"', '"title": "OpenDIY Desktop"');
  assert.equal(sha(restored), productionConfigSHA, 'Only exact three config values may differ from d03');
  return config;
}
test('TC40-IDENTITY exact three values and full window/resources merge remain intact', () => {
  const config = exactIdentity(read('src-tauri/tauri.conf.json'));
  const overlay = JSON.parse(read('src-tauri/tauri.bundle.conf.json'));
  assert.deepEqual(Object.keys(overlay), ['$schema', 'bundle']);
  assert.deepEqual(Object.keys(overlay.bundle), ['resources']);
  const merged = { ...config, bundle: { ...config.bundle, ...overlay.bundle } };
  assert.deepEqual(merged, config, 'Resources-only unchanged overlay cannot replace windows or other config');
  assert.deepEqual(merged.app.windows, [{ label: 'main', title: TITLE, dragDropEnabled: false, width: 1400, height: 900, minWidth: 800, minHeight: 540 }]);
  assert.equal(merged.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(merged.plugins.updater.pubkey, '');
});
for (const [name, alter] of [
  ['wrong identifier', s => s.replace(ID, 'io.agentgrid.desktop')],
  ['missing width', s => s.replace('        "width": 1400,\n', '')],
]) test('TC40-IDENTITY rejects ' + name, () => {
  assert.throws(() => exactIdentity(Buffer.from(alter(read('src-tauri/tauri.conf.json').toString('utf8')))));
});
test('TC40-CI dedicated branch single packaging job preserves signing/resources/selftest path', () => {
  const workflow = read('.github/workflows/windows-test163.yml').toString('utf8');
  assert.match(workflow, /on:\n  push:\n    branches: \[test\/desktop163-signed-isolation\]/);
  assert.match(workflow, /if: github\.ref == 'refs\/heads\/test\/desktop163-signed-isolation'/);
  assert.deepEqual(workflow.split('jobs:\n')[1].match(/^  [a-zA-Z0-9_]+:/gm), ['  test163_package:']);
  for (const literal of [
    'CARGO_BUILD_JOBS: 2', 'CARGO_INCREMENTAL: 0', 'shared-key: windows', 'cache: npm',
    'run: npm ci', 'run: node scripts/notices.mjs', 'npm run tauri -- build --config src-tauri/tauri.bundle.conf.json --config $updaterConfig --bundles nsis -- --locked',
    'a8c985f62b8182b7fc54d892fb36a27140c3f27e590ae5e891d42ff494b01803',
    'vars.OPENDIY_UPDATER_PUBLIC_KEY', 'secrets.TAURI_SIGNING_PRIVATE_KEY', 'secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD',
    'does not match the public key', 'THIRD_PARTY_NOTICES.md', 'Compress-Archive',
    "'--self-test','--duration-seconds','60'", 'WindowStyle Hidden', 'self-test-execution.json',
    'run: node scripts/updater-manifest.mjs src-tauri/target/release/bundle/nsis artifacts/test163-package/updater',
    'source-binding.json', 'package-binding.json', 'Merged config differs beyond exact three identity values',
    'Build changed pinned input', 'actions/upload-artifact@v4',
  ]) assert.ok(workflow.includes(literal), literal);
  assert.equal((workflow.match(/secrets\./g) || []).length, 2, 'Only existing signing step env references private values');
  assert.doesNotMatch(workflow, /workflow_dispatch:|pull_request:|contents: write|gh release|create-release|upload-release-asset|Get-Process|Get-CimInstance|Stop-Process|Remove-Item/);
  const positions = ['run: npm ci', 'run: node scripts/notices.mjs', 'name: Bind unchanged product', 'name: Build updater-signed', 'name: Prepare portable', 'name: Prepare updater assets', 'name: Verify unchanged inputs'].map(s => workflow.indexOf(s));
  assert.ok(positions.every((n, i) => n >= 0 && (i === 0 || n > positions[i - 1])));
});
test('TC40-MANIFEST original generator accepts TEST163 NSIS filename without changing source', () => {
  const base = path.resolve('artifacts/test40'); fs.mkdirSync(base, { recursive: true });
  const root = fs.mkdtempSync(path.join(base, 'test163-manifest-')), bundle = path.join(root, 'nsis');
  fs.mkdirSync(bundle); fs.mkdirSync(path.join(root, 'src-tauri'));
  fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify({ version: '1.6.3' }));
  fs.writeFileSync(path.join(root, 'src-tauri/tauri.conf.json'), read('src-tauri/tauri.conf.json'));
  const name = TITLE + '_1.6.3_x64-setup.exe', bytes = Buffer.from('synthetic TEST163 fixture, not an executable'), signature = 'synthetic-public-signature\r\n';
  fs.writeFileSync(path.join(bundle, name), bytes); fs.writeFileSync(path.join(bundle, name + '.sig'), signature);
  const outputDirectory = path.join(root, 'updater');
  prepareUpdaterArtifacts({ projectRoot: root, bundleDirectory: bundle, outputDirectory });
  const normalized = 'OpenDIYDesktop-1.6.3-windows-x64-setup.exe';
  assert.deepEqual(read(path.join(outputDirectory, normalized)), bytes);
  assert.equal(read(path.join(outputDirectory, normalized + '.sig')).toString('utf8'), signature);
  const manifest = JSON.parse(read(path.join(outputDirectory, 'latest.json')));
  assert.equal(manifest.version, '1.6.3'); assert.equal(manifest.platforms['windows-x86_64'].signature, signature.trim());
  assert.equal(read(path.join(outputDirectory, 'SHA256SUMS.txt')).toString('utf8').trim().split('\n').length, 3);
});
test('TC40-SCOPE frozen product/lock/permissions/build source remains byte-identical', () => {
  for (const pin of preserved) assert.equal(sha(read(pin.path)), pin.sha256, pin.path);
  const allowed = new Set(['src-tauri/tauri.conf.json', '.github/workflows/windows-test163.yml', 'scripts/test163-package.test.mjs', 'docs/desktop163-test40-cases.ko.md']);
  const changed = execFileSync('git', ['diff', '--name-only', BASE], { encoding: 'utf8', windowsHide: true }).trim().split(/\r?\n/).filter(Boolean);
  assert.deepEqual(changed.filter(file => !allowed.has(file)), []);
});
test('TC40-HELPER actual workflow rewrite is confined to three default-off identity tokens', () => {
  const workflow = read('.github/workflows/windows-test163.yml').toString('utf8');
  const embedded = workflow.match(/\$patch=@'\n([\s\S]*?)\n          '@/);
  assert.ok(embedded, 'Actual embedded helper rewrite script required');
  const code = embedded[1].split('\n').map(line => line.slice(10)).join('\n');
  const file = 'src-tauri/src/desktop_updates.rs', original = read(file), writes = [], output = [];
  const fakeFs = {
    readFileSync: name => { assert.equal(name, file); return original; },
    writeFileSync: (name, bytes) => { assert.equal(name, file); writes.push(Buffer.from(bytes)); },
  };
  vm.runInNewContext(code, {
    require: name => { if (name === 'node:fs') return fakeFs; if (name === 'node:crypto') return crypto; if (name === 'node:assert/strict') return assert; throw Error(name); },
    process: { argv: ['node', 'rebind.cjs', file] }, Buffer, console: { log: value => output.push(value) },
  }, { timeout: 1000 });
  assert.equal(writes.length, 1); assert.equal(output.length, 1);
  const binding = JSON.parse(output[0]), patched = writes[0].toString('utf8');
  assert.equal(binding.replacements, 3); assert.equal(binding.identifier, ID);
  assert.equal(binding.originalSourceSHA256, sha(original)); assert.equal(binding.helperSourceSHA256, sha(writes[0]));
  assert.deepEqual(Buffer.from(patched.replaceAll('"io.agentgrid.desktop.test163"', '"io.agentgrid.desktop.test"')), original);
  assert.equal(patched.split('\n').filter((line, index) => line !== original.toString('utf8').split('\n')[index]).length, 3);
  assert.ok(patched.includes('input.identifier = "io.agentgrid.desktop".into();'), 'Production identifier rejection unchanged');
  const productFinal = workflow.indexOf('name: Verify unchanged inputs and bind final package'), helperStart = workflow.indexOf('name: Compile TEST163 default-off helper');
  assert.ok(productFinal >= 0 && helperStart > productFinal, 'Product binding must precede helper rebind/compile');
  for (const literal of ['[IO.File]::WriteAllBytes((Join-Path $PWD $source),$original)', 'sourceRestoredSHA256', 'productPinsUnchanged', 'cargo build --locked', 'cargo test --locked', '6 passed; 0 failed; 0 ignored;', 'identity-only.diff']) assert.ok(workflow.includes(literal), literal);
  fs.mkdirSync('artifacts/test40', { recursive: true });
  fs.writeFileSync('artifacts/test40/helper-source-preview.json', JSON.stringify({ ...binding, actualRustSourceWrite: false, localBuild: 'NOT_RUN', changedLines: 3, preservedProductionIdentifierRejection: true }, null, 2));
});
