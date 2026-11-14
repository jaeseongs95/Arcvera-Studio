// Test-only real Native/Core reads on the root's newly created fixture sessions.
import assert from 'node:assert/strict';

export async function checkNativeRuntimeExtensions({ page, nativeCall, cwd, sessions, until, maskedScreenshot }) {
  const before = await nativeCall('list_sessions');
  assert(sessions.length === 2 && before.length === 2);
  const selected = before.find(session => session.id === sessions[0].id);
  assert(selected?.active && selected.threadId === sessions[0].threadId);
  const call = request => nativeCall('conversation_request', { id: selected.id, generation: selected.generation, request });
  const inventory = await nativeCall('extensions_list', { cwd });
  const server = inventory.mcpServers?.items.find(item => item.name.startsWith('desktop210_probe_') && item.transport === 'stdio');
  assert(server, 'Own prepared local MCP fixture is required');
  const tools = await until(() => call({ type: 'mcpStatus', expectedThreadId: selected.threadId, serverName: server.name }),
    value => value.toolsState === 'available', 'local MCP tools observed for the selected native thread', 20000);
  assert.equal(tools.threadId, selected.threadId); assert.equal(tools.serverName, server.name);
  assert(tools.toolNames.some(name => name.includes('desktop_local_probe_v1')));
  const hooks = await call({ type: 'hooksStatus', expectedThreadId: selected.threadId });
  assert.equal(hooks.threadId, selected.threadId);
  assert.equal(hooks.provenance, 'loadedThreadHooksList');
  assert.equal(hooks.executionGate, 'unknown'); assert.equal(hooks.applicationState, 'unknown');
  assert(Array.isArray(hooks.hooks.items));
  const allowed = new Set(['id','eventName','handlerType','source','enabled','isManaged','trustStatus']);
  for (const hook of hooks.hooks.items) assert(Object.keys(hook).every(key => allowed.has(key)), 'Only readonly metadata may cross the Native projection');
  let staleRejected = false;
  try { await call({ type: 'hooksStatus', expectedThreadId: 'stale-other-thread' }); } catch { staleRejected = true; }
  assert(staleRejected);
  await page.locator(`.terminal-card[data-id="${selected.id}"] .terminal-host`).click();
  await page.locator('#settings-open').click();
  await page.locator('#extensions-open').click();
  await until(() => page.locator('#extensions-dialog').getAttribute('aria-busy'), value => value === 'false', 'native extension inventory displayed');
  await page.locator('#extensions-tab-mcpServers').click();
  await page.locator('#extensions-list .extensions-row').filter({ hasText: server.name }).click();
  await page.locator('#extensions-mcp-runtime').getByRole('button', { name: '현재 대화 상태조회', exact: true }).click();
  await until(() => page.locator('#extensions-mcp-runtime').textContent(), text => text.includes('desktop_local_probe_v1'), 'actual MCP runtime panel displays discovered local tool');
  await maskedScreenshot('native-mcp-runtime.png');
  await page.locator('#extensions-tab-hooks').click();
  await page.locator('#extensions-hook-runtime button').first().click();
  await until(() => page.locator('#extensions-dialog').getAttribute('aria-busy'), value => value === 'false', 'actual Hook runtime panel read completes');
  assert.equal(await page.locator('#extensions-error').isVisible(), false);
  assert((await page.locator('#extensions-hook-runtime').textContent()).includes('미확인'));
  await maskedScreenshot('native-hook-runtime.png');
  await page.locator('#extensions-close').click();
  await page.locator('#settings-close').click();
  let composition;
  const compositionExpectation = process.env.AGENT_GRID_NATIVE_COMPOSITION_EXPECT;
  if (compositionExpectation) {
    assert(['available', 'unavailable'].includes(compositionExpectation), 'Explicit composition availability expectation is required');
    const result = await call({ type: 'contextComposition', expectedThreadId: selected.threadId, cursor: 0 });
    if (compositionExpectation === 'available') {
      assert.equal(result.supported, true); assert.equal(result.snapshot.threadId, selected.threadId);
      assert.equal(typeof result.snapshot.snapshotId, 'string'); assert.match(result.snapshot.contextIndexRevision, /^\d+$/);
      assert(Array.isArray(result.snapshot.fragments) && result.snapshot.fragments.length <= 25);
    } else {
      assert.equal(result.supported, false); assert(['unsupported', 'unavailable'].includes(result.reason));
      assert.equal(result.snapshot, undefined);
    }
    let wrongCompositionTargetRejected = false;
    try { await call({ type: 'contextComposition', expectedThreadId: 'stale-other-thread', cursor: 0 }); } catch { wrongCompositionTargetRejected = true; }
    assert(wrongCompositionTargetRejected);
    const stillAvailable = await call({ type: 'mcpStatus', expectedThreadId: selected.threadId, serverName: server.name });
    assert.equal(stillAvailable.toolsState, 'available', 'The legacy missing RPC must not stop or replace the working runtime');
    await page.locator(`.terminal-card[data-id="${selected.id}"] .context-usage`).click();
    await until(() => page.locator('.session-context-dialog .context-composition-status').textContent(), text =>
      compositionExpectation === 'available' ? /스냅샷|조각/.test(text) : /지원|미확인/.test(text), 'actual composition result shown without fabricated counts');
    if (compositionExpectation === 'unavailable') assert.equal(await page.locator('.session-context-dialog .context-composition-fragments > li').count(), 0);
    await maskedScreenshot('native-context-composition.png');
    await page.locator('.session-context-dialog').getByRole('button', { name: '닫기', exact: true }).click();
    composition = { expectation: compositionExpectation, supported: result.supported, reason: result.reason, wrongTargetRejected: true, runtimeRemainsAvailable: true, actualPanelObserved: true };
  }
  let providerRequests;
  if (process.env.AGENT_GRID_NATIVE_PROVIDER_ORIGIN) {
    const url = new URL(process.env.AGENT_GRID_NATIVE_PROVIDER_ORIGIN);
    assert(url.protocol === 'http:' && url.hostname === '127.0.0.1' && !url.username && !url.password);
    const health = await (await fetch(new URL('/health', url), { signal: AbortSignal.timeout(5000) })).json();
    assert(health.testOnly); assert.equal(health.completedRequests, 0); providerRequests = health.completedRequests;
  }
  const after = await nativeCall('list_sessions');
  for (const previous of before) { const current = after.find(session => session.id === previous.id); assert(current?.active && current.generation === previous.generation && current.threadId === previous.threadId); }
  return { threadId: selected.threadId, nativeGeneration: selected.generation, serverName: server.name, toolsState: tools.toolsState,
    observedToolCount: tools.toolNames.length, hookCount: hooks.hooks.items.length, hookGate: hooks.executionGate,
    staleThreadRejected: true, nativePanelsObserved: true, composition, sessionIdsGenerationsAndThreadsPreserved: true, modelTurnSent: false, providerRequests,
    limitations: ['Core1.0.1 baseline, not Core1.1.0', 'No tool invocation or hook handler execution', 'No per-thread reload/config version transition was tested'] };
}
