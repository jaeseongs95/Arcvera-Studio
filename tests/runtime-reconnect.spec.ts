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
    const old = { id: 'old', generation: 1, threadId: 'same-thread', cwd: 'D:\\Work', title: '보존할 대화', state: options.state ?? 'idle', active: true, projectId: null, projectAssigned: false, projectRevision: 0, ...(options.metadata as object ?? {}) };
    const originals = [old, ...((options.others as any[]) ?? [])];
    const fixture = state.reconnectFixture = { calls: [] as any[], live: [...originals], applied: false, starts: 0, selected: false, holdList: options.holdList };
    (fixture as any).emitOutput = (data: string, id = old.id, seq = 1) => { const owner = fixture.live.find((s: any) => s.id === id); listeners.get('terminal-output')?.({event:'terminal-output',id:1,payload:{id,generation:owner?.generation ?? 1,seq,data}}); };
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
        if (command === 'list_projects') return options.projects ?? [];
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
          if (!originals.some(s => s.id === args.id)) {
            if (options.cleanupFailure) throw Error('new cleanup unknown');
            Object.assign(target, { state: 'exited', active: false }); emit(target); return null;
          }
          if (options.nativeActiveRace) throw Error('new active turn preserved');
          if (options.stopFailure) throw Error('종료를 확인할 수 없습니다.');
          Object.assign(target, { state: 'exited', active: false }); emit(target); return null;
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
          // Match native StartRequest defaults, including automatic null project identity.
          const info = { ...args.request, projectId: args.request.projectId ?? null, projectAssigned: !!(args.request.projectAssigned || args.request.projectId), projectRevision: 0, id: `new-${++fixture.starts}`, generation: 1, threadId: args.request.threadId ?? 'other-thread', state: 'idle', active: true };
          fixture.live.push(info); emit(info);
          if (options.startupStatusOnly) { (fixture as any).emitOutput('\x1b[5n', info.id); await new Promise(resolve => setTimeout(resolve, 150)); }
          else if (options.startupOutput !== false) { (fixture as any).emitOutput('STARTUP_MARKER' + (options.splitStartup ? '\x1b[' : '\x1b[6n'), info.id); if (options.splitStartup) { await new Promise(resolve => setTimeout(resolve, 20)); (fixture as any).emitOutput('6n', info.id, 2); } await new Promise(resolve => setTimeout(resolve, 150)); }
          if (options.changedAfterStart) { old.generation++; emit(old); }
          return info;
        }
        if (command === 'choose_conversation_attachments') return [{ type: 'file', path: 'D:\\Work\\draft.txt', name: 'draft.txt' }];
        if (command === 'session_write' && (options.writeFailure || (options.initialWriteFailure && args.id !== old.id))) throw Error('write outcome unknown');
        if (command === 'conversation_request') {
          const kind = args.request.type;
          if (kind === 'detach' && args.id === old.id && fixture.selected && options.activeDuringDetach) { old.state = 'running'; emit(old); }
          if (kind === 'contextStatus') return { passiveSubscriptionSupported: true, connected: true, canCompact: false, compactSupported: false };
          if (kind === 'attach' && args.id !== old.id && options.attachFailure) throw Error('새 연결의 구독 실패');
          if (kind === 'attach') return { threadId: fixture.live.find(s => s.id === args.id)?.threadId, model: 'fixture-model', reasoningEffort: 'high' };
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
  await expect(page.locator('#app-server-executable-choose')).toBeEnabled({ timeout: 10000 });
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

test('RC-AUTO-INPUT real xterm CPR replies do not mark a user composer draft', async ({page}) => {
 await setup(page);await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();
 await page.evaluate(()=>(window as any).reconnectFixture.emitOutput('\x1b[6n'));
 await expect.poll(async()=>(await calls(page,'session_write')).some((x:any)=>/^\x1b\[\d+;\d+R$/.test(x.args.data))).toBe(true);
 await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();await choose(page);await closeSettings(page);
 await expect(page.locator('.terminal-card[data-id="new-1"] .conversation-input')).toHaveValue('초안 보존 한글🙂');
 expect(await calls(page,'apply_runtime_transition')).toHaveLength(1);
});

for (const splitStartup of [false, true]) test(`RC-STARTUP-OUTPUT split=${splitStartup} reconnect retains initial output until TUI can answer`, async ({ page }) => {
  await setup(page, { startupOutput: true, splitStartup });
  await choose(page); await closeSettings(page);
  const next = page.locator('.terminal-card[data-id="new-1"]');
  await expect(next.locator('.conversation-input')).toHaveValue('초안 보존 한글🙂');
  await next.locator('.conversation-toggle').click();
  await expect.poll(async () => (await calls(page, 'session_write')).some(c => c.args.id === 'new-1' && /^\x1b\[\d+;\d+R$/.test(c.args.data))).toBe(true);
  await expect(next.locator('.xterm-rows')).toContainText('STARTUP_MARKER');
  expect((await calls(page, 'conversation_request')).filter(c => c.args.request.type === 'send')).toEqual([]);
});


for (const method of ['typing', 'text', 'paste-event', 'composition-event', 'Shift+Enter', 'Alt+Enter', 'unknown-write']) test(`RC-INPUT-ORIGINS ${method} keeps input preservation refusal`, async ({ page }) => {
  await setup(page, { writeFailure: method === 'unknown-write' });
  await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();
  const input = page.locator('.xterm-helper-textarea'); await input.focus();
  if (method === 'paste-event') await input.evaluate(el => { const data = new DataTransfer(); data.setData('text/plain', 'own-paste'); el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true })); });
  else if (method === 'composition-event') { await input.dispatchEvent('compositionstart', { data: '한' }); await page.keyboard.insertText('한글'); await input.dispatchEvent('compositionend', { data: '한글' }); }
  else if (method === 'text') await page.keyboard.insertText('own-text');
  else if (method.includes('Enter')) await input.press(method);
  else await input.pressSequentially('own-input');
  await expect.poll(async () => (await calls(page, 'session_write')).length).toBeGreaterThan(0);
  await page.locator('.terminal-card[data-id="old"] .conversation-toggle').click();
  await choose(page); await closeSettings(page);
  expect(await calls(page, 'session_suspend_idle')).toEqual([]);
  expect(await calls(page, 'apply_runtime_transition')).toEqual([]);
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  await expect(page.locator('.conversation-input')).toHaveValue('초안 보존 한글🙂');
});

