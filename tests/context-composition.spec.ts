import { expect, test, type Page } from '@playwright/test';

// Synthetic source fixture only. No Core, CLI, Native, provider usage, or session epoch gate is exercised.
const threadId = 'thread-a';
function fragment(fields: Record<string, unknown> = {}) {
  return { sourceId: 'source-1', sourceOrdinal: '9007199254740993', residentOccurrence: '0', taskId: 'task-a',
    kind: 'user', selected: true, required: null, selectionReason: 'user_instruction', residency: 'resident',
    residentSerializedBytes: 7, estimatedTokens: 9, ...fields };
}
const snapshot = {
  threadId, snapshotId: 'snapshot-a', contextIndexRevision: '18446744073709551615', historyVersion: '9007199254740993',
  asOf: '2026-11-14T01:02:03.000Z',
  // Intentionally not additive. A presentation component must not recompute these reported values.
  sizes: { residentSerializedBytes: 9000, selectedResidentSerializedBytes: 700, excludedResidentSerializedBytes: 2,
    archiveSerializedBytes: null, selectedBodyEstimatedTokens: 321, estimateProvenance: 'selected_resident_before_request_normalization',
    navigationEstimatedTokens: 11, baseEstimatedTokens: null, toolEstimatedTokens: 4, totalEstimatedTokens: null },
  limits: { requestedContextWindowOverride: 1000, effectiveContextWindow: null, provenance: 'session_configuration_override' },
  fragments: [fragment()],
  cache: { scope: 'response', provenance: 'inclusive_response_input_usage', status: 'hit', cachedInputTokens: 12,
    writeInputTokens: 3, uncachedInputTokens: 5, totalInputTokens: 20, observedAt: '2026-11-14T01:00:00Z',
    observationId: 'observation-a', requestContextIndexRevision: '9007199254740993', revisionStatus: 'stale',
    requestedTtlSeconds: 300, effectiveTtlSeconds: null, ttlSupport: 'configured_provider_policy',
    ttlApplication: 'request_cache_activity_observed', requestStartedAt: '2026-11-14T00:59:00Z',
    minimumValidUntil: '2020-01-01T00:00:00Z', minimumValidUntilProvenance: 'configured_request_policy_and_reported_cache_activity',
    minimumValidUntilScope: 'reported_write_tokens_only_not_individual_fragments', estimatedExpiry: null, serverConfirmedExpiry: null },
  nextCursor: 100,
};
const invalidMessage = '컨텍스트 구성 결과를 확인할 수 없습니다. 새로고침해 주세요.';
async function fixture(page: Page, selectedThread: unknown = threadId) {
  await page.route('**/context-composition-harness', route => route.fulfill({ contentType: 'text/html',
    body: '<!doctype html><html><head><style>body{margin:0;padding:12px;box-sizing:border-box}button{font:inherit}</style></head><body></body></html>' }));
  await page.goto('/context-composition-harness');
  await page.evaluate(async selectedThread => {
    const modulePath = '/src/context-composition.ts';
    const languagePath = '/src/i18n.ts';
    const { createContextCompositionPanel } = await import(modulePath);
    const { setLanguage } = await import(languagePath);
    setLanguage('ko');
    const f = (window as any).compositionFixture = { calls: [] as unknown[], setLanguage };
    f.panel = createContextCompositionPanel({ threadId: selectedThread,
      onRefresh: () => f.calls.push({ type: 'refresh' }),
      onNext: (snapshotId: string, cursor: number) => f.calls.push({ type: 'next', snapshotId, cursor }) });
    document.body.append(f.panel.element);
  }, selectedThread);
}
async function render(page: Page, value: unknown): Promise<boolean> {
  return page.evaluate(value => (window as any).compositionFixture.panel.render(value), value);
}
const field = (page: Page, name: string) => page.locator(`[data-context-field="${name}"]`);
async function calls(page: Page) { return page.evaluate(() => (window as any).compositionFixture.calls); }

