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
    staleThreadRejected: true, nativePanelsObserved: true, sessionIdsGenerationsAndThreadsPreserved: true, modelTurnSent: false, providerRequests,
    limitations: ['Core1.0.1 baseline, not Core1.1.0', 'No tool invocation or hook handler execution', 'No per-thread reload/config version transition was tested'] };
}
