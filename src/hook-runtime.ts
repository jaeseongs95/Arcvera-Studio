import { onLanguageChange, t } from './i18n';

export interface HookRuntimeTarget {
  id: string;
  generation: number;
  threadId: string;
  title: string;
}
export interface HookRuntimePanelOptions {
  target?: HookRuntimeTarget;
  onInspect: () => void;
}
export interface HookRuntimePanel {
  element: HTMLElement;
  render(value: unknown): boolean;
  showPending(): void;
  showUnconfirmed(): void;
  setDisabled(disabled: boolean): void;
  dispose(): void;
}

const eventValues = ['preModelRequest', 'postModelRequest', 'preToolUse', 'permissionRequest', 'postToolUse',
  'preCompact', 'postCompact', 'sessionStart', 'sessionEnd', 'userPromptSubmit', 'subagentStart', 'subagentStop', 'stop', 'interrupt'] as const;
const handlerValues = ['command', 'mcpTool', 'prompt', 'agent'] as const;
const sourceValues = ['system', 'user', 'project', 'mdm', 'sessionFlags', 'plugin', 'cloudRequirements',
  'cloudManagedConfig', 'legacyManagedConfigFile', 'legacyManagedConfigMdm', 'unknown'] as const;
const trustValues = ['managed', 'untrusted', 'trusted', 'modified'] as const;
type EventName = typeof eventValues[number];
type HandlerType = typeof handlerValues[number];
type HookSource = typeof sourceValues[number];
type TrustStatus = typeof trustValues[number];
type Labels = [string, string];
const eventLabels: Record<EventName, Labels> = {
  preModelRequest: ['모델 요청 전', 'Before model request'], postModelRequest: ['모델 요청 후', 'After model request'],
  preToolUse: ['도구 사용 전', 'Before tool use'], permissionRequest: ['권한 요청', 'Permission request'],
  postToolUse: ['도구 사용 후', 'After tool use'], preCompact: ['압축 전', 'Before compaction'], postCompact: ['압축 후', 'After compaction'],
  sessionStart: ['세션 시작', 'Session start'], sessionEnd: ['세션 종료', 'Session end'],
  userPromptSubmit: ['사용자 입력 제출', 'User prompt submitted'], subagentStart: ['하위 에이전트 시작', 'Subagent start'],
  subagentStop: ['하위 에이전트 종료', 'Subagent stop'], stop: ['응답 종료', 'Response stop'], interrupt: ['중단', 'Interrupt'],
};
const handlerLabels: Record<HandlerType, Labels> = {
  command: ['명령', 'Command'], mcpTool: ['MCP 도구', 'MCP tool'], prompt: ['프롬프트', 'Prompt'], agent: ['에이전트', 'Agent'],
};
const sourceLabels: Record<HookSource, Labels> = {
  system: ['시스템', 'System'], user: ['사용자', 'User'], project: ['프로젝트', 'Project'], mdm: ['기기 관리', 'Device management'],
  sessionFlags: ['세션 설정', 'Session flags'], plugin: ['플러그인', 'Plugin'], cloudRequirements: ['클라우드 요구사항', 'Cloud requirements'],
  cloudManagedConfig: ['클라우드 관리 설정', 'Cloud managed config'], legacyManagedConfigFile: ['기존 관리 설정 파일', 'Legacy managed config file'],
  legacyManagedConfigMdm: ['기존 기기 관리 설정', 'Legacy device management config'], unknown: ['미확인', 'Unknown'],
};
const trustLabels: Record<TrustStatus, Labels> = {
  managed: ['관리됨', 'Managed'], untrusted: ['신뢰 안 됨', 'Untrusted'], trusted: ['신뢰됨', 'Trusted'], modified: ['변경됨', 'Modified'],
};
interface HookItem {
  id: string;
  eventName: EventName;
  handlerType: HandlerType;
  source: HookSource;
  enabled: boolean;
  isManaged: boolean;
  trustStatus: TrustStatus;
}
interface FeatureRuntime { featureEnabled: boolean; baseHandlerCount: number }
interface Snapshot { items: HookItem[]; warningCount: number; errorCount: number; runtime: FeatureRuntime | null }
type View = 'initial' | 'pending' | 'unconfirmed' | 'invalid' | 'status';
const encoder = new TextEncoder();
const outerKeys = ['operation', 'threadId', 'provenance', 'executionGate', 'applicationState', 'hooks'];
const hookKeys = ['items', 'warningCount', 'errorCount'];
const itemKeys = ['id', 'eventName', 'handlerType', 'source', 'enabled', 'isManaged', 'trustStatus'];

