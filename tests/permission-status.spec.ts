import { expect, test, type Page } from '@playwright/test';

async function fixture(page: Page) {
  await page.goto('/');
  await page.evaluate(async () => {
    const state = window as any;
    const { createPermissionStatus } = await import('/src/permission-status.ts');
    const data = state.permissionFixture = {
      calls: [] as any[], failure: false, hold: false, release: () => {},
      response: {
        supported: true, status: 'known',
        permissions: { approvalPolicy: 'on-request', approvalsReviewer: 'user', sandboxPolicy: { type: 'workspaceWrite', networkAccess: false, writableRoots: ['D:/fixture'], excludeTmpdirEnvVar: true, excludeSlashTmp: false }, activePermissionProfile: { id: ':workspace', extends: null } },
        pendingApprovals: { status: 'known', requests: [] },
      } as any,
    };
    state.permissionStatus = createPermissionStatus({ async call(command: string, args: any) {
      data.calls.push({ command, args });
      const response = structuredClone(data.response);
      if (data.hold) { data.hold = false; await new Promise<void>(resolve => { data.release = resolve; }); }
      if (data.failure) throw Error('PRIVATE_TOKEN_PATH_NEVER_REAL');
      return response;
    } });
    state.openPermission = (id = 'session-A', generation = 7, title = '테스트 세션') => state.permissionStatus.open({ id, generation, title });
  });
}

test('PERM06 권한 팝업은 정확한 세션을 한 번 조회하고 읽기 전용으로 표시한다', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => (window as any).openPermission('session-A', 7, '<img src=x onerror=alert(1)>'));
  const dialog = page.getByRole('dialog', { name: '세션 권한 확인' });
  await expect(dialog).toContainText('이 세션의 다음 turn에 사용할 기본 설정입니다.');
  await expect(dialog).toContainText('샌드박스는 호환용 요약이며, 진행 중인 작업과 운영체제의 실제 권한 전체를 나타내지 않습니다.');
  await expect(dialog).toContainText('요청 시 승인');
  await expect(dialog).toContainText('작업 공간 쓰기');
  await expect(dialog).toContainText('D:/fixture');
  await expect(dialog).toContainText(':workspace');
  await expect(dialog).toContainText('<img src=x onerror=alert(1)>');
  await expect(dialog.locator('img, input, select, textarea')).toHaveCount(0);
  await expect(dialog.getByRole('button')).toHaveCount(2);
  await expect(dialog.getByRole('button', { name: '닫기', exact: true })).toBeFocused();
  expect(await page.evaluate(() => (window as any).permissionFixture.calls)).toEqual([{ command: 'conversation_request', args: { id: 'session-A', generation: 7, request: { type: 'permissionStatus' } } }]);
});

test('PERM07 미지원, 필드 미확인, 통신 실패를 구분하고 오류 원문을 숨긴다', async ({ page }) => {
  await fixture(page);
  const cases = [
    [{ supported: false, reason: 'methodUnsupported' }, '이 실행 환경은 권한 상태 조회를 지원하지 않습니다.'],
    [{ supported: false, reason: 'noAuthoritativeSettings' }, '이 실행 환경이 확인 가능한 권한 설정을 제공하지 않습니다.'],
    [{ supported: true, status: 'unknown', reason: 'invalidPermissionFields' }, '권한 필드의 형식을 확인할 수 없습니다.'],
  ] as const;
  for (const [response, message] of cases) {
    await page.evaluate(response => { (window as any).permissionFixture.response = response; }, response);
    await page.evaluate(() => (window as any).openPermission());
    await expect(page.getByRole('dialog', { name: '세션 권한 확인' })).toContainText(message);
    await expect(page.locator('.permission-fields')).toHaveCount(0);
  }
  await page.evaluate(() => { (window as any).permissionFixture.failure = true; });
  await page.evaluate(() => (window as any).openPermission());
  const dialog = page.getByRole('dialog', { name: '세션 권한 확인' });
  await expect(dialog).toContainText('권한 상태를 조회하지 못했습니다.');
  await expect(dialog).not.toContainText('PRIVATE_TOKEN_PATH_NEVER_REAL');
  expect(await page.evaluate(() => (window as any).permissionFixture.calls.length)).toBe(4);
});

