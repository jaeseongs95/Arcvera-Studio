import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

assert.equal(process.platform, 'win32');
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Disposable Windows CI only');
const repository = fileURLToPath(new URL('../', import.meta.url));
const mutant = fs.mkdtempSync(path.join(process.env.RUNNER_TEMP || os.tmpdir(), 'arcvera-shortcut-mutant-'));
const installer = path.join(mutant, 'src-tauri', 'installer');
fs.mkdirSync(installer, { recursive: true });
const template = fs.readFileSync(path.join(repository, 'src-tauri/installer/installer.nsi'), 'utf8');
let changes = 0;
const mutated = template.replace(/^(\s*)!insertmacro ArcveraCreateShortcutIfAbsent ("[^"\r\n]+")$/gm, (_, indent, shortcut) => {
  changes++;
  return `${indent}CreateShortcut ${shortcut} "$INSTDIR\\\${MAINBINARYNAME}.exe"\n${indent}!insertmacro SetLnkAppUserModelId ${shortcut}`;
});
assert.equal(changes, 3, 'Remove precisely the three F1 creation guards');
fs.writeFileSync(path.join(installer, 'installer.nsi'), mutated);
fs.copyFileSync(path.join(repository, 'src-tauri/installer/hooks.nsh'), path.join(installer, 'hooks.nsh'));
const evidence = path.join(repository, 'artifacts', 'installer-shortcuts');
fs.mkdirSync(evidence, { recursive: true });
fs.writeFileSync(path.join(evidence, 'mutation-installer.nsi'), mutated);
const argv = ['--test', 'scripts/installer-shortcuts.test.mjs'];
function run(label, source) {
  const startedAt = new Date().toISOString();
  const result = spawnSync(process.execPath, argv, { cwd: repository, encoding: 'utf8', timeout: 180000,
    windowsHide: true, env: { ...process.env, ARCVERA_INSTALLER_SOURCE: source, ARCVERA_SHORTCUT_EVIDENCE: label } });
  const receipt = { executable: process.execPath, argv, startedAt, finishedAt: new Date().toISOString(),
    status: result.status, signal: result.signal, error: result.error?.message, stdout: result.stdout, stderr: result.stderr };
  fs.writeFileSync(path.join(evidence, `${label}-execution.json`), JSON.stringify(receipt, null, 2));
  process.stdout.write(result.stdout || '');
  process.stderr.write(result.stderr || '');
  assert.ifError(result.error);
  return { receipt, report: JSON.parse(fs.readFileSync(path.join(evidence, label, 'result.json'), 'utf8')) };
}
const red = run('before', mutant);
assert.equal(red.receipt.status, 1, 'The unguarded production mutation must fail');
assert.ok(red.report.violations.some(value => value.includes('foreign/unreadable shortcut bytes changed')), 'Failure must reach the original F1 boundary');
const green = run('after', repository);
assert.equal(green.receipt.status, 0, 'The same tests must pass with the actual production guards');
assert.equal(red.report.testSha256, green.report.testSha256, 'Identical test bytes');
assert.equal(red.report.scenarioCount, 80);
assert.equal(green.report.scenarioCount, 80);
assert.deepEqual(green.report.violations, []);
fs.writeFileSync(path.join(evidence, 'sensitivity-result.json'), JSON.stringify({ verdict: 'PASS', mutation: 'Three creation guards removed; original unconditional CreateShortcut restored',
  testSha256: green.report.testSha256, beforeScenarios: 80, afterScenarios: 80,
  beforeViolations: red.report.violations.length, afterViolations: 0,
  boundary: green.report.boundary }, null, 2));
console.log('NSIS shortcut sensitivity: unguarded mutation RED, identical fixed tests GREEN (80 scenarios each).');
