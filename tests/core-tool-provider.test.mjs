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

test('TP09: Core timing-only prefix and exact successful payload complete the same bound call', async (t) => {
  for (const seconds of ['0.0000', '0.5000', '9999999999.9999']) {
    await t.test(seconds, async (t) => {
      const provider = await start(t);
      await first(provider);
      const body = output();
      body.input[0].output.unshift({ type: 'input_text', text: `Wall time: ${seconds} seconds\nOutput:` });
      const response = await provider.post(body);
      assert.equal(response.status, 200);
      const events = eventsFromSse(await response.text());
      assert.equal(events[1].item.content[0].text, 'desktop-local-tool-provider-fixture');
      assert.equal(events[2].type, 'response.completed');
      assert.equal(events[2].response.usage.total_tokens, 0);
      assert.deepEqual(await provider.exited, { value: 0, signal: null });
      await assert.rejects(fetch(`${provider.origin}/health`, { signal: AbortSignal.timeout(1000) }));
      const receipts = provider.events().filter((event) => event.event === 'synthetic-response');
      assert.equal(receipts.length, 2);
      assert.equal(receipts[1].toolResultConfirmed, true);
      assert.equal(receipts[1].sha256, createHash('sha256').update(JSON.stringify(body)).digest('hex'));
      assert.ok(!provider.stdout().includes('Wall time:'));
      assert.ok(!provider.stdout().includes('desktop-local-fixture'));
      assert.equal(provider.stderr(), '');
    });
  }
});

test('TP12: bounded optional wire id is opaque and never replaces call authority', async (t) => {
  for (const [kind, id, timing] of [['bare', 'tool_output_1', false], ['timing', secret, true], ['byte-bound', 'é'.repeat(128), true]]) {
    await t.test(kind, async (t) => {
      const provider = await start(t); await first(provider);
      const body = output(); body.input[0].id = id;
      if (timing) body.input[0].output.unshift({ type:'input_text', text:'Wall time: 0.0090 seconds\nOutput:' });
      const response = await provider.post(body); assert.equal(response.status, 200);
      assert.ok(!provider.stdout().includes(id));
    });
  }
  const bad = { null:null, number:1, boolean:false, object:{}, array:[], empty:'', blank:' ', limit:'é'.repeat(129), control:'x\u0000', bidi:'x\u202e' };
  for (const [kind, id] of Object.entries(bad)) await t.test(kind, async (t) => {
    const provider = await start(t); await first(provider);
    const body = output(); body.input[0].id=id;
    await rejected(await provider.post(body),422);
    assert.equal(provider.events().filter(event=>event.toolResultConfirmed).length,0);
  });
  for (const kind of ['call-id','payload','unknown-key']) await t.test(kind,async(t)=>{
    const provider=await start(t);await first(provider);const body=output();body.input[0].id='tool_output_1';
    if(kind==='call-id')body.input[0].call_id='other';
    if(kind==='payload')body.input[0].output[0].text='other';
    if(kind==='unknown-key')body.input[0].private=secret;
    await rejected(await provider.post(body),422);
  });
});

