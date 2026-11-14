// Opt-in actual Tauri IPC -> shared Core fixture test; no model turn or real auth.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const root = process.env.AGENT_GRID_NATIVE_OUTPUT;
assert(root && path.isAbsolute(root) && root.toLowerCase().replaceAll('\\', '/').startsWith('d:/codex/build/'), 'Explicit owned build output required');
const launch = JSON.parse(await readFile(path.join(root, 'launch.json'), 'utf8'));
assert.equal(launch.expectedIdentifier, 'io.agentgrid.desktop.integration210');
assert.equal(launch.coreHome.replaceAll('\\', '/').toLowerCase(), 'd:/codex/build/s210c1/home');
assert(Number.isSafeInteger(launch.pid) && launch.pid > 0);
assert.equal(createHash('sha256').update(await readFile(launch.exe)).digest('hex'), launch.sha256.toLowerCase());
const cdp = new URL(launch.cdpUrl);
assert(cdp.protocol === 'http:' && cdp.hostname === '127.0.0.1' && cdp.port && !cdp.username && !cdp.password);
const cwd = await realpath('D:/codex/build/s210c1/project');
const report = { startedAt: new Date().toISOString(), checks: [], errors: [], scope: 'Actual development WebView2/Tauri IPC and public Core 1.0.1 shared daemon; no model turn, thread reload, real authentication or final installer' };
const hash = value => createHash('sha256').update(value).digest('hex');
const name = 'desktop210_probe_' + randomUUID().replaceAll('-', '');
const sentinel = 'TEST_ONLY_' + randomUUID();
let page; let browser; let owned = false; let exited = false;
async function call(command, args = {}) { return page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args }); }
async function check(label, work) { const start = Date.now(); try { const detail = await work(); const evidence = detail?.mcpServers ? { mcpServers: detail.mcpServers } : detail; report.checks.push({ label, result: 'PASS', durationMs: Date.now() - start, detail: evidence }); return detail; } catch (error) { report.checks.push({ label, result: 'FAIL', durationMs: Date.now() - start }); throw error; } }
const inventory = () => call('extensions_list', { cwd });
const mutate = request => call('extensions_mutate', { cwd, request });
const row = snapshot => snapshot.mcpServers?.items.find(item => item.name === name);
const configPath = path.join(launch.coreHome, 'config.toml');
async function ownConfig() {
  const actual = await realpath(configPath);
  assert(actual.toLowerCase().startsWith((await realpath(launch.coreHome)).toLowerCase() + path.sep));
  // This fixture was created empty by the root; only synthetic sentinel values are used.
  return readFile(actual, 'utf8');
}
try {
  await mkdir(root, { recursive: true });
  browser = await chromium.connectOverCDP(cdp.href, { timeout: 20000 });
  const candidates = browser.contexts().flatMap(context => context.pages()).filter(candidate => new URL(candidate.url()).hostname === 'tauri.localhost');
  assert.equal(candidates.length, 1, 'Exactly one packaged fixture page required');
  page = candidates[0]; page.setDefaultTimeout(15000);
  assert.equal(await call('plugin:app|identifier'), launch.expectedIdentifier, 'Refuse any production app identifier before reading application data');
  owned = true;
  await page.locator('#app[aria-busy="false"]').waitFor({ timeout: 60000 });
  await check('own native target and executable binding', async () => {
    assert.deepEqual(await call('list_sessions'), []);
    const settings = await call('get_codex_settings');
    assert.equal((await realpath(settings.resolvedPath)).toLowerCase(), (await realpath(launch.coreCli)).toLowerCase());
    return { identifier: launch.expectedIdentifier, nativePid: launch.pid, workspace: launch.workspace, sessions: 0 };
  });
  let snapshot = await check('actual Core inventory', async () => {
    const value = await inventory();
    assert(value.mcpServers && typeof value.mcpServers.configVersion === 'string', JSON.stringify(value.errors));
    assert(!row(value));
    return value;
  });
  const service = fileURLToPath(new URL('./fixtures/local-mcp-server.mjs', import.meta.url));
  snapshot = await check('actual add and redacted readback', async () => {
    const value = await mutate({ operation: 'addMcpServer', name, transport: { type: 'stdio', command: process.execPath, args: [service], env: { DESKTOP_TEST_SENTINEL: sentinel } } });
    assert(row(value)?.editable && row(value).transport === 'stdio', JSON.stringify({ row: row(value) }));
    assert(!JSON.stringify(value).includes(sentinel));
    assert((await ownConfig()).includes(sentinel));
    return value;
  });
  const staleVersion = snapshot.mcpServers.configVersion;
  snapshot = await check('actual typed edit preserves protected env', async () => {
    const value = await mutate({ operation: 'editMcpServer', name, expectedVersion: snapshot.mcpServers.configVersion, patch: { transport: { type: 'stdio', args: [service, 'changed argument', ''] } } });
    assert(row(value)?.editable);
    assert((await ownConfig()).includes(sentinel));
    assert(!JSON.stringify(value).includes(sentinel));
    assert.notEqual(value.mcpServers.configVersion, staleVersion);
    return value;
  });
  await check('actual stale CAS refuses without changing bytes', async () => {
    const before = hash(await ownConfig()); let rejected = false;
    try { await mutate({ operation: 'editMcpServer', name, expectedVersion: staleVersion, patch: { enabled: false } }); } catch { rejected = true; }
    assert(rejected); assert.equal(hash(await ownConfig()), before);
    return { rejected, configurationBytesPreserved: true };
  });
  snapshot = await check('actual transport switch clears opposite fields', async () => {
    const value = await mutate({ operation: 'editMcpServer', name, expectedVersion: snapshot.mcpServers.configVersion, patch: { transport: { type: 'streamableHttp', url: 'http://127.0.0.1:1/mcp' } } });
    assert.equal(row(value)?.transport, 'streamableHttp');
    assert(!(await ownConfig()).includes(sentinel));
    return value;
  });
  await check('actual delete and readback', async () => {
    const value = await mutate({ operation: 'removeMcpServer', name, expectedVersion: snapshot.mcpServers.configVersion });
    assert(!row(value)); assert(!(await ownConfig()).includes(name));
    return { targetAbsent: true };
  });
  report.result = 'PASS';
} catch (error) {
  report.result = 'FAIL';
  report.errors.push(String(error?.message ?? error).replaceAll(sentinel, '<synthetic-sentinel-redacted>').slice(0, 1200));
} finally {
  if (owned && page && !page.isClosed()) {
    try {
      assert.deepEqual(await call('list_sessions'), [], 'Unexpected sessions: refuse exit');
      for (let count = 0; count < 4 && await page.locator('dialog[open]').count(); count++) await page.keyboard.press('Escape');
      await page.screenshot({ path: path.join(root, 'native-mcp.png'), fullPage: true });
      await page.locator('#exit').click();
      await page.locator('#exit-dialog').waitFor({ state: 'visible' });
      const closed = page.waitForEvent('close', { timeout: 30000 });
      await page.locator('#confirm-exit').click(); await closed; exited = true;
    } catch (error) { report.errors.push('Own fixture exit: ' + String(error?.message ?? error).slice(0, 250)); }
  }
  report.exited = exited; report.finishedAt = new Date().toISOString();
  if (!exited || report.errors.length) report.result = 'FAIL';
  await writeFile(path.join(root, 'native-mcp.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ result: report.result, checks: report.checks.map(({ label, result }) => ({ label, result })), errors: report.errors, exited }));
  // Only disconnect our inspector; do not Browser.close() an attached WebView.
  process.exit(report.result === 'PASS' ? 0 : 1);
}
