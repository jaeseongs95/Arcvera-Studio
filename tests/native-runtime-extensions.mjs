// Test-only real Native/Core reads on the root's newly created fixture sessions.
import assert from 'node:assert/strict';

export async function checkNativeRuntimeExtensions({ page, nativeCall, cwd, sessions, until, maskedScreenshot }) {
  const before = await nativeCall('list_sessions');
  assert(sessions.length === 2 && before.length === 2);
  const selected = before.find(session => session.id === sessions[0].id);
  assert(selected?.active && selected.threadId === sessions[0].threadId);
  const call = request => nativeCall('conversation_request', { id: selected.id, generation: selected.generation, request });
  const inventory = await nativeCall('extensions_list', { cwd });
  const expectedServer = process.env.AGENT_GRID_NATIVE_MCP_SERVER;
  if (expectedServer) assert(/^[A-Za-z0-9_-]{1,128}$/.test(expectedServer));
  const coreVersion = process.env.AGENT_GRID_NATIVE_CORE_VERSION ?? '1.0.1';
  assert(['1.0.1', '1.1.0'].includes(coreVersion));
  const server = inventory.mcpServers?.items.find(item => (expectedServer ? item.name === expectedServer : item.name.startsWith('desktop210_probe_')) && item.transport === 'stdio');
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
  let contextSettings, boundedLoad;
  if (coreVersion === '1.1.0') {
    const targets = before.map(session => ({ session, request: request => nativeCall('conversation_request', { id: session.id, generation: session.generation, request }) }));
    const original = await Promise.all(targets.map(t => t.request({ type: 'contextSettings' })));
    assert(original.every(v => v.supported === true && v.canChange === true));
    const change = async (target, value) => {
      const card = page.locator('.terminal-card[data-id="' + target.session.id + '"]');
      await card.locator('.context-usage').click();
      const dialog = page.locator('.session-context-dialog');
      const button = dialog.getByRole('button', { name: value === null ? '기본값 복원' : '적용', exact: true });
      await until(() => button.isEnabled(), Boolean, 'context editor read completed');
      if (value !== null) await dialog.locator('input').fill(String(value));
      await button.click();
      await until(() => dialog.getAttribute('aria-busy'), v => v === 'false', 'context write acknowledgement');
      const actual = await target.request({ type: 'contextSettings' });
      assert.equal(actual.modelContextWindow, value);
      const summary = await dialog.locator('.session-context-summary').innerText();
      assert(summary.includes(value === null ? '기본값' : value.toLocaleString('ko-KR')));
      assert(summary.includes(actual.effectiveContextWindow === null ? '기본값' : actual.effectiveContextWindow.toLocaleString('ko-KR')));
      await dialog.getByRole('button', { name: '닫기', exact: true }).click();
      return actual;
    };
    const first = await change(targets[0], 8192);
    assert.deepEqual(await targets[1].request({ type: 'contextSettings' }), original[1]);
    const second = await change(targets[1], 16384);
    assert.equal((await targets[0].request({ type: 'contextSettings' })).modelContextWindow, 8192);
    const conflict = await targets[0].request({ type: 'setContextWindow', modelContextWindow: 4096, expectedModelContextWindow: 16384 });
    assert.equal(conflict.status, 'conflict');
    assert.equal((await targets[0].request({ type: 'contextSettings' })).modelContextWindow, 8192);
    for (let i = 0; i < targets.length; i++) await change(targets[i], original[i].modelContextWindow);
    contextSettings = { distinctRequestedValues: [first.modelContextWindow, second.modelContextWindow], otherThreadPreserved: true, staleConflict: true, originalsRestored: true };
    const screens = await Promise.all(targets.map(t => page.locator('.terminal-card[data-id="' + t.session.id + '"] .xterm-screen').elementHandle()));
    const durationsMs = [];
    try {
      for (let round = 0; round < 6; round++) for (const target of targets) {
        const start = performance.now();
        await page.locator('.terminal-card[data-id="' + target.session.id + '"] .terminal-host').click();
        const settings = await target.request({ type: 'contextSettings' });
        const composition = await target.request({ type: 'contextComposition', expectedThreadId: target.session.threadId, cursor: 0 });
        assert.equal(settings.supported, true); assert.equal(composition.supported, true);
        assert.equal(composition.snapshot.threadId, target.session.threadId);
        durationsMs.push(performance.now() - start);
      }
      const current = await nativeCall('list_sessions');
      for (let i = 0; i < targets.length; i++) {
        const previous = targets[i].session, actual = current.find(v => v.id === previous.id);
        assert(actual?.active && actual.generation === previous.generation && actual.threadId === previous.threadId);
        assert(await screens[i].evaluate(el => el.isConnected && el.closest('.terminal-card')?.querySelector('.xterm-screen') === el));
      }
      boundedLoad = { rounds: 6, pairedReads: durationsMs.length, durationsMs, sessionsPreserved: true, terminalDomPreserved: true, sustainedLoadVerified: false };
    } finally { for (const screen of screens) await screen?.dispose(); }
  }
  let providerRequests;
  if (process.env.AGENT_GRID_NATIVE_PROVIDER_ORIGIN) {
    const url = new URL(process.env.AGENT_GRID_NATIVE_PROVIDER_ORIGIN);
    assert(url.protocol === 'http:' && url.hostname === '127.0.0.1' && !url.username && !url.password);
    const health = await (await fetch(new URL('/health', url), { signal: AbortSignal.timeout(5000) })).json();
    assert.equal(health.testOnly, true);
    const protocol = process.env.AGENT_GRID_NATIVE_PROVIDER_PROTOCOL ?? 'local';
    assert(['local', 'tool'].includes(protocol));
    if (protocol === 'tool') {
      assert.equal(health.stage, 0); assert.equal(health.mainAttempts, 0);
      assert.equal(health.inFlight, false);
      assert(Number.isInteger(health.titleAttempts) && health.titleAttempts >= 0 && health.titleAttempts <= 1);
      providerRequests = health.mainAttempts;
    } else {
      assert.equal(health.completedRequests, 0); providerRequests = health.completedRequests;
    }
  }
  const after = await nativeCall('list_sessions');
  for (const previous of before) { const current = after.find(session => session.id === previous.id); assert(current?.active && current.generation === previous.generation && current.threadId === previous.threadId); }
  return { threadId: selected.threadId, nativeGeneration: selected.generation, serverName: server.name, toolsState: tools.toolsState,
    observedToolCount: tools.toolNames.length, hookCount: hooks.hooks.items.length, hookGate: hooks.executionGate,
    staleThreadRejected: true, nativePanelsObserved: true, composition, sessionIdsGenerationsAndThreadsPreserved: true, modelTurnSent: false, providerRequests,
    contextSettings, boundedLoad, coreVersion, limitations: [ 'No tool invocation or hook handler execution', 'No per-thread reload/config version transition was tested'] };
}
