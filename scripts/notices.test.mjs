import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { test } from 'node:test';

test('notices include full vendored MIT/Apache license files with underscores', () => {
  const base = path.resolve('artifacts');
  fs.mkdirSync(base, { recursive: true });
  const fixture = fs.mkdtempSync(path.join(base, 'notices-case-'));
  const dependency = path.join(fixture, 'vendor');
  fs.mkdirSync(dependency);
  const mit = 'fixture MIT license body: copyright and permission';
  const apache = 'fixture Apache license body: terms and conditions';
  fs.writeFileSync(path.join(dependency, 'LICENSE_MIT'), mit);
  fs.writeFileSync(path.join(dependency, 'LICENSE_APACHE-2.0'), apache);
  fs.writeFileSync(path.join(dependency, 'LICENSE.spdx'), 'fixture SPDX expression');
  fs.writeFileSync(path.join(fixture, 'package-lock.json'), JSON.stringify({ packages: {} }));
  const metadata = { packages: [{
    name: 'tauri-plugin-updater', version: '2.14.1', source: null,
    license: 'Apache-2.0 OR MIT', manifest_path: path.join(dependency, 'Cargo.toml'),
  }] };
  // Exercise the actual generator and filesystem. Replace only cargo metadata;
  // this test never runs Cargo, installs dependencies, or reads user config.
  const child = `
    import cp from 'node:child_process';
    import { syncBuiltinESMExports } from 'node:module';
    import assert from 'node:assert/strict';
    cp.execFileSync = (file, argv) => {
      assert.equal(file, 'cargo');
      assert.deepEqual(argv, ['metadata','--locked','--format-version','1','--manifest-path','src-tauri/Cargo.toml']);
      return ${JSON.stringify(JSON.stringify(metadata))};
    };
    syncBuiltinESMExports();
    await import(${JSON.stringify(pathToFileURL(path.resolve('scripts/notices.mjs')).href)});
  `;
  const result = spawnSync(process.execPath, ['--input-type=module', '--eval', child], {
    cwd: fixture, encoding: 'utf8', windowsHide: true, timeout: 10_000,
  });
  fs.writeFileSync(path.join(fixture, 'generator-result.json'), JSON.stringify({
    exitCode: result.status, signal: result.signal, error: result.error?.code ?? null,
    stdout: result.stdout, stderr: result.stderr, mockBoundary: 'cargo metadata only',
  }, null, 2));
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, result.stderr);
  const notices = fs.readFileSync(path.join(fixture, 'THIRD_PARTY_NOTICES.md'), 'utf8');
  assert.ok(notices.includes(mit), 'full MIT license text must be included');
  assert.ok(notices.includes(apache), 'full Apache license text must be included');
  assert.ok(notices.includes('fixture SPDX expression'));
});
