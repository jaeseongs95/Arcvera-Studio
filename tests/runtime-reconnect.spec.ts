import { test, expect, type Page } from '@playwright/test';

// This controlled Tauri boundary tests the real frontend, not native socket recovery.
// Actual isolated TEST/source27 acceptance is required separately.
async function setup(page: Page, options: Record<string, unknown> = {}) {
  await page.addInitScript(options => {
    const state = window as any;
    state.isTauri = true;
    let callback = 0;
    const callbacks = new Map<number, (value: any) => void>();
    const listeners = new Map<string, (value: any) => void>();
    const old = { id: 'old', generation: 1, threadId: 'same-thread', cwd: 'D:\\Work', title: '보존할 대화', state: options.state ?? 'idle', active: true, projectId: null, projectAssigned: false, projectRevision: 0 };
    const fixture = state.reconnectFixture = { calls: [] as any[], live: [old], applied: false, starts: 0, selected: false, holdList: options.holdList };
    const emit = (info: any) => listeners.get('session-changed')?.({ event: 'session-changed', id: 1, payload: { ...info } });
    const settings = () => ({ savedPath: null, resolvedPath: 'D:\\runtime\\codex.exe', source: 'environment', error: null, appServer: { savedPath: fixture.selected ? 'D:\\runtime\\new-server.exe' : null, resolvedPath: 'D:\\runtime\\new-server.exe', source: 'saved', error: null, kind: 'standalone' } });
    state.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    state.__TAURI_INTERNALS__ = {
      transformCallback(handler: (value: any) => void) { callbacks.set(++callback, handler); return callback; }, unregisterCallback() {},
      async invoke(command: string, args: any = {}) {
        fixture.calls.push({ command, args: structuredClone(args) });
        if (command === 'plugin:event|listen') { listeners.set(args.event, callbacks.get(args.handler)!); return callback; }
        if (command.startsWith('plugin:event|')) return null;
        if (command === 'get_ui_language') return 'ko';
        if (command === 'load_layout') return { version: 1, columns: 1, sessions: [] };
        if (command === 'list_projects') return [];
        if (command === 'list_sessions') {
          if (fixture.selected && fixture.holdList) await new Promise<void>(resolve => { (fixture as any).releaseList = () => { fixture.holdList = false; resolve(); }; });
          return structuredClone(fixture.live);
        }
        if (command === 'get_codex_settings') return settings();
        if (command === 'choose_app_server_executable') {
          if (options.selection === 'cancel') return null;
          if (options.selection === 'error') throw Error('선택한 파일은 실행 파일이 아닙니다.');
          fixture.selected = true; return settings();
        }
        if (command === 'runtime_transition_preview') return { canApply: true, fingerprint: 'known-selection', reason: null };
        if (command === 'session_suspend_idle') {
          const target = fixture.live.find((s: any) => s.id === args.id);
          if (!target || target.generation !== args.generation) throw Error('wrong generation');
          if (args.id !== old.id) {
            if (options.cleanupFailure) throw Error('new cleanup unknown');
            Object.assign(target, { state: 'exited', active: false }); emit(target); return null;
          }
          if (options.nativeActiveRace) throw Error('new active turn preserved');
          if (options.stopFailure) throw Error('종료를 확인할 수 없습니다.');
          if (args.id !== old.id || args.generation !== old.generation) throw Error('wrong generation');
          Object.assign(old, { state: 'exited', active: false }); emit(old); return null;
        }
        if (command === 'apply_runtime_transition') {
          if (options.holdApply) await new Promise<void>(resolve => { (fixture as any).releaseApply = resolve; });
          if (options.apply === 'error') throw Error('교체 응답을 받지 못했습니다.');
          if (options.apply === 'unknown') return { status: 'shutdownOutcomeUnknown', success: false, outcome: 'unknown', message: '교체 결과를 확인할 수 없습니다.' };
          fixture.applied = true;
          if (options.changedGeneration) { old.generation++; emit(old); }
          return { status: 'updated', success: true, outcome: 'updated', message: null };
        }
        if (command === 'start_session') {
          if (options.resumeFailure) throw Error('동일 대화 재연결 실패');
          if (args.request.threadId && fixture.live.some((s: any) => s.active && s.threadId === args.request.threadId)) throw Error('duplicate active thread');
          const info = { ...args.request, id: `new-${++fixture.starts}`, generation: 1, threadId: args.request.threadId ?? 'other-thread', state: 'idle', active: true };
          fixture.live.push(info); emit(info);
          if (options.changedAfterStart) { old.generation++; emit(old); }
          return info;
        }
        if (command === 'choose_conversation_attachments') return [{ type: 'file', path: 'D:\\Work\\draft.txt', name: 'draft.txt' }];
        if (command === 'conversation_request') {
          const kind = args.request.type;
          if (kind === 'detach' && args.id === old.id && fixture.selected && options.activeDuringDetach) { old.state = 'running'; emit(old); }
          if (kind === 'contextStatus') return { passiveSubscriptionSupported: true, connected: true, canCompact: false, compactSupported: false };
          if (kind === 'attach' && args.id !== old.id && options.attachFailure) throw Error('새 연결의 구독 실패');
          if (kind === 'attach') return { threadId: 'same-thread', model: 'fixture-model', reasoningEffort: 'high' };
          if (kind === 'poll') return { events: [], pendingRequests: [], connected: true, gap: false };
          if (fixture.applied && args.id === old.id && ['page', 'items'].includes(kind)) throw Error('old transport disconnected');
          if (['page', 'items', 'models'].includes(kind)) return { data: [], nextCursor: null };
          return {};
        }
        return null;
      },
    };
  }, options);
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('aria-busy', 'false');
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-toggle')).toBeVisible();
  await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();
  await page.locator('.terminal-card[data-id="old"] .conversation-input').fill('초안 보존 한글🙂');
}