test('CC01 초기 상태와 부적합한 thread는 구분하고 대상 없는 callback을 차단한다', async ({ page }) => {
  await fixture(page);
  await expect(page.getByRole('status')).toContainText('새로고침');
  expect(await calls(page)).toEqual([]);
  for (const id of ['', ' ', 'x'.repeat(257), 'bad\nthread', '\u202ethread', null]) {
    await fixture(page, id);
    expect(await render(page, snapshot)).toBe(false);
    await expect(page.getByRole('status')).toContainText('현재 대화');
    await expect(page.getByRole('button', { name: '구성 새로고침', exact: true })).toBeDisabled();
    await page.evaluate(() => document.querySelector<HTMLButtonElement>('button')!.click());
    expect(await calls(page)).toEqual([]);
  }
});

test('CC02 서버 byte와 heuristic을 그대로 표시하고 null과 한도를 실측 usage와 분리한다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, snapshot)).toBe(true);
  for (const [name, text] of Object.entries({ 'sizes.residentSerializedBytes': '9000 B',
    'sizes.selectedResidentSerializedBytes': '700 B', 'sizes.excludedResidentSerializedBytes': '2 B',
    'sizes.archiveSerializedBytes': '미로딩 / 미확인', 'sizes.selectedBodyEstimatedTokens': '321',
    'sizes.navigationEstimatedTokens': '11', 'sizes.baseEstimatedTokens': '미확인', 'sizes.toolEstimatedTokens': '4',
    'sizes.totalEstimatedTokens': '미확인', 'limits.requestedContextWindowOverride': '1000',
    'limits.effectiveContextWindow': '미확인' })) await expect(field(page, name)).toHaveText(text);
  await expect(page.getByText(/heuristic 추정치이며 provider 실측 usage가 아닙니다/)).toBeVisible();
  await expect(field(page, 'sizes.estimateProvenance')).toContainText('요청 정규화 전');
  await expect(field(page, 'limits.provenance')).toContainText('세션 설정');
});

test('CC03 큰 u64와 동일 source의 서로 다른 occurrence 및 nullable 조각을 보존한다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...snapshot, fragments: [fragment(), fragment({ residentOccurrence: '18446744073709551615' }),
    fragment({ sourceId: null, sourceOrdinal: null, residentOccurrence: null, taskId: null, kind: null,
      required: null, selected: false, selectionReason: null, residency: 'unknown', residentSerializedBytes: null, estimatedTokens: null })] })).toBe(true);
  await expect(field(page, 'contextIndexRevision')).toHaveText('18446744073709551615');
  await expect(field(page, 'historyVersion')).toHaveText('9007199254740993');
  const rows = page.getByRole('listitem');
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toContainText('9007199254740993');
  await expect(rows.nth(1)).toContainText('18446744073709551615');
  await expect(rows.nth(2).locator('[data-context-field="required"]')).toHaveText('미확인');
  await expect(rows.nth(2).locator('[data-context-field="residentSerializedBytes"]')).toHaveText('미확인');
  await expect(rows.nth(2).locator('[data-context-field="selected"]')).toHaveText('제외');
});

test('CC04 응답 집계 캐시와 TTL 하한·추정·확정은 구분하고 과거 하한을 만료로 해석하지 않는다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, snapshot)).toBe(true);
  for (const [name, value] of Object.entries({ cachedInputTokens: '12', writeInputTokens: '3', uncachedInputTokens: '5',
    totalInputTokens: '20', observedAt: snapshot.cache.observedAt, requestContextIndexRevision: '9007199254740993',
    requestedTtlSeconds: '300 s', effectiveTtlSeconds: '미확인', minimumValidUntil: '2020-01-01T00:00:00Z',
    estimatedExpiry: '미확인', serverConfirmedExpiry: '미확인' })) await expect(field(page, `cache.${name}`)).toHaveText(value);
  await expect(field(page, 'cache.revisionStatus')).toHaveText('이전 revision 관측');
  await expect(field(page, 'cache.provenance')).toContainText('포함');
  await expect(field(page, 'cache.minimumValidUntilScope')).toContainText('보고된 쓰기 토큰');
  await expect(page.getByText(/읽기로 TTL이 다시 시작되지 않으며/)).toBeVisible();
  await expect(page.getByText('최소 유효 하한 (서버 확정 아님)', { exact: true })).toBeVisible();
  await expect(page.getByText('서버 확정 만료시각', { exact: true })).toBeVisible();
  await expect(page.locator('section.context-composition')).not.toContainText('만료됨');
  expect(await calls(page)).toEqual([]);
  // The reported status, not a renderer comparison or read count, owns freshness.
  await render(page, { ...snapshot, cache: { ...snapshot.cache, revisionStatus: 'current', provenance: 'additive_response_input_usage',
    cachedInputTokens: 0, writeInputTokens: null, uncachedInputTokens: null, totalInputTokens: null } });
  await expect(field(page, 'cache.revisionStatus')).toHaveText('현재 revision 관측');
  await expect(field(page, 'cache.provenance')).toContainText('별도 합산');
  await expect(field(page, 'cache.cachedInputTokens')).toHaveText('0');
  await expect(field(page, 'cache.totalInputTokens')).toHaveText('미확인');
  for (const [status, text] of [['miss', '캐시 읽기 0 관측'], ['invalid', '관측값 유효하지 않음'], ['unknown', '미확인']]) {
    expect(await render(page, { ...snapshot, cache: { ...snapshot.cache, status, cachedInputTokens: null,
      ttlApplication: 'requested_unconfirmed', ttlSupport: 'unknown', serverConfirmedExpiry: '2026-11-14T01:05:00Z' } })).toBe(true);
    await expect(field(page, 'cache.status')).toHaveText(text);
    await expect(field(page, 'cache.cachedInputTokens')).toHaveText('미확인');
    await expect(field(page, 'cache.ttlApplication')).toHaveText('요청됨 · 적용 미확인');
    await expect(field(page, 'cache.serverConfirmedExpiry')).toHaveText('2026-11-14T01:05:00Z');
  }
});

