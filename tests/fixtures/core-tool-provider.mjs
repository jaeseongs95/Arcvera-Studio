// Test-only, two-stage Responses fixture. Never imported by product code.
// Core: core/tests/common/responses.rs ev_function_call_with_namespace;
// protocol/src/models.rs CallToolResult -> input_text content items.
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';

const options = { toolName: '', lifetimeMs: 30000, requestTimeoutMs: 5000 };
const seen = new Set();
let valid = true;
for (let index = 2; index < process.argv.length; index += 2) {
  const flag = process.argv[index];
  const raw = process.argv[index + 1];
  if (!['--tool-name', '--lifetime-ms', '--request-timeout-ms'].includes(flag) || seen.has(flag) || raw === undefined) {
    valid = false;
    break;
  }
  seen.add(flag);
  if (flag === '--tool-name') {
    if (raw.length > 256 || !/^mcp__[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/.test(raw)) { valid = false; break; }
    options.toolName = raw;
  } else {
    const value = Number(raw);
    const minimum = flag === '--lifetime-ms' ? 500 : 100;
    const maximum = flag === '--lifetime-ms' ? 60000 : 10000;
    if (!/^\d+$/.test(raw) || !Number.isSafeInteger(value) || value < minimum || value > maximum) { valid = false; break; }
    options[flag === '--lifetime-ms' ? 'lifetimeMs' : 'requestTimeoutMs'] = value;
  }
}
if (!valid || !options.toolName) {
  process.stderr.write('Local tool fixture options rejected.\n');
  process.exit(1);
}
const [namespace, name] = options.toolName.split('.');
const CALL_ID = 'desktop-fixture-call-1';
const MAX_BYTES = 8 * 1024 * 1024;
const MAX_POSTS = 2;
const ERROR = '{"error":"local tool fixture rejected request"}';
const log = (event) => process.stdout.write(`${JSON.stringify({ testOnly: true, ...event })}\n`);
const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
let stage = 0;
let attempts = 0;
let inFlight = false;
let closing = false;
let lifetime;

function reject(request, response, status) {
  // Consume/discard without retaining or reflecting the request, then close.
  request.resume();
  response.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store', connection: 'close' });
  response.end(ERROR);
}
function advertised(body) {
  if (!Array.isArray(body.tools)) return false;
  let count = 0;
  for (const group of body.tools) {
    if (!isObject(group) || group.type !== 'namespace' || group.name !== namespace || !Array.isArray(group.tools)) continue;
    for (const tool of group.tools) {
      if (!isObject(tool) || tool.type !== 'function' || tool.name !== name) continue;
      if (tool.defer_loading !== undefined && tool.defer_loading !== false) return false;
      count += 1;
    }
  }
  return count === 1;
}
function exactResult(item) {
  if (!isObject(item) || Object.keys(item).some((key) => !['type', 'call_id', 'output'].includes(key))) return false;
  if (item.call_id !== CALL_ID || !Array.isArray(item.output) || item.output.length !== 1) return false;
  const content = item.output[0];
  return isObject(content) && Object.keys(content).length === 2 && content.type === 'input_text'
    && content.text === 'desktop-local-fixture';
}
function accepted(body) {
  if (!isObject(body) || !Array.isArray(body.input)) return false;
  const outputs = body.input.filter((item) => isObject(item) && item.type === 'function_call_output');
  if (stage === 0) return outputs.length === 0 && advertised(body);
  return stage === 1 && outputs.length === 1 && exactResult(outputs[0]);
}
function eventsFor(currentStage) {
  const id = `desktop-tool-fixture-response-${currentStage}`;
  return [
    { type: 'response.created', response: { id } },
    { type: 'response.output_item.done', item: currentStage === 1
      ? { type: 'function_call', call_id: CALL_ID, namespace, name, arguments: '{}' }
      : { type: 'message', role: 'assistant', id: 'desktop-tool-fixture-message-2',
        content: [{ type: 'output_text', text: 'desktop-local-tool-provider-fixture' }] } },
    { type: 'response.completed', response: { id,
      // Synthetic, not physical provider usage or cache evidence.
      usage: { input_tokens: 0, input_tokens_details: null, output_tokens: 0, output_tokens_details: null, total_tokens: 0 } } },
  ];
}

const server = createServer((request, response) => {
  // Budget every Responses POST, including invalid credentials/content type and
  // concurrent rejection. Health probes never reserve a model-response slot.
  if (request.method === 'POST' && request.url === '/v1/responses') {
    if (attempts >= MAX_POSTS) { reject(request, response, 429); return; }
    attempts += 1;
  }
  if (request.headers.authorization !== undefined || request.headers['proxy-authorization'] !== undefined || request.headers.cookie !== undefined) {
    reject(request, response, 400);
    return;
  }
  if (request.method === 'GET' && request.url === '/health') {
    response.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store', connection: 'close' });
    response.end(JSON.stringify({ testOnly: true, stage, attempts, maxRequests: MAX_POSTS, inFlight }));
    return;
  }
  if (request.method !== 'POST' || request.url !== '/v1/responses') { reject(request, response, 404); return; }
  if (!/^application\/json(?:\s*;|$)/i.test(request.headers['content-type'] ?? '') || request.headers['content-encoding'] !== undefined) {
    reject(request, response, 415);
    return;
  }
  // No async boundary between counting and claiming admission; no retry.
  if (inFlight || closing) { reject(request, response, 429); return; }
  if (Number(request.headers['content-length']) > MAX_BYTES) { reject(request, response, 413); return; }
  inFlight = true;
  let active = true;
  let bytes = 0;
  let chunks = [];
  const hash = createHash('sha256');
  const release = () => {
    if (!active) return;
    active = false;
    inFlight = false;
    chunks = [];
    clearTimeout(timer);
  };
  const timer = setTimeout(() => {
    if (!active) return;
    release();
    reject(request, response, 408);
  }, options.requestTimeoutMs);
  request.on('error', release);
  request.on('aborted', release);
  response.on('finish', release);
  response.on('close', release);
  request.on('data', (chunk) => {
    if (!active) return;
    bytes += chunk.length;
    if (bytes > MAX_BYTES) { release(); reject(request, response, 413); return; }
    hash.update(chunk);
    chunks.push(chunk);
  });
  request.on('end', () => {
    if (!active) return;
    let body;
    try { body = JSON.parse(Buffer.concat(chunks, bytes).toString('utf8')); }
    catch { release(); reject(request, response, 422); return; }
    chunks = [];
    if (!accepted(body)) { body = undefined; release(); reject(request, response, 422); return; }
    body = undefined;
    stage += 1;
    log({ event: 'synthetic-response', stage, wireBytes: bytes, sha256: hash.digest('hex'),
      toolAdvertised: true, toolResultConfirmed: stage === 2 });
    const events = eventsFor(stage);
    response.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'close' });
    response.end(events.map((event) => `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`).join(''), () => {
      if (stage === 2) stop();
    });
  });
});
function stop() {
  if (closing) return;
  closing = true;
  clearTimeout(lifetime);
  server.close(() => { log({ event: 'stopped', stage }); });
  server.closeAllConnections();
}
server.maxConnections = 4;
server.headersTimeout = 5000;
server.requestTimeout = 10000;
server.keepAliveTimeout = 1000;
server.on('error', () => {
  process.stderr.write('Local tool fixture server error.\n');
  process.exitCode = 1;
  stop();
});
server.on('clientError', (_error, socket) => socket.destroy());
for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, stop);
lifetime = setTimeout(stop, options.lifetimeMs);
server.listen(0, '127.0.0.1', () => {
  log({ event: 'listening', origin: `http://127.0.0.1:${server.address().port}`, maxRequests: MAX_POSTS });
});
