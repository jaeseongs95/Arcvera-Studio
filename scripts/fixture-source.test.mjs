import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';

const script = path.resolve('scripts/fixture-source.ps1');
fs.mkdirSync(path.resolve('artifacts'), { recursive: true });
const root = fs.mkdtempSync(path.resolve('artifacts/fixture-source-'));
const git = (cwd, ...args) => execFileSync('git', args, { cwd, windowsHide: true, stdio: 'pipe' });
function fixture(name) {
  const cwd = path.join(root, name); fs.mkdirSync(cwd);
  fs.writeFileSync(path.join(cwd, 'main.rs'), 'fn main() {}\n');
  fs.writeFileSync(path.join(cwd, 'THIRD_PARTY_NOTICES.md'), 'baseline notices\n');
  git(cwd, 'init', '-q'); git(cwd, 'add', '.');
  git(cwd, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture');
  return cwd;
}
function manifest(cwd) {
  const result = spawnSync('pwsh', ['-NoProfile', '-NonInteractive', '-Command',
    '$ErrorActionPreference="Stop"; . $env:FIXTURE_SOURCE_SCRIPT; ConvertTo-Json -InputObject @(Get-FixtureSourceManifest -Repository $env:FIXTURE_SOURCE_REPO) -Compress'],
  { cwd, windowsHide: true, encoding: 'utf8', timeout: 15000,
    env: { ...process.env, FIXTURE_SOURCE_SCRIPT: script, FIXTURE_SOURCE_REPO: cwd } });
  fs.writeFileSync(path.join(cwd, 'execution.json'), JSON.stringify({ status: result.status, signal: result.signal, error: result.error?.code, stdout: result.stdout, stderr: result.stderr }, null, 2));
  assert.equal(result.error, undefined);
  return result;
}
test('FIX01 clean source is bound to tracked bytes', () => {
  const result = manifest(fixture('clean'));
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), [{ path: 'main.rs', sha256: createHash('sha256').update('fn main() {}\n').digest('hex') }]);
});
test('FIX02 platform notices are generated inputs, not modified source', () => {
  const cwd = fixture('notices'); fs.writeFileSync(path.join(cwd, 'THIRD_PARTY_NOTICES.md'), 'Windows generated notices\n');
  const result = manifest(cwd); assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout).map(item => item.path), ['main.rs']);
});
test('FIX03 modified code is rejected beside generated notices', () => {
  const cwd = fixture('code'); fs.writeFileSync(path.join(cwd, 'main.rs'), 'fn main() { panic!() }\n');
  fs.writeFileSync(path.join(cwd, 'THIRD_PARTY_NOTICES.md'), 'generated\n');
  const result = manifest(cwd); assert.notEqual(result.status, 0); assert.match(result.stderr, /Tracked source differs/);
});
test('FIX04 staged code change remains rejected', () => {
  const cwd = fixture('staged'); fs.writeFileSync(path.join(cwd, 'main.rs'), 'fn main() { panic!() }\n'); git(cwd, 'add', 'main.rs');
  const result = manifest(cwd); assert.notEqual(result.status, 0); assert.match(result.stderr, /Tracked source differs/);
});
