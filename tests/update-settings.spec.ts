import { test, expect, type Page } from '@playwright/test';

async function setup(page: Page, enabled = false) {
  await page.goto('/');
  await page.evaluate(async enabled => {
    document.body.innerHTML = '<section id="fixture"></section>';
    const state = window as any; state.updateCalls = []; state.preference = enabled;
    state.snapshots = {
      desktop: { status: 'available', currentVersion: '1.3.2', latestVersion: '1.4.0', installSupported: true },
      codex: { status: 'current', currentVersion: '0.167.0', latestVersion: '0.167.0', installSupported: false },
    };
    const { createUpdateSettings } = await import('/src/update-settings.ts');
    const options = { host: document.querySelector<HTMLElement>('#fixture')!, call: async (command: string, args: any) => {
      state.updateCalls.push({ command, args });
      if (command === 'get_update_preferences') return { backgroundChecks: state.preference };
      if (command === 'set_update_preferences') {
        if (state.saveFailure) throw new Error('fixture');
        state.preference = args.backgroundChecks; return { backgroundChecks: state.preference };
      }
      if (command.startsWith('check_')) {
        if (state.checkFailure) throw new Error('fixture');
        if (state.delayCheck) return new Promise(resolve => { state.resolveCheck = resolve; });
        return state.snapshots[command.includes('desktop') ? 'desktop' : 'codex'];
      }
      if (command.startsWith('install_')) { if (state.installFailure) throw typeof state.installFailure === 'string' ? state.installFailure : new Error('fixture'); return null; }
      throw new Error('Unexpected fixture command');
    }};
    state.mountUpdates = async () => { state.updates?.dispose(); state.updates = createUpdateSettings(options); await state.updates.start(); };
    await state.mountUpdates();
  }, enabled);
}

test('US01 OFF skips background checks while both manual checks remain available', async ({ page }) => {
  await setup(page);
  await expect(page.locator('input[type=checkbox]')).not.toBeChecked();
  await expect(page.getByRole('button', { name: '버전 확인' })).toHaveCount(2);
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('check_')))).toEqual([]);
  await page.locator('[data-target=codex] button').click();
  await expect(page.locator('[data-target=codex]')).toContainText('최신 버전');
});

test('US02 preference is confirmed, survives recreation, and a failed save reverts the UI', async ({ page }) => {
  await setup(page);
  await page.locator('input').check();
  await expect(page.locator('.update-confirmation')).toBeVisible();
  await page.getByRole('button', { name: '아니오', exact: true }).click();
  expect(await page.evaluate(() => (window as any).preference)).toBe(true);
  await page.evaluate(async () => { const state = window as any; state.snapshots.desktop.status = 'current'; await state.mountUpdates(); });
  await expect(page.locator('input')).toBeChecked();
  await page.evaluate(() => { (window as any).saveFailure = true; });
  await page.locator('input').click();
  await expect(page.locator('.update-error')).toContainText('저장하지 못했습니다');
  await expect(page.locator('input')).toBeChecked();
  await page.evaluate(() => { (window as any).saveFailure = false; });
  await page.locator('input').uncheck();
  await expect(page.locator('input')).not.toBeChecked();
  expect(await page.evaluate(() => (window as any).preference)).toBe(false);
});

test('US03/04 check and No never install; Yes sends the exact target and version once', async ({ page }) => {
  await setup(page);
  await page.locator('[data-target=desktop] button').click();
  await expect(page.locator('.update-confirmation')).toContainText('1.4.0');
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('install_')))).toEqual([]);
  await page.getByRole('button', { name: '아니오', exact: true }).click();
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('install_')))).toEqual([]);
  await page.locator('[data-target=desktop] button').click();
  await page.getByRole('button', { name: '예', exact: true }).click();
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('install_')))).toEqual([{ command: 'install_desktop_update', args: { confirmed: true, version: '1.4.0' } }]);
});

test('US05 failures and unconfigured installs are never reported as current or installed', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { (window as any).checkFailure = true; });
  await page.locator('[data-target=desktop] button').click();
  await expect(page.locator('[data-target=desktop]')).toContainText('확인에 실패');
  await expect(page.locator('.update-confirmation')).not.toBeVisible();
  await page.evaluate(() => { const state = window as any; state.checkFailure = false; state.snapshots.codex = { status: 'available', currentVersion: 'r37', latestVersion: 'r38', installSupported: false, reason: '설치 계약 미확인' }; });
  await page.locator('[data-target=codex] button').click();
  await expect(page.locator('.update-confirmation')).toContainText('설치 계약 미확인');
  await expect(page.getByRole('button', { name: '예', exact: true })).toBeDisabled();
});

test('US06 OFF discards a late background reply, then manual checks still work', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { (window as any).delayCheck = true; });
  await page.locator('input').check();
  await expect.poll(() => page.evaluate(() => typeof (window as any).resolveCheck)).toBe('function');
  await page.locator('input').uncheck();
  await page.evaluate(() => { const state = window as any; state.delayCheck = false; state.resolveCheck(state.snapshots.desktop); });
  await expect(page.locator('.update-confirmation')).not.toBeVisible();
  await expect(page.locator('[data-target=desktop] button')).toBeEnabled();
  await page.locator('[data-target=desktop] button').click();
  await expect(page.locator('.update-confirmation')).toBeVisible();
});

test('US08 simultaneous product offers are queued without installing either product', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { (window as any).snapshots.codex = { status: 'available', currentVersion: 'r37', latestVersion: 'r38', installSupported: true }; });
  await page.locator('input').check();
  await expect(page.locator('.update-confirmation')).toContainText('Arcvera-Studio');
  await expect.poll(() => page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('check_')).length)).toBe(2);
  await page.getByRole('button', { name: '아니오', exact: true }).click();
  await expect(page.locator('.update-confirmation')).toContainText('OpenDIY Codex');
  await page.getByRole('button', { name: '아니오', exact: true }).click();
  await expect(page.locator('.update-confirmation')).not.toBeVisible();
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('install_')))).toEqual([]);
});

test('US09 a failed install cannot retry without a fresh explicit check', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { (window as any).installFailure = '진행 중인 세션이 있어 업데이트를 보류했습니다.'; });
  await page.locator('[data-target=desktop] button').click();
  await page.getByRole('button', { name: '예', exact: true }).click();
  await expect(page.locator('.update-install-result')).toContainText('완료하지 못했습니다');
  await expect(page.locator('.update-install-result')).toContainText('진행 중인 세션이 있어 업데이트를 보류했습니다.');
  await expect(page.getByRole('button', { name: '예', exact: true })).toBeDisabled();
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('install_')).length)).toBe(1);
  await page.getByRole('button', { name: '아니오', exact: true }).click();
  await page.locator('[data-target=desktop] button').click();
  await page.evaluate(() => { (window as any).installFailure = false; });
  await page.getByRole('button', { name: '예', exact: true }).click();
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: any) => c.command.startsWith('install_')).length)).toBe(2);
});
