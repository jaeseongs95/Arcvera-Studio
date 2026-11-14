// Test-only public boundary: owned CLI child and dynamic loopback HTTP.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { request as httpRequest } from 'node:http';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const fixture = fileURLToPath(new URL('./fixtures/core-tool-provider.mjs', import.meta.url));
const toolName = 'mcp__desktop_probe.desktop_local_probe_v1';
const callId = 'desktop-fixture-call-1';
const secret = 'DO_NOT_ECHO_BODY_OR_CREDENTIAL_789';
const advertisement = () => ({ input: [{ role: 'user', content: secret }], tools: [
  { type: 'namespace', name: 'mcp__desktop_probe', tools: [
    { type: 'function', name: 'desktop_local_probe_v1', parameters: { type: 'object', properties: {} } },
  ] },
] });
const output = () => ({ input: [{ type: 'function_call_output', call_id: callId,
  output: [{ type: 'input_text', text: 'desktop-local-fixture' }] }], tools: advertisement().tools });
const fixedError = '{"error":"local tool fixture rejected request"}';
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function start(t, options = []) {
  const child = spawn(process.execPath, [fixture, '--tool-name', toolName, ...options], {
    windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stdout = '', stderr = '', code;
  const exited = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (value, signal) => { code = { value, signal }; resolve(code); });
  });
  child.stdout.setEncoding('utf8').on('data', (chunk) => { stdout += chunk; });
  child.stderr.setEncoding('utf8').on('data', (chunk) => { stderr += chunk; });
  const events = () => stdout.trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
  t.after(async () => {
    if (!code) child.kill(); // Only this test's own child.
    await Promise.race([exited, delay(2000).then(() => { throw new Error('Owned fixture failed to exit'); })]);
    assert.ok(stdout.length < 65536 && stderr.length < 65536);
    assert.ok(!stdout.includes(secret) && !stderr.includes(secret));
  });
  const until = Date.now() + 3000;
  while (Date.now() < until) {
    const ready = events().find((event) => event.event === 'listening');
    if (ready) {
      assert.match(ready.origin, /^http:\/\/127\.0\.0\.1:\d+$/);
      return { origin: ready.origin, events, stdout: () => stdout, stderr: () => stderr, exited,
        async post(body, headers = {}) {
          return fetch(`${ready.origin}/v1/responses`, { method: 'POST',
            headers: { 'content-type': 'application/json', ...headers },
            body: typeof body === 'string' ? body : JSON.stringify(body), signal: AbortSignal.timeout(3000) });
        } };
    }
    if (code) throw new Error('Owned fixture exited before listening');
    await delay(10);
  }
  throw new Error('Owned fixture did not listen before deadline');
}

async function rejected(response, status) {
  assert.equal(response.status, status);
  assert.equal(await response.text(), fixedError);
}
function eventsFromSse(body) {
  return body.trim().split('\n\n').map((frame) => {
    const lines = frame.split('\n');
    const event = JSON.parse(lines.find((line) => line.startsWith('data: ')).slice(6));
    assert.equal(lines[0], `event: ${event.type}`);
    return event;
  });
}
async function first(provider) {
  const response = await provider.post(advertisement());
  assert.equal(response.status, 200);
  const events = eventsFromSse(await response.text());
  assert.deepEqual(events[1], { type: 'response.output_item.done', item: {
    type: 'function_call', call_id: callId, namespace: 'mcp__desktop_probe',
    name: 'desktop_local_probe_v1', arguments: '{}',
  } });
  assert.ok(!events.some((event) => event.item?.type === 'message'));
  return events;
}

