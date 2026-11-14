// Test-only loopback Responses fixture. No upstream calls, credentials, or request-body storage.
// SSE shape: Core core/tests/common/responses.rs ev_response_created/ev_assistant_message/ev_completed.
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';

const options = { port: 0, maxRequests: 1 };
const seen = new Set();
for (let index = 2; index < process.argv.length; index += 2) {
  const flag = process.argv[index];
  const raw = process.argv[index + 1];
  if (!['--port', '--max-requests'].includes(flag) || seen.has(flag) || !raw || !/^\d+$/.test(raw)) {
    throw new Error('Expected unique --port 0..65535 and/or --max-requests 1..16.');
  }
  seen.add(flag);
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || (flag === '--port' ? value > 65535 : value < 1 || value > 16)) {
    throw new Error('Fixture option is out of range.');
  }
  options[flag === '--port' ? 'port' : 'maxRequests'] = value;
}

const MAX_WIRE_BYTES = 8 * 1024 * 1024;
let completedRequests = 0;
let inFlight = false;
const log = (event) => process.stdout.write(`${JSON.stringify({ testOnly: true, ...event })}\n`);
const reject = (request, response, statusCode) => {
  request.resume();
  response.writeHead(statusCode, { 'content-type': 'application/json', connection: 'close' });
  response.end('{"error":"local fixture rejected request"}');
};

const server = createServer((request, response) => {
  if (request.method === 'GET' && request.url === '/health') {
    response.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
    response.end(JSON.stringify({ testOnly: true, completedRequests, maxRequests: options.maxRequests, inFlight }));
    return;
  }
  if (request.method !== 'POST' || request.url !== '/v1/responses') {
    reject(request, response, 404);
    return;
  }
  // Reject accidental credential attachment without returning or logging its value.
  if (request.headers.authorization !== undefined || request.headers['proxy-authorization'] !== undefined || request.headers.cookie !== undefined) {
    reject(request, response, 400);
    return;
  }
  if (!/^application\/json(?:\s*;|$)/i.test(request.headers['content-type'] ?? '')) {
    reject(request, response, 415);
    return;
  }
  if (inFlight || completedRequests >= options.maxRequests) {
    reject(request, response, 429);
    return;
  }
  inFlight = true;
  let active = true;
  let bytes = 0;
  const hash = createHash('sha256');
  const release = () => { if (active) { active = false; inFlight = false; } };
  request.on('error', release);
  response.on('close', release);
  request.on('data', (chunk) => {
    if (!active) return;
    bytes += chunk.length;
    if (bytes > MAX_WIRE_BYTES) {
      release();
      reject(request, response, 413);
      return;
    }
    hash.update(chunk);
  });
  request.on('end', () => {
    if (!active) return;
    completedRequests += 1;
    const responseId = `desktop-fixture-response-${completedRequests}`;
    const events = [
      { type: 'response.created', response: { id: responseId } },
      { type: 'response.output_item.done', item: {
        type: 'message', role: 'assistant', id: `desktop-fixture-message-${completedRequests}`,
        content: [{ type: 'output_text', text: 'desktop-local-provider-fixture' }],
      } },
      { type: 'response.completed', response: { id: responseId,
        usage: { input_tokens: 0, input_tokens_details: null, output_tokens: 0, output_tokens_details: null, total_tokens: 0 },
      } },
    ];
    log({ event: 'synthetic-response', sequence: completedRequests, wireBytes: bytes, sha256: hash.digest('hex') });
    response.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'close' });
    response.end(events.map((event) => `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`).join(''));
  });
});
server.maxConnections = 4;
server.headersTimeout = 5000;
server.requestTimeout = 10000;
server.keepAliveTimeout = 1000;
server.on('error', () => { process.stderr.write('Local provider fixture server error.\n'); process.exitCode = 1; });
server.on('clientError', (_error, socket) => socket.destroy());
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, () => { server.close(); server.closeAllConnections(); });
}
server.listen(options.port, '127.0.0.1', () => {
  log({ event: 'listening', origin: `http://127.0.0.1:${server.address().port}`, maxRequests: options.maxRequests });
});
