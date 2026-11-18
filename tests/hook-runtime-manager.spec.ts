import { expect, test, type Page } from '@playwright/test';

// Parent lifetime/binding source fixture; it does not execute Native or Core.
const selected = { id: 'owned-session', generation: 7, threadId: 'owned-thread', title: '현재 대화' };
async function fixture(page: Page) {
  await page.route('**/hook-manager-harness', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body></body></html>' }));
  await page.goto('/hook-manager-harness');
  await page.evaluate(async selected => {
    const source = '/src/extensions.ts'; const language = '/src/i18n.ts'; const styles = '/src/style.css';
    await import(styles);
    const { createExtensionsManager } = await import(source);
    const { setLanguage } = await import(language); setLanguage('ko');
    const f = (window as any).hookManagerFixture = { calls: [] as any[], hold: false, pending: [] as any[], exitDone: false, setLanguage,
      result(threadId: string) { return { operation: 'hooksStatus', threadId, provenance: 'loadedThreadHooksList', executionGate: 'unknown', applicationState: 'unknown', hooks: { items: [{ id: 'hook-' + 'a'.repeat(64), eventName: 'preToolUse', handlerType: 'command', source: 'user', enabled: true, isManaged: false, trustStatus: 'untrusted' }], warningCount: 0, errorCount: 0 } }; },
      snapshot() { return { executable: 'D:\\fixture\\arcvera.exe', cwd: 'D:\\fixture', skillDestinations: { user: 'D:\\fixture-home\\skills', project: null }, plugins: { installed: [], available: [] }, skills: [], hooks: { items: [], warnings: [], errors: [] }, marketplaces: [], mcpServers: { items: [], userFile: 'D:\\fixture-home\\config.toml', configVersion: 'v1' }, errors: [] }; },
    };
    f.manager = createExtensionsManager({ call: async (command: string, args: any) => {
      f.calls.push({ command, args });
      if (command === 'extensions_list') return f.snapshot();
      if (command === 'conversation_request') { if (f.hold) return new Promise(resolve => f.pending.push(resolve)); return f.result(args.request.expectedThreadId); }
      throw Error('Unexpected fixture command');
    }, notify() {} });
    f.manager.open('D:\\fixture', selected);
  }, selected);
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await page.locator('#extensions-tab-hooks').click();
}
const inspect = (page: Page) => page.locator('#extensions-hook-runtime button').first();

test('HP01 empty configured list still exposes the correctly bound current-thread read', async ({ page }) => {
  await fixture(page); await inspect(page).click();
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  expect(await page.evaluate(() => (window as any).hookManagerFixture.calls.filter((entry: any) => entry.command === 'conversation_request'))).toEqual([
    { command: 'conversation_request', args: { id: selected.id, generation: selected.generation, request: { type: 'hooksStatus', expectedThreadId: selected.threadId } } },
  ]);
  await expect(page.locator('#extensions-hook-runtime')).toContainText('hook-' + 'a'.repeat(64));
  await expect(page.locator('#extensions-hook-runtime')).not.toContainText('사용 중');
  await page.screenshot({ path: test.info().outputPath('hook-panel.png'), fullPage: true });
});

test('HP02 language and metadata refresh preserve a same-target observation', async ({ page }) => {
  await fixture(page); await inspect(page).click();
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await page.evaluate(() => (window as any).hookManagerFixture.setLanguage('en'));
  await expect(page.locator('#extensions-hook-runtime')).toContainText('hook-' + 'a'.repeat(64));
  await page.locator('#extensions-refresh').click();
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await expect(page.locator('#extensions-hook-runtime')).toContainText('hook-' + 'a'.repeat(64));
  expect(await page.evaluate(() => (window as any).hookManagerFixture.calls.filter((entry: any) => entry.command === 'conversation_request').length)).toBe(1);
});

test('HP03 closing and reopening cannot attach an old reply to a different thread', async ({ page }) => {
  await fixture(page); await page.evaluate(() => { (window as any).hookManagerFixture.hold = true; });
  await inspect(page).click(); await page.locator('#extensions-close').click();
  await page.evaluate(() => { const f = (window as any).hookManagerFixture; f.manager.open('D:\\fixture', { id: 'next', generation: 8, threadId: 'next-thread', title: '다음 대화' }); });
  await page.evaluate(() => { const f = (window as any).hookManagerFixture; f.pending.shift()(f.result('owned-thread')); });
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  await page.locator('#extensions-tab-hooks').click();
  await expect(page.locator('#extensions-hook-runtime')).toContainText('다음 대화');
  await expect(page.locator('#extensions-hook-runtime')).not.toContainText('hook-' + 'a'.repeat(64));
});

test('HP04 exit waits for an accepted runtime read and does not submit another request', async ({ page }) => {
  await fixture(page); await page.evaluate(() => { (window as any).hookManagerFixture.hold = true; });
  await inspect(page).click();
  await page.evaluate(() => { const f = (window as any).hookManagerFixture; f.manager.beginExit().then(() => { f.exitDone = true; }); });
  expect(await page.evaluate(() => (window as any).hookManagerFixture.exitDone)).toBe(false);
  await page.evaluate(() => { const f = (window as any).hookManagerFixture; f.pending.shift()(f.result('owned-thread')); });
  await expect.poll(() => page.evaluate(() => (window as any).hookManagerFixture.exitDone)).toBe(true);
  expect(await page.evaluate(() => (window as any).hookManagerFixture.calls.filter((entry: any) => entry.command === 'conversation_request').length)).toBe(1);
});

test('HP05 the minimum desktop viewport wraps readonly identifiers without horizontal overflow', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 540 });
  await fixture(page); await inspect(page).click();
  await expect(page.locator('#extensions-dialog')).toHaveAttribute('aria-busy', 'false');
  const width = await page.locator('#extensions-dialog').evaluate(element => ({ scroll: element.scrollWidth, client: element.clientWidth }));
  expect(width.scroll).toBeLessThanOrEqual(width.client + 1);
  await expect(inspect(page)).toBeEnabled();
  await page.screenshot({ path: test.info().outputPath('hook-panel-minimum.png'), fullPage: true });
});