test('TP01/07: exact two-stage exchange, synthetic usage, safe receipt and automatic owned shutdown', async (t) => {
  const provider = await start(t);
  await first(provider);
  const response = await provider.post(output());
  assert.equal(response.status, 200);
  const events = eventsFromSse(await response.text());
  assert.equal(events[1].item.type, 'message');
  assert.equal(events[1].item.content[0].text, 'desktop-local-tool-provider-fixture');
  assert.deepEqual(events[2].response.usage, { input_tokens: 0, input_tokens_details: null,
    output_tokens: 0, output_tokens_details: null, total_tokens: 0 });
  assert.deepEqual(await provider.exited, { value: 0, signal: null });
  await assert.rejects(fetch(`${provider.origin}/health`, { signal: AbortSignal.timeout(1000) }));
  const receipts = provider.events().filter((event) => event.event === 'synthetic-response');
  assert.equal(receipts.length, 2);
  for (const [index, receipt] of receipts.entries()) {
    const bytes = Buffer.from(JSON.stringify(index === 0 ? advertisement() : output()));
    assert.equal(receipt.wireBytes, bytes.length);
    assert.equal(receipt.sha256, createHash('sha256').update(bytes).digest('hex'));
    assert.equal(receipt.stage, index + 1);
    assert.equal(receipt.toolAdvertised, true);
    assert.equal(receipt.toolResultConfirmed, index === 1);
    assert.deepEqual(Object.keys(receipt).sort(), ['event', 'sha256', 'stage', 'testOnly',
      'toolAdvertised', 'toolResultConfirmed', 'wireBytes'].sort());
  }
  assert.equal(provider.stderr(), '');
});

test('TP02: missing, flat, duplicate, deferred or malformed selected advertisement is rejected', async (t) => {
  for (const kind of ['missing', 'flat', 'duplicate', 'deferred', 'bad-deferred']) {
    await t.test(kind, async (t) => {
      const provider = await start(t);
      const body = advertisement();
      if (kind === 'missing') body.tools = [];
      if (kind === 'flat') body.tools = [{ type: 'function', name: toolName }];
      if (kind === 'duplicate') body.tools.push(structuredClone(body.tools[0]));
      if (kind === 'deferred') body.tools[0].tools[0].defer_loading = true;
      if (kind === 'bad-deferred') body.tools[0].tools[0].defer_loading = secret;
      await rejected(await provider.post(body), 422);
      assert.equal(provider.events().filter((event) => event.event === 'synthetic-response').length, 0);
    });
  }
});

test('TP03: output cannot precede the call, and only the exact successful result completes', async (t) => {
  await t.test('wrong-order', async (t) => {
    const provider = await start(t);
    await rejected(await provider.post(output()), 422);
  });
  for (const kind of ['missing', 'call-id', 'error-string', 'wrong-text', 'duplicate', 'extra-content', 'error-flag']) {
    await t.test(kind, async (t) => {
      const provider = await start(t);
      await first(provider);
      const body = output();
      if (kind === 'missing') body.input = [];
      if (kind === 'call-id') body.input[0].call_id = secret;
      if (kind === 'error-string') body.input[0].output = secret;
      if (kind === 'wrong-text') body.input[0].output[0].text = secret;
      if (kind === 'duplicate') body.input.push(structuredClone(body.input[0]));
      if (kind === 'extra-content') body.input[0].output.push({ type: 'input_text', text: secret });
      if (kind === 'error-flag') body.input[0].is_error = true;
      await rejected(await provider.post(body), 422);
      assert.equal(provider.events().filter((event) => event.toolResultConfirmed).length, 0);
    });
  }
});

test('TP04/07: credentials, wrong path/type/encoding and invalid JSON return fixed safe errors', async (t) => {
  for (const header of ['authorization', 'proxy-authorization', 'cookie']) {
    await t.test(header, async (t) => {
      const provider = await start(t);
      await rejected(await provider.post(advertisement(), { [header]: secret }), 400);
    });
  }
  await t.test('path-and-type', async (t) => {
    const provider = await start(t);
    await rejected(await fetch(`${provider.origin}/${secret}`), 404);
    await rejected(await provider.post(advertisement(), { 'content-type': 'text/plain' }), 415);
    await rejected(await provider.post(advertisement(), { 'content-encoding': 'gzip' }), 415);
  });
  await t.test('invalid-json', async (t) => {
    const provider = await start(t);
    await rejected(await provider.post(`{"private":"${secret}"`), 422);
  });
});

