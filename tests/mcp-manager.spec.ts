import { expect, test, type Page } from '@playwright/test';

// Source-module host fixture: verifies UI/IPC behavior, not actual CLI persistence.
async function fixture(page: Page, withSession = false) {
  await page.goto('/');
  await page.evaluate(async withSession => {
    const state = window as any;
    const { createExtensionsManager } = await import('/src/extensions.ts');
    // The app already owns a closed manager. Remove that exact fixture surface
    // before mounting this isolated host; duplicate IDs must still fail loudly.
    const appDialogs = document.querySelectorAll('#extensions-dialog');
    if (appDialogs.length !== 1 || !(appDialogs[0] instanceof HTMLDialogElement) || appDialogs[0].open) throw Error('Expected exactly one closed app extension dialog');
    appDialogs[0].remove();
    const f = state.mcpFixture = {
      mutations: [] as any[], failure: '', hold: false, release: () => {}, unavailable: false,
      configVersion: 'config:v1',
      runtimeCalls: [] as any[], runtimeReply: null as any, holdRuntime: false, releaseRuntime: () => {},
      target: { id: 'native-session', generation: 7, threadId: 'thread-original', title: '검증 대화' },
      items: [{ name: 'kept', transport: 'stdio', enabled: true, configuredIn: ['user'], editable: true }],
      snapshot() {
        return {
          executable: 'D:\\fixture\\codex.exe', cwd: 'D:\\fixture-project',
          plugins: { installed: [], available: [] }, marketplaces: [], skills: [], hooks: { items: [], warnings: [], errors: [] },
          skillDestinations: { user: 'D:\\fixture\\skills', project: null },
          mcpServers: this.unavailable ? null : { items: structuredClone(this.items), userFile: 'D:\\fixture\\config.toml', configVersion: this.configVersion },
          errors: this.unavailable ? [{ section: 'mcpServers', message: 'MCP config/read: selected CLI unsupported' }] : [],
        };
      },
    };
    state.mcpManager = createExtensionsManager({
      async call(command: string, args: any) {
        if (command === 'extensions_list') return f.snapshot();
        if (command === 'conversation_request') {
          f.runtimeCalls.push(structuredClone(args));
          const reply = structuredClone(f.runtimeReply ?? (args.request.type === 'reloadMcp'
            ? { operation: 'reloadMcp', threadId: args.request.expectedThreadId, outcome: 'queued' }
            : { operation: 'mcpStatus', threadId: args.request.expectedThreadId, serverName: args.request.serverName, found: true, runtimeStatus: 'connected', authStatus: 'unknown', toolsState: 'available', toolNames: ['fixture_tool_v2'], nextCursor: null }));
          if (f.holdRuntime) await new Promise<void>(resolve => { f.releaseRuntime = resolve; });
          return reply;
        }
        if (command === 'extensions_mutate') {
          f.mutations.push(structuredClone(args));
          if (f.hold) await new Promise<void>(resolve => { f.release = resolve; });
          if (f.failure) throw Error(f.failure);
          if (args.request.operation === 'addMcpServer') f.items.push({ name: args.request.name, transport: args.request.transport.type, enabled: true, configuredIn: ['user'], editable: true });
          else if (args.request.operation === 'editMcpServer') {
            const item = f.items.find(item => item.name === args.request.name);
            if (!item || args.request.expectedVersion !== f.configVersion) throw Error('Fixture stale configuration');
            if (args.request.patch.transport) item.transport = args.request.patch.transport.type;
            if (args.request.patch.enabled !== undefined) item.enabled = args.request.patch.enabled;
          } else if (args.request.operation === 'removeMcpServer') {
            if (args.request.expectedVersion !== f.configVersion) throw Error('Fixture stale configuration');
            f.items = f.items.filter(item => item.name !== args.request.name);
          } else throw Error('Unexpected fixture mutation');
          f.configVersion += ':next';
          return f.snapshot();
        }
        throw Error('Unexpected fixture command');
      },
      notify() {},
    });
    state.mcpManager.open('D:\\fixture-project', withSession ? f.target : undefined);
  }, withSession);
  await page.locator('#extensions-tab-mcpServers').click();
  await expect(page.locator('#extensions-mcp-add')).toBeEnabled();
}

