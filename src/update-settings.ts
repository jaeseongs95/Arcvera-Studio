import { bindText, onLanguageChange, t } from './i18n';
import './update-settings.css';

type Target = 'desktop' | 'codex';
interface Snapshot {
  status: 'current' | 'available' | 'unconfigured' | 'unknown';
  currentVersion: string;
  latestVersion?: string;
  installSupported: boolean;
  reason?: string;
}
interface Options {
  host: HTMLElement;
  call<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}
const BACKGROUND_INTERVAL = 6 * 60 * 60 * 1000;

/** Checking never downloads. The native adapter owns provenance and installation. */
export function createUpdateSettings({ host, call }: Options) {
  host.classList.add('update-settings');
  host.innerHTML = '<h3 class="update-heading"></h3><label class="update-preference"><input type="checkbox" disabled><span></span></label><p class="update-help"></p><p class="update-error" role="alert" hidden></p><div class="update-targets"></div>';
  const toggle = host.querySelector<HTMLInputElement>('input')!;
  const preferenceLabel = host.querySelector<HTMLElement>('.update-preference span')!;
  const error = host.querySelector<HTMLElement>('.update-error')!;
  const targets = host.querySelector<HTMLElement>('.update-targets')!;
  const rows = new Map<Target, { status: HTMLElement; button: HTMLButtonElement }>();
  const snapshots = new Map<Target, Snapshot>();
  const checking = new Set<Target>();
  const declined = new Map<Target, string>();
  const offers = new Map<Target, { snapshot: Snapshot; background: boolean }>();
  let enabled = false, started = false, disposed = false, saving = false;
  let backgroundEpoch = 0, timer: ReturnType<typeof setTimeout> | undefined;
  let pending: { target: Target; version: string; background: boolean } | undefined;
  let installing = false;
  const prompt = document.createElement('dialog');
  prompt.className = 'update-confirmation';
  prompt.setAttribute('aria-labelledby', 'update-confirmation-title');
  prompt.innerHTML = '<h2 id="update-confirmation-title"></h2><p class="update-confirmation-message"></p><p class="update-install-result" role="status"></p><div><button class="update-no quiet-button" type="button"></button><button class="update-yes primary-button" type="button"></button></div>';
  document.body.append(prompt);
  const yes = prompt.querySelector<HTMLButtonElement>('.update-yes')!;
  const no = prompt.querySelector<HTMLButtonElement>('.update-no')!;
  const result = prompt.querySelector<HTMLElement>('.update-install-result')!;
  const name = (target: Target) => target === 'desktop' ? 'OpenDIY Desktop' : 'OpenDIY Codex';
  function message(snapshot?: Snapshot) {
    if (!snapshot) return t('아직 확인하지 않았습니다.', 'Not checked yet.');
    const versions = snapshot.latestVersion ? `${snapshot.currentVersion} → ${snapshot.latestVersion}` : snapshot.currentVersion;
    switch (snapshot.status) {
      case 'current': return `${versions} · ${t('최신 버전입니다.', 'Up to date.')}`;
      case 'available': return `${versions} · ${t('새 버전이 있습니다.', 'An update is available.')}${snapshot.installSupported ? '' : ` · ${snapshot.reason || t('앱 내 설치가 아직 구성되지 않았습니다.', 'In-app installation is not configured.')}`}`;
      case 'unconfigured': return `${versions} · ${snapshot.reason || t('업데이트가 구성되지 않았습니다.', 'Updates are not configured.')}`;
      default: return `${versions} · ${snapshot.reason || t('현재 버전을 비교하지 못했습니다.', 'Could not compare versions.')}`;
    }
  }
  function render() {
    bindText(host.querySelector<HTMLElement>('.update-heading')!, '업데이트', 'Updates');
    bindText(preferenceLabel, '백그라운드에서 업데이트 확인', 'Check for updates in the background');
    bindText(host.querySelector<HTMLElement>('.update-help')!, '새 버전이 있으면 먼저 묻습니다. 예를 선택하기 전에는 다운로드하거나 설치하지 않습니다. 꺼도 수동으로 확인할 수 있습니다.', 'Ask when an update is available. Nothing downloads or installs before you choose Yes. Manual checks remain available when this is off.');
    for (const [target, row] of rows) {
      row.status.textContent = checking.has(target) ? t('버전 확인 중…', 'Checking version…') : message(snapshots.get(target));
      bindText(row.button, '버전 확인', 'Check version');
      row.button.disabled = checking.has(target) || installing || disposed;
    }
    bindText(prompt.querySelector<HTMLElement>('h2')!, '업데이트', 'Update');
    bindText(no, '아니오', 'No'); bindText(yes, '예', 'Yes');
    if (pending) prompt.querySelector<HTMLElement>('.update-confirmation-message')!.textContent = t(`${name(pending.target)} ${pending.version} 새 버전이 있습니다. 업데이트 하시겠습니까?`, `${name(pending.target)} ${pending.version} is available. Would you like to update?`);
    no.disabled = installing;
    yes.disabled = installing || !pending || snapshots.get(pending.target)?.installSupported !== true;
  }
  for (const target of ['desktop', 'codex'] as const) {
    const row = document.createElement('section'); row.className = 'update-target'; row.dataset.target = target;
    const heading = document.createElement('strong'); heading.textContent = name(target);
    const status = document.createElement('p'); status.className = 'update-target-status'; status.setAttribute('role', 'status');
    const button = document.createElement('button'); button.type = 'button'; button.className = 'quiet-button';
    button.onclick = () => void check(target, false);
    row.append(heading, status, button); targets.append(row); rows.set(target, { status, button });
  }
  function offer(target: Target, snapshot: Snapshot, background: boolean) {
    if (disposed || installing || !snapshot.latestVersion) return;
    if (prompt.open) { offers.set(target, { snapshot, background }); return; }
    pending = { target, version: snapshot.latestVersion, background }; result.textContent = snapshot.installSupported ? '' : snapshot.reason || t('앱 내 설치가 아직 구성되지 않았습니다.', 'In-app installation is not configured.'); render(); prompt.showModal(); no.focus();
  }
  async function check(target: Target, background: boolean) {
    if (disposed || installing || checking.has(target) || (background && !enabled)) return;
    const epoch = backgroundEpoch; checking.add(target); render();
    try {
      const snapshot = await call<Snapshot>(`check_${target}_update`);
      if (disposed || (background && (!enabled || epoch !== backgroundEpoch))) return;
      if (!snapshot || !['current', 'available', 'unconfigured', 'unknown'].includes(snapshot.status) || typeof snapshot.currentVersion !== 'string' || typeof snapshot.installSupported !== 'boolean' || (snapshot.status === 'available' && (typeof snapshot.latestVersion !== 'string' || !snapshot.latestVersion))) throw new Error(t('업데이트 응답 형식을 확인하지 못했습니다.', 'Invalid update response.'));
      snapshots.set(target, snapshot);
      if (snapshot.status === 'available' && (!background || declined.get(target) !== snapshot.latestVersion)) offer(target, snapshot, background);
    } catch (failure) {
      if (!disposed && (!background || (enabled && epoch === backgroundEpoch))) snapshots.set(target, { status: 'unknown', currentVersion: snapshots.get(target)?.currentVersion ?? '—', installSupported: false, reason: t('업데이트 확인에 실패했습니다. 수동으로 다시 확인하세요.', 'Update check failed. Try a manual check.') });
    } finally { checking.delete(target); if (!disposed) render(); }
  }
  function schedule() {
    clearTimeout(timer);
    if (!disposed && enabled) timer = setTimeout(() => void background(), BACKGROUND_INTERVAL);
  }
  async function background() {
    for (const target of ['desktop', 'codex'] as const) { if (disposed || !enabled) break; await check(target, true); }
    schedule();
  }
  function decline() {
    if (installing) return;
    if (pending) declined.set(pending.target, pending.version);
    pending = undefined; prompt.close(); render();
    const next = offers.entries().next().value;
    if (next) { offers.delete(next[0]); if (!next[1].background || enabled) offer(next[0], next[1].snapshot, next[1].background); }
  }
  no.onclick = decline;
  prompt.addEventListener('cancel', event => { event.preventDefault(); decline(); });
  yes.onclick = async () => {
    if (!pending || installing || disposed || snapshots.get(pending.target)?.installSupported !== true) return;
    const selected = pending; installing = true; result.textContent = t('다운로드·서명 확인 후 업데이트합니다.', 'Downloading and verifying the update before installation.'); render();
    try {
      await call(`install_${selected.target}_update`, { confirmed: true, version: selected.version });
      pending = undefined; snapshots.delete(selected.target);
      if (!disposed) result.textContent = t('업데이트 처리 결과와 버전을 확인하세요.', 'Check the update operation result and version.');
    } catch {
      pending = undefined; snapshots.delete(selected.target);
      if (!disposed) result.textContent = t('업데이트를 완료하지 못했습니다. 현재 앱과 버전을 확인한 후 다시 시도하세요.', 'The update did not complete. Check the application and version before retrying.');
    } finally { installing = false; if (!disposed) render(); }
  };
  toggle.onchange = async () => {
    if (disposed || saving) return;
    const requested = toggle.checked; saving = true; toggle.disabled = true; error.hidden = true;
    try {
      const saved = await call<{ backgroundChecks: boolean }>('set_update_preferences', { backgroundChecks: requested });
      if (disposed) return;
      if (!saved || saved.backgroundChecks !== requested) throw new Error('Unconfirmed preference');
      enabled = saved.backgroundChecks; backgroundEpoch++; toggle.checked = enabled;
      if (!enabled) { clearTimeout(timer); for (const [target, offer] of offers) if (offer.background) offers.delete(target); if (pending?.background && !installing) decline(); }
      else void background();
    } catch {
      if (!disposed) { toggle.checked = enabled; error.hidden = false; error.textContent = t('업데이트 확인 설정을 저장하지 못했습니다.', 'Could not save the update preference.'); }
    } finally { saving = false; if (!disposed) toggle.disabled = false; }
  };
  const stopLanguage = onLanguageChange(render); render();
  return {
    async start() {
      if (started || disposed) return; started = true;
      try {
        const preferences = await call<{ backgroundChecks: boolean }>('get_update_preferences');
        if (disposed) return;
        if (!preferences || typeof preferences.backgroundChecks !== 'boolean') throw new Error('Invalid preference');
        enabled = preferences.backgroundChecks; toggle.checked = enabled; toggle.disabled = false;
        if (enabled) void background();
      } catch { if (!disposed) { error.hidden = false; error.textContent = t('업데이트 확인 설정을 읽지 못했습니다. 수동 확인은 사용할 수 있습니다.', 'Could not read the update preference. Manual checks remain available.'); } }
    },
    dispose() { disposed = true; backgroundEpoch++; clearTimeout(timer); offers.clear(); stopLanguage(); prompt.close(); prompt.remove(); },
  };
}
