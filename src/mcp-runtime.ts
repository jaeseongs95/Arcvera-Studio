import { onLanguageChange, t } from './i18n';

export interface McpRuntimeTarget {
  id: string;
  generation: number;
  threadId: string;
  title: string;
}

export interface McpRuntimePanelOptions {
  target?: McpRuntimeTarget;
  serverName: string;
  onReload: () => void;
  onInspect: (cursor?: string) => void;
}

export interface McpRuntimePanel {
  element: HTMLElement;
  render(value: unknown): boolean;
  showPending(): void;
  showUnconfirmed(): void;
  setDisabled(disabled: boolean): void;
  dispose(): void;
}

const runtimeValues = ['unknown', 'notStarted', 'starting', 'connected', 'authenticationRequired', 'failed', 'cancelled', 'disabled'] as const;
const authValues = ['unknown', 'unsupported', 'notLoggedIn', 'bearerToken', 'oAuth'] as const;
const toolsValues = ['available', 'empty', 'failed', 'unknown'] as const;
type RuntimeStatus = typeof runtimeValues[number];
type AuthStatus = typeof authValues[number];
type ToolsState = typeof toolsValues[number];
type Labels = [string, string];
const runtimeLabels: Record<RuntimeStatus, Labels> = {
  unknown: ['미확인', 'Unknown'], notStarted: ['시작 전', 'Not started'], starting: ['연결 중', 'Starting'],
  connected: ['연결됨', 'Connected'], authenticationRequired: ['인증 필요', 'Authentication required'],
  failed: ['연결 실패', 'Connection failed'], cancelled: ['취소됨', 'Cancelled'], disabled: ['비활성화됨', 'Disabled'],
};
const authLabels: Record<AuthStatus, Labels> = {
  unknown: ['미확인', 'Unknown'], unsupported: ['인증 상태 제공 안 됨', 'Authentication status unsupported'],
  notLoggedIn: ['로그인 안 됨', 'Not logged in'], bearerToken: ['Bearer 토큰', 'Bearer token'], oAuth: ['OAuth', 'OAuth'],
};
const toolsLabels: Record<ToolsState, Labels> = {
  available: ['조회된 도구 목록', 'Discovered tools'], empty: ['조회된 도구 없음', 'No tools discovered'],
  failed: ['도구 목록 조회 실패', 'Tool list failed'], unknown: ['도구 목록 미확인', 'Tool list unknown'],
};
interface StatusSnapshot {
  found: boolean;
  runtimeStatus: RuntimeStatus;
  authStatus: AuthStatus;
  toolsState: ToolsState;
  toolNames: string[];
  nextCursor: string | null;
}
type View = 'initial' | 'pending' | 'unconfirmed' | 'invalid' | 'queued' | 'status';
const encoder = new TextEncoder();

function identifier(value: unknown, maxBytes: number): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= maxBytes
    && value.trim().length > 0 && !/[\u0000-\u001f\u007f-\u009f]/u.test(value)
    && encoder.encode(value).byteLength <= maxBytes;
}
function record(value: unknown): value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return (prototype === Object.prototype || prototype === null) && Object.keys(value).length <= 512;
}
function enumValue<T extends string>(value: unknown, allowed: readonly T[]): value is T {
  return typeof value === 'string' && allowed.includes(value as T);
}
function snapshotTarget(value: unknown): McpRuntimeTarget | undefined {
  if (!record(value) || !identifier(value.id, 256) || !identifier(value.threadId, 256)
    || !identifier(value.title, 1024) || typeof value.generation !== 'number'
    || !Number.isSafeInteger(value.generation) || value.generation < 0) return undefined;
  return { id: value.id, generation: value.generation, threadId: value.threadId, title: value.title };
}