function record(value: unknown): value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
function shape(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (!record(value)) return false;
  const present = Object.keys(value);
  return present.length === keys.length && present.every(key => keys.includes(key));
}
function identifier(value: unknown, maxBytes: number): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= maxBytes && value.trim().length > 0
    && !/[\u0000-\u001f\u007f-\u009f]/u.test(value) && encoder.encode(value).byteLength <= maxBytes;
}
function enumValue<T extends string>(value: unknown, allowed: readonly T[]): value is T {
  return typeof value === 'string' && allowed.includes(value as T);
}
function count(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= 128;
}
function snapshotTarget(value: unknown): HookRuntimeTarget | undefined {
  try {
    if (!record(value) || !identifier(value.id, 256) || !identifier(value.threadId, 256) || !identifier(value.title, 1024)
      || typeof value.generation !== 'number' || !Number.isSafeInteger(value.generation) || value.generation < 0) return undefined;
    return { id: value.id, generation: value.generation, threadId: value.threadId, title: value.title };
  } catch { return undefined; }
}
function snapshot(value: unknown, threadId: string): Snapshot | undefined {
  if (!record(value) || !shape(value, Object.prototype.hasOwnProperty.call(value, 'runtime') ? [...outerKeys, 'runtime'] : outerKeys)
    || value.operation !== 'hooksStatus' || value.threadId !== threadId
    || value.provenance !== 'loadedThreadHooksList' || value.executionGate !== 'unknown' || value.applicationState !== 'unknown'
    || !shape(value.hooks, hookKeys) || !Array.isArray(value.hooks.items) || value.hooks.items.length > 4096
    || !count(value.hooks.warningCount) || !count(value.hooks.errorCount)) return undefined;
  let runtime: FeatureRuntime | null = null;
  if (Object.prototype.hasOwnProperty.call(value, 'runtime') && value.runtime !== null) {
    if (!shape(value.runtime, ['featureEnabled', 'baseHandlerCount']) || typeof value.runtime.featureEnabled !== 'boolean'
      || typeof value.runtime.baseHandlerCount !== 'number' || !Number.isSafeInteger(value.runtime.baseHandlerCount)
      || value.runtime.baseHandlerCount < 0) return undefined;
    runtime = { featureEnabled: value.runtime.featureEnabled, baseHandlerCount: value.runtime.baseHandlerCount };
  }
  const items: HookItem[] = [];
  const seen = new Set<string>();
  for (const item of value.hooks.items) {
    if (!shape(item, itemKeys) || typeof item.id !== 'string' || item.id.length !== 69 || !/^hook-[0-9a-f]{64}$/u.test(item.id)
      || seen.has(item.id) || !enumValue(item.eventName, eventValues) || !enumValue(item.handlerType, handlerValues)
      || !enumValue(item.source, sourceValues) || !enumValue(item.trustStatus, trustValues)
      || typeof item.enabled !== 'boolean' || typeof item.isManaged !== 'boolean') return undefined;
    seen.add(item.id);
    items.push({ id: item.id, eventName: item.eventName, handlerType: item.handlerType, source: item.source,
      enabled: item.enabled, isManaged: item.isManaged, trustStatus: item.trustStatus });
  }
  return { items, warningCount: value.hooks.warningCount, errorCount: value.hooks.errorCount, runtime };
}