test('TP10/11: only the exact bounded Core timing frame may precede the exact result', async (t) => {
  const badHeaders = {
    'fraction-short': 'Wall time: 0.500 seconds\nOutput:',
    'fraction-long': 'Wall time: 0.50000 seconds\nOutput:',
    'integer-long': 'Wall time: 10000000000.0000 seconds\nOutput:',
    'leading-zero': 'Wall time: 00.5000 seconds\nOutput:',
    negative: 'Wall time: -0.5000 seconds\nOutput:',
    'plus-sign': 'Wall time: +0.5000 seconds\nOutput:',
    exponent: 'Wall time: 5e-1 seconds\nOutput:',
    nan: 'Wall time: NaN seconds\nOutput:',
    infinity: 'Wall time: Infinity seconds\nOutput:',
    units: 'Wall time: 0.5000 milliseconds\nOutput:',
    crlf: 'Wall time: 0.5000 seconds\r\nOutput:',
    'trailing-newline': 'Wall time: 0.5000 seconds\nOutput:\n',
    'extra-text': `Wall time: 0.5000 seconds\nOutput:${secret}`,
    prefix: `${secret}Wall time: 0.5000 seconds\nOutput:`,
    control: 'Wall time: 0.5000 seconds\nOutput:\u0000',
  };
  const shapes = ['metadata-key', 'metadata-type', 'metadata-value', 'metadata-null', 'missing-result',
    'duplicate-metadata', 'reversed', 'excess', 'call-id', 'result-text', 'result-key', 'output-key'];
  for (const kind of [...Object.keys(badHeaders), ...shapes]) {
    await t.test(kind, async (t) => {
      const provider = await start(t);
      await first(provider);
      const body = output();
      const item = body.input[0];
      const header = { type: 'input_text', text: badHeaders[kind] ?? 'Wall time: 0.5000 seconds\nOutput:' };
      item.output.unshift(header);
      if (kind === 'metadata-key') header.private = secret;
      if (kind === 'metadata-type') header.type = 'output_text';
      if (kind === 'metadata-value') header.text = secret;
      if (kind === 'metadata-null') item.output[0] = null;
      if (kind === 'missing-result') item.output.pop();
      if (kind === 'duplicate-metadata') item.output[1] = structuredClone(header);
      if (kind === 'reversed') item.output.reverse();
      if (kind === 'excess') item.output.push(structuredClone(header));
      if (kind === 'call-id') item.call_id = secret;
      if (kind === 'result-text') item.output[1].text = secret;
      if (kind === 'result-key') item.output[1].private = secret;
      if (kind === 'output-key') item.private = secret;
      await rejected(await provider.post(body), 422);
      assert.equal(provider.events().filter((event) => event.toolResultConfirmed).length, 0);
      assert.ok(!provider.stdout().includes('Wall time:'));
      assert.ok(!provider.stdout().includes('desktop-local-fixture'));
    });
  }
});

// Explicit GUI-only auxiliary contract. Original tests above remain unchanged.
const titleModel = 'desktop-local-fixture';
const titleBody = () => ({ model: titleModel, tools: [], prompt_cache_key: secret,
  input: Array.from({ length: 4 }, () => ({ type: 'message', role: 'user', content: [{ type: 'input_text', text: secret }] })),
  text: { format: { type: 'json_schema', strict: true, name: 'codex_output_schema', schema: {
    type: 'object', properties: { title: { type: 'string', minLength: 1, maxLength: 36 } },
    required: ['title'], additionalProperties: false,
  } } },
});
const guiOptions = ['--title-model', titleModel];
const mainReceipts = (provider) => provider.events().filter((event) => event.event === 'synthetic-response');
const titleReceipts = (provider) => provider.events().filter((event) => event.event === 'synthetic-title-response');
const guiHealth = async (provider) => (await fetch(`${provider.origin}/health`, { signal: AbortSignal.timeout(1000) })).json();
async function titleResponse(provider, body = titleBody()) {
  const response = await provider.post(body); assert.equal(response.status, 200);
  const events = eventsFromSse(await response.text());
  assert.equal(events[1].item.type, 'message');
  assert.equal(events[1].item.role, 'assistant');
  const title = JSON.parse(events[1].item.content[0].text);
  assert.deepEqual(title, { title: 'Synthetic GUI fixture title' });
  assert.ok(title.title.length > 0 && title.title.length <= 36);
  assert.ok(!events.some((event) => event.item?.type === 'function_call'));
  assert.deepEqual(events[2].response.usage, { input_tokens: 0, input_tokens_details: null,
    output_tokens: 0, output_tokens_details: null, total_tokens: 0 });
}
async function mainCompletion(provider) {
  const response = await provider.post({ ...output(), model: titleModel });
  assert.equal(response.status, 200);
  const events = eventsFromSse(await response.text());
  assert.equal(events[1].item.content[0].text, 'desktop-local-tool-provider-fixture');
}
async function mainFirst(provider) {
  // Reuse the original public SSE oracle while supplying the same model as title.
  return first({ post: (body) => provider.post({ ...body, model: titleModel }) });
}

