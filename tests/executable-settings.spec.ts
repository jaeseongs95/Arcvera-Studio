import { test, expect, type Page } from '@playwright/test';

const chosenPath = 'D:\\도구\\Codex CLI\\codex.exe';
const automaticPath = 'C:\\tools\\codex.exe';
const automaticServerPath = 'C:\\tools\\codex-app-server.exe';
const chosenServerPath = 'D:\\도구\\Codex CLI\\codex-app-server.exe';
const manualServerPath = 'D:\\서버\\custom-server.exe';
async function mockSettings(page: Page, options: { holdChoose?: boolean; holdServerChoose?: boolean; holdReplacement?: boolean; unavailable?: boolean; getFailure?: boolean } = {}) {
  await page.addInitScript(({ chosenPath, automaticPath, automaticServerPath, chosenServerPath, manualServerPath, options }) => {
    const state = window as any;
    state.isTauri = true;
    let callback = 0;
    state.settingsFixture = {
      calls: [], choices: 0, serverChoices: 0, releaseChoose: () => {}, releaseServerChoose: () => {}, serverOutcome: 'save', getFailure: options.getFailure,
      replacementOutcome: 'updated', replacementSupported: true, releaseReplacement: () => {},
      settings: options.unavailable ? { savedPath: null, resolvedPath: null, source: 'unavailable', error: 'Codex 실행 파일을 찾을 수 없습니다' }
        : { savedPath: null, resolvedPath: automaticPath, source: 'environment', error: null },
    };
    state.settingsFixture.settings.appServer = options.unavailable
      ? { savedPath: null, resolvedPath: null, source: 'unavailable', error: '앱 서버 실행 파일을 찾을 수 없습니다', kind: 'cli' }
      : { savedPath: null, resolvedPath: automaticServerPath, source: 'sibling', error: null, kind: 'standalone' };
    let owned = { id: 'live-owned', generation: 9, title: '열린 작업', cwd: 'D:\\fixture', threadId: '01900000-0000-7000-8000-000000000001', state: 'idle', active: true };
    let starts = 0;
    const automaticServer = () => ({ savedPath: null, resolvedPath: state.settingsFixture.settings.resolvedPath === chosenPath ? chosenServerPath : automaticServerPath, source: 'sibling', error: null, kind: 'standalone' });
    state.__TAURI_INTERNALS__ = {
      transformCallback: () => ++callback, unregisterCallback() {},
      async invoke(command: string, args: any = {}) {
        const fixture = state.settingsFixture;
        fixture.calls.push({ command, args });
        if (command.startsWith('plugin:event|')) return callback;
        if (command === 'get_ui_language') return 'ko';
        if (command === 'set_ui_language') return { show: args.language === 'en' ? 'Open OpenDIY Desktop' : 'OpenDIY Desktop 열기', exit: args.language === 'en' ? 'Exit…' : '종료…' };
        if (command === 'load_layout') return { version: 1, columns: 2, sessions: [] };
        if (command === 'list_projects') return [];
        if (command === 'list_sessions') return [structuredClone(owned)];
        if (command === 'session_suspend_idle') {
          if (args.id !== owned.id || args.generation !== owned.generation || !owned.active) throw Error('wrong owned connection');
          owned = { ...owned, active: false, state: 'exited' }; return null;
        }
        if (command === 'start_session') {
          if (owned.active || args.request.threadId !== owned.threadId) throw Error('duplicate or changed thread');
          owned = { ...owned, ...args.request, id: `reconnected-${++starts}`, generation: 1, state: 'idle', active: true }; return structuredClone(owned);
        }
        if (command === 'get_codex_settings') { if (fixture.getFailure) throw new Error('설정 파일을 읽을 수 없습니다'); return fixture.settings; }
        if (command === 'choose_codex_executable') {
          const choice = ++fixture.choices;
          if (options.holdChoose) await new Promise<void>(resolve => { fixture.releaseChoose = resolve; });
          else { if (choice === 1) return null; if (choice === 2) throw new Error('유효한 Windows 실행 파일이 아닙니다'); }
          const appServer = fixture.settings.appServer;
          fixture.settings = { savedPath: chosenPath, resolvedPath: chosenPath, source: 'saved', error: null, appServer };
          if (!appServer.savedPath) fixture.settings.appServer = automaticServer();
          return fixture.settings;
        }
        if (command === 'set_codex_executable') {
          if (args.path !== null) throw new Error('expected automatic reset');
          const appServer = fixture.settings.appServer;
          fixture.settings = { savedPath: null, resolvedPath: automaticPath, source: 'path', error: null, appServer };
          if (!appServer.savedPath) fixture.settings.appServer = automaticServer();
          return fixture.settings;
        }
        if (command === 'choose_app_server_executable') {
          fixture.serverChoices++;
          if (options.holdServerChoose) await new Promise<void>(resolve => { fixture.releaseServerChoose = resolve; });
          if (fixture.serverOutcome === 'cancel') return null;
          if (fixture.serverOutcome === 'reject') throw new Error('서버 설정 파일을 저장할 수 없습니다');
          fixture.settings = { ...fixture.settings, appServer: { savedPath: manualServerPath, resolvedPath: manualServerPath, source: 'saved', error: null, kind: 'standalone' } };
          return fixture.settings;
        }
        if (command === 'set_app_server_executable') {
          if (args.path !== null) throw new Error('expected automatic server reset');
          fixture.settings = { ...fixture.settings, appServer: automaticServer() };
          return fixture.settings;
        }
        if (command === 'runtime_transition_preview') return {
          pair: { cliVersion: '0.167.0', selectedServerExecutable: fixture.settings.appServer.resolvedPath, selectedServerVersion: '0.167.0', managedCliExecutable: automaticPath, managedVersion: '0.166.0', runningVersion: '0.166.0', socketPath: 'D:\\fixture.sock', state: 'mismatch', message: null },
          canApply: fixture.replacementSupported, fingerprint: 'selected-server', reason: fixture.replacementSupported ? null : '전체 패키지가 필요합니다. 기존 서버는 종료하지 않았습니다.', packageRoot: 'D:\\fixture',
        };
        if (command === 'apply_runtime_transition') {
          if (options.holdReplacement) await new Promise<void>(resolve => { fixture.releaseReplacement = resolve; });
          return { status: fixture.replacementOutcome, success: fixture.replacementOutcome === 'updated', outcome: fixture.replacementOutcome === 'updated' ? 'updated' : 'unknown', message: fixture.replacementOutcome === 'updated' ? null : '서버 종료 결과 미확인' };
        }
        return null;
      },
    };
  }, { chosenPath, automaticPath, automaticServerPath, chosenServerPath, manualServerPath, options });
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('aria-busy', 'false');
  await page.locator('#settings-open').click();
  await page.locator('#codex-settings-open').click();
}