test('CC05 미래 String 상태·근거·이유는 미확인으로 표시하고 원값은 반사하지 않는다', async ({ page }) => {
  await fixture(page);
  const future = 'PRIVATE_FUTURE_SENTINEL';
  const cache = { ...snapshot.cache } as Record<string, unknown>;
  for (const name of ['scope', 'provenance', 'status', 'revisionStatus', 'ttlSupport', 'ttlApplication',
    'minimumValidUntilProvenance', 'minimumValidUntilScope']) cache[name] = future;
  expect(await render(page, { ...snapshot, sizes: { ...snapshot.sizes, estimateProvenance: future },
    limits: { ...snapshot.limits, provenance: future }, cache, fragments: [fragment({ selectionReason: future })] })).toBe(true);
  for (const name of ['cache.status', 'cache.scope', 'cache.ttlSupport', 'cache.ttlApplication', 'cache.revisionStatus',
    'sizes.estimateProvenance', 'limits.provenance', 'selectionReason']) await expect(field(page, name)).toHaveText('미확인');
  await expect(page.locator('body')).not.toContainText(future);
  await page.evaluate(() => (window as any).compositionFixture.setLanguage('en'));
  await expect(field(page, 'cache.status')).toHaveText('Unknown');
  await expect(page.locator('body')).not.toContainText(future);
});

test('CC06 빈 목록·미지원·stale·미확인을 구분하며 이전 목록과 상세를 제거한다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...snapshot, fragments: [], nextCursor: null })).toBe(true);
  await expect(page.getByRole('status')).toContainText('이 페이지에 조각이 없습니다');
  await expect(field(page, 'sizes.residentSerializedBytes')).toHaveText('9000 B');
  for (const [reason, text] of [['unsupported', '지원을 확인할 수 없습니다'], ['stale', '새로고침해 주세요'],
    ['unavailable', '상태 미확인']]) {
    await render(page, snapshot);
    await page.evaluate(reason => (window as any).compositionFixture.panel.showUnavailable(reason), reason);
    await expect(page.getByRole('status')).toContainText(text);
    await expect(page.getByRole('listitem')).toHaveCount(0);
    await expect(field(page, 'sizes.residentSerializedBytes')).toHaveCount(0);
    await expect(page.getByRole('button', { name: '다음 구성 페이지', exact: true })).toBeHidden();
    await expect(page.getByRole('button', { name: '구성 새로고침', exact: true })).toBeEnabled();
  }
});