/** Pure result view. The caller owns requests, safe error notices and thread/generation epochs. */
export function createHookRuntimePanel(options: HookRuntimePanelOptions): HookRuntimePanel {
  const target = snapshotTarget(options.target);
  let alive = true;
  let disabled = false;
  let view: View = 'initial';
  let result: Snapshot | undefined;
  // ceiling: one validated inventory of 4096 items; explicitly reveal 50 more per click.
  let shown = 50;
  const element = document.createElement('section');
  element.className = 'hook-runtime-panel';
  const heading = document.createElement('h3');
  const title = document.createElement('p');
  const boundary = document.createElement('p');
  const summary = document.createElement('p');
  summary.setAttribute('role', 'status');
  summary.setAttribute('aria-live', 'polite');
  summary.setAttribute('aria-atomic', 'true');
  const actions = document.createElement('div');
  const inspect = document.createElement('button');
  const more = document.createElement('button');
  for (const button of [inspect, more]) { button.type = 'button'; button.className = 'quiet-button'; }
  if (target) actions.append(inspect, more);
  const details = document.createElement('dl');
  const gateLabel = document.createElement('dt');
  const gateValue = document.createElement('dd');
  const applicationLabel = document.createElement('dt');
  const applicationValue = document.createElement('dd');
  details.append(gateLabel, gateValue, applicationLabel, applicationValue);
  const feature = document.createElement('p');
  const baseCount = document.createElement('p');
  const runtimeBoundary = document.createElement('p');
  const diagnostics = document.createElement('p');
  const diagnosticNotice = document.createElement('p');
  const list = document.createElement('ul');
  const displayed = document.createElement('p');
  element.append(heading, title, boundary, summary, actions, details, feature, baseCount, runtimeBoundary,
    diagnostics, diagnosticNotice, list, displayed);

  function syncDisabled() {
    for (const button of [inspect, more]) button.disabled = disabled || view === 'pending';
    element.setAttribute('aria-busy', view === 'pending' ? 'true' : 'false');
  }
  function appendItems(start: number) {
    if (!result || view !== 'status') return;
    for (const item of result.items.slice(start, shown)) {
      const row = document.createElement('li');
      const name = document.createElement('strong');
      name.textContent = `${t(...eventLabels[item.eventName])} · ${t(...handlerLabels[item.handlerType])}`;
      const identity = document.createElement('p');
      // This digest identifies read-only inventory; no action receives it as a mutation key.
      identity.textContent = `${t('조회 ID', 'Inventory ID')}: ${item.id}`;
      const metadata = document.createElement('p');
      metadata.textContent = t(
        `사용 설정: ${item.enabled ? '켜짐' : '꺼짐'} · 신뢰: ${trustLabels[item.trustStatus][0]} · 출처: ${sourceLabels[item.source][0]} · 관리 정책: ${item.isManaged ? '관리됨' : '사용자 설정'}`,
        `Configured: ${item.enabled ? 'On' : 'Off'} · Trust: ${trustLabels[item.trustStatus][1]} · Source: ${sourceLabels[item.source][1]} · Management: ${item.isManaged ? 'Managed' : 'User settings'}`,
      );
      row.append(name, identity, metadata);
      list.append(row);
    }
  }
  function syncCount() {
    const total = view === 'status' ? result?.items.length ?? 0 : 0;
    list.hidden = total === 0;
    displayed.hidden = total === 0;
    displayed.textContent = total ? t(`${Math.min(shown, total)} / ${total}개 표시`, `Showing ${Math.min(shown, total)} of ${total}`) : '';
    more.hidden = total <= shown;
  }
  function draw() {
    if (!alive) return;
    heading.textContent = t('현재 대화 Hook', 'Current-chat Hooks');
    element.setAttribute('aria-label', heading.textContent);
    title.hidden = target === undefined;
    title.textContent = target?.title ?? '';
    boundary.textContent = t('기본 제공 Hook·요청별 추가 Hook을 포함한 전체 실행 목록은 아닙니다.',
      'This inventory does not cover every executing Hook, such as built-ins and per-turn additions.');
    inspect.textContent = t('현재 대화 Hook 조회', 'Inspect current-chat Hooks');
    more.textContent = t('Hook 더 보기', 'Show more Hooks');
    gateLabel.textContent = t('실행 gate', 'Execution gate');
    applicationLabel.textContent = t('적용 상태', 'Application state');
    const showingStatus = target !== undefined && view === 'status' && result !== undefined;
    details.hidden = !showingStatus;
    gateValue.textContent = showingStatus ? t('미확인', 'Unknown') : '';
    applicationValue.textContent = showingStatus ? t('미확인', 'Unknown') : '';
    feature.hidden = baseCount.hidden = runtimeBoundary.hidden = !showingStatus;
    const runtime = showingStatus ? result!.runtime : null;
    feature.textContent = showingStatus ? t(
      `일반 Hook 구성 기능: ${runtime === null ? '미확인' : runtime.featureEnabled ? '켜짐' : '꺼짐'}`,
      `Ordinary Hook configuration feature: ${runtime === null ? 'Unknown' : runtime.featureEnabled ? 'On' : 'Off'}`,
    ) : '';
    baseCount.textContent = showingStatus ? t(
      `설치된 기본 handler 수: ${runtime === null ? '미확인' : runtime.baseHandlerCount}`,
      `Installed base handlers: ${runtime === null ? 'Unknown' : runtime.baseHandlerCount}`,
    ) : '';
    runtimeBoundary.textContent = showingStatus ? t(
      '기능이 꺼져도 기본 제공 Hook은 동작할 수 있습니다. 기본 handler 수에는 요청별 환경·실행기 추가분이 포함되지 않습니다.',
      'Built-in Hooks may operate even when this feature is off. The base-handler count excludes per-turn environment/executor additions.',
    ) : '';
    diagnostics.hidden = !showingStatus;
    diagnostics.textContent = showingStatus ? t(`경고: ${result!.warningCount} · 오류: ${result!.errorCount}`,
      `Warnings: ${result!.warningCount} · Errors: ${result!.errorCount}`) : '';
    diagnosticNotice.hidden = !showingStatus || (result!.warningCount === 0 && result!.errorCount === 0);
    diagnosticNotice.textContent = !diagnosticNotice.hidden ? t('Hook 진단이 있습니다. 자세한 원문은 이 조회 화면에 표시하지 않습니다.',
      'Hook diagnostics are present. This inventory view does not display their raw content.') : '';
    list.setAttribute('aria-label', t('조회된 현재 대화 Hook', 'Inspected current-chat Hooks'));
    list.replaceChildren();
    appendItems(0);
    syncCount();
    if (!target) summary.textContent = t('현재 대화를 선택한 뒤 Hook 목록을 조회해 주세요.', 'Select a current chat to inspect its Hooks.');
    else if (view === 'pending') summary.textContent = t('현재 대화 Hook 목록 조회 중…', 'Inspecting current-chat Hooks…');
    else if (view === 'unconfirmed') summary.textContent = t('현재 대화 Hook 상태 미확인. 목록을 다시 조회해 주세요.', 'Current-chat Hook status is unconfirmed. Inspect the inventory again.');
    else if (view === 'invalid') summary.textContent = t('현재 대화 Hook 결과를 확인할 수 없습니다. 목록을 다시 조회해 주세요.', 'Cannot verify the current-chat Hook result. Inspect the inventory again.');
    else if (showingStatus) summary.textContent = result!.items.length === 0
      ? t('현재 대화에서 조회된 Hook이 없습니다. 실행 여부는 미확인입니다.', 'No Hooks were found for the current chat. Execution remains unknown.')
      : t('현재 대화의 Hook 목록을 조회했습니다. 사용 설정과 신뢰는 실행 확인과 다릅니다.', 'Current-chat Hook inventory inspected. Configuration and trust do not confirm execution.');
    else summary.textContent = t('현재 대화의 Hook 목록을 조회해 주세요.', 'Inspect the Hook inventory for the current chat.');
    syncDisabled();
  }
  function clear(nextView: View) { result = undefined; shown = 50; view = nextView; draw(); }
  const onInspect = () => { if (alive && target && !disabled && view !== 'pending') options.onInspect(); };
  const onMore = () => {
    if (!alive || !target || disabled || view !== 'status' || !result || shown >= result.items.length) return;
    const previous = shown;
    shown = Math.min(shown + 50, result.items.length);
    appendItems(previous);
    syncCount();
    if (more.hidden) inspect.focus();
  };
  inspect.addEventListener('click', onInspect);
  more.addEventListener('click', onMore);
  const unsubscribe = onLanguageChange(draw);
  draw();
  return {
    element,
    render(value: unknown): boolean {
      if (!alive || !target) return false;
      let accepted: Snapshot | undefined;
      try { accepted = snapshot(value, target.threadId); } catch { accepted = undefined; }
      if (!accepted) { clear('invalid'); return false; }
      result = accepted; shown = 50; view = 'status'; draw(); return true;
    },
    showPending() { if (alive && target) clear('pending'); },
    showUnconfirmed() { if (alive && target) clear('unconfirmed'); },
    setDisabled(value: boolean) { if (alive) { disabled = value; syncDisabled(); } },
    dispose() {
      if (!alive) return;
      alive = false; unsubscribe();
      inspect.removeEventListener('click', onInspect); more.removeEventListener('click', onMore);
      result = undefined; element.replaceChildren(); element.remove();
    },
  };
}
