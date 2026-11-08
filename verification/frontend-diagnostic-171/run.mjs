import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { chromium } from '@playwright/test';

const plan = JSON.parse(fs.readFileSync(new URL('./plan.json', import.meta.url)));
const channel = process.argv[2];
assert(plan.browsers.includes(channel));
const mode = process.argv[3] ?? 'targeted';
assert(['targeted', 'source-1'].includes(mode));
const out = path.resolve(`diagnostic-${channel}`);
fs.mkdirSync(out);
const env = { ...process.env, AGENT_GRID_SOURCE_FIXTURES_ONLY: '1', AGENT_GRID_TEST_OUTPUT: `${out}/traces` };
if (channel === 'msedge') env.PLAYWRIGHT_CHANNEL = 'msedge';
else delete env.PLAYWRIGHT_CHANNEL;
const checked = spawnSync('git', ['diff', '--exit-code', plan.source, 'HEAD', '--', 'src', 'src-tauri', 'tests', 'playwright.config.ts', 'package.json', 'package-lock.json', '.github/workflows/windows.yml'], { encoding: 'utf8', windowsHide: true });
assert.equal(checked.status, 0, checked.stderr || checked.stdout);
const browser = await chromium.launch(channel === 'msedge' ? { channel } : {});
try { fs.writeFileSync(`${out}/browser.json`, JSON.stringify({ channel, mode, version: browser.version(), source: plan.source, commit: process.env.GITHUB_SHA, run: process.env.GITHUB_RUN_ID, attempt: process.env.GITHUB_RUN_ATTEMPT }, null, 2)); }
finally { await browser.close(); }
const grep = plan.cases.map(c => c.title.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
const selection = mode === 'targeted' ? [...plan.cases.map(c => c.file), '--grep', grep, '--repeat-each', String(plan.repeatEach)] : ['--shard=1/2'];
const args = [path.resolve('node_modules/@playwright/test/cli.js'), 'test', ...selection, '--workers', String(plan.workers), '--retries=0', '--trace=on', '--reporter=line'];
function execute(name, extra) {
  const started = new Date().toISOString();
  const child = spawnSync(process.execPath, [...args, ...extra], { env, encoding: 'utf8', windowsHide: true, timeout: plan.limits.testProcessMs, maxBuffer: 8 * 1024 * 1024 });
  fs.writeFileSync(`${out}/${name}.stdout.txt`, child.stdout ?? '', { flag: 'wx' });
  fs.writeFileSync(`${out}/${name}.stderr.txt`, child.stderr ?? '', { flag: 'wx' });
  fs.writeFileSync(`${out}/${name}.execution.json`, JSON.stringify({ executable: process.execPath, argv: [...args, ...extra], started, finished: new Date().toISOString(), exitCode: child.status, signal: child.signal, error: child.error?.message ?? null }, null, 2), { flag: 'wx' });
  assert.equal(child.error, undefined, child.error?.message);
  assert.equal(child.signal, null);
  return child;
}
const listed = execute('list', ['--list']);
assert.equal(listed.status, 0, listed.stderr);
assert.match(listed.stdout, mode === 'targeted' ? /Total: 20 tests in 2 files/ : /Total: 169 tests in /, 'The frozen original case set must be selected');
const actual = execute('actual', []);
console.log(JSON.stringify({ channel, exitCode: actual.status, stdout: actual.stdout, stderr: actual.stderr }));
process.exitCode = actual.status === 0 ? 0 : 1;
