import { expect, test, type Page } from '@playwright/test';

async function workspace(page: Page, options: { mode?: 'known' | 'empty' | 'unsupported' | 'unknown' | 'error'; count?: number; connected?: boolean; hold?: boolean } = {}) {
  await page.addInitScript(options => {
    const state = window as any; state.isTauri = true;
    let callback = 0;
    const callbacks = new Map<number, (event: any) => void>();
    const listeners = new Map<string, (event: any) => void>();
    const releases = new Map<string, () => void>();
    const fixture = state.permissionVisibility = {
      calls: [] as any[], targets: [] as string[], connected: options.connected ?? true, hold: options.hold ?? false,
      inFlight: 0, maxInFlight: 0,
      sessions: Array.from({ length: options.count ?? 1 }, (_, index) => ({ id: `permission-${index}`, generation: index + 3, title: `권한 세션 ${index}`, cwd: 'D:/permission-fixture', threadId: `thread-${index}`, state: 'idle', active: true })),
      visibility(visible: boolean) { listeners.get('application-visibility')?.({ payload: visible }); },
      change(index: number, fields: any) { Object.assign(fixture.sessions[index], fields); listeners.get('session-changed')?.({ payload: { ...fixture.sessions[index] } }); },
      release(id: string, generation: number) { releases.get(`${id}:${generation}`)?.(); },
    };
    const permissionReply = () => {
      if (options.mode === 'empty') return {};
      if (options.mode === 'unsupported') return { supported: false, status: 'unsupported', reason: 'methodUnsupported' };
      if (options.mode === 'unknown') return { supported: true, status: 'unknown', reason: 'invalidPermissionFields' };
      return { supported: true, status: 'known', permissions: { approvalPolicy: 'on-request', approvalsReviewer: 'user', sandboxPolicy: { type: 'readOnly', networkAccess: false }, activePermissionProfile: null }, pendingApprovals: { status: 'known', requests: [] } };
    };
    state.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    state.__TAURI_INTERNALS__ = {
      transformCallback(handler: (event: any) => void) { callbacks.set(++callback, handler); return callback; }, unregisterCallback() {},
      async invoke(command: string, args: any = {}) {
        fixture.calls.push({ command, args });
        if (command === 'plugin:event|listen') { listeners.set(args.event, callbacks.get(args.handler)!); return callback; }
        if (command.startsWith('plugin:event|')) return callback;
        if (command === 'get_ui_language') return 'ko';
        if (command === 'load_layout') return { version: 1, columns: 2, sessions: [] };
        if (command === 'list_projects') return [];
        if (command === 'list_sessions') return fixture.sessions;
        if (command === 'conversation_request') {
          if (args.request.type === 'contextStatus') return { threadId: fixture.sessions.find(session => session.id === args.id)?.threadId, connected: fixture.connected, guiConnected: false, tokenUsageFresh: false, canCompact: false, passiveSubscriptionSupported: false, subagentsSupported: false, cronSupported: false, compactSupported: false };
          if (args.request.type === 'permissionStatus') {
            fixture.targets.push(fixture.sessions.find(session => session.id === args.id)?.threadId ?? '');
            const response = permissionReply(); fixture.inFlight++; fixture.maxInFlight = Math.max(fixture.maxInFlight, fixture.inFlight);
            try {
              if (fixture.hold) await new Promise<void>(resolve => { releases.set(`${args.id}:${args.generation}`, resolve); });
              if (options.mode === 'error') throw Error('FIXTURE_CONNECTION_ERROR');
              return response;
            } finally { fixture.inFlight--; }
          }
          throw Error(`Unexpected conversation mutation: ${args.request.type}`);
        }
        return null;
      },
    };
  }, options);
  await page.goto('/');
  await expect(page.locator('.terminal-card')).toHaveCount(options.count ?? 1);
  await expect.poll(() => page.evaluate(() => (window as any).permissionVisibility.calls.filter((call: any) => call.command === 'conversation_request' && call.args.request.type === 'contextStatus').length)).toBeGreaterThan(0);
}

const permissionCalls = (page: Page) => page.evaluate(() => (window as any).permissionVisibility.calls.filter((call: any) => call.command === 'conversation_request' && call.args.request.type === 'permissionStatus').map((call: any) => call.args));

for (const mode of ['empty', 'unsupported', 'unknown', 'error'] as const) {
  test(`PV01 actual main hides unconfirmed permission control: ${mode}`, async ({ page }) => {
    await workspace(page, { mode });
    const card = page.locator('.terminal-card');
    await expect(card.locator('.permission-status-open')).toBeHidden();
    await expect(card.locator('.maximize')).toBeVisible(); await expect(card.locator('.maximize')).toBeEnabled();
    await expect(card.locator('.stop')).toBeVisible(); await expect(card.locator('.stop')).toBeEnabled();
    await expect(card.locator('.session-id-copy')).toBeVisible(); await expect(card.locator('.session-id-copy')).toBeEnabled();
    await expect.poll(() => permissionCalls(page)).toHaveLength(1);
    await card.locator('.maximize').click(); await page.keyboard.press('Escape');
    expect(await permissionCalls(page)).toHaveLength(1);
  });
}

test('PV02 authoritative support exposes the actual selected id and generation', async ({ page }) => {
  await workspace(page, { count: 2 });
  const card = page.locator('.terminal-card').nth(1);
  await expect(card.locator('.permission-status-open')).toBeVisible();
  await expect.poll(() => permissionCalls(page)).toHaveLength(2);
  await card.locator('.permission-status-open').click();
  await expect(page.getByRole('dialog', { name: '세션 권한 확인' })).toContainText('요청 시 승인');
  const calls = await permissionCalls(page);
  expect(calls.map(call => [call.id, call.generation])).toEqual([['permission-0', 3], ['permission-1', 4], ['permission-1', 4]]);
});

