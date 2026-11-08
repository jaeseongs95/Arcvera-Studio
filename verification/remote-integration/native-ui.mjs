import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import net from 'node:net';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { chromium } from 'playwright';

const base = path.resolve('verification/remote-integration');
const root = path.join(base, 'native-' + randomUUID());
const exe = path.join(base, 'package-29cf53f/opendiy-desktop.exe');
const report = { candidate: '29cf53f7ac658fa5c3fe18ddc6755b9961126ab6', merge: '40d0253de47b25a6ff32912c9f29bf0de40dd92c', started: new Date().toISOString(), root, cases: [], scope: 'Actual packaged Tauri UI + pinned relay + same Engine, no CLI/TUI/model/physical mobile/production connection' };
await fs.mkdir(root);
const check = (name, detail = {}) => report.cases.push({ name, pass: true, ...detail });
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(fn, label, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await fn(); if (value) return value; await pause(100); }
  throw Error('Deadline: ' + label);
}
let app, browser, instance, page, exit;
const stdout = [], stderr = [];
try {
  assert.equal(sha(await fs.readFile(exe)), '4643876993028c7bc66808b267d2a2dbf6e05d984b0c08ec709ff7bdd600de66');
  check('exact-packaged-executable');
  const source = await fetch('https://raw.githubusercontent.com/jaeseongs95/opendiy-remote/56c1968cbdc1626005993f1dc32f64d936c10f00/server/remote-server.mjs', { signal: AbortSignal.timeout(20000) });
  assert(source.ok);
  const bytes = Buffer.from(await source.arrayBuffer());
  assert.equal(sha(bytes), 'c85949fbecc32e8ad2512b60415bc7efae06f408c4e03b01109b312fa5299f90');
  const modulePath = path.join(root, 'remote-server.mjs'); await fs.writeFile(modulePath, bytes);
  const { createRemoteServer } = await import(pathToFileURL(modulePath));
  for (const folder of ['workspace', 'codex-home', 'appdata', 'localappdata', 'webview', 'temp', 'project']) await fs.mkdir(path.join(root, folder));
  instance = createRemoteServer({ mode: 'relay', workspaceDir: path.join(root, 'relay'), desktopDbPath: false, distDir: root, codexHomeDir: path.join(root, 'codex-home'), token: 'relay-browser-test-only', hostSecret: 'relay-host-test-only', codexBin: path.join(root, 'never-run-codex.exe') });
  await new Promise(resolve => instance.server.listen(0, '127.0.0.1', resolve));
  const address = `http://127.0.0.1:${instance.server.address().port}`;
  const reservation = net.createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const cdp = reservation.address().port; await new Promise(resolve => reservation.close(resolve));
  const env = { ...process.env, AGENT_GRID_WORKSPACE_DIR: path.join(root, 'workspace'), CODEX_HOME: path.join(root, 'codex-home'), HOME: root,
    APPDATA: path.join(root, 'appdata'), LOCALAPPDATA: path.join(root, 'localappdata'), TEMP: path.join(root, 'temp'), TMP: path.join(root, 'temp'),
    AGENT_GRID_CODEX: path.join(root, 'never-run-codex.exe'), WEBVIEW2_USER_DATA_FOLDER: path.join(root, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${cdp}` };
  app = spawn(exe, [], { cwd: root, env, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  app.stdout.on('data', b => stdout.push(b)); app.stderr.on('data', b => stderr.push(b));
  exit = new Promise(resolve => app.once('exit', (code, signal) => resolve({ code, signal })));
  app.once('error', error => { report.launchError = error.message; });
  report.appPid = app.pid;
  await until(async () => { if (app.exitCode !== null) throw Error('App exited ' + app.exitCode); try { return (await fetch(`http://127.0.0.1:${cdp}/json/version`, { signal: AbortSignal.timeout(500) })).ok; } catch { return false; } }, 'owned WebView CDP');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdp}`);
  page = await until(async () => browser.contexts().flatMap(c => c.pages()).find(p => /tauri|localhost/.test(p.url())), 'owned native page');
  page.setDefaultTimeout(5000);
  await page.locator('#settings-open').waitFor();
  const native = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  const settings = await native('get_codex_settings');
  assert.equal(settings.source, 'environment'); assert.equal(settings.resolvedPath, null);
  assert.equal((await native('list_sessions')).length, 0);
  check('owned-native-startup-cli-unavailable-no-sessions');
  await page.locator('#settings-open').click(); await page.locator('#relay-open').click();
  await page.locator('#relay-url').fill(address); await page.locator('#relay-secret').fill('relay-host-test-only'); await page.locator('#relay-connect').click();
  await until(async () => (await native('relay_status')).state === 'connected', 'actual registered');
  await page.locator('#relay-status').filter({ hasText: '중계기 연결됨' }).waitFor();
  assert.equal(await page.locator('#relay-secret').inputValue(), '');
  const connected = await native('relay_status'); assert(!JSON.stringify(connected).includes('relay-host-test-only'));
  await page.screenshot({ path: path.join(root, 'connected.png') });
  check('settings-ui-real-registered-secret-cleared', { status: connected });
  async function remote(command, args = {}) {
    const response = await fetch(address + '/api/invoke', { method: 'POST', headers: { Authorization: 'Bearer relay-browser-test-only', 'Content-Type': 'application/json' }, body: JSON.stringify({ command, args }), signal: AbortSignal.timeout(10000) });
    return { status: response.status, body: await response.json() };
  }
  const created = await remote('create_project', { name: 'native-remote-owned', cwd: path.join(root, 'project') });
  assert.equal(created.body.ok, true);
  const local = await native('list_projects'); assert.equal(local.length, 1); assert.equal(local[0].name, 'native-remote-owned');
  const listed = await remote('list_projects'); assert.deepEqual(listed.body.result, local);
  check('real-tauri-dispatch-shares-local-engine', { projectId: local[0].id });
  await fs.writeFile(path.join(root, 'project', 'owned.txt'), 'owned fixture');
  const listing = await remote('list_directory', { path: path.join(root, 'project'), includeFiles: true });
  assert.equal(listing.body.ok, true); assert.equal(listing.body.result.entries[0].name, 'owned.txt'); assert(listing.body.result.currentPath);
  check('real-native-directory-contract');
  const denied = await remote('complete_exit', { confirmed: true }); assert.equal(denied.body.ok, false);
  check('remote-exit-denied', { http: denied.status });
  await page.locator('#relay-disconnect').click();
  await until(async () => (await native('relay_status')).state === 'disconnected', 'explicit release');
  assert.equal((await native('relay_status')).releaseConfirmed, true);
  check('native-ui-explicit-disconnect-confirmed');
  await page.locator('#relay-secret').fill('wrong-test-secret'); await page.locator('#relay-connect').click();
  await until(async () => (await native('relay_status')).state === 'error', '401 denied');
  const bad = await native('relay_status'); assert.match(bad.error, /401/);
  await page.locator('#relay-disconnect').click(); await until(async () => (await native('relay_status')).state === 'disconnected', 'bad connection closed');
  check('actual-401-no-success-status');
  await page.locator('#relay-close').click(); await page.locator('#settings-close').click();
  try { await native('complete_exit', { confirmed: true }); }
  catch (error) { report.exitRequestObservation = error.message; }
  const stopped = await Promise.race([exit, pause(10000).then(() => null)]);
  assert(stopped, 'native normal exit deadline'); assert.equal(stopped.code, 0); report.exit = stopped;
  check('owned-native-normal-exit');
  const db = await fs.readFile(path.join(root, 'workspace/workspace.sqlite3'));
  assert(!db.includes(Buffer.from('relay-host-test-only'))); check('owned-metadata-no-host-secret');
  report.pass = true;
} catch (error) { report.pass = false; report.error = error.stack; process.exitCode = 1; }
finally {
  if (app && app.exitCode === null) {
    try { if (page) await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('complete_exit', { confirmed: true })); } catch {}
    const stopped = await Promise.race([exit, pause(10000).then(() => null)]);
    if (!stopped) { report.forcedOwnedAppTermination = true; app.kill(); await exit; }
  }
  if (browser) await browser.close();
  if (instance) { instance.server.closeAllConnections(); await instance.close(); }
  await fs.writeFile(path.join(root, 'app.stdout.txt'), Buffer.concat(stdout)); await fs.writeFile(path.join(root, 'app.stderr.txt'), Buffer.concat(stderr));
  report.finished = new Date().toISOString();
  await fs.writeFile(path.join(root, 'result.json'), JSON.stringify(report, null, 2));
  await fs.writeFile(path.join(base, 'native-ui-last.json'), JSON.stringify({ root, pass: report.pass, cases: report.cases.length, error: report.error }, null, 2));
  console.log(JSON.stringify({ root, pass: report.pass, cases: report.cases.length, error: report.error }));
}