for (const options of [{ startupOutput: false }, { initialWriteFailure: true }, { startupStatusOnly: true }]) test(`RC-STARTUP-UNCONFIRMED ${JSON.stringify(options)} preserves original draft and cleans new owner`, async ({ page }) => {
  await setup(page, options); await choose(page);
  await expect(page.locator('#codex-settings-error')).toBeVisible(); await closeSettings(page);
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  await expect(page.locator('.conversation-input')).toHaveValue('초안 보존 한글🙂');
  expect(await page.evaluate(() => (window as any).reconnectFixture.live.filter((s: any) => s.active))).toEqual([]);
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(1);
  expect((await calls(page, 'conversation_request')).filter(c => c.args.id === 'new-1' && c.args.request.type === 'attach')).toEqual([]);
});

const reconnectProjects = [
  { id: 'a', name: '프로젝트 A', cwd: 'D:\\Work', createdAt: 1, favorite: false },
  { id: 'b', name: '프로젝트 B', cwd: 'D:\\Other', createdAt: 2, favorite: false },
];

test('RC-HIDDEN-PROJECT 선택하지 않은 B도 초기 CPR 후 같은 대화와 초안을 복원한다', async ({ page }) => {
  const b = { id: 'old-b', generation: 3, threadId: 'thread-b', cwd: 'D:\\Other', title: 'B 대화', state: 'idle', active: true, projectId: 'b', projectAssigned: true, projectRevision: 0 };
  await setup(page, { projects: reconnectProjects, metadata: { projectId: 'a', projectAssigned: true }, others: [b] });
  const oldB = page.locator('.terminal-card[data-id="old-b"]');
  await oldB.locator('.conversation-toggle').click();
  await oldB.locator('.conversation-input').fill('숨은 B 초안🙂');
  await page.locator('#project-tabs [data-project-key="id:a"]').click();
  await expect(oldB).toBeHidden();
  await choose(page);
  await expect(page.locator('#codex-settings-error')).toBeHidden();
  await closeSettings(page);
  await expect(page.locator('.terminal-card')).toHaveCount(2);
  await expect(page.locator('#project-tabs [aria-selected="true"]')).toHaveAttribute('data-project-key', 'id:a');
  const owners = await page.evaluate(() => (window as any).reconnectFixture.live.filter((s: any) => s.active));
  expect(owners).toHaveLength(2);
  for (const [threadId, projectId, cwd, draft] of [['same-thread', 'a', 'D:\\Work', '초안 보존 한글🙂'], ['thread-b', 'b', 'D:\\Other', '숨은 B 초안🙂']]) {
    const owner = owners.find((s: any) => s.threadId === threadId)!;
    expect(owner).toMatchObject({ projectId, projectAssigned: true, cwd, projectRevision: 0 });
    const card = page.locator(`.terminal-card[data-id="${owner.id}"]`);
    await expect(card.locator('.conversation-input')).toHaveValue(draft);
    const allCalls = await page.evaluate(() => (window as any).reconnectFixture.calls);
    const writes = allCalls.filter((c: any) => c.command === 'session_write' && c.args.id === owner.id && /^\x1b\[\d+;\d+R$/.test(c.args.data));
    expect(writes).toHaveLength(1);
    expect(writes[0].args.generation).toBe(owner.generation);
    expect(allCalls.indexOf(writes[0])).toBeLessThan(allCalls.findIndex((c: any) => c.command === 'conversation_request' && c.args.id === owner.id && c.args.request.type === 'attach'));
    if (projectId === 'b') {
      await expect(card).toBeHidden();
      // Synthetic dispatch deliberately challenges the hidden-input gate.
      const before = (await calls(page, 'session_write')).length;
      await card.locator('.xterm-helper-textarea').dispatchEvent('keydown', { key: 'x', code: 'KeyX', keyCode: 88, bubbles: true });
      await card.locator('.xterm-helper-textarea').dispatchEvent('input', { data: 'hidden-user', inputType: 'insertText', bubbles: true });
      expect(await calls(page, 'session_write')).toHaveLength(before);
    }
  }
  expect(await calls(page, 'apply_runtime_transition')).toHaveLength(1);
  expect((await calls(page, 'conversation_request')).filter(c => c.args.request.type === 'send')).toEqual([]);
});

for (const classification of ['auto-registered', 'auto-cwd', 'unassigned', 'explicit']) for (const tab of ['all', 'group']) test(`RC-PROJECT-PRESERVE-${classification}-${tab}`, async ({ page }) => {
  const assigned = classification === 'unassigned' || classification === 'explicit';
  const projectId = classification === 'explicit' ? 'b' : null;
  const expectedKey = classification === 'auto-registered' ? 'id:a' : classification === 'auto-cwd' ? 'cwd:d:\\work' : classification === 'unassigned' ? 'unassigned' : 'id:b';
  await setup(page, { projects: classification === 'auto-cwd' ? [] : reconnectProjects, metadata: { projectId, projectAssigned: assigned } });
  if (tab === 'group') await page.locator(`#project-tabs [data-project-key="${expectedKey.replaceAll('\\', '\\\\')}"]`).click();
  await choose(page);
  await expect(page.locator('#codex-settings-error')).toBeHidden();
  await closeSettings(page);
  const next = page.locator('.terminal-card[data-id="new-1"]');
  await expect(next).toBeVisible();
  await expect(next.locator('.conversation-input')).toHaveValue('초안 보존 한글🙂');
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  const owner = await page.evaluate(() => (window as any).reconnectFixture.live.find((s: any) => s.active));
  expect(owner).toMatchObject({ projectId, projectAssigned: assigned, threadId: 'same-thread', cwd: 'D:\\Work', projectRevision: 0 });
  // The real rendered group exercises sessionProjectKey in source and bundled
  // tests alike; do not require a Vite-only source-module import here.
  const group = page.locator('#project-tabs [data-project-key]').filter({ has: page.locator('small') });
  const counts = await group.evaluateAll(tabs => tabs.map(tab => ({ key: (tab as HTMLElement).dataset.projectKey, count: tab.querySelector('small')?.textContent })));
  expect(counts).toContainEqual({ key: expectedKey, count: '1' });
  await expect(page.locator('#project-tabs [aria-selected="true"]')).toHaveAttribute('data-project-key', tab === 'all' ? '' : expectedKey);
  await expect.poll(async () => (await calls(page, 'save_layout')).at(-1)?.args.layout.sessions.find((s: any) => s.threadId === 'same-thread')).toMatchObject({ projectId, projectAssigned: assigned, cwd: 'D:\\Work' });
});
