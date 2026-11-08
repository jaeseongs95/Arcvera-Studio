// Windows CI only: run the exact public relay, using exclusively owned test data.
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createInterface } from 'node:readline';
if (process.env.GITHUB_ACTIONS !== 'true') throw new Error('CI fixture only');
const root = path.resolve(process.argv[2]);
const sourceUrl = 'https://raw.githubusercontent.com/jaeseongs95/opendiy-remote/56c1968cbdc1626005993f1dc32f64d936c10f00/server/remote-server.mjs';
const response = await fetch(sourceUrl, { signal: AbortSignal.timeout(20000) });
if (!response.ok) throw new Error(`Pinned relay source HTTP ${response.status}`);
const bytes = Buffer.from(await response.arrayBuffer());
if (createHash('sha256').update(bytes).digest('hex') !== 'c85949fbecc32e8ad2512b60415bc7efae06f408c4e03b01109b312fa5299f90') throw new Error('Pinned relay digest mismatch');
mkdirSync(root, { recursive: true });
writeFileSync(path.join(root, 'remote-server.mjs'), bytes);
const { createRemoteServer } = await import(pathToFileURL(path.join(root, 'remote-server.mjs')));
const instance = createRemoteServer({ mode: 'relay', workspaceDir: root, distDir: root, desktopDbPath: false, codexHomeDir: path.join(root, 'codex-home'),
  token: 'relay-browser-test-only', hostSecret: 'relay-host-test-only', codexBin: path.join(root, 'never-run-codex.exe') });
// Controlled transport fault, test-only: the real relay still owns every
// successful response. Hold event requests so the native producer reaches its
// actual bounded queue; never substitute a successful server response.
const requestHandlers = instance.server.listeners('request');
let pauseEvents = false;
const heldEvents = new Set();
instance.server.removeAllListeners('request');
instance.server.on('request', (req, res) => {
  if (pauseEvents && req.url === '/api/relay/event') {
    heldEvents.add(req);
    req.once('close', () => heldEvents.delete(req));
    writeFileSync(path.join(root, 'event-post-blocked'), 'blocked');
    return;
  }
  for (const handler of requestHandlers) handler.call(instance.server, req, res);
});
await new Promise(resolve => instance.server.listen(0, '127.0.0.1', resolve));
const address = `http://127.0.0.1:${instance.server.address().port}`;
console.log(JSON.stringify({ address, sourceSha256: 'c85949fbecc32e8ad2512b60415bc7efae06f408c4e03b01109b312fa5299f90' }));
process.stdin.resume();
createInterface({ input: process.stdin }).on('line', line => {
  if (line === 'drop-owned-connections') instance.server.closeAllConnections();
  if (line === 'pause-owned-events') {
    pauseEvents = true;
    writeFileSync(path.join(root, 'event-pause-ready'), 'ready');
  }
});
process.stdin.on('end', async () => { await instance.close(); });
