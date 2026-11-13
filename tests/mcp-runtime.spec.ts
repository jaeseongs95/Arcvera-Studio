import { expect, test, type Page } from '@playwright/test';

// Isolated source fixture: no Core, CLI, native command or real MCP connection.
const target = { id: 'session-a', generation: 3, threadId: 'thread-a', title: '선택한 대화' };
const status = {
  operation: 'mcpStatus', threadId: target.threadId, serverName: 'sample', found: true,
  runtimeStatus: 'connected', authStatus: 'oAuth', toolsState: 'available',
  toolNames: ['lookup'], nextCursor: null,
};
const invalidMessage = '현재 대화 MCP 결과를 확인할 수 없습니다. 상태를 다시 조회해 주세요.';

async function fixture(page: Page, selected: unknown = target) {
  await page.route('**/mcp-runtime-harness', route => route.fulfill({
    contentType: 'text/html', body: '<!doctype html><html><body></body></html>',
  }));
  await page.goto('/mcp-runtime-harness');
  await page.evaluate(async selected => {
    const modulePath = '/src/mcp-runtime.ts';
    const languagePath = '/src/i18n.ts';
    const { createMcpRuntimePanel } = await import(modulePath);
    const { setLanguage } = await import(languagePath);
    setLanguage('ko');
    const f = (window as any).mcpRuntimeFixture = { calls: [] as unknown[], setLanguage, target: selected };
    f.panel = createMcpRuntimePanel({ target: selected ?? undefined, serverName: 'sample',
      onReload: () => f.calls.push({ operation: 'reload' }),
      onInspect: (cursor?: string) => f.calls.push({ operation: 'inspect', cursor: cursor ?? null }),
    });
    document.body.append(f.panel.element);
  }, selected);
}
async function render(page: Page, value: unknown): Promise<boolean> {
  return page.evaluate(value => (window as any).mcpRuntimeFixture.panel.render(value), value);
}
async function calls(page: Page) {
  return page.evaluate(() => (window as any).mcpRuntimeFixture.calls);
}

test('대상이 없거나 잘못되면 안내만 표시한다', async ({ page }) => {
  for (const selected of [null, { ...target, generation: -1 }, { ...target, threadId: ' ' }]) {
    await fixture(page, selected);
    await expect(page.getByRole('status')).toContainText('현재 대화를 선택');
    await expect(page.getByRole('button')).toHaveCount(0);
    expect(await render(page, status)).toBe(false);
  }
});

test('표시된 대상의 검토와 조회를 구분하고 키보드로 callback을 보낸다', async ({ page }) => {
  await fixture(page);
  await expect(page.getByText(target.title, { exact: true })).toBeVisible();
  await expect(page.getByText('sample', { exact: true })).toBeVisible();
  const reload = page.getByRole('button', { name: '현재 대화 MCP 다시연결 검토', exact: true });
  await reload.focus();
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: '현재 대화 상태조회', exact: true }).click();
  expect(await calls(page)).toEqual([{ operation: 'reload' }, { operation: 'inspect', cursor: null }]);
  await page.evaluate(() => { (window as any).mcpRuntimeFixture.target.threadId = 'changed'; });
  expect(await render(page, status)).toBe(true);
});

