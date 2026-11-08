import { expect, test, type Page } from '@playwright/test';

// Source-module host fixture: verifies UI/IPC behavior, not actual CLI persistence.
async function fixture(page: Page) {
  await page.goto('/');
  await page.evaluate(async () => {
    const state = window as any;
    const { createExtensionsManager } = await import('/src/extensions.ts');
    // The app already owns a closed manager. Remove that exact fixture surface
    // before mounting this isolated host; duplicate IDs must still fail loudly.
    const appDialogs = document.querySelectorAll('#extensions-dialog');
    if (appDialogs.length !== 1 || !(appDialogs[0] instanceof HTMLDialogElement) || appDialogs[0].open) throw Error('Expected exactly one closed app extension dialog');
    appDialogs[0].remove();
    const f = state.mcpFixture = {
      mutations: [] as any[], failure: '', hold: false, release: () => {}, unavailable: false,
      items: [{ name: 'kept', transport: 'stdio', enabled: true, configuredIn: ['user'] }],
      snapshot() {
        return {
          executable: 'D:\\fixture\\codex.exe', cwd: 'D:\\fixture-project',
          plugins: { installed: [], available: [] }, marketplaces: [], skills: [], hooks: { items: [], warnings: [], errors: [] },
          skillDestinations: { user: 'D:\\fixture\\skills', project: null },
          mcpServers: this.unavailable ? null : { items: structuredClone(this.items), userFile: 'D:\\fixture\\config.toml' },
          errors: this.unavailable ? [{ section: 'mcpServers', message: 'MCP config/read: selected CLI unsupported' }] : [],
        };
      },
    };
    state.mcpManager = createExtensionsManager({
      async call(command: string, args: any) {
        if (command === 'extensions_list') return f.snapshot();
        if (command === 'extensions_mutate') {
          f.mutations.push(structuredClone(args));
          if (f.hold) await new Promise<void>(resolve => { f.release = resolve; });
          if (f.failure) throw Error(f.failure);
          f.items.push({ name: args.request.name, transport: args.request.transport.type, enabled: true, configuredIn: ['user'] });
          return f.snapshot();
        }
        throw Error('Unexpected fixture command');
      },
      notify() {},
    });
    state.mcpManager.open('D:\\fixture-project');
  });
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