test('M01/M10 STDIO args remain literal; review excludes secrets and save clears input', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-name').fill('server.name');
  await page.locator('#extensions-mcp-command').fill('fixture-command');
  await page.locator('#extensions-mcp-args').fill('["literal argument", "quote\\\"arg", ""]');
  await page.locator('#extensions-mcp-env').fill('{"FIXTURE_TOKEN":"SENTINEL"}');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-review')).toContainText('server.name · stdio');
  await expect(page.locator('#extensions-review')).not.toContainText('SENTINEL');
  await expect(page.locator('#extensions-review')).not.toContainText('fixture-command');
  await page.locator('#extensions-review-confirm').click();
  await expect(page.locator('#extensions-list')).toContainText('server.name');
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations[0])).toEqual({
    cwd: 'D:\\fixture-project', request: { operation: 'addMcpServer', name: 'server.name', transport: { type: 'stdio', command: 'fixture-command', args: ['literal argument', 'quote"arg', ''], env: { FIXTURE_TOKEN: 'SENTINEL' } } },
  });
  await expect(page.locator('#extensions-mcp-env')).toHaveValue('');
  await expect(page.locator('#extensions-mcp-command')).toHaveValue('');
  await expect(page.locator('#extensions-detail')).toContainText('새 세션');
  await expect(page.locator('#extensions-detail')).not.toContainText('SENTINEL');
});

test('M02 HTTP stores URL and bearer environment name without reflecting URL in review', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-name').fill('http-server');
  await page.locator('#extensions-mcp-transport').selectOption('streamableHttp');
  await expect(page.locator('#extensions-mcp-stdio')).toBeHidden();
  await page.locator('#extensions-mcp-url').fill('https://fixture.invalid/mcp?token=SENTINEL');
  await page.locator('#extensions-mcp-bearer').fill('FIXTURE_TOKEN');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-review')).not.toContainText('SENTINEL');
  await page.locator('#extensions-review-confirm').click();
  await expect(page.locator('#extensions-list')).toContainText('http-server');
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations[0].request.transport)).toEqual({ type: 'streamableHttp', url: 'https://fixture.invalid/mcp?token=SENTINEL', bearerTokenEnvVar: 'FIXTURE_TOKEN' });
  await expect(page.locator('#extensions-mcp-url')).toHaveValue('');
});

test('M03/M09 duplicate names and malformed JSON never submit a mutation', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-name').fill('kept');
  await page.locator('#extensions-mcp-command').fill('fixture-command');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-error')).toContainText('같은 이름');
  await page.locator('#extensions-mcp-name').fill('fresh');
  await page.locator('#extensions-mcp-env').fill('{"bad":"SENTINEL"');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-error')).toContainText('입력 형식');
  await expect(page.locator('#extensions-error')).not.toContainText('SENTINEL');
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations.length)).toBe(0);
});

test('M10/M11 pending mutation drains at exit, does not resubmit, and close erases inputs', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => { (window as any).mcpFixture.hold = true; });
  await page.locator('#extensions-mcp-name').fill('pending-server');
  await page.locator('#extensions-mcp-command').fill('fixture-command');
  await page.locator('#extensions-mcp-env').fill('{"FIXTURE_TOKEN":"SENTINEL"}');
  await page.locator('#extensions-mcp-add').click();
  await page.locator('#extensions-review-confirm').click();
  await expect(page.locator('#extensions-mcp-add')).toBeDisabled();
  await page.evaluate(() => {
    const state = window as any; state.exitFinished = false;
    state.mcpManager.beginExit().then(() => { state.exitFinished = true; });
  });
  expect(await page.evaluate(() => (window as any).exitFinished)).toBe(false);
  await page.locator('#extensions-close').click();
  await expect(page.locator('#extensions-mcp-env')).toHaveValue('');
  await page.evaluate(() => { (window as any).mcpFixture.release(); });
  await expect.poll(() => page.evaluate(() => (window as any).exitFinished)).toBe(true);
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations.length)).toBe(1);
});

test('M11 MCP read failure disables addition and reports its section independently', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => { (window as any).mcpFixture.unavailable = true; });
  await page.locator('#extensions-refresh').click();
  await expect(page.locator('#extensions-mcp-add')).toBeDisabled();
  await expect(page.locator('#extensions-error')).toContainText('selected CLI unsupported');
  await expect(page.locator('#extensions-mcp-scope')).toContainText('확인하지 못했습니다');
  await page.locator('#extensions-tab-plugins').click();
  await expect(page.locator('#extensions-error')).toBeHidden();
});