test('queued는 접수이고 unconfirmed와 오류는 이전 연결 정보를 지운다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, status)).toBe(true);
  await expect(page.getByText('연결됨', { exact: true })).toBeVisible();
  expect(await render(page, { operation: 'reloadMcp', threadId: target.threadId, outcome: 'queued' })).toBe(true);
  await expect(page.getByRole('status')).toContainText('접수');
  await expect(page.getByRole('status')).toContainText('연결 완료');
  await expect(page.getByText('연결됨', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('listitem')).toHaveCount(0);
  for (const reason of ['ackMissing', 'targetMismatch']) {
    expect(await render(page, { operation: 'reloadMcp', threadId: target.threadId, outcome: 'unconfirmed', reason })).toBe(true);
    await expect(page.getByRole('status')).toContainText('미확인');
  }
  await render(page, status);
  await page.evaluate(() => (window as any).mcpRuntimeFixture.panel.showUnconfirmed());
  await expect(page.getByText('연결됨', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('listitem')).toHaveCount(0);
});

test('runtime·인증·available/empty/failed/unknown을 나누고 raw 필드는 출력하지 않는다', async ({ page }) => {
  await fixture(page);
  const name = '<img src=x onerror=alert(1)>';
  expect(await render(page, { ...status, toolNames: [name], toolsError: 'PRIVATE_ERROR',
    schema: { secret: 'PRIVATE_SCHEMA' }, url: 'https://private.invalid', env: { key: 'PRIVATE_ENV' } })).toBe(true);
  await expect(page.getByRole('listitem')).toHaveText([name]);
  await expect(page.locator('img,script')).toHaveCount(0);
  await expect(page.locator('body')).not.toContainText('PRIVATE_');
  await expect(page.locator('body')).not.toContainText('private.invalid');
  await expect(page.getByText('OAuth', { exact: true })).toBeVisible();
  for (const toolsState of ['empty', 'failed', 'unknown']) {
    const expected = toolsState === 'empty' ? '조회된 도구 없음' : toolsState === 'failed' ? '도구 목록 조회 실패' : '도구 목록 미확인';
    expect(await render(page, { ...status, runtimeStatus: 'authenticationRequired', authStatus: 'notLoggedIn', toolsState,
      toolNames: toolsState === 'unknown' ? ['not-confirmed'] : [] })).toBe(true);
    await expect(page.getByText(expected, { exact: true })).toBeVisible();
    await expect(page.getByText('인증 필요', { exact: true })).toBeVisible();
    await expect(page.getByText('로그인 안 됨', { exact: true })).toBeVisible();
    await expect(page.getByRole('listitem')).toHaveCount(0);
  }
  expect(await render(page, { ...status, found: false, runtimeStatus: 'unknown', authStatus: 'unknown', toolsState: 'unknown', toolNames: [] })).toBe(true);
  await expect(page.getByRole('status')).toContainText('이 페이지에서 서버를 찾지 못했습니다');
  for (const [runtimeStatus, expected] of [['notStarted', '시작 전'], ['starting', '연결 중'], ['failed', '연결 실패'],
    ['cancelled', '취소됨'], ['disabled', '비활성화됨']]) {
    expect(await render(page, { ...status, runtimeStatus })).toBe(true);
    await expect(page.getByText(expected, { exact: true })).toBeVisible();
  }
});

test('다른 대상·enum·중복·누락·상한 오류를 고정 문구와 false로 거절한다', async ({ page }) => {
  await fixture(page);
  const invalid = [
    null, [], { ...status, operation: 'PRIVATE_OPERATION' }, { ...status, threadId: 'PRIVATE_THREAD' },
    { ...status, serverName: 'PRIVATE_SERVER' }, { ...status, runtimeStatus: 'ready' },
    { ...status, authStatus: 'oauth' }, { ...status, toolsState: 'ready' }, { ...status, found: 'true' },
    { ...status, tools: ['wrong-array-name'], toolNames: undefined },
    { ...status, toolNames: ['same', 'same'] }, { ...status, toolNames: ['x'.repeat(257)] },
    { ...status, toolNames: ['가'.repeat(86)] }, { ...status, toolNames: [''] },
    { ...status, toolNames: Array.from({ length: 257 }, (_, i) => `tool-${i}`) },
    { ...status, nextCursor: '' }, { ...status, nextCursor: 'c'.repeat(257) },
    { ...status, nextCursor: 'bad\n' }, { ...status, nextCursor: undefined },
    { ...status, toolsState: 'available', toolNames: [] }, { ...status, toolsState: 'empty' },
    { ...status, found: false },
    { operation: 'reloadMcp', threadId: target.threadId, outcome: 'ready' },
    { operation: 'reloadMcp', threadId: 'other', outcome: 'queued' },
    { operation: 'reloadMcp', threadId: target.threadId, outcome: 'unconfirmed', reason: 'private' },
  ];
  for (const value of invalid) {
    await render(page, status);
    expect(await render(page, value)).toBe(false);
    await expect(page.getByRole('status')).toHaveText(invalidMessage);
    await expect(page.getByText('연결됨', { exact: true })).toHaveCount(0);
    await expect(page.getByRole('listitem')).toHaveCount(0);
    await expect(page.locator('body')).not.toContainText('PRIVATE_');
  }
});

test('256개 경계를 50개씩 표시하고 다음 페이지는 명시 callback으로만 조회한다', async ({ page }) => {
  await fixture(page);
  const names = Array.from({ length: 256 }, (_, i) => i === 0 ? '가'.repeat(85) + 'x' : `tool-${i}`);
  const cursor = 'c'.repeat(256);
  expect(await render(page, { ...status, toolNames: names, nextCursor: cursor })).toBe(true);
  await expect(page.getByRole('listitem')).toHaveCount(50);
  expect(await calls(page)).toEqual([]);
  for (const expected of [100, 150, 200, 250, 256]) {
    await page.getByRole('button', { name: '도구 더 보기', exact: true }).click();
    await expect(page.getByRole('listitem')).toHaveCount(expected);
  }
  await expect(page.getByRole('button', { name: '도구 더 보기', exact: true })).toBeHidden();
  await page.getByRole('button', { name: '다음 상태 페이지 조회', exact: true }).click();
  expect(await calls(page)).toEqual([{ operation: 'inspect', cursor }]);
  expect(await render(page, { ...status, toolNames: ['next-page'] })).toBe(true);
  await expect(page.getByRole('listitem')).toHaveText(['next-page']);
  await expect(page.locator('body')).not.toContainText(cursor);
});

test('pending과 parent disabled를 언어·결과 전환 뒤에도 유지한다', async ({ page }) => {
  await fixture(page);
  await render(page, { ...status, nextCursor: 'next', toolNames: Array.from({ length: 51 }, (_, i) => `t${i}`) });
  await page.evaluate(() => { const f = (window as any).mcpRuntimeFixture; f.panel.setDisabled(true); f.setLanguage('en'); });
  for (const button of await page.getByRole('button').all()) await expect(button).toBeDisabled();
  await page.evaluate(() => { const f = (window as any).mcpRuntimeFixture;
    for (const b of f.panel.element.querySelectorAll('button')) { b.disabled = false; b.click(); }
    f.setLanguage('ko'); });
  expect(await calls(page)).toEqual([]);
  await render(page, status);
  await page.evaluate(() => { const f = (window as any).mcpRuntimeFixture; f.panel.showUnconfirmed(); f.setLanguage('ko'); });
  for (const button of await page.getByRole('button').all()) await expect(button).toBeDisabled();
  await page.evaluate(() => { const f = (window as any).mcpRuntimeFixture; f.panel.setDisabled(false); f.panel.showPending();
    for (const b of f.panel.element.querySelectorAll('button')) b.click(); });
  await expect(page.getByRole('status')).toContainText('처리 중');
  await expect(page.getByRole('listitem')).toHaveCount(0);
  expect(await calls(page)).toEqual([]);
  await render(page, status);
  await expect(page.getByRole('button', { name: '현재 대화 상태조회', exact: true })).toBeEnabled();
});

test('언어를 전환해도 목록 위치를 보존하고 dispose 뒤 DOM과 callback을 살리지 않는다', async ({ page }) => {
  await fixture(page);
  await render(page, { ...status, toolNames: Array.from({ length: 80 }, (_, i) => `t${i}`), nextCursor: 'next' });
  await page.getByRole('button', { name: '도구 더 보기', exact: true }).click();
  await page.evaluate(() => (window as any).mcpRuntimeFixture.setLanguage('en'));
  await expect(page.getByRole('button', { name: 'Review current-chat MCP reconnect', exact: true })).toBeVisible();
  await expect(page.getByRole('listitem')).toHaveCount(80);
  await expect(page.getByText('Connected', { exact: true })).toBeVisible();
  expect(await calls(page)).toEqual([]);
  const result = await page.evaluate(status => {
    const f = (window as any).mcpRuntimeFixture;
    const element = f.panel.element;
    const retained = Array.from(element.querySelectorAll('button')) as HTMLButtonElement[];
    f.panel.dispose(); f.panel.dispose();
    retained.forEach(b => { b.disabled = false; b.click(); });
    f.setLanguage('ko'); f.panel.showPending(); f.panel.showUnconfirmed(); f.panel.setDisabled(false);
    return { accepted: f.panel.render(status), connected: element.isConnected, children: element.childElementCount, calls: f.calls };
  }, status);
  expect(result).toEqual({ accepted: false, connected: false, children: 0, calls: [] });
  await expect(page.getByRole('button')).toHaveCount(0);
});