test('GT01: opt-in title is independent before, between or after both exact main stages', async (t) => {
  for (const order of [['title', 'main1', 'main2'], ['main1', 'title', 'main2'], ['main1', 'main2', 'title']]) {
    await t.test(order.join('-'), async (t) => {
      const provider = await start(t, guiOptions);
      assert.equal(provider.events().find((event) => event.event === 'listening').maxRequests, 3);
      for (const step of order) {
        if (step === 'title') await titleResponse(provider);
        if (step === 'main1') await mainFirst(provider);
        if (step === 'main2') await mainCompletion(provider);
      }
      assert.deepEqual(await provider.exited, { value: 0, signal: null });
      await assert.rejects(fetch(`${provider.origin}/health`, { signal: AbortSignal.timeout(1000) }));
      assert.deepEqual(mainReceipts(provider).map((event) => [event.stage, event.toolResultConfirmed]), [[1, false], [2, true]]);
      const receipts = titleReceipts(provider); assert.equal(receipts.length, 1);
      assert.equal(receipts[0].mainStage, order.indexOf('title'));
      assert.equal(receipts[0].syntheticTitleConfirmed, true);
      assert.ok(!Object.hasOwn(receipts[0], 'stage') && !Object.hasOwn(receipts[0], 'toolResultConfirmed'));
      assert.deepEqual(Object.keys(receipts[0]).sort(), ['event', 'mainStage', 'sha256', 'syntheticTitleConfirmed', 'testOnly', 'wireBytes'].sort());
      const bytes = Buffer.from(JSON.stringify(titleBody()));
      assert.equal(receipts[0].wireBytes, bytes.length);
      assert.equal(receipts[0].sha256, createHash('sha256').update(bytes).digest('hex'));
    });
  }
});

test('GT02: opt-in title requires the exact frozen schema, format, model and disabled tools', async (t) => {
  for (const kind of ['schema-type', 'schema-extra', 'title-type', 'minimum', 'maximum', 'required',
    'additional-properties', 'title-extra', 'format-type', 'format-name', 'format-strict', 'format-extra',
    'model', 'tools', 'missing-tools', 'empty-input', 'non-message', 'missing-format', 'null-format', 'primitive-text']) {
    await t.test(kind, async (t) => {
      const provider = await start(t, guiOptions); const body = titleBody();
      const format = body.text.format; const schema = format.schema;
      if (kind === 'schema-type') schema.type = 'array';
      if (kind === 'schema-extra') schema.private = secret;
      if (kind === 'title-type') schema.properties.title.type = 'number';
      if (kind === 'minimum') schema.properties.title.minLength = 2;
      if (kind === 'maximum') schema.properties.title.maxLength = 37;
      if (kind === 'required') schema.required.push('private');
      if (kind === 'additional-properties') schema.additionalProperties = true;
      if (kind === 'title-extra') schema.properties.title.description = secret;
      if (kind === 'format-type') format.type = secret;
      if (kind === 'format-name') format.name = secret;
      if (kind === 'format-strict') format.strict = false;
      if (kind === 'format-extra') format.private = secret;
      if (kind === 'model') body.model = secret;
      if (kind === 'tools') body.tools = advertisement().tools;
      if (kind === 'missing-tools') delete body.tools;
      if (kind === 'empty-input') body.input = [];
      if (kind === 'non-message') body.input.push(output().input[0]);
      if (kind === 'missing-format') delete body.text.format;
      if (kind === 'null-format') body.text.format = null;
      if (kind === 'primitive-text') body.text = secret;
      await rejected(await provider.post(body), 422);
      assert.equal(titleReceipts(provider).length, 0); assert.equal(mainReceipts(provider).length, 0);
      const health = await guiHealth(provider);
      const main = ['missing-format', 'primitive-text'].includes(kind);
      assert.equal(health.stage, 0); assert.equal(health.mainAttempts, main ? 1 : 0);
      assert.equal(health.titleAttempts, main ? 0 : 1); assert.equal(health.titleCompleted, false);
    });
  }
});

test('GT03: successful and rejected title attempts both prevent a second title response', async (t) => {
  for (const validFirst of [true, false]) {
    await t.test(validFirst ? 'duplicate-success' : 'rejected-title-consumes-budget', async (t) => {
      const provider = await start(t, guiOptions);
      if (validFirst) await titleResponse(provider);
      else { const body = titleBody(); body.text.format.schema.properties.title.maxLength = 37; await rejected(await provider.post(body), 422); }
      await rejected(await provider.post(titleBody()), 429);
      await mainFirst(provider);
      await rejected(await provider.post(output()), 429);
      const health = await guiHealth(provider);
      assert.equal(health.attempts, 3); assert.equal(health.titleAttempts, 1); assert.equal(health.mainAttempts, 1);
      assert.equal(health.titleCompleted, validFirst); assert.equal(health.stage, 1);
      assert.equal(titleReceipts(provider).length, validFirst ? 1 : 0);
      assert.equal(mainReceipts(provider).filter((event) => event.toolResultConfirmed).length, 0);
    });
  }
});