test('CC07 pending과 부모 disabled는 언어·결과 변경에도 callback을 막는다', async ({ page }) => {
  await fixture(page); await render(page, snapshot);
  await page.evaluate(() => (window as any).compositionFixture.panel.showPending());
  await expect(page.getByRole('status')).toContainText('조회 중');
  await expect(page.locator('section.context-composition')).toHaveAttribute('aria-busy', 'true');
  await expect(page.getByRole('listitem')).toHaveCount(0);
  await page.evaluate(() => document.querySelector<HTMLButtonElement>('button')!.click());
  expect(await calls(page)).toEqual([]);
  await page.evaluate(() => { const f = (window as any).compositionFixture; f.panel.setDisabled(true); f.setLanguage('en'); });
  await render(page, snapshot);
  for (const button of await page.getByRole('button').all()) await expect(button).toBeDisabled();
  await expect(page.locator('section.context-composition')).toHaveAttribute('aria-busy', 'false');
  await page.evaluate(() => { for (const button of document.querySelectorAll<HTMLButtonElement>('button')) button.click(); });
  expect(await calls(page)).toEqual([]);
  await page.evaluate(() => { const f = (window as any).compositionFixture; f.panel.showUnavailable('unavailable'); f.setLanguage('ko'); });
  await expect(page.getByRole('button', { name: '구성 새로고침', exact: true })).toBeDisabled();
  await page.evaluate(() => (window as any).compositionFixture.panel.setDisabled(false));
  await page.getByRole('button', { name: '구성 새로고침', exact: true }).click();
  expect(await calls(page)).toEqual([{ type: 'refresh' }]);
});

test('CC08 다음 페이지는 명시 callback만 보내며 결과는 페이지를 교체한다', async ({ page }) => {
  await fixture(page);
  expect(await page.evaluate(value => {
    const accepted = (window as any).compositionFixture.panel.render(value);
    value.snapshotId = 'mutated-after-render'; value.nextCursor = 9; value.fragments[0].sourceId = 'mutated-after-render';
    return accepted;
  }, snapshot)).toBe(true);
  expect(await calls(page)).toEqual([]);
  await page.getByRole('button', { name: '다음 구성 페이지', exact: true }).click();
  expect(await calls(page)).toEqual([{ type: 'next', snapshotId: 'snapshot-a', cursor: 100 }]);
  expect(await render(page, { ...snapshot, snapshotId: 'snapshot-b', fragments: [fragment({ sourceId: 'second-page', residentOccurrence: '100' })], nextCursor: null })).toBe(true);
  await expect(page.getByRole('listitem')).toHaveCount(1);
  await expect(page.getByRole('listitem')).toContainText('second-page');
  await expect(page.getByRole('listitem')).not.toContainText('source-1');
  await expect(page.getByRole('button', { name: '다음 구성 페이지', exact: true })).toBeHidden();
  expect(await calls(page)).toHaveLength(1);
});

test('CC09 다른 thread·잘못된 형식·숫자·식별·시각 경계를 고정 문구로 거부한다', async ({ page }) => {
  await fixture(page);
  const invalid: unknown[] = [null, [], { ...snapshot, threadId: 'PRIVATE_THREAD' },
    ...[-1, 0.5, Number.MAX_SAFE_INTEGER + 1, '7', null].map(residentSerializedBytes => ({ ...snapshot,
      sizes: { ...snapshot.sizes, residentSerializedBytes } })),
    ...[0, -1, 0.5].map(effectiveContextWindow => ({ ...snapshot, limits: { ...snapshot.limits, effectiveContextWindow } })),
    ...[12, '01', '-1', '18446744073709551616', '1.5'].map(contextIndexRevision => ({ ...snapshot, contextIndexRevision })),
    { ...snapshot, cache: { ...snapshot.cache, requestContextIndexRevision: 1 } },
    { ...snapshot, cache: { ...snapshot.cache, observedAt: 'PRIVATE_TIME' } },
    { ...snapshot, cache: { ...snapshot.cache, status: 'x'.repeat(257) } },
    { ...snapshot, asOf: '2026-99-99T25:00:00Z' }, { ...snapshot, nextCursor: -1 },
    ...[{ selected: null }, { required: 0 }, { kind: 'PRIVATE_KIND' }, { residency: 'PRIVATE_RESIDENCY' },
      { sourceId: 'x'.repeat(257) }, { taskId: 'bad\nvalue' }, { sourceOrdinal: 12 }, { residentOccurrence: '01' },
      { estimatedTokens: Infinity }].map(fields => ({ ...snapshot, fragments: [fragment(fields)] })),
  ];
  const missing = structuredClone(snapshot) as any; delete missing.cache.serverConfirmedExpiry; invalid.push(missing);
  const missingNullable = structuredClone(snapshot) as any; delete missingNullable.fragments[0].required; invalid.push(missingNullable);
  for (const value of invalid) {
    expect(await render(page, snapshot)).toBe(true);
    expect(await render(page, value)).toBe(false);
    await expect(page.getByRole('status')).toHaveText(invalidMessage);
    await expect(page.getByRole('listitem')).toHaveCount(0);
    await expect(field(page, 'contextIndexRevision')).toHaveCount(0);
    await expect(page.locator('body')).not.toContainText('PRIVATE_');
  }
  expect(await render(page, { ...snapshot, sizes: { ...snapshot.sizes, residentSerializedBytes: Number.MAX_SAFE_INTEGER },
    cache: { ...snapshot.cache, requestContextIndexRevision: null }, nextCursor: 0 })).toBe(true);
  await expect(field(page, 'sizes.residentSerializedBytes')).toHaveText('9007199254740991 B');
  await page.getByRole('button', { name: '다음 구성 페이지', exact: true }).click();
  expect(await calls(page)).toEqual([{ type: 'next', snapshotId: 'snapshot-a', cursor: 0 }]);
});