test('PERM05 불완전 관측은 승인 요청 없음으로 확정하지 않는다', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => { (window as any).permissionFixture.response.pendingApprovals = { status: 'unknown', requests: [] }; });
  await page.evaluate(() => (window as any).openPermission());
  const dialog = page.getByRole('dialog', { name: '세션 권한 확인' });
  await expect(dialog).toContainText('현재 승인 요청 목록 미확인');
  await expect(dialog).not.toContainText('대기 중인 승인 요청 없음');
  await page.evaluate(() => { (window as any).permissionFixture.response.pendingApprovals = { status: 'known', requests: [{ requestId: 'req-1', method: 'item/permissions/requestApproval', metadataAvailable: true }] }; });
  await dialog.getByRole('button', { name: '새로고침', exact: true }).click();
  await expect(dialog).toContainText('관측된 승인 요청 1건');
  await expect(dialog).toContainText('추가 권한 · req-1');
});

test('PERM08 이전 세대의 늦은 응답이 새 세션을 덮지 않는다', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => { const s = window as any; s.permissionFixture.hold = true; void s.openPermission('session-A', 7, '이전 세션'); });
  await expect(page.getByRole('dialog')).toContainText('확인하고 있습니다.');
  await page.evaluate(() => { const s = window as any; s.permissionFixture.response.permissions.activePermissionProfile.id = 'new-profile'; void s.openPermission('session-B', 8, '새 세션'); });
  await expect(page.getByRole('dialog')).toContainText('새 세션');
  await expect(page.getByRole('dialog')).toContainText('확인하고 있습니다.');
  await page.evaluate(() => (window as any).permissionFixture.release());
  await expect(page.getByRole('dialog')).toContainText('새 세션');
  await expect(page.getByRole('dialog')).toContainText('new-profile');
  await expect(page.getByRole('dialog')).not.toContainText(':workspace');
});

test('PERM08 PERM09 닫기와 dispose 뒤 응답은 팝업을 되살리지 않는다', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => { const s = window as any; s.permissionFixture.hold = true; void s.openPermission(); });
  await page.keyboard.press('Escape');
  await expect(page.locator('.permission-status-dialog')).toHaveCount(0);
  await page.evaluate(() => (window as any).permissionFixture.release());
  await expect(page.locator('.permission-status-dialog')).toHaveCount(0);
  await page.evaluate(() => { const s = window as any; s.permissionFixture.hold = true; void s.openPermission(); });
  await page.evaluate(() => { const s = window as any; s.permissionStatus.dispose(); s.permissionFixture.release(); });
  await page.evaluate(() => (window as any).openPermission());
  await expect(page.locator('.permission-status-dialog')).toHaveCount(0);
});

test('PERM09 동일한 진행 중 조회는 합치고 명시 새로고침만 다시 조회한다', async ({ page }) => {
  await fixture(page);
  await page.evaluate(() => { const s = window as any; s.permissionFixture.hold = true; void s.openPermission(); });
  await page.evaluate(() => (window as any).openPermission());
  expect(await page.evaluate(() => (window as any).permissionFixture.calls.length)).toBe(1);
  await page.evaluate(() => (window as any).permissionFixture.release());
  const dialog = page.getByRole('dialog', { name: '세션 권한 확인' });
  await expect(dialog.getByRole('button', { name: '새로고침', exact: true })).toBeEnabled();
  await dialog.getByRole('button', { name: '새로고침', exact: true }).click();
  expect(await page.evaluate(() => (window as any).permissionFixture.calls.length)).toBe(2);
});