function statusSnapshot(value: Record<string, unknown>, threadId: string, serverName: string): StatusSnapshot | undefined {
  if (value.threadId !== threadId || value.serverName !== serverName || typeof value.found !== 'boolean'
    || !enumValue(value.runtimeStatus, runtimeValues) || !enumValue(value.authStatus, authValues)
    || !enumValue(value.toolsState, toolsValues) || !Array.isArray(value.toolNames) || value.toolNames.length > 256
    || (value.nextCursor !== null && !identifier(value.nextCursor, 256))) return undefined;
  const names: string[] = [];
  const seen = new Set<string>();
  for (const name of value.toolNames) {
    if (!identifier(name, 256) || seen.has(name)) return undefined;
    seen.add(name);
    names.push(name);
  }
  if (!value.found && (value.runtimeStatus !== 'unknown' || value.authStatus !== 'unknown'
    || value.toolsState !== 'unknown' || names.length !== 0)) return undefined;
  if ((value.toolsState === 'available' && names.length === 0)
    || ((value.toolsState === 'empty' || value.toolsState === 'failed') && names.length !== 0)) return undefined;
  return {
    found: value.found, runtimeStatus: value.runtimeStatus, authStatus: value.authStatus, toolsState: value.toolsState,
    // Core can supply names without confirming the tool list. Do not present those names as available.
    toolNames: value.toolsState === 'available' ? names : [], nextCursor: value.nextCursor,
  };
}

