import { onLanguageChange, t } from './i18n';
import './permission-status.css';

interface Options {
  call<T>(command: string, args: Record<string, unknown>): Promise<T>;
  availabilityChanged?(id: string, generation: number): void;
}
export interface PermissionSession { id: string; generation: number; title: string }
interface Availability {
  id: string; generation: number; threadId: string; eligible: boolean;
  state: 'waiting' | 'pending' | 'supported' | 'unsupported' | 'unknown' | 'error';
  reason?: string;
}
type Json = Record<string, unknown>;
const object = (value: unknown): Json | undefined => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Json : undefined;
const text = (value: unknown) => typeof value === 'string' ? value : '';

/** A single on-demand snapshot. This controller never attaches or changes settings. */
export function createPermissionStatus(options: Options) {
  let disposed = false; let epoch = 0; let loading = false;
  let session: PermissionSession | undefined; let dialog: HTMLDialogElement | undefined;
  let snapshot: unknown; let failed = false;
  const availability = new Map<string, Availability>();
  let discovering = false;
  let connection: Promise<void> = Promise.resolve();

  // Discovery and explicit popup reads share one transport slot. Obsolete queued
  // requests are dropped before sending; an already sent read keeps its deadline.
  async function query(selected: Pick<PermissionSession, 'id' | 'generation'>, current: () => boolean) {
    const previous = connection; let release!: () => void;
    connection = new Promise<void>(resolve => { release = resolve; });
    await previous;
    try {
      if (!current()) return { sent: false };
      const value = await options.call<unknown>('conversation_request', { id: selected.id, generation: selected.generation, request: { type: 'permissionStatus' } });
      return { sent: true, value };
    } finally { release(); }
  }
  function acceptAvailability(entry: Availability, value: unknown) {
    const response = object(value);
    entry.state = response?.supported === false ? 'unsupported' : response?.supported === true && response.status === 'known' && object(response.permissions) ? 'supported' : 'unknown';
    entry.reason = text(response?.reason) || undefined;
  }
  function discover() {
    if (disposed || discovering) return;
    const entry = [...availability.values()].find(value => value.eligible && value.state === 'waiting');
    if (!entry) return;
    discovering = true; entry.state = 'pending';
    const current = () => !disposed && availability.get(entry.id) === entry;
    void query(entry, () => current() && entry.eligible).then(result => {
      if (!current()) return;
      if (!result.sent) { entry.state = 'waiting'; return; }
      // Hiding a card does not discard a valid same-generation reply. The host
      // keeps it hidden and can reuse this result when the card becomes visible.
      acceptAvailability(entry, result.value);
    }).catch(() => {
      if (current()) { entry.state = 'error'; entry.reason = 'transport'; }
    }).finally(() => {
      discovering = false;
      if (current()) options.availabilityChanged?.(entry.id, entry.generation);
      discover();
    });
  }

  function close() { epoch++; dialog?.close(); dialog?.remove(); dialog = undefined; session = undefined; snapshot = undefined; loading = false; }
  function paragraph(parent: HTMLElement, value: string, className?: string) {
    const element = document.createElement('p'); element.textContent = value;
    if (className) element.className = className;
    parent.append(element); return element;
  }
  function row(parent: HTMLElement, label: string, value: string) {
    const term = document.createElement('dt'); term.textContent = label;
    const detail = document.createElement('dd'); detail.textContent = value;
    parent.append(term, detail);
  }
  const unknown = () => t('미확인', 'Unknown');
  function approval(value: unknown) {
    const labels: Record<string, string> = { untrusted: t('신뢰되지 않은 명령 승인', 'Approve untrusted commands'), 'on-request': t('요청 시 승인', 'Approval on request'), never: t('승인 요청 안 함', 'Never request approval') };
    if (typeof value === 'string') return labels[value] ?? unknown();
    const granular = object(object(value)?.granular);
    if (!granular) return unknown();
    const labelsByKey: Record<string, string> = { sandbox_approval: t('샌드박스 승인', 'Sandbox approval'), rules: t('규칙', 'Rules'), skill_approval: t('스킬 승인', 'Skill approval'), request_permissions: t('권한 요청', 'Permission requests'), mcp_elicitations: t('MCP 질문', 'MCP elicitation') };
    return Object.entries(labelsByKey).filter(([key]) => typeof granular[key] === 'boolean').map(([key, label]) => `${label}: ${granular[key] ? t('켜짐', 'On') : t('꺼짐', 'Off')}`).join(' · ') || unknown();
  }
  function render() {
    if (!dialog || !session) return;
    const restoreFocus = dialog.contains(document.activeElement);
    dialog.replaceChildren();
    const heading = document.createElement('h2'); heading.id = 'permission-status-heading'; heading.textContent = t('세션 권한 확인', 'Session permissions'); dialog.append(heading);
    paragraph(dialog, session.title, 'permission-session-title');
    paragraph(dialog, t('이 세션의 다음 turn에 사용할 기본 설정입니다. 샌드박스는 호환용 요약이며, 진행 중인 작업과 운영체제의 실제 권한 전체를 나타내지 않습니다.', 'Default settings for subsequent turns in this session. The sandbox is a compatibility summary, not the full effective permissions of running actions or the operating system.'));
    const content = document.createElement('div'); content.className = 'permission-status-content'; content.setAttribute('role', 'status'); content.setAttribute('aria-live', 'polite'); dialog.append(content);
    if (loading) paragraph(content, t('권한 상태를 확인하고 있습니다.', 'Checking permission status.'));
    else if (failed) paragraph(content, t('권한 상태를 조회하지 못했습니다. 세션 연결을 확인한 뒤 다시 조회하세요.', 'Could not read permission status. Check the session connection and refresh.'));
    else {
      const value = object(snapshot);
      const permissions = object(value?.permissions);
      if (value?.supported === false) paragraph(content, value.reason === 'methodUnsupported'
        ? t('이 실행 환경은 권한 상태 조회를 지원하지 않습니다.', 'This runtime does not support permission status queries.')
        : t('이 실행 환경이 확인 가능한 권한 설정을 제공하지 않습니다.', 'This runtime does not provide confirmed permission settings.'));
      else if (value?.status !== 'known' || value.supported !== true || !permissions) paragraph(content, t('권한 필드의 형식을 확인할 수 없습니다. 권한 상태는 미확인입니다.', 'The permission fields could not be verified. Permission status is unknown.'));
      else {
        const fields = document.createElement('dl'); fields.className = 'permission-fields'; content.append(fields);
        row(fields, t('승인 정책', 'Approval policy'), approval(permissions.approvalPolicy));
        const reviewers: Record<string, string> = { user: t('사용자', 'User'), auto_review: t('자동 검토', 'Automatic review'), guardian_subagent: t('자동 검토', 'Automatic review') };
        row(fields, t('승인 검토 담당', 'Approval reviewer'), reviewers[text(permissions.approvalsReviewer)] ?? unknown());
        const sandbox = object(permissions.sandboxPolicy);
        const kinds: Record<string, string> = { dangerFullAccess: t('전체 접근', 'Full access'), readOnly: t('읽기 전용', 'Read only'), workspaceWrite: t('작업 공간 쓰기', 'Workspace write'), externalSandbox: t('외부 샌드박스', 'External sandbox') };
        row(fields, t('샌드박스', 'Sandbox'), kinds[text(sandbox?.type)] ?? unknown());
        if (sandbox?.type !== 'dangerFullAccess') {
          const access = sandbox?.networkAccess;
          row(fields, t('네트워크', 'Network'), access === true || access === 'enabled' ? t('허용', 'Enabled') : access === false || access === 'restricted' ? t('제한', 'Restricted') : unknown());
        }
        if (sandbox?.type === 'workspaceWrite') {
          const roots = Array.isArray(sandbox.writableRoots) ? sandbox.writableRoots.filter((value): value is string => typeof value === 'string') : [];
          row(fields, t('추가 쓰기 경로', 'Additional writable paths'), roots.length ? roots.join('\n') : t('추가 경로 없음', 'No additional paths'));
          for (const [key, ko, en] of [['excludeTmpdirEnvVar', 'TMPDIR 경로 제외', 'Exclude TMPDIR'], ['excludeSlashTmp', '/tmp 제외', 'Exclude /tmp']]) {
            row(fields, t(ko, en), typeof sandbox[key] === 'boolean' ? sandbox[key] ? t('켜짐', 'On') : t('꺼짐', 'Off') : unknown());
          }
        }
        const profile = object(permissions.activePermissionProfile);
        row(fields, t('권한 프로필', 'Permission profile'), text(profile?.id) || unknown());
        if (text(profile?.extends)) row(fields, t('상위 프로필', 'Parent profile'), text(profile?.extends));
      }
      const pending = object(value?.pendingApprovals);
      const requests = Array.isArray(pending?.requests) ? pending.requests : [];
      const known = pending?.status === 'known' && Array.isArray(pending.requests);
      paragraph(content, known ? requests.length ? t(`관측된 승인 요청 ${requests.length}건`, `${requests.length} observed approval requests`) : t('대기 중인 승인 요청 없음', 'No pending approval requests') : t('현재 승인 요청 목록 미확인', 'Current approval request list is unknown'), 'permission-pending-status');
      const methods: Record<string, string> = { 'item/commandExecution/requestApproval': t('명령 실행', 'Command execution'), 'item/fileChange/requestApproval': t('파일 변경', 'File change'), 'item/permissions/requestApproval': t('추가 권한', 'Additional permissions') };
      for (const entry of requests) {
        const request = object(entry); const kind = methods[text(request?.method)];
        if (kind && text(request?.requestId)) paragraph(content, `${kind} · ${text(request?.requestId)}${request?.metadataAvailable === true ? '' : ` · ${unknown()}`}`, 'permission-pending-request');
      }
    }
    const actions = document.createElement('div'); actions.className = 'permission-status-actions';
    const refresh = document.createElement('button'); refresh.type = 'button'; refresh.textContent = t('새로고침', 'Refresh'); refresh.disabled = loading; refresh.onclick = () => void read();
    const dismiss = document.createElement('button'); dismiss.type = 'button'; dismiss.className = 'permission-status-close'; dismiss.textContent = t('닫기', 'Close'); dismiss.onclick = close;
    actions.append(refresh, dismiss); dialog.append(actions);
    if (restoreFocus) dismiss.focus();
  }
  async function read() {
    if (disposed || !session || loading) return;
    const version = ++epoch; const selected = session; loading = true; failed = false; snapshot = undefined; render();
    try {
      const result = await query(selected, () => !disposed && version === epoch && !!dialog);
      if (disposed || version !== epoch || !dialog) return;
      if (!result.sent) return;
      snapshot = result.value;
      const entry = availability.get(selected.id);
      if (entry?.generation === selected.generation) { acceptAvailability(entry, result.value); options.availabilityChanged?.(entry.id, entry.generation); }
    } catch {
      if (disposed || version !== epoch || !dialog) return;
      failed = true;
      const entry = availability.get(selected.id);
      if (entry?.generation === selected.generation) { entry.state = 'error'; entry.reason = 'transport'; options.availabilityChanged?.(entry.id, entry.generation); }
    } finally {
      if (!disposed && version === epoch && dialog) { loading = false; render(); }
    }
  }
  const language = onLanguageChange(render);
  return {
    updateAvailability(selected: Pick<PermissionSession, 'id' | 'generation'> & { threadId?: string }, eligible: boolean) {
      if (disposed) return false;
      let entry = availability.get(selected.id);
      if (entry && (entry.generation !== selected.generation || entry.threadId !== selected.threadId)) { availability.delete(selected.id); entry = undefined; }
      if (!entry && eligible && selected.threadId) { entry = { id: selected.id, generation: selected.generation, threadId: selected.threadId, eligible, state: 'waiting' }; availability.set(selected.id, entry); }
      if (entry) entry.eligible = eligible;
      discover();
      return eligible && entry?.state === 'supported';
    },
    forget(id: string) { availability.delete(id); if (session?.id === id) close(); },
    async open(selected: PermissionSession) {
      if (disposed) return;
      if (dialog && session?.id === selected.id && session.generation === selected.generation && loading) { dialog.querySelector<HTMLButtonElement>('.permission-status-close')?.focus(); return; }
      close(); session = { ...selected }; failed = false;
      dialog = document.createElement('dialog'); dialog.className = 'permission-status-dialog'; dialog.setAttribute('aria-labelledby', 'permission-status-heading');
      dialog.addEventListener('cancel', event => { event.preventDefault(); close(); });
      document.body.append(dialog); render(); dialog.showModal();
      const request = read(); dialog.querySelector<HTMLButtonElement>('.permission-status-close')?.focus();
      await request;
    },
    dispose() { if (disposed) return; disposed = true; close(); availability.clear(); language(); },
  };
}
