import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';

const { version } = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));

test('AGPL notice and versioned source are keyboard accessible without automatic browser launch', async ({ page }) => {
  await page.addInitScript(() => {
    const state = window as any; state.licenseCalls = [];
    state.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command !== 'open_external_url') throw Error(`Unexpected command ${command}`);
      state.licenseCalls.push(args.url);
    } };
  });
  await page.goto('/?demo');
  await page.locator('#settings-open').click();
  await page.locator('#license-open').focus(); await page.keyboard.press('Enter');
  const dialog = page.locator('#license-dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('AGPL-3.0-or-later');
  await expect(dialog).toContainText('Copyright (c) 2026 Agent Grid contributors');
  await expect(dialog.locator('pre')).toContainText('GNU AFFERO GENERAL PUBLIC LICENSE');
  const release = `https://github.com/jaeseongs95/Arcvera-Studio/releases/tag/v${version}`;
  await expect(page.locator('#license-source')).toHaveAttribute('href', release);
  expect(await page.evaluate(() => (window as any).licenseCalls)).toEqual([]);
  await page.locator('#license-source').focus(); await page.keyboard.press('Enter');
  expect(await page.evaluate(() => (window as any).licenseCalls)).toEqual([release]);
  await page.keyboard.press('Escape'); await expect(dialog).not.toBeVisible();
  await expect(page.locator('#license-open')).toBeFocused();
});
