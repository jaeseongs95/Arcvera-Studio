import { expect, test, type Page } from '@playwright/test';

const official = { conversation: true, manualCompact: true, tokenUsage: true, subagents: false, cron: false, remoteControl: false };
const extended = { ...official, subagents: true, cron: true, remoteControl: true };

async function fixture(page: Page, profile?: typeof official) {
  await page.addInitScript(profile => {
    const state = window as any; state.isTauri = true;
    const callbacks = new Map<number, any>(); const listeners = new Map<string, number>(); let callback = 0;
    const f = state.featureFixture = {
      calls: [] as any[],
      events: [] as any[],
      session: { id: 'feature-session', generation: 1, title: '기능별 세션', cwd: 'D:/Work', threadId: 'thread-feature', state: 'idle', active: true, ...(profile ? { runtimeFeatures: profile } : {}) },
      context: { revision: 1, connected: true, guiConnected: true, active: false, canCompact: true, compacting: false, tokenUsageFresh: true, tokenUsage: { last: { totalTokens: 1000 }, modelContextWindow: 10000 } },
      publish(profile: any, generation = this.session.generation) {
        this.session = { ...this.session, generation, runtimeFeatures: profile };
        callbacks.get(listeners.get('session-changed')!)?.({ payload: structuredClone(this.session) });
      },
    };
    state.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    state.__TAURI_INTERNALS__ = {
      transformCallback(handler: any) { callbacks.set(++callback, handler); return callback; }, unregisterCallback() {},
      async invoke(command: string, args: any = {}) {
        f.calls.push({ command, args });
        if (command === 'plugin:event|listen') { listeners.set(args.event, args.handler); return callback; }
        if (command.startsWith('plugin:event|')) return null;
        if (command === 'get_ui_language') return 'ko';
        if (command === 'load_layout') return { version: 1, columns: 1, sessions: [] };
        if (command === 'list_projects') return [];
        if (command === 'list_sessions') return [structuredClone(f.session)];
        if (command === 'list_pending_questions') return { items: [{ id: 'question', sessionId: f.session.id, threadId: f.session.threadId, generation: f.session.generation, requestId: 'question-request', kind: 'question', canRespond: true, questions: [{ id: 'answer', question: '계속할까요?' }] }] };
        if (command === 'conversation_request') {
          if (args.request.type === 'contextStatus') return structuredClone(f.context);
          if (args.request.type === 'attach') return { threadId: f.session.threadId, activeTurnKnown: true, model: 'runtime-model', reasoningEffort: 'high' };
          if (args.request.type === 'compact') { Object.assign(f.context, { revision: 2, compacting: true, canCompact: false }); return { status: 'started' }; }
          if (args.request.type === 'poll') return { events: f.events.splice(0), pendingRequests: [], connected: true, gap: false };
          if (args.request.type === 'send') { f.events.push({ seq: 1, method: 'turn/completed', params: { turn: { id: 'official-turn' } } }); return { turn: { id: 'official-turn' } }; }
          return { data: [], nextCursor: null };
        }
        if (command === 'cron_request') return { supported: true, connected: true, scope: 'cliLifetime', jobs: [] };
        if (command === 'subagents_request') return { supported: true, connected: true, parentRuntimeId: '11111111-1111-4111-8111-111111111111', agents: [] };
        if (command === 'mobile_control') return { isolated: true, shared: false, mutationsAllowed: true, status: { status: 'disabled', serverName: '', environmentId: null }, pairing: null };
        return null;
      },
    };
  }, profile);
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('aria-busy', 'false');
  await expect(page.locator('.terminal-card')).toBeVisible();
}

const featureCalls = (page: Page) => page.evaluate(() => (window as any).featureFixture.calls.filter((entry: any) => ['conversation_request', 'subagents_request', 'cron_request', 'mobile_control', 'list_pending_questions'].includes(entry.command)));

test('공통 정책은 관측된 참값만 허용하고 대화 없는 수동 압축은 거부한다', async ({ page }) => {
  await page.goto('/');
  const profiles = await page.evaluate(async () => {
    const { runtimeFeatures } = await import('/src/runtime-features.ts');
    return [runtimeFeatures(), runtimeFeatures({ runtimeFeatures: {} }), runtimeFeatures({ runtimeFeatures: { conversation: 'true', manualCompact: true, subagents: 1, cron: 'true' } as any }), runtimeFeatures({ runtimeFeatures: { tokenUsage: true, manualCompact: true } })];
  });
  for (const profile of profiles.slice(0, 3)) expect(Object.values(profile)).toEqual([false, false, false, false, false, false]);
  expect(profiles[3]).toMatchObject({ tokenUsage: true, conversation: false, manualCompact: false });
});