test('CC10 100행을 유지하고 101행·무한 길이 배열을 거부한다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...snapshot, fragments: Array.from({ length: 100 }, (_, index) => fragment({ residentOccurrence: String(index) })) })).toBe(true);
  await expect(page.getByRole('listitem')).toHaveCount(100);
  expect(await render(page, { ...snapshot, fragments: Array.from({ length: 101 }, () => fragment()) })).toBe(false);
  await expect(page.getByRole('listitem')).toHaveCount(0);
  const accepted = await page.evaluate(value => {
    const fragments: unknown[] = []; fragments.length = 1_000_000;
    return (window as any).compositionFixture.panel.render({ ...value, fragments });
  }, snapshot);
  expect(accepted).toBe(false);
  await expect(page.getByRole('status')).toHaveText(invalidMessage);
});

test('CC11 HTML ID는 textContent이며 raw 비밀 추가필드와 accessor를 거부한다', async ({ page }) => {
  await fixture(page);
  const html = '<img src=x onerror=alert(1)>';
  expect(await render(page, { ...snapshot, fragments: [fragment({ sourceId: html, taskId: '<script>private()</script>' })] })).toBe(true);
  await expect(page.getByRole('listitem')).toContainText(html);
  await expect(page.locator('img,script')).toHaveCount(0);
  for (const value of [{ ...snapshot, body: 'PRIVATE_BODY' }, { ...snapshot, cache: { ...snapshot.cache, authorization: 'PRIVATE_AUTH' } },
    { ...snapshot, fragments: [fragment({ content: 'PRIVATE_CONTENT' })] }]) {
    expect(await render(page, value)).toBe(false);
    await expect(page.locator('body')).not.toContainText('PRIVATE_');
  }
  const result = await page.evaluate(value => {
    let touched = false;
    Object.defineProperty(value, 'threadId', { enumerable: true, get() { touched = true; return 'thread-a'; } });
    return { accepted: (window as any).compositionFixture.panel.render(value), touched };
  }, snapshot);
  expect(result).toEqual({ accepted: false, touched: false });
});

test('CC12 폐기 이후 지연 render·보관한 버튼·언어 변경은 DOM과 callback을 살리지 않는다', async ({ page }) => {
  await fixture(page); await render(page, snapshot);
  const result = await page.evaluate(value => {
    const f = (window as any).compositionFixture;
    const element = f.panel.element;
    const buttons = [...element.querySelectorAll('button')];
    f.panel.dispose();
    for (const button of buttons) button.click();
    f.panel.showPending(); f.panel.showUnavailable('stale'); f.panel.setDisabled(false);
    const accepted = f.panel.render(value);
    document.body.append(element); // retained external reference must stay empty even on language change
    f.setLanguage('en'); f.panel.dispose();
    return { accepted, children: element.childElementCount, text: element.textContent, connected: element.isConnected, calls: f.calls };
  }, snapshot);
  expect(result).toEqual({ accepted: false, children: 0, text: '', connected: true, calls: [] });
});