test('PV03 existing connection readiness gates one discovery without polling retries', async ({ page }) => {
  await workspace(page, { connected: false });
  await expect(page.locator('.permission-status-open')).toBeHidden();
  expect(await permissionCalls(page)).toEqual([]);
  await page.evaluate(() => { const s = (window as any).permissionVisibility; s.connected = true; s.visibility(false); s.visibility(true); });
  await expect(page.locator('.permission-status-open')).toBeVisible();
  await expect.poll(() => permissionCalls(page)).toHaveLength(1);
  const previous = await page.evaluate(() => (window as any).permissionVisibility.calls.filter((call: any) => call.args?.request?.type === 'contextStatus').length);
  await page.evaluate(() => { const s = (window as any).permissionVisibility; s.visibility(false); s.visibility(true); });
  await expect.poll(() => page.evaluate(() => (window as any).permissionVisibility.calls.filter((call: any) => call.args?.request?.type === 'contextStatus').length)).toBeGreaterThan(previous);
  await page.locator('[data-columns="1"]').click(); await page.locator('[data-columns="2"]').click();
  expect(await permissionCalls(page)).toHaveLength(1);
});

test('PV04 global single discovery pauses hidden waiting work and preserves same-generation replies', async ({ page }) => {
  await workspace(page, { count: 3, hold: true });
  await expect.poll(() => permissionCalls(page)).toHaveLength(1);
  expect(await page.evaluate(() => (window as any).permissionVisibility.maxInFlight)).toBe(1);
  await page.evaluate(() => { const s = (window as any).permissionVisibility; s.visibility(false); s.release('permission-0', 3); });
  await expect.poll(() => page.evaluate(() => (window as any).permissionVisibility.inFlight)).toBe(0);
  expect(await permissionCalls(page)).toHaveLength(1);
  await expect(page.locator('.permission-status-open:visible')).toHaveCount(0);
  await page.evaluate(() => { const s = (window as any).permissionVisibility; s.change(2, { active: false, state: 'exited' }); s.visibility(true); });
  await page.locator('.terminal-card').nth(2).locator('.stop').click();
  await expect(page.locator('.terminal-card')).toHaveCount(2);
  await expect.poll(() => permissionCalls(page)).toHaveLength(2);
  await page.evaluate(() => (window as any).permissionVisibility.release('permission-1', 4));
  await expect(page.locator('.terminal-card').nth(1).locator('.permission-status-open')).toBeVisible();
  await expect(page.locator('.terminal-card').first().locator('.permission-status-open')).toBeVisible();
  expect(await permissionCalls(page)).toHaveLength(2);
  expect(await page.evaluate(() => (window as any).permissionVisibility.maxInFlight)).toBe(1);
});

test('PV05 late prior generation cannot expose a replacement before its own confirmation', async ({ page }) => {
  await workspace(page, { hold: true });
  await expect.poll(() => permissionCalls(page)).toHaveLength(1);
  await page.evaluate(() => { const s = (window as any).permissionVisibility; s.change(0, { generation: 8, title: '새 세대' }); s.release('permission-0', 3); });
  await expect.poll(() => permissionCalls(page)).toHaveLength(2);
  await expect(page.locator('.permission-status-open')).toBeHidden();
  await page.evaluate(() => (window as any).permissionVisibility.release('permission-0', 8));
  await expect(page.locator('.permission-status-open')).toBeVisible();
  expect((await permissionCalls(page)).map(call => [call.id, call.generation])).toEqual([['permission-0', 3], ['permission-0', 8]]);
});

test('PV06 a removed card cannot be revived by a late discovery reply', async ({ page }) => {
  await workspace(page, { hold: true });
  await expect.poll(() => permissionCalls(page)).toHaveLength(1);
  await page.evaluate(() => (window as any).permissionVisibility.change(0, { active: false, state: 'exited' }));
  await page.locator('.terminal-card .stop').click();
  await expect(page.locator('.terminal-card')).toHaveCount(0);
  await page.evaluate(() => (window as any).permissionVisibility.release('permission-0', 3));
  await expect.poll(() => page.evaluate(() => (window as any).permissionVisibility.inFlight)).toBe(0);
  await expect(page.locator('.permission-status-open, .permission-status-dialog')).toHaveCount(0);
});

test('PV07 same-generation thread replacement discards old support and waits for its own reply', async ({ page }) => {
  await workspace(page, { hold: true });
  await expect.poll(() => permissionCalls(page)).toHaveLength(1);
  await page.evaluate(() => { const s = (window as any).permissionVisibility; s.change(0, { threadId: 'replacement-thread', title: '바뀐 대화' }); s.visibility(false); s.visibility(true); s.release('permission-0', 3); });
  await expect.poll(() => permissionCalls(page)).toHaveLength(2);
  await expect(page.locator('.permission-status-open')).toBeHidden();
  await page.evaluate(() => (window as any).permissionVisibility.release('permission-0', 3));
  await expect(page.locator('.permission-status-open')).toBeVisible();
  await expect(page.locator('.terminal-title')).toHaveText('바뀐 대화');
  expect((await permissionCalls(page)).map(call => [call.id, call.generation])).toEqual([['permission-0', 3], ['permission-0', 3]]);
  expect(await page.evaluate(() => (window as any).permissionVisibility.targets)).toEqual(['thread-0', 'replacement-thread']);
});