test('executable picker cancel, invalid file, saved selection and automatic reset preserve the live session', async ({ page }, testInfo) => {
  await mockSettings(page);
  await expect(page.locator('#codex-executable-source')).toHaveText('환경변수');
  await expect(page.locator('#codex-executable-resolved')).toHaveText(automaticPath);
  await expect(page.locator('#codex-executable-path')).toHaveAttribute('readonly', '');
  await page.locator('#codex-executable-choose').click();
  await expect(page.locator('#codex-settings-status')).toContainText('취소');
  await expect(page.locator('#codex-executable-resolved')).toHaveText(automaticPath);
  await page.locator('#codex-executable-choose').click();
  await expect(page.locator('#codex-settings-error')).toHaveText('유효한 Windows 실행 파일이 아닙니다');
  await expect(page.locator('#codex-executable-path')).toHaveValue('');
  await expect(page.locator('#codex-executable-resolved')).toHaveText(automaticPath);
  await page.locator('#codex-executable-choose').click();
  await expect(page.locator('#codex-executable-path')).toHaveValue(chosenPath);
  await expect(page.locator('#codex-executable-resolved')).toHaveText(chosenPath);
  await expect(page.locator('#codex-executable-source')).toHaveText('직접 지정');
  await expect(page.locator('#codex-settings-error')).toBeHidden();
  await page.screenshot({ path: testInfo.outputPath('executable-settings.png'), fullPage: true });
  await page.locator('#codex-executable-reset').click();
  await expect(page.locator('#codex-executable-path')).toHaveValue('');
  await expect(page.locator('#codex-executable-source')).toHaveText('PATH 자동 찾기');
  const calls = await page.evaluate(() => (window as any).settingsFixture.calls);
  expect(calls.filter((call: any) => call.command === 'set_codex_executable')).toEqual([{ command: 'set_codex_executable', args: { path: null } }]);
  expect(calls.filter((call: any) => ['start_session', 'session_stop', 'complete_exit'].includes(call.command))).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(page.locator('#codex-settings-dialog')).toBeHidden();
  await expect(page.locator('.terminal-card')).toHaveCount(1);
  await expect(page.locator('.terminal-identity')).toHaveAttribute('title', '01900000-0000-7000-8000-000000000001');
});

