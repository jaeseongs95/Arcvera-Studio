import { test, expect, type Page } from '@playwright/test';

async function setup(page: Page) {
  await page.addInitScript(() => {
    const state = window as any; state.isTauri = true; state.toolCalls = [];
    let callback = 0;
    state.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    state.__TAURI_INTERNALS__ = {
      transformCallback() { return ++callback; }, unregisterCallback() {},
      async invoke(command: string, args: any = {}) {
        state.toolCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'get_ui_language') return 'ko';
        if (command === 'load_layout') return { version: 1, columns: 2, sessions: [] };
        if (command === 'list_projects') return [];
        if (command === 'list_sessions') return Array.from({ length: 4 }, (_, index) => ({ id: `tools-${index}`, generation: 1, title: `세션 ${index} 긴 제목`, cwd: 'D:\\Work', threadId: `thread-${index}`, state: 'idle', active: true }));
        if (command === 'cron_request') return { supported: true, connected: true, scope: 'cliLifetime', jobs: [], hasMore: false };
        if (command === 'conversation_request') {
          if (args.request.type === 'contextStatus') return { connected: true, tokenUsageFresh: true, tokenUsage: { last: { totalTokens: 500 }, modelContextWindow: 1000 }, canCompact: true, passiveSubscriptionSupported: true, subagentsSupported: true, cronSupported: true, compactSupported: true };
          if (args.request.type === 'attach') return { threadId: `thread-${args.id.slice(-1)}`, model: 'fixture-model' };
          if (args.request.type === 'poll') return { events: [], pendingRequests: [], connected: true, gap: false };
          return { data: [], nextCursor: null };
        }
        return null;
      },
    };
  });
  await page.goto('/');
  await expect(page.locator('.terminal-card')).toHaveCount(4);
  await expect(page.locator('.terminal-card').first().locator('.context-compact')).toBeVisible();
}

async function assertBounds(page: Page) {
  const failures = await page.locator('.terminal-card:visible').evaluateAll(cards => cards.flatMap(card => {
    const outer = card.getBoundingClientRect();
    const buttons = [...card.querySelectorAll('button')].filter(button => button.getClientRects().length);
    const errors: string[] = [];
    for (const button of buttons) {
      const rect = button.getBoundingClientRect();
      if (rect.left < outer.left || rect.right > outer.right || rect.top < outer.top || rect.bottom > outer.bottom) errors.push(`outside:${button.className}`);
      for (const other of buttons) {
        if (other === button) continue;
        const target = other.getBoundingClientRect();
        if (rect.left < target.right - .5 && rect.right > target.left + .5 && rect.top < target.bottom - .5 && rect.bottom > target.top + .5) errors.push(`overlap:${button.className}:${other.className}`);
      }
    }
    if (card.querySelector('.terminal-host')!.getBoundingClientRect().height <= 0) errors.push('no terminal height');
    return errors;
  }));
  expect(failures).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(await page.evaluate(() => innerWidth));
}

test('ST01 management uses the real footer and retains session command binding', async ({ page }) => {
  await setup(page);
  const card = page.locator('.terminal-card').first();
  await expect(card.locator('.terminal-header button')).toHaveCount(2);
  await expect(card.locator('.terminal-header .maximize, .terminal-header .stop')).toHaveCount(2);
  for (const selector of ['.project-move', '.session-id-copy', '.context-usage', '.context-compact', '.subagents-open', '.cron-open', '.conversation-toggle', '.session-restart']) {
    await expect(card.locator(`.terminal-footer ${selector}`)).toBeVisible();
  }
  await card.locator('.cron-open').click();
  await expect(page.locator('#cron-dialog')).toBeVisible();
  expect(await page.evaluate(() => (window as any).toolCalls.filter((item: any) => item.command === 'cron_request').map((item: any) => [item.args.id, item.args.generation]))).toEqual([['tools-0', 1]]);
});

for (const [width, height] of [[800, 1000], [1440, 620]]) {
  test(`ST02 controls remain inside four narrow cards at ${width}x${height}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height }); await setup(page);
    await page.locator('[data-columns="4"]').click();
    await assertBounds(page);
    const handle = page.getByRole('separator', { name: '1번째 열 크기 조절' });
    await handle.focus(); await page.keyboard.press('ArrowLeft');
    await assertBounds(page);
    await page.screenshot({ path: testInfo.outputPath('session-tools.png') });
  });
}

test('ST03 keyboard follows header then footer and retains window selection', async ({ page }) => {
  await setup(page); const cards = page.locator('.terminal-card'); const card = cards.first();
  await card.locator('.stop').focus(); await page.keyboard.press('Tab');
  // xterm intentionally consumes Tab as CLI input. Check its entry separately,
  // then start at the footer to verify management controls' browser tab order.
  await expect(card.locator('.xterm-helper-textarea')).toBeFocused();
  await card.locator('.project-move').focus();
  await page.keyboard.press('Tab'); await expect(card.locator('.session-id-copy')).toBeFocused();
  await page.keyboard.press('Shift+Tab'); await expect(card.locator('.project-move')).toBeFocused();
  await card.locator('.cron-open').focus(); await page.keyboard.press('Enter');
  await expect(page.locator('#cron-dialog')).toBeVisible(); await page.keyboard.press('Escape');
  await card.locator('.maximize').focus(); await page.keyboard.press('Space');
  await expect(cards.nth(1)).toBeHidden(); await page.keyboard.press('Escape');
  await expect(cards.nth(1)).toBeVisible(); await page.keyboard.press('Alt+2');
  await expect(cards.nth(1)).toHaveClass(/focused/);
});

test('ST04 actual handle reorders cards and footer clicks do not drag', async ({ page }) => {
  await page.goto('/?demo=1'); const cards = page.locator('.terminal-card');
  await expect(cards).toHaveCount(4);
  const before = await cards.evaluateAll(elements => elements.map(element => (element as HTMLElement).dataset.id));
  await cards.first().locator('.drag-handle').dragTo(cards.nth(1).locator('.terminal-header'));
  await expect.poll(() => cards.evaluateAll(elements => elements.map(element => (element as HTMLElement).dataset.id))).toEqual([before[1], before[0], ...before.slice(2)]);
  await cards.first().locator('.project-move').click();
  await expect(page.locator('.terminal-card.dragging')).toHaveCount(0);
  expect(await cards.evaluateAll(elements => elements.map(element => (element as HTMLElement).dataset.id))).toEqual([before[1], before[0], ...before.slice(2)]);
});

test('ST05 permission button queries only the selected session generation', async ({ page }) => {
  await setup(page);
  await page.locator('.terminal-card').nth(2).locator('.terminal-footer .permission-status-open').click();
  await expect(page.getByRole('dialog', { name: '세션 권한 확인' })).toBeVisible();
  expect(await page.evaluate(() => (window as any).toolCalls.filter((item: any) => item.command === 'conversation_request' && item.args.request.type === 'permissionStatus').map((item: any) => item.args))).toEqual([{ id: 'tools-2', generation: 1, request: { type: 'permissionStatus' } }]);
});

test('ST06 GitHub shortcut uses settings and the fixed native browser opener', async ({ page }) => {
  await setup(page);
  await page.getByRole('button', { name: '설정 열기' }).click();
  await page.locator('#settings-dialog #github-open').click();
  expect(await page.evaluate(() => (window as any).toolCalls.filter((item: any) => item.command === 'open_external_url').map((item: any) => item.args))).toEqual([{ url: 'https://github.com/jaeseongs95' }]);
});
