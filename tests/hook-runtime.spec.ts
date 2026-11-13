import { expect, test, type Page } from '@playwright/test';

// Isolated source fixture: no real Core, CLI, native request, or thread/generation gate.
const target = { id: 'session-a', generation: 3, threadId: 'thread-a', title: '선택한 대화' };
function row(index: number, fields: Record<string, unknown> = {}) {
  return { id: `hook-${index.toString(16).padStart(64, '0')}`, eventName: 'preToolUse', handlerType: 'command',
    source: 'project', enabled: true, isManaged: false, trustStatus: 'trusted', ...fields };
}
const status = { operation: 'hooksStatus', threadId: target.threadId, provenance: 'loadedThreadHooksList',
  executionGate: 'unknown', applicationState: 'unknown', hooks: { items: [row(1)], warningCount: 0, errorCount: 0 } };
const invalidMessage = '현재 대화 Hook 결과를 확인할 수 없습니다. 목록을 다시 조회해 주세요.';

async function fixture(page: Page, selected: unknown = target) {
  await page.route('**/hook-runtime-harness', route => route.fulfill({
    contentType: 'text/html', body: '<!doctype html><html><body></body></html>',
  }));
  await page.goto('/hook-runtime-harness');
  await page.evaluate(async selected => {
    const modulePath = '/src/hook-runtime.ts';
    const languagePath = '/src/i18n.ts';
    const { createHookRuntimePanel } = await import(modulePath);
    const { setLanguage } = await import(languagePath);
    setLanguage('ko');
    const f = (window as any).hookRuntimeFixture = { calls: [] as string[], target: selected, setLanguage };
    f.panel = createHookRuntimePanel({ target: selected ?? undefined, onInspect: () => f.calls.push('inspect') });
    document.body.append(f.panel.element);
  }, selected);
}
async function render(page: Page, value: unknown): Promise<boolean> {
  return page.evaluate(value => (window as any).hookRuntimeFixture.panel.render(value), value);
}
async function calls(page: Page) { return page.evaluate(() => (window as any).hookRuntimeFixture.calls); }

test('HU01 대상이 없거나 부적합하면 안내만 제공한다', async ({ page }) => {
  for (const selected of [null, { ...target, generation: -1 }, { ...target, generation: 0.1 },
    { ...target, threadId: ' ' }, { ...target, id: 'x'.repeat(257) }, { ...target, title: '가'.repeat(342) }]) {
    await fixture(page, selected);
    await expect(page.getByRole('status')).toContainText('현재 대화를 선택');
    await expect(page.getByRole('button')).toHaveCount(0);
    expect(await render(page, status)).toBe(false);
  }
});

test('HU02 표시된 대상의 조회를 키보드로 시작하고 target 복사본을 유지한다', async ({ page }) => {
  await fixture(page);
  await expect(page.getByText(target.title, { exact: true })).toBeVisible();
  const inspect = page.getByRole('button', { name: '현재 대화 Hook 조회', exact: true });
  await inspect.focus(); await page.keyboard.press('Enter');
  expect(await calls(page)).toEqual(['inspect']);
  await page.evaluate(() => {
    const f = (window as any).hookRuntimeFixture;
    f.target.threadId = 'changed'; f.target.title = 'changed';
  });
  expect(await render(page, status)).toBe(true);
  await expect(page.getByText(target.title, { exact: true })).toBeVisible();
});

test('HU03 동일 event의 다른 ID를 유지하고 사용 설정과 신뢰·실행 미확인을 분리한다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...status, hooks: { items: [row(1), row(2, { enabled: false, trustStatus: 'modified', isManaged: true })],
    warningCount: 0, errorCount: 0 } })).toBe(true);
  const items = page.getByRole('listitem');
  await expect(items).toHaveCount(2);
  await expect(items.nth(0)).toContainText(row(1).id);
  await expect(items.nth(1)).toContainText(row(2).id);
  await expect(items.nth(0)).toContainText('사용 설정: 켜짐');
  await expect(items.nth(0)).toContainText('신뢰: 신뢰됨');
  await expect(items.nth(1)).toContainText('사용 설정: 꺼짐');
  await expect(items.nth(1)).toContainText('신뢰: 변경됨');
  await expect(items.nth(1)).toContainText('관리 정책: 관리됨');
  await expect(page.getByText('실행 gate', { exact: true })).toBeVisible();
  await expect(page.getByText('적용 상태', { exact: true })).toBeVisible();
  await expect(page.locator('dd')).toHaveText(['미확인', '미확인']);
  await expect(page.getByText(/기본 제공 Hook·요청별 추가 Hook/)).toBeVisible();
  await expect(page.getByRole('button')).toHaveCount(1); // no mutation/trust action; expansion is hidden
});