test('reopening settings waits for the outstanding picker and reads its saved result without a second mutation', async ({ page }) => {
  await mockSettings(page, { holdChoose: true });
  await expect(page.locator('#codex-executable-choose')).toBeEnabled();
  await page.locator('#codex-executable-choose').click();
  await expect(page.locator('#codex-executable-choose')).toBeDisabled();
  await page.locator('#codex-settings-close').click();
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#codex-executable-choose')).toBeDisabled();
  expect(await page.evaluate(() => (window as any).settingsFixture.calls.filter((call: any) => call.command === 'get_codex_settings').length)).toBe(1);
  await page.evaluate(() => (window as any).settingsFixture.releaseChoose());
  await expect(page.locator('#codex-executable-choose')).toBeEnabled();
  await expect(page.locator('#codex-executable-path')).toHaveValue(chosenPath);
  await expect(page.locator('#codex-executable-source')).toHaveText('직접 지정');
  expect(await page.evaluate(() => (window as any).settingsFixture.choices)).toBe(1);
  await page.locator('#codex-settings-close').click();
  await page.evaluate(() => { (window as any).settingsFixture.getFailure = true; });
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#codex-settings-error')).toContainText('설정 파일을 읽을 수 없습니다');
  await expect(page.locator('#codex-executable-path')).toHaveValue('');
  await expect(page.locator('#codex-executable-resolved')).toHaveText('확인하지 못했습니다');
});

for (const options of [{ unavailable: true }, { getFailure: true }]) {
  test(`settings reports ${options.unavailable ? 'missing executable' : 'read failure'} and allows recovery through the native chooser`, async ({ page }) => {
    await mockSettings(page, { ...options, holdChoose: true });
    await expect(page.locator('#codex-settings-error')).toContainText(options.unavailable ? '실행 파일을 찾을 수 없습니다' : '설정 파일을 읽을 수 없습니다');
    await expect(page.locator('#codex-executable-choose')).toBeEnabled();
    await page.locator('#codex-executable-choose').click();
    await page.evaluate(() => (window as any).settingsFixture.releaseChoose());
    await expect(page.locator('#codex-executable-resolved')).toHaveText(chosenPath);
    await expect(page.locator('#codex-settings-error')).toBeHidden();
  });
}


test('앱 서버 자동짝 선택과 수동 선택·취소·저장 실패·자동 복원을 구분한다', async ({ page }, testInfo) => {
  await mockSettings(page);
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(automaticServerPath);
  await expect(page.locator('#app-server-executable-source')).toHaveText('CLI와 같은 폴더의 짝 서버');
  await expect(page.locator('#app-server-executable-path')).toHaveAttribute('readonly', '');
  await page.evaluate(() => { (window as any).settingsFixture.serverOutcome = 'cancel'; });
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#codex-settings-status')).toContainText('취소');
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(automaticServerPath);
  await page.evaluate(() => { (window as any).settingsFixture.serverOutcome = 'reject'; });
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#codex-settings-error')).toHaveText('서버 설정 파일을 저장할 수 없습니다');
  await expect(page.locator('#app-server-executable-path')).toHaveValue('');
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(automaticServerPath);
  await page.evaluate(() => { (window as any).settingsFixture.serverOutcome = 'save'; });
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#app-server-executable-path')).toHaveValue(manualServerPath);
  await expect(page.locator('#app-server-executable-source')).toHaveText('직접 지정');
  await expect(page.locator('#codex-settings-error')).toBeHidden();
  await page.locator('#codex-executable-reset').click();
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(manualServerPath);
  await page.locator('#app-server-executable-reset').click();
  await expect(page.locator('#app-server-executable-path')).toHaveValue('');
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(automaticServerPath);
  await expect(page.locator('#codex-settings-scope')).toContainText('기존 공유 서버를 종료');
  await page.screenshot({ path: testInfo.outputPath('runtime-pair-settings-ko.png'), fullPage: true });
  await page.setViewportSize({ width: 800, height: 540 });
  await page.locator('#app-server-executable-choose').scrollIntoViewIfNeeded();
  const geometry = await page.locator('#codex-settings-dialog').evaluate(node => ({ width: node.clientWidth, scrollWidth: node.scrollWidth }));
  expect(geometry.scrollWidth).toBe(geometry.width);
  await expect(page.locator('#app-server-executable-choose')).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath('runtime-pair-settings-minimum.png'), fullPage: true });
  const calls = await page.evaluate(() => (window as any).settingsFixture.calls);
  expect(calls.filter((call: any) => call.command === 'set_app_server_executable')).toEqual([{ command: 'set_app_server_executable', args: { path: null } }]);
  expect(calls.filter((call: any) => call.command === 'session_suspend_idle')).toHaveLength(2);
  expect(calls.filter((call: any) => call.command === 'start_session')).toHaveLength(2);
  expect(calls.filter((call: any) => ['session_stop', 'complete_exit'].includes(call.command))).toEqual([]);
});