test('CC13 언어 전환은 값·현재 page·부모 disabled를 보존하고 callback하지 않는다', async ({ page }) => {
  await fixture(page); await render(page, snapshot);
  await page.evaluate(() => { const f = (window as any).compositionFixture; f.panel.setDisabled(true); f.setLanguage('en'); });
  await expect(page.getByRole('heading', { name: 'Current-chat context composition', exact: true })).toBeVisible();
  await expect(field(page, 'historyVersion')).toHaveText('9007199254740993');
  await expect(field(page, 'cache.minimumValidUntil')).toHaveText('2020-01-01T00:00:00Z');
  await expect(field(page, 'sizes.archiveSerializedBytes')).toHaveText('Unloaded / unknown');
  await expect(page.getByRole('button', { name: 'Next composition page', exact: true })).toBeDisabled();
  await expect(page.getByRole('listitem')).toHaveCount(1);
  await page.evaluate(() => (window as any).compositionFixture.setLanguage('ko'));
  await expect(page.getByRole('heading', { name: '현재 대화 컨텍스트 구성', exact: true })).toBeVisible();
  expect(await calls(page)).toEqual([]);
});

test('CC14 좁은 패널은 가로로 넘치지 않고 이름 있는 버튼을 키보드로 사용한다', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await fixture(page);
  await render(page, { ...snapshot, fragments: [fragment({ sourceId: '가'.repeat(85), taskId: 'x'.repeat(256) })] });
  const refresh = page.getByRole('button', { name: '구성 새로고침', exact: true });
  await refresh.focus(); await page.keyboard.press('Enter');
  expect(await calls(page)).toEqual([{ type: 'refresh' }]);
  await expect(refresh).toBeFocused();
  const dimensions = await page.locator('section.context-composition').evaluate(element => ({ width: element.clientWidth,
    scroll: element.scrollWidth, focus: getComputedStyle(element.querySelector('button')!).outlineStyle }));
  expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.width + 1);
  expect(dimensions.focus).not.toBe('none');
});

const requestSizes = {
  observationId: 'prepared-a', recordedAt: '2026-11-14T01:01:00Z', scope: 'prepared_sampling_prompt',
  provenance: 'utf8_json_components_before_provider_normalization', historyVersion: snapshot.historyVersion,
  contextIndexRevision: snapshot.contextIndexRevision, inputSerializedBytes: 10, baseInstructionsSerializedBytes: 20,
  toolDefinitionsSerializedBytes: 30, outputSchemaSerializedBytes: 40, componentPayloadBytes: 100,
};
const requestCountFields = ['inputSerializedBytes', 'baseInstructionsSerializedBytes', 'toolDefinitionsSerializedBytes',
  'outputSchemaSerializedBytes', 'componentPayloadBytes'];

test('CC15 준비 관측 legacy 누락과 null은 미확인이며 이전 byte를 0으로 채우지 않는다', async ({ page }) => {
  await fixture(page);
  for (const value of [snapshot, { ...snapshot, requestSizes: null }]) {
    expect(await render(page, { ...snapshot, requestSizes })).toBe(true);
    expect(await render(page, value)).toBe(true);
    await expect(page.getByRole('heading', { name: '직전 준비 요청 크기', exact: true })).toBeVisible();
    await expect(field(page, 'requestSizes.observation')).toHaveText('준비 관측 미확인');
    await expect(field(page, 'requestSizes.componentPayloadBytes')).toHaveCount(0);
    await expect(field(page, 'requestSizes.observationId')).toHaveCount(0);
  }
});