test('HU04 empty·미확인·pending을 구분하며 이전 inventory를 지운다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...status, hooks: { items: [], warningCount: 0, errorCount: 0 } })).toBe(true);
  await expect(page.getByRole('status')).toContainText('조회된 Hook이 없습니다');
  await expect(page.getByRole('status')).toContainText('미확인');
  await render(page, status);
  await page.evaluate(() => (window as any).hookRuntimeFixture.panel.showUnconfirmed());
  await expect(page.getByRole('status')).toContainText('상태 미확인');
  await expect(page.getByRole('listitem')).toHaveCount(0);
  await render(page, status);
  await page.evaluate(() => (window as any).hookRuntimeFixture.panel.showPending());
  await expect(page.getByRole('status')).toContainText('조회 중');
  await expect(page.getByRole('listitem')).toHaveCount(0);
  await expect(page.locator('section')).toHaveAttribute('aria-busy', 'true');
  await expect(page.getByRole('button', { name: '현재 대화 Hook 조회', exact: true })).toBeDisabled();
  await page.evaluate(() => document.querySelector<HTMLButtonElement>('button')!.click());
  expect(await calls(page)).toEqual([]);
  await page.evaluate(() => (window as any).hookRuntimeFixture.panel.showUnconfirmed());
  await page.getByRole('button', { name: '현재 대화 Hook 조회', exact: true }).click();
  expect(await calls(page)).toEqual(['inspect']);
});

test('HU05 다른 대상·enum·필수 필드·중복 ID·숫자 경계를 고정 문구로 거부한다', async ({ page }) => {
  await fixture(page);
  const invalid: unknown[] = [null, [], { ...status, threadId: 'PRIVATE_THREAD' }, { ...status, operation: 'PRIVATE_OPERATION' },
    { ...status, provenance: 'PRIVATE_PROVENANCE' }, { ...status, executionGate: 'enabled' }, { ...status, applicationState: 'applied' },
    ...['eventName', 'handlerType', 'source', 'trustStatus'].map(field => ({ ...status,
      hooks: { ...status.hooks, items: [row(1, { [field]: 'PRIVATE_ENUM' })] } })),
    { ...status, hooks: { ...status.hooks, items: [row(1), row(1)] } },
    ...['hook-short', `hook-${'A'.repeat(64)}`, '<img src=x>', 'PRIVATE_ID'].map(id => ({ ...status,
      hooks: { ...status.hooks, items: [row(1, { id })] } })),
    { ...status, hooks: { ...status.hooks, items: [row(1, { enabled: 'true' })] } },
    { ...status, hooks: { ...status.hooks, items: [row(1, { isManaged: null })] } },
    ...[-1, 0.5, 129, Number.MAX_SAFE_INTEGER + 1].map(warningCount => ({ ...status, hooks: { ...status.hooks, warningCount } })),
    { ...status, hooks: { ...status.hooks, errorCount: 129 } },
  ];
  const missing = structuredClone(status) as any; delete missing.hooks.items[0].enabled; invalid.push(missing);
  const missingCount = structuredClone(status) as any; delete missingCount.hooks.errorCount; invalid.push(missingCount);
  for (const value of invalid) {
    expect(await render(page, status)).toBe(true);
    expect(await render(page, value)).toBe(false);
    await expect(page.getByRole('status')).toHaveText(invalidMessage);
    await expect(page.getByRole('listitem')).toHaveCount(0);
    await expect(page.locator('body')).not.toContainText('PRIVATE_');
  }
});

test('HU06 원문 필드와 HTML·비밀은 거부하고 target 제목은 텍스트로만 쓴다', async ({ page }) => {
  const title = '<img src=x onerror=alert(1)>';
  await fixture(page, { ...target, title });
  await expect(page.getByText(title, { exact: true })).toBeVisible();
  for (const field of ['key', 'command', 'sourcePath', 'currentHash', 'matcher', 'server', 'tool', 'pluginId']) {
    const value = { ...status, hooks: { ...status.hooks, items: [row(1, { [field]: 'PRIVATE_SECRET<img src=x>' })] } };
    expect(await render(page, value)).toBe(false);
  }
  expect(await render(page, { ...status, raw: 'PRIVATE_SECRET' })).toBe(false);
  expect(await render(page, { ...status, hooks: { ...status.hooks, warnings: ['PRIVATE_SECRET'] } })).toBe(false);
  await expect(page.locator('img,script')).toHaveCount(0);
  await expect(page.locator('body')).not.toContainText('PRIVATE_SECRET');
  expect(await page.evaluate(() => {
    const f = (window as any).hookRuntimeFixture;
    const value = Object.create({ operation: 'hooksStatus' });
    return f.panel.render(value);
  })).toBe(false);
});