test('GT04: opt-in never allows a third parsed main attempt after a rejected main request', async (t) => {
  const provider = await start(t, guiOptions);
  await rejected(await provider.post({ input: [], tools: [] }), 422);
  await mainFirst(provider); await rejected(await provider.post(output()), 429);
  const health = await guiHealth(provider);
  assert.equal(health.attempts, 3); assert.equal(health.mainAttempts, 2); assert.equal(health.titleAttempts, 0);
  assert.equal(health.stage, 1); assert.equal(health.titleCompleted, false);
  assert.equal(mainReceipts(provider).filter((event) => event.toolResultConfirmed).length, 0);
});

test('GT05: preclassification failures and concurrency consume the total three-POST ceiling', async (t) => {
  await t.test('auth-and-type', async (t) => {
    const provider = await start(t, guiOptions);
    await rejected(await provider.post(titleBody(), { authorization: secret }), 400);
    await rejected(await provider.post(titleBody(), { 'content-type': 'text/plain' }), 415);
    await titleResponse(provider); await rejected(await provider.post(advertisement()), 429);
    const health = await guiHealth(provider);
    assert.equal(health.attempts, 3); assert.equal(health.mainAttempts, 0); assert.equal(health.titleAttempts, 1);
    assert.equal(health.stage, 0); assert.equal(health.titleCompleted, true);
  });
  await t.test('size-and-invalid-json', async (t) => {
    const provider = await start(t, guiOptions);
    await rejected(await provider.post('x'.repeat(8 * 1024 * 1024 + 1)), 413);
    await rejected(await provider.post('{'), 422);
    await mainFirst(provider); await rejected(await provider.post(titleBody()), 429);
    const health = await guiHealth(provider);
    assert.equal(health.attempts, 3); assert.equal(health.mainAttempts, 1); assert.equal(health.titleAttempts, 0);
  });
  await t.test('inflight', async (t) => {
    const provider = await start(t, guiOptions); const pending = partialPost(provider.origin);
    t.after(() => pending.request.destroy()); await waitInFlight(provider);
    await rejected(await provider.post(titleBody()), 429);
    pending.request.end(JSON.stringify(advertisement()).slice(1)); assert.equal((await pending.response).status, 200);
    await titleResponse(provider); await rejected(await provider.post(output()), 429);
    const health = await guiHealth(provider);
    assert.equal(health.attempts, 3); assert.equal(health.mainAttempts, 1); assert.equal(health.titleAttempts, 1); assert.equal(health.stage, 1);
  });
});

test('GT05: main completion without title stays incomplete until the original lifetime closes it', async (t) => {
  const provider = await start(t, [...guiOptions, '--lifetime-ms', '700']);
  await mainFirst(provider); await mainCompletion(provider);
  const health = await guiHealth(provider);
  assert.equal(health.stage, 2); assert.equal(health.titleAttempts, 0); assert.equal(health.titleCompleted, false);
  assert.equal(titleReceipts(provider).length, 0);
  assert.deepEqual(await provider.exited, { value: 0, signal: null });
});

test('GT06: no title option preserves strict title rejection and the original two-POST budget', async (t) => {
  const provider = await start(t);
  await rejected(await provider.post(titleBody()), 422); await first(provider);
  await rejected(await provider.post(output()), 429);
  const health = await guiHealth(provider);
  assert.equal(health.attempts, 2); assert.equal(health.maxRequests, 2); assert.equal(health.stage, 1);
  assert.ok(!Object.hasOwn(health, 'titleAttempts')); assert.equal(titleReceipts(provider).length, 0);
});

test('GT07: title-model CLI rejects missing, duplicate and unsafe IDs without reflection', async () => {
  for (const suffix of [['--title-model'], ['--title-model', ''], ['--title-model', ' '],
    ['--title-model', titleModel, '--title-model', titleModel], ['--title-model', 'x'.repeat(257)],
    ['--title-model', '한'.repeat(86)], ['--title-model', secret + '\n'], ['--title-model', secret + '\u200b'],
    ['--title-model', ' ' + titleModel], ['--title-model', titleModel + ' ']]) {
    const child = spawn(process.execPath, [fixture, '--tool-name', toolName, ...suffix], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = ''; child.stdout.on('data', (chunk) => { stdout += chunk; }); child.stderr.on('data', (chunk) => { stderr += chunk; });
    const result = await Promise.race([
      new Promise((resolve, reject) => { child.once('error', reject); child.once('exit', (code) => resolve(code)); }),
      delay(3000).then(() => { child.kill(); throw new Error('Invalid title CLI failed to exit'); }),
    ]);
    assert.equal(result, 1); assert.equal(stdout, ''); assert.equal(stderr, 'Local tool fixture options rejected.\n');
    assert.ok(!stderr.includes(secret));
  }
});