test('TP05: over 8MiB is rejected and attempts are bounded to two', async (t) => {
  const provider = await start(t);
  await rejected(await provider.post('x'.repeat(8 * 1024 * 1024 + 1)), 413);
  await rejected(await provider.post({}), 422);
  await rejected(await provider.post(advertisement()), 429);
});

test('TP05: credential/type rejection consumes the same two-attempt budget', async (t) => {
  const provider = await start(t);
  await rejected(await provider.post(advertisement(), { authorization: secret }), 400);
  await rejected(await provider.post(advertisement(), { 'content-type': 'text/plain' }), 415);
  await rejected(await provider.post(advertisement()), 429);
});

function partialPost(origin) {
  const request = httpRequest(`${origin}/v1/responses`, { method: 'POST',
    headers: { 'content-type': 'application/json' }, agent: false });
  const response = new Promise((resolve, reject) => {
    request.on('error', reject);
    request.on('response', (incoming) => {
      let text = '';
      incoming.setEncoding('utf8').on('data', (chunk) => { text += chunk; });
      incoming.on('end', () => resolve({ status: incoming.statusCode, text }));
    });
  });
  request.write('{');
  return { request, response };
}
async function waitInFlight(provider) {
  for (let count = 0; count < 50; count++) {
    const health = await (await fetch(`${provider.origin}/health`, { signal: AbortSignal.timeout(1000) })).json();
    if (health.inFlight) return;
    await delay(10);
  }
  throw new Error('Owned request did not become active');
}

test('TP05: chunked body cannot bypass the 8MiB data-path limit', async (t) => {
  const provider = await start(t);
  const pending = partialPost(provider.origin);
  t.after(() => pending.request.destroy());
  pending.request.end(' '.repeat(8 * 1024 * 1024)); // Opening brace makes total > limit.
  assert.deepEqual(await pending.response, { status: 413, text: fixedError });
});

test('TP05: one inflight admission, a concurrent attempt consumes the finite POST budget', async (t) => {
  const provider = await start(t);
  const pending = partialPost(provider.origin);
  t.after(() => pending.request.destroy());
  await waitInFlight(provider);
  await rejected(await provider.post(advertisement()), 429);
  pending.request.end(JSON.stringify(advertisement()).slice(1));
  assert.equal((await pending.response).status, 200);
  await rejected(await provider.post(output()), 429);
});

test('TP06: slow body times out and lifetime closes only the owned server', async (t) => {
  const provider = await start(t, ['--request-timeout-ms', '100', '--lifetime-ms', '700']);
  const pending = partialPost(provider.origin);
  t.after(() => pending.request.destroy());
  assert.deepEqual(await pending.response, { status: 408, text: fixedError });
  assert.deepEqual(await provider.exited, { value: 0, signal: null });
  await assert.rejects(fetch(`${provider.origin}/health`, { signal: AbortSignal.timeout(1000) }));
});

test('TP08: CLI rejects missing/unknown/duplicate/out-of-range options without echoing input', async () => {
  const variants = [[], ['--tool-name', secret], ['--tool-name', toolName, '--unknown', secret],
    ['--tool-name', toolName, '--tool-name', toolName],
    ['--tool-name', toolName, '--lifetime-ms', '0'], ['--tool-name', toolName, '--request-timeout-ms', '10001']];
  for (const args of variants) {
    const child = spawn(process.execPath, [fixture, ...args], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    const result = await Promise.race([
      new Promise((resolve, reject) => { child.once('error', reject); child.once('exit', (code) => resolve(code)); }),
      delay(3000).then(() => { child.kill(); throw new Error('Invalid CLI failed to exit'); }),
    ]);
    assert.equal(result, 1);
    assert.equal(stdout, '');
    assert.equal(stderr, 'Local tool fixture options rejected.\n');
    assert.ok(!stderr.includes(secret));
  }
});