test('HU07 50개씩 명시 표시하고 4096개를 보존하되 4097개는 거부한다', async ({ page }) => {
  await fixture(page);
  const large = { ...status, hooks: { ...status.hooks, items: Array.from({ length: 4096 }, (_, i) => row(i + 1)) } };
  expect(await render(page, large)).toBe(true);
  await expect(page.getByRole('listitem')).toHaveCount(50);
  const more = page.getByRole('button', { name: 'Hook 더 보기', exact: true });
  await more.click(); await expect(page.getByRole('listitem')).toHaveCount(100);
  // Expand through the public button without a network request or scrolling thousands of times.
  await page.evaluate(() => {
    const button = Array.from(document.querySelectorAll<HTMLButtonElement>('button')).find(button => button.textContent === 'Hook 더 보기')!;
    while (!button.hidden) button.click();
  });
  await expect(page.getByRole('listitem')).toHaveCount(4096);
  await expect(page.getByRole('listitem').last()).toContainText(row(4096).id);
  await expect(page.getByRole('button', { name: '현재 대화 Hook 조회', exact: true })).toBeFocused();
  expect(await calls(page)).toEqual([]);
  expect(await render(page, { ...large, hooks: { ...large.hooks, items: [...large.hooks.items, row(4097)] } })).toBe(false);
  expect(await render(page, { ...status, hooks: { ...status.hooks, items: [row(1), ...Array.from({ length: 50 }, (_, i) => row(i + 2))] } })).toBe(true);
  await expect(page.getByRole('listitem')).toHaveCount(50);
  await more.click(); await expect(page.getByRole('listitem')).toHaveCount(51);
});

test('HU08 언어 전환·재렌더 뒤에도 disabled와 표시 개수를 유지한다', async ({ page }) => {
  await fixture(page);
  await render(page, { ...status, hooks: { ...status.hooks, items: Array.from({ length: 101 }, (_, i) => row(i + 1)) } });
  await page.getByRole('button', { name: 'Hook 더 보기', exact: true }).click();
  await page.evaluate(() => { const f = (window as any).hookRuntimeFixture; f.panel.setDisabled(true); f.setLanguage('en'); });
  await expect(page.getByRole('listitem')).toHaveCount(100);
  await expect(page.getByRole('button', { name: 'Inspect current-chat Hooks', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Show more Hooks', exact: true })).toBeDisabled();
  await expect(page.getByRole('listitem').first()).toContainText('Configured: On');
  await render(page, status);
  await expect(page.getByRole('button', { name: 'Inspect current-chat Hooks', exact: true })).toBeDisabled();
  await page.evaluate(() => { const f = (window as any).hookRuntimeFixture; f.panel.showPending(); f.setLanguage('ko'); f.panel.setDisabled(false); });
  await expect(page.getByRole('status')).toContainText('조회 중');
  await expect(page.getByRole('button', { name: '현재 대화 Hook 조회', exact: true })).toBeDisabled();
  await page.evaluate(() => (window as any).hookRuntimeFixture.panel.showUnconfirmed());
  await expect(page.getByRole('button', { name: '현재 대화 Hook 조회', exact: true })).toBeEnabled();
  expect(await calls(page)).toEqual([]);
});

test('HU09 dispose 뒤 retained button·지연응답·언어 전환은 DOM과 callback을 되살리지 않는다', async ({ page }) => {
  await fixture(page); await render(page, status);
  const result = await page.evaluate(value => {
    const f = (window as any).hookRuntimeFixture; const element = f.panel.element;
    const button = element.querySelector('button');
    f.panel.dispose(); button.click(); f.panel.showPending(); f.panel.showUnconfirmed(); f.panel.setDisabled(false);
    f.setLanguage('en'); const accepted = f.panel.render(value); f.panel.dispose();
    return { accepted, connected: element.isConnected, childCount: element.childNodes.length, calls: f.calls };
  }, status);
  expect(result).toEqual({ accepted: false, connected: false, childCount: 0, calls: [] });
  await expect(page.locator('section')).toHaveCount(0);
});

test('HU10 진단은 개수로만 표시하며 미확인에서 미지원 상태를 만들어내지 않는다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...status, hooks: { items: [], warningCount: 128, errorCount: 1 } })).toBe(true);
  await expect(page.getByText('경고: 128 · 오류: 1', { exact: true })).toBeVisible();
  await expect(page.getByText('Hook 진단이 있습니다. 자세한 원문은 이 조회 화면에 표시하지 않습니다.', { exact: true })).toBeVisible();
  await page.evaluate(() => (window as any).hookRuntimeFixture.panel.showUnconfirmed());
  await expect(page.getByRole('status')).toContainText('상태 미확인');
  await expect(page.locator('body')).not.toContainText('지원하지');
  await expect(page.getByText('경고: 128 · 오류: 1', { exact: true })).toHaveCount(0);
  // Unsupported Core classification is a separate safe parent notice, not this panel's DTO.
});