test('새 서버 선택은 별도 버전 맞추기 없이 교체하며 같은 경로의 명시 재선택도 적용한다', async ({ page }) => {
  await mockSettings(page);
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#codex-settings-status')).toContainText('서버 교체');
  let calls = await page.evaluate(() => (window as any).settingsFixture.calls);
  expect(calls.filter((item: any) => item.command === 'runtime_transition_preview')).toEqual([{ command: 'runtime_transition_preview', args: { replaceExisting: true } }]);
  expect(calls.filter((item: any) => item.command === 'apply_runtime_transition')).toEqual([{ command: 'apply_runtime_transition', args: { confirmed: true, fingerprint: 'selected-server', replaceExisting: true } }]);
  await page.locator('#app-server-executable-choose').click();
  calls = await page.evaluate(() => (window as any).settingsFixture.calls);
  expect(calls.filter((item: any) => item.command === 'apply_runtime_transition')).toHaveLength(2);
});

test('서버 교체 결과 불명은 저장 성공으로 표시하거나 자동 재시도하지 않는다', async ({ page }) => {
  await mockSettings(page);
  await page.evaluate(() => { (window as any).settingsFixture.replacementOutcome = 'shutdownOutcomeUnknown'; });
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#codex-settings-error')).toContainText('서버 종료 결과 미확인');
  await expect(page.locator('#codex-settings-status')).not.toContainText('저장되었습니다');
  expect(await page.evaluate(() => (window as any).settingsFixture.calls.filter((item: any) => item.command === 'apply_runtime_transition').length)).toBe(1);
});

test('서버 선택 취소는 교체 명령을 실행하지 않는다', async ({ page }) => {
  await mockSettings(page);
  await page.evaluate(() => { (window as any).settingsFixture.serverOutcome = 'cancel'; });
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#codex-settings-status')).toContainText('취소');
  expect(await page.evaluate(() => (window as any).settingsFixture.calls.filter((item: any) => item.command === 'apply_runtime_transition').length)).toBe(0);
});

test('교체 진행 중 닫기·재열기와 중복 클릭은 서버 교체를 다시 실행하지 않는다', async ({ page }) => {
  await mockSettings(page, { holdReplacement: true });
  await page.locator('#app-server-executable-choose').click();
  await expect.poll(() => page.evaluate(() => (window as any).settingsFixture.calls.filter((item: any) => item.command === 'apply_runtime_transition').length)).toBe(1);
  await expect(page.locator('#app-server-executable-choose')).toBeDisabled();
  await page.locator('#app-server-executable-choose').evaluate(button => button.dispatchEvent(new MouseEvent('click', { bubbles: true })));
  await page.locator('#codex-settings-close').click();
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#app-server-executable-choose')).toBeDisabled();
  await page.evaluate(() => (window as any).settingsFixture.releaseReplacement());
  await expect(page.locator('#app-server-executable-choose')).toBeEnabled();
  await expect(page.locator('#app-server-executable-path')).toHaveValue(manualServerPath);
  expect(await page.evaluate(() => (window as any).settingsFixture.calls.filter((item: any) => item.command === 'apply_runtime_transition').length)).toBe(1);
});