test('CC16 네 JSON component byte와 서버 합계를 기존 추정치·캐시와 분리하고 0을 보존한다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...snapshot, requestSizes })).toBe(true);
  for (const [name, value] of Object.entries({ inputSerializedBytes: '10 B', baseInstructionsSerializedBytes: '20 B',
    toolDefinitionsSerializedBytes: '30 B', outputSchemaSerializedBytes: '40 B', componentPayloadBytes: '100 B',
    recordedAt: requestSizes.recordedAt, observationId: 'prepared-a' })) await expect(field(page, `requestSizes.${name}`)).toHaveText(value);
  await expect(field(page, 'requestSizes.scope')).toHaveText('준비된 sampling prompt');
  await expect(field(page, 'requestSizes.provenance')).toHaveText('Provider 정규화 전 UTF-8 JSON component');
  await expect(field(page, 'sizes.residentSerializedBytes')).toHaveText('9000 B');
  await expect(field(page, 'sizes.selectedBodyEstimatedTokens')).toHaveText('321');
  await expect(field(page, 'cache.totalInputTokens')).toHaveText('20');
  await expect(page.getByText(/네 JSON component만의 byte 관측/)).toBeVisible();
  await expect(page.getByText(/provider 정규화·전송 봉투·delta 크기·과금 token/)).toBeVisible();
  const zero = { ...requestSizes, inputSerializedBytes: 0, baseInstructionsSerializedBytes: 0,
    toolDefinitionsSerializedBytes: 0, outputSchemaSerializedBytes: 0, componentPayloadBytes: 0 };
  expect(await render(page, { ...snapshot, requestSizes: zero })).toBe(true);
  for (const name of requestCountFields) await expect(field(page, `requestSizes.${name}`)).toHaveText('0 B');
  expect(await render(page, { ...snapshot, requestSizes: { ...zero, inputSerializedBytes: Number.MAX_SAFE_INTEGER,
    componentPayloadBytes: Number.MAX_SAFE_INTEGER } })).toBe(true);
  await expect(field(page, 'requestSizes.componentPayloadBytes')).toHaveText('9007199254740991 B');
  expect(await calls(page)).toEqual([]);
});

test('CC17 두 revision 문자열 모두 같을 때만 일치로 표시하고 대소·세대를 추론하지 않는다', async ({ page }) => {
  await fixture(page);
  expect(await render(page, { ...snapshot, requestSizes })).toBe(true);
  await expect(field(page, 'requestSizes.revisionMatch')).toHaveText('표시 snapshot과 revision 일치');
  await expect(field(page, 'requestSizes.historyVersion')).toHaveText('9007199254740993');
  await expect(field(page, 'requestSizes.contextIndexRevision')).toHaveText('18446744073709551615');
  for (const changes of [{ historyVersion: '9007199254740992' }, { historyVersion: '9007199254740994' },
    { contextIndexRevision: '0' }, { historyVersion: '0', contextIndexRevision: '0' }]) {
    expect(await render(page, { ...snapshot, requestSizes: { ...requestSizes, ...changes } })).toBe(true);
    await expect(field(page, 'requestSizes.revisionMatch')).toHaveText('다른 준비 revision / 현재 구성 일치 미확인');
    await expect(field(page, 'cache.revisionStatus')).toHaveText('이전 revision 관측');
  }
});

test('CC18 잘못된 준비 관측의 형식·정밀도·합계는 이전 관측과 목록을 지우고 거부한다', async ({ page }) => {
  await fixture(page);
  const invalid: unknown[] = [[], 'PRIVATE_RAW', 0,
    ...requestCountFields.flatMap(name => [-1, 0.5, Number.MAX_SAFE_INTEGER + 1, '10', null, false].map(value => ({ ...requestSizes, [name]: value }))),
    { ...requestSizes, componentPayloadBytes: 99 },
    // Individually safe components can exceed a safe total; never round or clamp their sum.
    { ...requestSizes, inputSerializedBytes: Number.MAX_SAFE_INTEGER, baseInstructionsSerializedBytes: 1,
      toolDefinitionsSerializedBytes: 1, outputSchemaSerializedBytes: 0, componentPayloadBytes: Number.MAX_SAFE_INTEGER },
    ...['01', '18446744073709551616', 1, null].flatMap(value => [
      { ...requestSizes, historyVersion: value }, { ...requestSizes, contextIndexRevision: value }]),
    { ...requestSizes, recordedAt: '2026-02-30T01:00:00Z' }, { ...requestSizes, recordedAt: null },
    ...['', 'x'.repeat(257), 'bad\nvalue', '가'.repeat(86)].map(observationId => ({ ...requestSizes, observationId })),
    { ...requestSizes, observationId: 1 }, { ...requestSizes, scope: null }, { ...requestSizes, provenance: 'x'.repeat(257) }];
  const missing = { ...requestSizes } as any; delete missing.outputSchemaSerializedBytes; invalid.push(missing);
  for (const value of invalid) {
    expect(await render(page, { ...snapshot, requestSizes })).toBe(true);
    expect(await render(page, { ...snapshot, requestSizes: value })).toBe(false);
    await expect(page.getByRole('status')).toHaveText(invalidMessage);
    await expect(page.getByRole('listitem')).toHaveCount(0);
    await expect(field(page, 'requestSizes.componentPayloadBytes')).toHaveCount(0);
    await expect(page.locator('body')).not.toContainText('PRIVATE_');
  }
});