const calls = (page: Page, command: string) => page.evaluate(command => (window as any).reconnectFixture.calls.filter((c: any) => c.command === command), command);
async function choose(page: Page) {
  await page.locator('#settings-open').click();
  await page.locator('#codex-settings-open').click();
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#app-server-executable-choose')).toBeEnabled();
}
async function closeSettings(page: Page) {
  await page.locator('#codex-settings-close').click();
  await page.locator('#settings-close').click();
}

test('RC-RECONNECT 교체 전 연결을 정리하고 같은 대화·초안·보기를 새 연결에 복원한다', async ({ page }) => {
  await setup(page);
  await choose(page);
  await closeSettings(page);
  const next = page.locator('.terminal-card[data-id="new-1"]');
  await expect(next, 'the original disconnected card is not a fresh connection').toBeVisible({ timeout: 5000 });
  await expect(next.locator('.conversation-host')).toBeVisible();
  await expect(next.locator('.conversation-input')).toHaveValue('초안 보존 한글🙂');
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  const sequence = await page.evaluate(() => (window as any).reconnectFixture.calls.map((c: any) => c.command));
  expect(sequence.indexOf('session_suspend_idle')).toBeLessThan(sequence.indexOf('apply_runtime_transition'));
  expect(sequence.indexOf('apply_runtime_transition')).toBeLessThan(sequence.indexOf('start_session'));
  expect(await calls(page, 'session_suspend_idle')).toEqual([{ command: 'session_suspend_idle', args: { id: 'old', generation: 1 } }]);
  expect(await calls(page, 'start_session')).toHaveLength(1);
  expect((await calls(page, 'start_session'))[0].args.request).toMatchObject({ threadId: 'same-thread', cwd: 'D:\\Work' });
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(1);
  expect((await calls(page, 'conversation_request')).filter(c => c.args.request.type === 'send')).toEqual([]);
});

for (const state of ['running', 'waiting', 'unknown']) test(`RC-UNSAFE ${state} 작업은 서버 교체 전에 보존한다`, async ({ page }) => {
  await setup(page, { state }); await choose(page); await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
  expect(await calls(page, 'session_suspend_idle')).toHaveLength(0);
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(0);
  expect(await calls(page, 'start_session')).toHaveLength(0);
});

test('RC-UNSAFE 첨부 초안은 종료·교체·첨부 정리 없이 보존한다', async ({ page }) => {
  await setup(page);
  await page.locator('.terminal-card[data-id="old"] .conversation-attach').click();
  await expect(page.locator('.conversation-attachment')).toContainText('draft.txt');
  await choose(page); await closeSettings(page);
  await expect(page.locator('.conversation-attachment')).toContainText('draft.txt');
  for (const command of ['session_suspend_idle', 'apply_runtime_transition', 'cleanup_conversation_attachment']) expect(await calls(page, command)).toEqual([]);
});

test('RC-STOPFAIL 종료 미확인은 교체와 새 연결을 시작하지 않고 초안을 남긴다', async ({ page }) => {
  await setup(page, { stopFailure: true }); await choose(page); await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
  expect(await calls(page, 'apply_runtime_transition')).toEqual([]);
  expect(await calls(page, 'start_session')).toEqual([]);
});

for (const apply of ['error', 'unknown']) test(`RC-APPLYUNKNOWN ${apply} 결과는 재교체·재시작 없이 초안을 남긴다`, async ({ page }) => {
  await setup(page, { apply }); await choose(page); await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(1);
  expect(await calls(page, 'start_session')).toEqual([]);
});

for (const selection of ['cancel', 'error']) test(`RC-CANCEL-ERROR ${selection} 뒤 기존 연결과 새 세션 진입이 열린다`, async ({ page }) => {
  await setup(page, { selection }); await choose(page); await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
  await page.locator('#new-session').click(); await page.locator('#session-title').fill('다른 대화'); await page.locator('#session-cwd').fill('D:\\Work'); await page.locator('#create-submit').click();
  await expect.poll(async () => (await calls(page, 'start_session')).length, { timeout: 5000 }).toBe(1);
  expect(await calls(page, 'session_suspend_idle')).toEqual([]);
  expect(await calls(page, 'apply_runtime_transition')).toEqual([]);
});