test('M210 edit preserves unspecified protected fields and binds the reviewed version', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-edit').click();
  await expect(page.locator('#extensions-mcp-name')).toHaveValue('kept');
  await expect(page.locator('#extensions-mcp-name')).toHaveAttribute('readonly', '');
  await expect(page.locator('#extensions-mcp-command')).toHaveValue('');
  await expect(page.locator('#extensions-mcp-args')).toHaveValue('');
  await page.locator('#extensions-mcp-args').fill('["changed argument", ""]');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-review')).toContainText('kept');
  await expect(page.locator('#extensions-review')).not.toContainText('changed argument');
  await page.locator('#extensions-review-confirm').click();
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations[0].request)).toEqual({
    operation: 'editMcpServer', name: 'kept', expectedVersion: 'config:v1',
    patch: { transport: { type: 'stdio', args: ['changed argument', ''] } },
  });
  await expect(page.locator('#extensions-mcp-title')).toHaveText('MCP 서버 추가');
});

test('M210 transport switch explains removed settings without exposing replacement values', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-edit').click();
  await page.locator('#extensions-mcp-transport').selectOption('streamableHttp');
  await page.locator('#extensions-mcp-url').fill('https://fixture.invalid/mcp?token=SENTINEL');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-review')).toContainText('STDIO');
  await expect(page.locator('#extensions-review')).toContainText('삭제');
  await expect(page.locator('#extensions-review')).not.toContainText('SENTINEL');
  await page.locator('#extensions-review-confirm').click();
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations[0].request.patch)).toEqual({
    transport: { type: 'streamableHttp', url: 'https://fixture.invalid/mcp?token=SENTINEL' },
  });
});

test('M210 removal is explicit, cancelable, scoped to one reviewed name and version', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-remove').click();
  await expect(page.locator('#extensions-review')).toContainText('kept');
  await page.locator('#extensions-review-cancel').click();
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations.length)).toBe(0);
  await page.locator('#extensions-mcp-remove').click();
  await page.locator('#extensions-review-confirm').click();
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations[0].request)).toEqual({ operation: 'removeMcpServer', name: 'kept', expectedVersion: 'config:v1' });
  await expect(page.locator('#extensions-list')).not.toContainText('kept');
});

test('M210 stale edits and non-editable configurations cannot submit changes', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-edit').click();
  await page.locator('#extensions-mcp-args').fill('[]');
  await page.evaluate(() => { (window as any).mcpFixture.configVersion = 'config:v2'; });
  await page.locator('#extensions-refresh').click();
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await page.locator('#extensions-mcp-add').click();
  await expect(page.locator('#extensions-error')).toContainText('구성이 변경');
  await expect(page.locator('#extensions-mcp-args')).toHaveValue('[]');
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations.length)).toBe(0);
  await page.locator('#extensions-mcp-cancel-edit').click();
  await page.evaluate(() => { (window as any).mcpFixture.items[0].editable = false; });
  await page.locator('#extensions-refresh').click();
  await expect(page.locator('#extensions-mcp-edit')).toHaveCount(0);
  await expect(page.locator('#extensions-mcp-remove')).toHaveCount(0);
});

test('M210 pending edit drains once and closing clears replacement secrets', async ({ page }) => {
  await fixture(page);
  await page.locator('#extensions-mcp-edit').click();
  await page.locator('#extensions-mcp-env').fill('{"TOKEN":"SENTINEL"}');
  await page.locator('#extensions-mcp-add').click();
  await page.evaluate(() => { (window as any).mcpFixture.hold = true; });
  await page.locator('#extensions-review-confirm').click();
  await expect(page.locator('#extensions-mcp-add')).toBeDisabled();
  await page.evaluate(() => {
    const state = window as any; state.exitFinished = false;
    state.mcpManager.beginExit().then(() => { state.exitFinished = true; });
  });
  await page.locator('#extensions-close').click();
  await expect(page.locator('#extensions-mcp-env')).toHaveValue('');
  await page.evaluate(() => { (window as any).mcpFixture.release(); });
  await expect.poll(() => page.evaluate(() => (window as any).exitFinished)).toBe(true);
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations.length)).toBe(1);
});