/** Pure current-thread result view. The caller owns review, requests and session/generation epochs. */
export function createMcpRuntimePanel(options: McpRuntimePanelOptions): McpRuntimePanel {
  const target = snapshotTarget(options.target);
  const serverName = identifier(options.serverName, 128) ? options.serverName : undefined;
  const hasTarget = target !== undefined && serverName !== undefined;
  let alive = true;
  let disabled = false;
  let view: View = 'initial';
  let result: StatusSnapshot | undefined;
  // ceiling: one validated page, at most 256 names; show 50 additional names per explicit click.
  let shown = 50;

  const element = document.createElement('section');
  element.className = 'mcp-runtime-panel';
  const heading = document.createElement('h3');
  const title = document.createElement('p');
  const server = document.createElement('p');
  const summary = document.createElement('p');
  summary.setAttribute('role', 'status');
  summary.setAttribute('aria-live', 'polite');
  summary.setAttribute('aria-atomic', 'true');
  const details = document.createElement('dl');
  function detail() {
    const label = document.createElement('dt');
    const value = document.createElement('dd');
    details.append(label, value);
    return { label, value };
  }
  const runtime = detail();
  const auth = detail();
  const tools = detail();
  const list = document.createElement('ul');
  const count = document.createElement('p');
  const actions = document.createElement('div');
  const reload = document.createElement('button');
  const inspect = document.createElement('button');
  const more = document.createElement('button');
  const next = document.createElement('button');
  for (const button of [reload, inspect, more, next]) {
    button.type = 'button';
    button.className = 'quiet-button';
  }
  if (hasTarget) actions.append(reload, inspect, more, next);
  element.append(heading, title, server, summary, actions, details, list, count);

  const removers: (() => void)[] = [];
  function listen(button: HTMLButtonElement, action: () => void) {
    const listener = () => { if (alive && hasTarget && !disabled && view !== 'pending') action(); };
    button.addEventListener('click', listener);
    removers.push(() => button.removeEventListener('click', listener));
  }
  listen(reload, () => options.onReload());
  listen(inspect, () => options.onInspect());
  listen(more, () => {
    if (view !== 'status' || !result || shown >= result.toolNames.length) return;
    shown = Math.min(shown + 50, result.toolNames.length);
    draw();
    if (more.hidden) (next.hidden ? inspect : next).focus();
  });
  listen(next, () => { if (view === 'status' && result?.nextCursor) options.onInspect(result.nextCursor); });

  function syncDisabled() {
    for (const button of [reload, inspect, more, next]) button.disabled = disabled || view === 'pending';
    element.setAttribute('aria-busy', view === 'pending' ? 'true' : 'false');
  }
  function draw() {
    if (!alive) return;
    heading.textContent = t('현재 대화 MCP', 'Current-chat MCP');
    element.setAttribute('aria-label', heading.textContent);
    title.hidden = !hasTarget;
    server.hidden = !hasTarget;
    title.textContent = hasTarget ? target!.title : '';
    server.textContent = hasTarget ? serverName! : '';
    reload.textContent = t('현재 대화 MCP 다시연결 검토', 'Review current-chat MCP reconnect');
    inspect.textContent = t('현재 대화 상태조회', 'Inspect current-chat status');
    more.textContent = t('도구 더 보기', 'Show more tools');
    next.textContent = t('다음 상태 페이지 조회', 'Inspect next status page');
    runtime.label.textContent = t('연결 상태', 'Runtime status');
    auth.label.textContent = t('인증 상태', 'Authentication status');
    tools.label.textContent = t('도구 목록', 'Tool list');
    list.setAttribute('aria-label', t('조회된 도구 이름', 'Discovered tool names'));
    const showingStatus = hasTarget && view === 'status' && result !== undefined;
    details.hidden = !showingStatus;
    runtime.value.textContent = showingStatus ? t(...runtimeLabels[result!.runtimeStatus]) : '';
    auth.value.textContent = showingStatus ? t(...authLabels[result!.authStatus]) : '';
    tools.value.textContent = showingStatus ? t(...toolsLabels[result!.toolsState]) : '';
    list.replaceChildren();
    const names = showingStatus ? result!.toolNames : [];
    for (const name of names.slice(0, shown)) {
      const item = document.createElement('li');
      item.textContent = name;
      list.append(item);
    }
    list.hidden = names.length === 0;
    count.hidden = names.length === 0;
    count.textContent = names.length ? t(`${Math.min(shown, names.length)} / ${names.length}개 표시`, `Showing ${Math.min(shown, names.length)} of ${names.length}`) : '';
    more.hidden = names.length <= shown;
    next.hidden = !showingStatus || result!.nextCursor === null;
    if (!hasTarget) summary.textContent = t('현재 대화를 선택한 뒤 MCP 상태를 확인해 주세요.', 'Select a current chat to inspect its MCP status.');
    else if (view === 'pending') summary.textContent = t('현재 대화 MCP 요청 처리 중…', 'Processing current-chat MCP request…');
    else if (view === 'queued') summary.textContent = t('재연결 요청이 접수되었습니다. 연결 완료는 상태조회로 확인해 주세요.', 'Reconnect request accepted. Inspect status to confirm the connection.');
    else if (view === 'unconfirmed') summary.textContent = t('현재 대화 MCP 상태 미확인. 상태를 다시 조회해 주세요.', 'Current-chat MCP status is unconfirmed. Inspect status again.');
    else if (view === 'invalid') summary.textContent = t('현재 대화 MCP 결과를 확인할 수 없습니다. 상태를 다시 조회해 주세요.', 'Cannot verify the current-chat MCP result. Inspect status again.');
    else if (showingStatus) summary.textContent = result!.found
      ? t('현재 대화의 서버 상태를 조회했습니다.', 'Server status inspected for the current chat.')
      : t('이 페이지에서 서버를 찾지 못했습니다. 전체 연결 상태는 미확인입니다.', 'Server not found on this page. Overall connection status is unknown.');
    else summary.textContent = t('현재 대화의 MCP 상태를 조회해 주세요.', 'Inspect MCP status for the current chat.');
    syncDisabled();
  }
  function clear(nextView: View) {
    result = undefined;
    shown = 50;
    view = nextView;
    draw();
  }
  const unsubscribe = onLanguageChange(draw);
  draw();

  return {
    element,
    render(value: unknown): boolean {
      if (!alive || !hasTarget) return false;
      let accepted: StatusSnapshot | 'queued' | 'unconfirmed' | undefined;
      try {
        if (record(value) && value.threadId === target!.threadId) {
          if (value.operation === 'mcpStatus') accepted = statusSnapshot(value, target!.threadId, serverName!);
          else if (value.operation === 'reloadMcp') {
            if (value.outcome === 'queued') accepted = 'queued';
            else if (value.outcome === 'unconfirmed' && (value.reason === 'ackMissing' || value.reason === 'targetMismatch')) accepted = 'unconfirmed';
          }
        }
      } catch {
        // Keep malformed client values out of DOM and discard previous successful state.
        accepted = undefined;
      }
      if (!accepted) { clear('invalid'); return false; }
      if (typeof accepted === 'string') clear(accepted);
      else { result = accepted; shown = 50; view = 'status'; draw(); }
      return true;
    },
    showPending() { if (alive && hasTarget) clear('pending'); },
    showUnconfirmed() { if (alive && hasTarget) clear('unconfirmed'); },
    setDisabled(value: boolean) { if (alive) { disabled = value; syncDisabled(); } },
    dispose() {
      if (!alive) return;
      alive = false;
      unsubscribe();
      removers.forEach(remove => remove());
      removers.length = 0;
      result = undefined;
      element.replaceChildren();
      element.remove();
    },
  };
}