test('기능 미관측 세션은 버튼·폴링·직접 클릭을 차단하고 관측 변경을 반영한다', async ({ page }) => {
  await fixture(page);
  for (const selector of ['.conversation-toggle', '.context-usage', '.context-compact', '.subagents-open', '.cron-open', '#mobile-control-open', '#notifications']) await expect(page.locator(selector)).toBeHidden();
  await page.locator('.conversation-toggle, .context-usage, .context-compact, .subagents-open, .cron-open, #notifications').evaluateAll(buttons => buttons.forEach(button => (button as HTMLButtonElement).click()));
  await page.waitForTimeout(2200);
  expect(await featureCalls(page)).toEqual([]);
  await page.evaluate(profile => (window as any).featureFixture.publish(profile), official);
  await expect(page.locator('.context-usage')).toHaveText('10%');
  await expect(page.locator('.conversation-toggle')).toBeVisible();
  await expect(page.locator('#notifications')).toBeVisible();
  for (const selector of ['.subagents-open', '.cron-open', '#mobile-control-open']) await expect(page.locator(selector)).toBeHidden();
  await page.locator('.subagents-open, .cron-open').evaluateAll(buttons => buttons.forEach(button => (button as HTMLButtonElement).click()));
  expect((await featureCalls(page)).filter((entry: any) => ['subagents_request', 'cron_request', 'mobile_control'].includes(entry.command))).toEqual([]);
});

test('공통 기능 프로필로 대화·압축·질문 응답이 동작하고 새 세대는 다시 관측을 기다린다', async ({ page }) => {
  await fixture(page, official);
  await page.locator('.conversation-toggle').click();
  await page.locator('.conversation-input').fill('공식 실행 환경으로 보내기');
  await page.locator('.conversation-send').click();
  await expect.poll(async () => (await featureCalls(page)).filter((entry: any) => entry.command === 'conversation_request' && entry.args.request.type === 'send').length).toBe(1);
  await expect(page.locator('.conversation-stop')).toBeDisabled();
  await page.locator('.context-compact').click();
  await expect.poll(async () => (await featureCalls(page)).filter((entry: any) => entry.command === 'conversation_request' && entry.args.request.type === 'compact').length).toBe(1);
  await page.locator('#notifications').click();
  await page.getByRole('textbox', { name: '계속할까요?' }).fill('네');
  await page.getByRole('button', { name: '응답 보내기' }).click();
  await expect.poll(() => page.evaluate(() => (window as any).featureFixture.calls.filter((entry: any) => entry.command === 'respond_pending_question').length)).toBe(1);
  await page.getByRole('button', { name: '질문함 닫기' }).click();
  await page.evaluate(() => (window as any).featureFixture.publish(undefined, 2));
  await expect(page.locator('.conversation-toggle')).toBeHidden();
  await expect(page.locator('.context-usage')).toBeHidden();
  await expect(page.locator('#notifications')).toBeHidden();
  await page.waitForTimeout(100);
  const count = (await featureCalls(page)).length;
  await page.waitForTimeout(3200);
  expect((await featureCalls(page)).length).toBe(count);
  await page.evaluate(profile => (window as any).featureFixture.publish(profile, 1), extended);
  await expect(page.locator('.cron-open')).toBeHidden();
});

test('명시적 확장만 관리 버튼을 표시하고 열린 관리 창도 프로필 철회 뒤 호출을 멈춘다', async ({ page }) => {
  await fixture(page, extended);
  await page.locator('#settings-open').click();
  await expect(page.locator('#mobile-control-open')).toBeVisible();
  await page.locator('#mobile-control-open').click();
  await expect.poll(async () => (await featureCalls(page)).filter((entry: any) => entry.command === 'mobile_control').length).toBe(1);
  await page.evaluate(profile => (window as any).featureFixture.publish(profile), official);
  await expect(page.locator('#mobile-control-dialog')).toBeHidden();
  await expect(page.locator('#mobile-control-open')).toBeHidden();
  await page.locator('#mobile-control-enable').evaluate((button: HTMLButtonElement) => button.click());
  await page.locator('#settings-close').click();
  await page.evaluate(profile => (window as any).featureFixture.publish(profile), extended);
  await page.locator('.cron-open').click();
  await expect.poll(async () => (await featureCalls(page)).filter((entry: any) => entry.command === 'cron_request').length).toBe(1);
  await page.evaluate(profile => (window as any).featureFixture.publish(profile), official);
  await expect(page.locator('.cron-refresh')).toBeDisabled();
  await page.locator('.cron-refresh').evaluate((button: HTMLButtonElement) => button.click());
  await page.getByRole('button', { name: '예약 작업 창 닫기' }).click();
  await page.evaluate(profile => (window as any).featureFixture.publish(profile), extended);
  await page.locator('.subagents-open').click();
  await expect.poll(async () => (await featureCalls(page)).filter((entry: any) => entry.command === 'subagents_request').length).toBe(1);
  await page.evaluate(profile => (window as any).featureFixture.publish(profile), official);
  await page.locator('.subagent-refresh').evaluate((button: HTMLButtonElement) => button.click());
  await page.waitForTimeout(3200);
  const calls = await featureCalls(page);
  expect(calls.filter((entry: any) => entry.command === 'cron_request')).toHaveLength(1);
  expect(calls.filter((entry: any) => entry.command === 'subagents_request')).toHaveLength(1);
  expect(calls.filter((entry: any) => entry.command === 'mobile_control')).toHaveLength(1);
});
