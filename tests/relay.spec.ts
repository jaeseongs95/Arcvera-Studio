import { test, expect, type Page } from '@playwright/test';

async function workspace(page: Page) {
  await page.addInitScript(() => {
    const w = window as any; w.isTauri = true;
    let callback = 0; const callbacks = new Map<number, Function>(); const listeners: Record<string, number> = {};
    w.relayFixture = { calls: [], hold: false, failure: false, finish: () => {},
      status: { generation: 0, revision: 0, state: 'disconnected', url: null, error: null, releaseConfirmed: null, pendingCommands: 0 },
      emit(status: any) { this.status = status; const handler = callbacks.get(listeners['relay-status']); handler?.({ event: 'relay-status', payload: status }); },
    };
    w.__TAURI_INTERNALS__ = {
      transformCallback(fn: Function) { const id = ++callback; callbacks.set(id, fn); return id; }, unregisterCallback(id: number) { callbacks.delete(id); },
      async invoke(command: string, args: any = {}) {
        const f = w.relayFixture;
        // Retain command names only. No secret is stored in the fixture's call log.
        f.calls.push(command);
        if (command === 'plugin:event|listen') { listeners[args.event] = args.handler; return ++callback; }
        if (command.startsWith('plugin:event|')) return null;
        if (command === 'get_ui_language') return 'ko';
        if (command === 'set_ui_language') return { show: 'Arcvera-Studio 열기', exit: '종료…' };
        if (command === 'load_layout') return { version: 1, columns: 2, sessions: [] };
        if (['list_projects', 'list_sessions', 'list_external'].includes(command)) return [];
        if (command === 'get_codex_settings') return { savedPath: null, resolvedPath: 'D:\\fixture\\codex.exe', source: 'path', error: null };
        if (command === 'relay_status') return { ...f.status };
        if (command === 'relay_connect') {
          if (f.hold) await new Promise(resolve => { f.finish = resolve; });
          if (f.failure) throw Error('IPC error with forbidden-fixture-secret');
          f.status = { ...f.status, generation: f.status.generation + 1, revision: 0, state: 'connecting', url: args.url, error: null };
          return { ...f.status };
        }
        if (command === 'relay_disconnect') { f.status = { ...f.status, revision: f.status.revision + 1, state: 'disconnected', releaseConfirmed: false, error: '로컬 연결은 닫혔지만 중계기의 연결 해제 응답을 확인하지 못했습니다.' }; return { ...f.status }; }
        return null;
      },
    };
  });
  await page.goto('/'); await expect(page.locator('#app')).toHaveAttribute('aria-busy', 'false');
  await page.locator('#settings-open').click(); await page.locator('#relay-open').click();
  await expect(page.locator('#relay-dialog')).toHaveAttribute('aria-busy', 'false');
}
async function connect(page: Page) {
  await page.locator('#relay-url').fill('https://relay.example.com');
  await page.locator('#relay-secret').fill('forbidden-fixture-secret');
  await page.locator('#relay-connect').click();
}
test('등록 전 연결 성공을 표시하지 않고 비밀을 지우며 실제 상태 이벤트와 해제 미확인을 표시한다', async ({ page }) => {
  await workspace(page); await connect(page);
  await expect(page.locator('#relay-status')).toHaveText('중계기 등록 대기 중');
  await expect(page.locator('#relay-secret')).toHaveValue('');
  await expect(page.locator('#relay-connect')).toBeDisabled();
  await page.evaluate(() => { const f = (window as any).relayFixture; f.emit({ ...f.status, revision: 1, state: 'connected' }); });
  await expect(page.locator('#relay-status')).toContainText('중계기 연결됨');
  await page.evaluate(() => { const f = (window as any).relayFixture; f.emit({ ...f.status, revision: 0, state: 'connecting' }); });
  await expect(page.locator('#relay-status')).toContainText('중계기 연결됨');
  await page.locator('#relay-disconnect').click();
  await expect(page.locator('#relay-status')).toHaveText('연결 해제됨');
  await expect(page.locator('#relay-error')).toContainText('응답을 확인하지 못했습니다');
  await expect(page.locator('#relay-connect')).toBeEnabled();
  expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain('forbidden-fixture-secret');
});
test('중복 연결과 응답 불명 후 자동 재실행을 막고 상태 조회 후에만 조작한다', async ({ page }) => {
  await workspace(page); await page.evaluate(() => { const f = (window as any).relayFixture; f.hold = true; f.failure = true; });
  await connect(page); await expect(page.locator('#relay-dialog')).toHaveAttribute('aria-busy', 'true');
  await expect(page.locator('#relay-connect')).toBeDisabled();
  await page.evaluate(() => (window as any).relayFixture.finish());
  await expect(page.locator('#relay-error')).toContainText('결과를 확인하지 못했습니다');
  await expect(page.locator('#relay-error')).not.toContainText('forbidden-fixture-secret');
  await expect(page.locator('#relay-connect')).toBeDisabled();
  expect(await page.evaluate(() => (window as any).relayFixture.calls.filter((x: string) => x === 'relay_connect').length)).toBe(1);
  await page.locator('#relay-refresh').click(); await expect(page.locator('#relay-connect')).toBeEnabled();
});
test('IME Enter는 연결하지 않고 Escape는 비밀을 지우며 초점을 돌려준다', async ({ page }) => {
  await workspace(page); await page.locator('#relay-url').fill('https://relay.example.com'); await page.locator('#relay-secret').fill('forbidden-fixture-secret');
  await page.locator('#relay-secret').dispatchEvent('keydown', { key: 'Enter', isComposing: true });
  expect(await page.evaluate(() => (window as any).relayFixture.calls.filter((x: string) => x === 'relay_connect').length)).toBe(0);
  await page.keyboard.press('Escape'); await expect(page.locator('#relay-dialog')).not.toBeVisible(); await expect(page.locator('#relay-open')).toBeFocused();
  await page.locator('#relay-open').click(); await expect(page.locator('#relay-secret')).toHaveValue('');
});
test('좁은 창에서 주소·비밀·연결 해제와 오류 메시지를 볼 수 있다', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 }); await workspace(page);
  await page.locator('#relay-url').fill('https://relay.example.com');
  await expect(page.locator('#relay-connect')).toBeInViewport(); await expect(page.locator('#relay-disconnect')).toBeInViewport();
  await page.screenshot({ path: 'verification/remote-integration/relay-dialog-800.png' });
});