test('RC-EXIT-FENCE 교체 중 이전 세대가 바뀌면 새 연결과 이전 카드 제거를 막는다', async ({ page }) => {
  await setup(page, { changedGeneration: true }); await choose(page); await closeSettings(page);
  expect(await calls(page, 'start_session')).toEqual([]);
  await expect(page.locator('.terminal-card[data-id="old"]')).toHaveCount(1);
});

test('RC-EXIT-FENCE 교체 중 종료는 응답 정산 뒤 종료하며 대화 재개 의도를 남긴다', async ({ page }) => {
  await setup(page, { holdApply: true });
  await page.locator('#settings-open').click(); await page.locator('#codex-settings-open').click(); await page.locator('#app-server-executable-choose').click();
  await expect.poll(async () => (await calls(page, 'apply_runtime_transition')).length).toBe(1);
  await closeSettings(page);
  await page.locator('#exit').click(); await page.locator('#confirm-exit').click();
  expect(await calls(page, 'complete_exit')).toEqual([]);
  await page.evaluate(() => (window as any).reconnectFixture.releaseApply());
  await expect.poll(async () => (await calls(page, 'complete_exit')).length).toBe(1);
  expect(await calls(page, 'start_session')).toEqual([]);
  const layouts = await calls(page, 'save_layout');
  expect(layouts.at(-1)?.args.layout.sessions).toEqual(expect.arrayContaining([expect.objectContaining({ threadId: 'same-thread', resumeOnStartup: true })]));
});

for (const failure of ['resumeFailure', 'attachFailure']) test(`RC-RECONNECT ${failure}는 기존 초안을 남기고 성공을 표시하지 않는다`, async ({ page }) => {
  await setup(page, { [failure]: true }); await choose(page);
  await expect(page.locator('#codex-settings-error')).toBeVisible();
  await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(1);
  expect(await calls(page, 'start_session')).toHaveLength(1);
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  expect(await page.evaluate(() => (window as any).reconnectFixture.live.filter((s: any) => s.active))).toHaveLength(0);
});

test('RC-UNSAFE 늦은 소유권 조회 동안 새 입력을 막고 교체 후 해제한다', async ({ page }) => {
  await setup(page, { holdList: true });
  await page.locator('#settings-open').click(); await page.locator('#codex-settings-open').click(); await page.locator('#app-server-executable-choose').click();
  await expect.poll(async () => page.evaluate(() => typeof (window as any).reconnectFixture.releaseList)).toBe('function');
  await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"]')).toHaveJSProperty('inert', true);
  expect(await calls(page, 'session_suspend_idle')).toEqual([]);
  expect(await calls(page, 'apply_runtime_transition')).toEqual([]);
  await page.evaluate(() => (window as any).reconnectFixture.releaseList());
  await expect(page.locator('.terminal-card[data-id="new-1"]')).toBeVisible();
  await expect(page.locator('.terminal-card[data-id="new-1"]')).toHaveJSProperty('inert', false);
});

for (const race of ['activeDuringDetach', 'nativeActiveRace']) test(`RC-IDLE-ONLY ${race} 새 작업은 보존하고 교체하지 않는다`, async ({ page }) => {
  await setup(page, { [race]: true }); await choose(page); await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
  expect(await calls(page, 'apply_runtime_transition')).toEqual([]);
  expect(await calls(page, 'start_session')).toEqual([]);
  if (race === 'activeDuringDetach') expect(await calls(page, 'session_suspend_idle')).toEqual([]);
  expect(await page.evaluate(() => (window as any).reconnectFixture.live.filter((s: any) => s.active))).toHaveLength(1);
});
test('RC-FAILED-NEW-OWNER start 이후 세대변경은 새 연결만 회수하고 원카드를 보존한다', async ({ page }) => {
  await setup(page, { changedAfterStart: true }); await choose(page); await closeSettings(page);
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  await expect(page.locator('.terminal-card[data-id="old"] .runtime-recovery-draft')).toHaveValue('초안 보존 한글🙂');
  expect(await page.evaluate(() => (window as any).reconnectFixture.live.filter((s: any) => s.active))).toHaveLength(0);
});
test('RC-FAILED-NEW-OWNER 정리 미확인은 오류와 원초안을 유지하고 재송신하지 않는다', async ({ page }) => {
  await setup(page, { attachFailure: true, cleanupFailure: true }); await choose(page);
  await expect(page.locator('#codex-settings-error')).toContainText('새 연결 정리 미확인');
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(1);
  expect(await calls(page, 'start_session')).toHaveLength(1);
  await closeSettings(page);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
});
test('RC-TUI-PRESERVE 전달된 터미널 입력은 지우거나 재송신하지 않고 교체를 거부한다', async ({ page }) => {
  await setup(page);
  await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();
  await page.locator('.terminal-card[data-id="old"] .xterm-helper-textarea').pressSequentially('unsent-composer');
  await expect.poll(async () => (await calls(page, 'session_write')).length).toBeGreaterThan(0);
  await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();
  await choose(page); await closeSettings(page);
  expect(await calls(page, 'session_suspend_idle')).toEqual([]);
  expect(await calls(page, 'apply_runtime_transition')).toEqual([]);
  await expect(page.locator('.terminal-card[data-id="old"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
});