test('전체 패키지를 확인하지 못하면 종료 요청 없이 선택한 경로와 원인을 표시한다', async ({ page }) => {
  await mockSettings(page);
  await page.evaluate(() => { (window as any).settingsFixture.replacementSupported = false; });
  await page.locator('#app-server-executable-choose').click();
  await expect(page.locator('#app-server-executable-path')).toHaveValue(manualServerPath);
  await expect(page.locator('#codex-settings-error')).toContainText('기존 서버는 종료하지 않았습니다');
  expect(await page.evaluate(() => (window as any).settingsFixture.calls.filter((item: any) => item.command === 'apply_runtime_transition').length)).toBe(0);
});

test('CLI를 바꾸면 자동 서버 경로도 갱신하고 CLI fallback과 서버 오류를 표시한다', async ({ page }) => {
  await mockSettings(page, { holdChoose: true });
  await page.locator('#codex-executable-choose').click();
  for (const id of ['codex-executable-choose', 'codex-executable-reset', 'app-server-executable-choose', 'app-server-executable-reset']) await expect(page.locator(`#${id}`)).toBeDisabled();
  await page.evaluate(() => (window as any).settingsFixture.releaseChoose());
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(chosenServerPath);
  await page.locator('#codex-settings-close').click();
  await page.evaluate(() => {
    const settings = (window as any).settingsFixture.settings;
    settings.appServer = { savedPath: null, resolvedPath: settings.resolvedPath, source: 'cli', error: null, kind: 'cli' };
  });
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#app-server-executable-resolved')).toHaveText(chosenPath);
  await expect(page.locator('#app-server-executable-source')).toHaveText('선택한 CLI의 app-server 명령');
  await expect(page.locator('#app-server-executable-kind')).toHaveText('CLI app-server 명령');
  await page.locator('#codex-settings-close').click();
  await page.evaluate(manualServerPath => {
    (window as any).settingsFixture.settings.appServer = { savedPath: manualServerPath, resolvedPath: null, source: 'unavailable', error: '지정한 서버를 사용할 수 없습니다', kind: 'standalone' };
  }, manualServerPath);
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#codex-executable-resolved')).toHaveText(chosenPath);
  await expect(page.locator('#app-server-executable-path')).toHaveValue(manualServerPath);
  await expect(page.locator('#codex-settings-error')).toHaveText('지정한 서버를 사용할 수 없습니다');
});

test('서버 선택 중 닫기·재열기와 언어 변경이 중복 변경이나 오래된 응답을 만들지 않는다', async ({ page }, testInfo) => {
  await mockSettings(page, { holdServerChoose: true });
  await page.locator('#app-server-executable-choose').click();
  for (const id of ['codex-executable-choose', 'codex-executable-reset', 'app-server-executable-choose', 'app-server-executable-reset']) await expect(page.locator(`#${id}`)).toBeDisabled();
  await page.locator('#codex-settings-close').click();
  await page.locator('#language-select').selectOption('en');
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#codex-settings-status')).toHaveText('Checking current settings…');
  await expect(page.locator('#app-server-executable-choose')).toHaveText('Choose server executable…');
  await expect(page.locator('#app-server-executable-choose')).toBeDisabled();
  expect(await page.evaluate(() => (window as any).settingsFixture.calls.filter((call: any) => call.command === 'get_codex_settings').length)).toBe(1);
  await page.evaluate(() => (window as any).settingsFixture.releaseServerChoose());
  await expect(page.locator('#app-server-executable-choose')).toBeEnabled();
  await expect(page.locator('#app-server-executable-path')).toHaveValue(manualServerPath);
  await expect(page.locator('#app-server-executable-source')).toHaveText('Selected file');
  await expect(page.locator('#app-server-executable-kind')).toHaveText('Standalone server executable');
  expect(await page.evaluate(() => (window as any).settingsFixture.serverChoices)).toBe(1);
  await page.screenshot({ path: testInfo.outputPath('runtime-pair-settings-en.png'), fullPage: true });
  await page.locator('#codex-settings-close').click();
  await page.evaluate(() => { (window as any).settingsFixture.getFailure = true; });
  await page.locator('#codex-settings-open').click();
  await expect(page.locator('#app-server-executable-path')).toHaveValue('');
  await expect(page.locator('#app-server-executable-resolved')).toHaveText('Unknown');
});