test('M210 runtime reload reviews one native session and acceptance is not readiness', async ({ page }) => {
  await fixture(page, true);
  const panel = page.locator('#extensions-mcp-runtime');
  await panel.getByRole('button', { name: /다시\s*연결|재연결/ }).click();
  await expect(page.locator('#extensions-review')).toContainText('검증 대화');
  await expect(page.locator('#extensions-review')).toContainText('접수는 연결·도구 준비 완료가 아니');
  await page.locator('#extensions-review-confirm').click();
  await expect(panel).toContainText('접수');
  await expect(panel).not.toContainText('연결됨');
  expect(await page.evaluate(() => (window as any).mcpFixture.runtimeCalls)).toEqual([{ id: 'native-session', generation: 7, request: { type: 'reloadMcp', expectedThreadId: 'thread-original' } }]);
  expect(await page.evaluate(() => (window as any).mcpFixture.mutations.length)).toBe(0);
});

test('M210 runtime discovery is scoped to the selected native session and server', async ({ page }, testInfo) => {
  await fixture(page, true);
  const panel = page.locator('#extensions-mcp-runtime');
  await panel.getByRole('button', { name: /상태.*조회/ }).click();
  await expect(panel).toContainText('fixture_tool_v2');
  await page.locator('#extensions-refresh').click();
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await expect(panel).toContainText('fixture_tool_v2');
  await page.evaluate(async () => { const { setLanguage } = await import('/src/i18n.ts'); setLanguage('en'); });
  await expect(panel).toContainText('fixture_tool_v2');
  await expect(panel).toContainText('Connected');
  expect(await page.evaluate(() => (window as any).mcpFixture.runtimeCalls.length)).toBe(1);
  expect(await page.evaluate(() => (window as any).mcpFixture.runtimeCalls[0])).toEqual({ id: 'native-session', generation: 7, request: { type: 'mcpStatus', expectedThreadId: 'thread-original', serverName: 'kept' } });
  await page.screenshot({ path: testInfo.outputPath('runtime-panel.png'), fullPage: true });
});

test('M210 late runtime reply cannot replace another conversation after reopening', async ({ page }) => {
  await fixture(page, true);
  await page.evaluate(() => {
    const f = (window as any).mcpFixture; f.holdRuntime = true;
    f.runtimeReply = { operation: 'mcpStatus', threadId: 'thread-original', serverName: 'kept', found: true, runtimeStatus: 'connected', authStatus: 'unknown', toolsState: 'available', toolNames: ['OLD_THREAD_TOOL'], nextCursor: null };
  });
  await page.locator('#extensions-mcp-runtime').getByRole('button', { name: /상태.*조회/ }).click();
  await expect.poll(() => page.evaluate(() => (window as any).mcpFixture.runtimeCalls.length)).toBe(1);
  await page.locator('#extensions-close').click();
  await page.evaluate(() => {
    const state = window as any; state.mcpFixture.runtimeReply = null;
    state.mcpManager.open('D:\\fixture-project', { id: 'session-b', generation: 8, threadId: 'thread-b', title: '다른 대화' });
    state.mcpFixture.holdRuntime = false; state.mcpFixture.releaseRuntime();
  });
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await expect(page.locator('#extensions-session')).toHaveText('다른 대화');
  await expect(page.locator('#extensions-mcp-runtime')).not.toContainText('OLD_THREAD_TOOL');
  await page.locator('#extensions-mcp-runtime').getByRole('button', { name: /상태.*조회/ }).click();
  await expect(page.locator('#extensions-mcp-runtime')).toContainText('fixture_tool_v2');
  expect(await page.evaluate(() => (window as any).mcpFixture.runtimeCalls[1].request.expectedThreadId)).toBe('thread-b');
});

test('M210 exit drains an accepted runtime request exactly once', async ({ page }) => {
  await fixture(page, true);
  await page.evaluate(() => { (window as any).mcpFixture.holdRuntime = true; });
  await page.locator('#extensions-mcp-runtime').getByRole('button', { name: /상태.*조회/ }).click();
  await page.evaluate(() => {
    const state = window as any; state.runtimeExitFinished = false;
    state.mcpManager.beginExit().then(() => { state.runtimeExitFinished = true; });
  });
  expect(await page.evaluate(() => (window as any).runtimeExitFinished)).toBe(false);
  await page.locator('#extensions-close').click();
  await page.evaluate(() => { (window as any).mcpFixture.releaseRuntime(); });
  await expect.poll(() => page.evaluate(() => (window as any).runtimeExitFinished)).toBe(true);
  expect(await page.evaluate(() => (window as any).mcpFixture.runtimeCalls.length)).toBe(1);
});
