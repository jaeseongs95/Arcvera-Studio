import { bindAttribute, bindText, onLanguageChange, t } from './i18n';

export interface RelayStatus {
  generation: number;
  revision: number;
  state: 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'error';
  url: string | null;
  error: string | null;
  releaseConfirmed: boolean | null;
  pendingCommands: number;
}
type Call = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export function createRelayControl(host: { call: Call }) {
  const button = document.createElement('button'); button.id = 'relay-open'; button.type = 'button'; button.className = 'side-link';
  button.setAttribute('aria-haspopup', 'dialog'); button.setAttribute('aria-controls', 'relay-dialog');
  bindText(button, 'OpenDIY Remote 연결', 'Connect OpenDIY Remote');
  const dialog = document.createElement('dialog'); dialog.id = 'relay-dialog'; dialog.setAttribute('aria-labelledby', 'relay-title');
  dialog.style.maxWidth = 'min(560px, calc(100vw - 32px))';
  dialog.innerHTML = `<div class="dialog-heading"><h2 id="relay-title"></h2><button type="button" class="icon-button" id="relay-close" autofocus>×</button></div>
    <p id="relay-help"></p><form id="relay-form"><label for="relay-url" id="relay-url-label"></label>
    <input id="relay-url" type="url" placeholder="https://relay.example.com" autocomplete="url" spellcheck="false" required maxlength="4096" style="width:100%;box-sizing:border-box">
    <label for="relay-secret" id="relay-secret-label"></label><input id="relay-secret" type="password" autocomplete="off" spellcheck="false" required maxlength="4096" style="width:100%;box-sizing:border-box">
    <p id="relay-secret-help"></p><p id="relay-status" role="status" aria-live="polite"></p><p id="relay-error" class="inline-error" role="alert" hidden></p>
    <p id="relay-pending" role="status" hidden></p><div class="dialog-actions"><button type="button" id="relay-refresh" class="quiet-button"></button>
    <button type="button" id="relay-disconnect" class="quiet-button"></button><button type="submit" id="relay-connect" class="primary-button"></button></div></form>`;
  document.body.append(dialog);
  const $ = <T extends HTMLElement>(selector: string) => dialog.querySelector<T>(selector)!;
  const url = $<HTMLInputElement>('#relay-url'), secret = $<HTMLInputElement>('#relay-secret');
  const connect = $<HTMLButtonElement>('#relay-connect'), disconnect = $<HTMLButtonElement>('#relay-disconnect'), refresh = $<HTMLButtonElement>('#relay-refresh');
  const statusText = $('#relay-status'), error = $('#relay-error'), pending = $('#relay-pending');
  let snapshot: RelayStatus | undefined, busy = false, query = 0;
  bindText($('#relay-title'), 'OpenDIY Remote 연결', 'Connect OpenDIY Remote');
  bindText($('#relay-help'), '개인 중계기에 이 Desktop을 연결합니다. 원격 브라우저에서 현재 프로젝트와 세션을 제어할 수 있습니다. 중계기는 별도로 준비해야 합니다.', 'Connect this Desktop to your personal relay to control its projects and sessions from a remote browser. Set up the relay separately.');
  bindText($('#relay-url-label'), '중계기 주소', 'Relay address');
  bindText($('#relay-secret-label'), '호스트 비밀키 (HostSecret)', 'Host secret (HostSecret)');
  bindText($('#relay-secret-help'), '호스트 비밀키는 중계기와 동일해야 합니다. 브라우저 로그인 토큰과 구분하세요. 이 앱은 비밀키를 저장하지 않으며 앱을 다시 실행하면 재입력이 필요합니다.', 'Use the relay’s host secret, which is separate from the browser login token. The app does not save it; enter it again after restarting.');
  bindAttribute($('#relay-close'), 'aria-label', '원격 연결 창 닫기', 'Close relay connection');
  bindText(connect, '연결', 'Connect'); bindText(disconnect, '연결 해제', 'Disconnect'); bindText(refresh, '상태 확인', 'Refresh status');
  const labels: Record<RelayStatus['state'], [string, string]> = {
    disconnected: ['연결 해제됨', 'Disconnected'], connecting: ['중계기 등록 대기 중', 'Waiting for relay registration'],
    connected: ['중계기 연결됨 · 원격 브라우저에서 사용할 수 있습니다', 'Connected to relay · ready for the remote browser'],
    reconnecting: ['연결 끊김 · 자동 재접속 중', 'Connection lost · reconnecting'], error: ['연결 오류 · 확인 후 연결 해제하고 다시 연결하세요', 'Connection error · disconnect before trying again'],
  };
  function render() {
    const active = !!snapshot && snapshot.state !== 'disconnected';
    dialog.setAttribute('aria-busy', String(busy));
    connect.disabled = busy || !snapshot || active; disconnect.disabled = busy || !active;
    refresh.disabled = busy; url.disabled = busy || active; secret.disabled = busy || active;
    statusText.textContent = snapshot ? t(...labels[snapshot.state]) : t('연결 상태 확인 필요', 'Connection status unknown');
    pending.hidden = !snapshot?.pendingCommands;
    pending.textContent = snapshot?.pendingCommands ? t(`이미 수락한 명령 ${snapshot.pendingCommands}개가 처리 중입니다. 연결을 해제해도 이 명령의 결과가 확정될 수 있습니다.`, `${snapshot.pendingCommands} accepted commands are still running and may complete after disconnection.`) : '';
  }
  function report(detail: string) { error.textContent = detail; error.hidden = false; }
  function accept(value: RelayStatus) {
    if (!value || !Number.isSafeInteger(value.generation) || !Number.isSafeInteger(value.revision) || !labels[value.state]) throw new Error(t('원격 연결 상태 응답이 올바르지 않습니다.', 'Invalid relay status response.'));
    if (snapshot && value.generation < snapshot.generation) return;
    if (snapshot && value.generation === snapshot.generation && value.revision < snapshot.revision) return;
    snapshot = value; error.hidden = !value.error; error.textContent = value.error ?? '';
    if (value.url && document.activeElement !== url) url.value = value.url;
    render();
  }
  async function read() {
    const version = ++query; busy = true; render();
    try { const value = await host.call<RelayStatus>('relay_status'); if (version === query) accept(value); }
    catch { if (version === query) report(t('연결 상태를 확인하지 못했습니다. 상태 확인을 다시 누르세요.', 'Could not read relay status. Refresh to check again.')); }
    finally { if (version === query) { busy = false; render(); } }
  }
  async function act(command: 'relay_connect' | 'relay_disconnect') {
    if (busy || !snapshot) return;
    if (command === 'relay_connect' && snapshot.state !== 'disconnected') return;
    const args = command === 'relay_connect' ? { url: url.value.trim(), secret: secret.value } : undefined;
    secret.value = ''; ++query; busy = true; error.hidden = true; render();
    try { accept(await host.call<RelayStatus>(command, args)); }
    catch {
      // An IPC error does not prove the connection action was not executed.
      snapshot = undefined;
      report(t('연결 요청 결과를 확인하지 못했습니다. 상태를 확인한 뒤 조작하세요.', 'Connection request outcome is unknown. Refresh status before taking another action.'));
    } finally { busy = false; render(); }
  }
  $('#relay-form').addEventListener('submit', event => { event.preventDefault(); if (!secret.value || !url.value) return; void act('relay_connect'); });
  $('#relay-form').addEventListener('keydown', event => { if (event instanceof KeyboardEvent && event.key === 'Enter' && event.isComposing) event.preventDefault(); });
  disconnect.onclick = () => void act('relay_disconnect'); refresh.onclick = () => void read();
  button.onclick = () => { dialog.showModal(); void read(); };
  $('#relay-close').onclick = () => dialog.close();
  dialog.addEventListener('close', () => { secret.value = ''; button.focus(); });
  onLanguageChange(render);
  return { button, accept };
}