test('CC19 미래 준비 출처는 미확인·HTML은 text이며 raw 비밀과 accessor는 비노출 거부한다', async ({ page }) => {
  await fixture(page);
  const html = '<img src=x onerror=alert(1)>';
  expect(await render(page, { ...snapshot, requestSizes: { ...requestSizes, observationId: html,
    scope: 'PRIVATE_SCOPE', provenance: 'PRIVATE_PROVENANCE' } })).toBe(true);
  await expect(field(page, 'requestSizes.observationId')).toHaveText(html);
  for (const name of ['scope', 'provenance']) await expect(field(page, `requestSizes.${name}`)).toHaveText('미확인');
  await expect(page.locator('img,script')).toHaveCount(0);
  await expect(page.locator('body')).not.toContainText('PRIVATE_');
  await page.evaluate(() => (window as any).compositionFixture.setLanguage('en'));
  await expect(field(page, 'requestSizes.provenance')).toHaveText('Unknown');
  await expect(page.locator('body')).not.toContainText('PRIVATE_');
  for (const name of ['body', 'authorization', 'input', 'schema']) {
    expect(await render(page, { ...snapshot, requestSizes: { ...requestSizes, [name]: 'PRIVATE_SECRET' } })).toBe(false);
    await expect(page.locator('body')).not.toContainText('PRIVATE_');
  }
  const accessor = await page.evaluate(({ snapshot, requestSizes }) => {
    const f = (window as any).compositionFixture;
    let touched = false;
    const nested = { ...requestSizes };
    Object.defineProperty(nested, 'scope', { enumerable: true, get() { touched = true; return 'PRIVATE_SECRET'; } });
    const nestedAccepted = f.panel.render({ ...snapshot, requestSizes: nested });
    Object.defineProperty(snapshot, 'requestSizes', { enumerable: true, get() { touched = true; return requestSizes; } });
    return { nestedAccepted, rootAccepted: f.panel.render(snapshot), touched };
  }, { snapshot, requestSizes });
  expect(accessor).toEqual({ nestedAccepted: false, rootAccepted: false, touched: false });
});

test('CC20 준비 관측은 언어·busy를 보존하고 페이지교체·stale·폐기 뒤 남지 않는다', async ({ page }) => {
  await fixture(page);
  const value = { ...snapshot, requestSizes };
  await render(page, value);
  await page.evaluate(() => { const f = (window as any).compositionFixture; f.panel.setDisabled(true); f.setLanguage('en'); });
  await expect(page.getByRole('heading', { name: 'Last prepared request sizes', exact: true })).toBeVisible();
  await expect(field(page, 'requestSizes.componentPayloadBytes')).toHaveText('100 B');
  await expect(field(page, 'requestSizes.revisionMatch')).toHaveText('Revisions match the displayed snapshot');
  await expect(page.getByRole('button', { name: 'Next composition page', exact: true })).toBeDisabled();
  await page.evaluate(() => { const f = (window as any).compositionFixture; for (const button of f.panel.element.querySelectorAll('button')) button.click(); });
  expect(await calls(page)).toEqual([]);
  expect(await render(page, { ...snapshot, snapshotId: 'page-b', requestSizes: null })).toBe(true);
  await expect(field(page, 'requestSizes.observation')).toHaveText('Prepared request observation unconfirmed');
  await expect(field(page, 'requestSizes.observationId')).toHaveCount(0);
  for (const state of ['pending', 'stale']) {
    await render(page, value);
    await page.evaluate(state => { const f = (window as any).compositionFixture;
      if (state === 'pending') f.panel.showPending(); else f.panel.showUnavailable('stale'); }, state);
    await expect(field(page, 'requestSizes.componentPayloadBytes')).toHaveCount(0);
  }
  await render(page, value);
  const disposed = await page.evaluate(value => {
    const f = (window as any).compositionFixture; const element = f.panel.element;
    const button = element.querySelector('button'); f.panel.dispose(); button.click();
    const accepted = f.panel.render(value); document.body.append(element); f.setLanguage('ko');
    return { accepted, text: element.textContent, children: element.childElementCount, calls: f.calls };
  }, value);
  expect(disposed).toEqual({ accepted: false, text: '', children: 0, calls: [] });
});
