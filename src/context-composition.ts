import { onLanguageChange, t } from './i18n';
import './context-composition.css';

export interface ContextCompositionPanelOptions {
  threadId: string;
  onRefresh: () => void;
  onNext: (snapshotId: string, cursor: number) => void;
}
export interface ContextCompositionPanel {
  element: HTMLElement;
  render(value: unknown): boolean;
  showPending(): void;
  showUnavailable(reason: 'unsupported' | 'stale' | 'unavailable'): void;
  setDisabled(disabled: boolean): void;
  dispose(): void;
}

type Labels = readonly [string, string];
type LabelMap = Readonly<Record<string, Labels>>;
type NullableCount = number | null;
const unknown: Labels = ['미확인', 'Unknown'];
const kindLabels: LabelMap = { user: ['사용자', 'User'], assistant: ['어시스턴트', 'Assistant'],
  control: ['제어', 'Control'], opaque: ['불투명', 'Opaque'], tool_call: ['도구 호출', 'Tool call'],
  tool_result: ['도구 결과', 'Tool result'], unknown };
const residencyLabels: LabelMap = { resident: ['상주', 'Resident'], notResident: ['비상주', 'Not resident'], unknown };
const reasonLabels: LabelMap = { unknown, unknown_source: ['출처 미확인', 'Unknown source'],
  legacy_identity: ['이전 식별 체계', 'Legacy identity'], user_instruction: ['사용자 지시', 'User instruction'],
  persistent_control: ['지속 제어', 'Persistent control'], opaque_context: ['불투명 컨텍스트', 'Opaque context'],
  unknown_kind: ['종류 미확인', 'Unknown kind'], unpaired_tool_source: ['짝이 없는 도구 출처', 'Unpaired tool source'],
  pending_tool_group: ['대기 중인 도구 묶음', 'Pending tool group'], unclassified_tool_group: ['미분류 도구 묶음', 'Unclassified tool group'],
  unclassified: ['미분류', 'Unclassified'], reachable_task: ['연결된 작업', 'Reachable task'] };
const estimateLabels: LabelMap = { unknown,
  selected_resident_before_request_normalization: ['요청 정규화 전 선택된 상주 본문', 'Selected resident body before request normalization'] };
const limitLabels: LabelMap = { unknown, session_configuration_override: ['세션 설정 override', 'Session configuration override'] };
const scopeLabels: LabelMap = { unknown, response: ['응답 집계', 'Aggregate response'] };
const cacheProvenanceLabels: LabelMap = { unknown, not_reported: ['보고 안 됨', 'Not reported'],
  inclusive_response_input_usage: ['응답 입력 usage에 포함된 캐시 수치', 'Cache counters included in response input usage'],
  additive_response_input_usage: ['응답 입력 usage와 별도 합산된 캐시 수치', 'Cache counters additive to response input usage'] };
const cacheStatusLabels: LabelMap = { unknown, hit: ['캐시 읽기 관측', 'Cache read observed'],
  miss: ['캐시 읽기 0 관측', 'Zero cache reads observed'], invalid: ['관측값 유효하지 않음', 'Invalid observation'] };
const revisionLabels: LabelMap = { unknown, current: ['현재 revision 관측', 'Observation of current revision'],
  stale: ['이전 revision 관측', 'Observation of an earlier revision'] };
const ttlSupportLabels: LabelMap = { unknown, configured_provider_policy: ['설정된 provider 정책', 'Configured provider policy'] };
const ttlApplicationLabels: LabelMap = { unknown, requested_unconfirmed: ['요청됨 · 적용 미확인', 'Requested · application unconfirmed'],
  request_cache_activity_observed: ['요청의 캐시 활동 관측', 'Request cache activity observed'] };
const minimumProvenanceLabels: LabelMap = { unknown,
  configured_request_policy_and_reported_cache_activity: ['요청 정책 설정과 보고된 캐시 활동', 'Configured request policy and reported cache activity'] };
const minimumScopeLabels: LabelMap = { unknown,
  reported_write_tokens_only_not_individual_fragments: ['보고된 쓰기 토큰만 · 개별 조각 아님', 'Reported write tokens only · not individual fragments'] };
const requestScopeLabels: LabelMap = { unknown,
  prepared_sampling_prompt: ['준비된 sampling prompt', 'Prepared sampling prompt'] };
const requestProvenanceLabels: LabelMap = { unknown,
  utf8_json_components_before_provider_normalization: ['Provider 정규화 전 UTF-8 JSON component', 'UTF-8 JSON components before provider normalization'] };

interface Sizes {
  residentSerializedBytes: number; selectedResidentSerializedBytes: number; excludedResidentSerializedBytes: number;
  archiveSerializedBytes: NullableCount; selectedBodyEstimatedTokens: number; estimateProvenance: string;
  navigationEstimatedTokens: number; baseEstimatedTokens: NullableCount; toolEstimatedTokens: NullableCount; totalEstimatedTokens: NullableCount;
}
interface Limits { requestedContextWindowOverride: NullableCount; effectiveContextWindow: NullableCount; provenance: string }
interface Fragment {
  sourceId: string | null; sourceOrdinal: string | null; residentOccurrence: string | null; taskId: string | null;
  kind: string | null; selected: boolean; required: boolean | null; selectionReason: string | null; residency: string;
  residentSerializedBytes: NullableCount; estimatedTokens: NullableCount;
}
interface Cache {
  scope: string; provenance: string; status: string;
  cachedInputTokens: NullableCount; writeInputTokens: NullableCount; uncachedInputTokens: NullableCount; totalInputTokens: NullableCount;
  observedAt: string | null; observationId: string | null; requestContextIndexRevision: string | null; revisionStatus: string;
  requestedTtlSeconds: NullableCount; effectiveTtlSeconds: NullableCount; ttlSupport: string; ttlApplication: string;
  requestStartedAt: string | null; minimumValidUntil: string | null; minimumValidUntilProvenance: string;
  minimumValidUntilScope: string; estimatedExpiry: string | null; serverConfirmedExpiry: string | null;
}
interface RequestSizes {
  observationId: string; recordedAt: string; scope: string; provenance: string; historyVersion: string; contextIndexRevision: string;
  inputSerializedBytes: number; baseInstructionsSerializedBytes: number; toolDefinitionsSerializedBytes: number;
  outputSchemaSerializedBytes: number; componentPayloadBytes: number;
}
interface Snapshot {
  threadId: string; snapshotId: string; contextIndexRevision: string; historyVersion: string; asOf: string;
  sizes: Sizes; limits: Limits; fragments: Fragment[]; cache: Cache; requestSizes: RequestSizes | null; nextCursor: number | null;
}
type View = 'initial' | 'pending' | 'unsupported' | 'stale' | 'unavailable' | 'invalid' | 'snapshot';
const encoder = new TextEncoder();
const fail = (): never => { throw new Error('Invalid context composition metadata'); };

// The parent passes a body-free Native projection. Never retain or stringify the unknown input.
function object(value: unknown, keys: readonly string[], optional: readonly string[] = []): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return fail();
  const ownKeys = Reflect.ownKeys(value);
  if (ownKeys.length < keys.length || ownKeys.length > keys.length + optional.length
    || ownKeys.some(key => typeof key !== 'string' || (!keys.includes(key) && !optional.includes(key)))) return fail();
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !('value' in descriptor)) return fail();
  }
  for (const key of optional) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor && !('value' in descriptor)) return fail();
  }
  return value as Record<string, unknown>;
}
function text(value: unknown, maxBytes = 256): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxBytes || !value.trim()
    || /[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/u.test(value)
    || encoder.encode(value).byteLength > maxBytes) return fail();
  for (const character of value) {
    const point = character.codePointAt(0)!;
    if (point >= 0xd800 && point <= 0xdfff) return fail();
  }
  return value;
}
function nullableText(value: unknown): string | null { return value === null ? null : text(value); }
function count(value: unknown, positive = false): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < (positive ? 1 : 0)) return fail();
  return value;
}
function nullableCount(value: unknown, positive = false): NullableCount { return value === null ? null : count(value, positive); }
function decimal(value: unknown): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/u.test(value)
    || (value.length === 20 && value > '18446744073709551615')) return fail();
  return value;
}
function nullableDecimal(value: unknown): string | null { return value === null ? null : decimal(value); }
function bool(value: unknown): boolean { return typeof value === 'boolean' ? value : fail(); }
function code(value: unknown, labels: LabelMap, strict = false): string {
  const candidate = text(value);
  if (Object.hasOwn(labels, candidate)) return candidate;
  return strict ? fail() : 'unknown';
}
function timestamp(value: unknown): string {
  const candidate = text(value, 64);
  const match = /^(\d{4})-(\d{2})-(\d{2})T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d(?:\.\d{1,9})?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)$/u.exec(candidate);
  if (!match) return fail();
  const year = Number(match[1]); const month = Number(match[2]); const day = Number(match[3]);
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (month < 1 || month > 12 || day < 1 || day > days[month - 1] || !Number.isFinite(Date.parse(candidate))) return fail();
  return candidate;
}
function nullableTimestamp(value: unknown): string | null { return value === null ? null : timestamp(value); }

function readRequestSizes(value: unknown): RequestSizes | null {
  if (value === null) return null;
  const row = object(value, ['observationId', 'recordedAt', 'scope', 'provenance', 'historyVersion', 'contextIndexRevision',
    'inputSerializedBytes', 'baseInstructionsSerializedBytes', 'toolDefinitionsSerializedBytes', 'outputSchemaSerializedBytes', 'componentPayloadBytes']);
  const inputSerializedBytes = count(row.inputSerializedBytes);
  const baseInstructionsSerializedBytes = count(row.baseInstructionsSerializedBytes);
  const toolDefinitionsSerializedBytes = count(row.toolDefinitionsSerializedBytes);
  const outputSchemaSerializedBytes = count(row.outputSchemaSerializedBytes);
  const componentPayloadBytes = count(row.componentPayloadBytes);
  // Compare exactly; separately safe component counts can still have an unsafe combined sum.
  if (BigInt(inputSerializedBytes) + BigInt(baseInstructionsSerializedBytes) + BigInt(toolDefinitionsSerializedBytes)
    + BigInt(outputSchemaSerializedBytes) !== BigInt(componentPayloadBytes)) return fail();
  return { observationId: text(row.observationId), recordedAt: timestamp(row.recordedAt),
    scope: code(row.scope, requestScopeLabels), provenance: code(row.provenance, requestProvenanceLabels),
    historyVersion: decimal(row.historyVersion), contextIndexRevision: decimal(row.contextIndexRevision),
    inputSerializedBytes, baseInstructionsSerializedBytes, toolDefinitionsSerializedBytes, outputSchemaSerializedBytes, componentPayloadBytes };
}

function readSnapshot(value: unknown, expectedThreadId: string): Snapshot {
  const root = object(value, ['threadId', 'snapshotId', 'contextIndexRevision', 'historyVersion', 'asOf', 'sizes', 'limits', 'fragments', 'cache', 'nextCursor'], ['requestSizes']);
  if (text(root.threadId) !== expectedThreadId || !Array.isArray(root.fragments) || root.fragments.length > 100) return fail();
  const sizes = object(root.sizes, ['residentSerializedBytes', 'selectedResidentSerializedBytes', 'excludedResidentSerializedBytes',
    'archiveSerializedBytes', 'selectedBodyEstimatedTokens', 'estimateProvenance', 'navigationEstimatedTokens',
    'baseEstimatedTokens', 'toolEstimatedTokens', 'totalEstimatedTokens']);
  const limits = object(root.limits, ['requestedContextWindowOverride', 'effectiveContextWindow', 'provenance']);
  const cache = object(root.cache, ['scope', 'provenance', 'status', 'cachedInputTokens', 'writeInputTokens', 'uncachedInputTokens',
    'totalInputTokens', 'observedAt', 'observationId', 'requestContextIndexRevision', 'revisionStatus', 'requestedTtlSeconds',
    'effectiveTtlSeconds', 'ttlSupport', 'ttlApplication', 'requestStartedAt', 'minimumValidUntil', 'minimumValidUntilProvenance',
    'minimumValidUntilScope', 'estimatedExpiry', 'serverConfirmedExpiry']);
  const fragments: Fragment[] = [];
  for (const item of root.fragments) {
    const row = object(item, ['sourceId', 'sourceOrdinal', 'residentOccurrence', 'taskId', 'kind', 'selected', 'required',
      'selectionReason', 'residency', 'residentSerializedBytes', 'estimatedTokens']);
    fragments.push({ sourceId: nullableText(row.sourceId), sourceOrdinal: nullableDecimal(row.sourceOrdinal),
      residentOccurrence: nullableDecimal(row.residentOccurrence), taskId: nullableText(row.taskId),
      kind: row.kind === null ? null : code(row.kind, kindLabels, true), selected: bool(row.selected),
      required: row.required === null ? null : bool(row.required), selectionReason: row.selectionReason === null ? null : code(row.selectionReason, reasonLabels),
      residency: code(row.residency, residencyLabels, true), residentSerializedBytes: nullableCount(row.residentSerializedBytes),
      estimatedTokens: nullableCount(row.estimatedTokens) });
  }
  // Allowlist projection: future fields cannot be copied implicitly into retained state or DOM.
  return { threadId: expectedThreadId, snapshotId: text(root.snapshotId), contextIndexRevision: decimal(root.contextIndexRevision),
    historyVersion: decimal(root.historyVersion), asOf: timestamp(root.asOf),
    sizes: { residentSerializedBytes: count(sizes.residentSerializedBytes), selectedResidentSerializedBytes: count(sizes.selectedResidentSerializedBytes),
      excludedResidentSerializedBytes: count(sizes.excludedResidentSerializedBytes), archiveSerializedBytes: nullableCount(sizes.archiveSerializedBytes),
      selectedBodyEstimatedTokens: count(sizes.selectedBodyEstimatedTokens), estimateProvenance: code(sizes.estimateProvenance, estimateLabels),
      navigationEstimatedTokens: count(sizes.navigationEstimatedTokens), baseEstimatedTokens: nullableCount(sizes.baseEstimatedTokens),
      toolEstimatedTokens: nullableCount(sizes.toolEstimatedTokens), totalEstimatedTokens: nullableCount(sizes.totalEstimatedTokens) },
    limits: { requestedContextWindowOverride: nullableCount(limits.requestedContextWindowOverride, true),
      effectiveContextWindow: nullableCount(limits.effectiveContextWindow, true), provenance: code(limits.provenance, limitLabels) },
    fragments, nextCursor: nullableCount(root.nextCursor),
    requestSizes: Object.hasOwn(root, 'requestSizes') ? readRequestSizes(root.requestSizes) : null,
    cache: { scope: code(cache.scope, scopeLabels), provenance: code(cache.provenance, cacheProvenanceLabels), status: code(cache.status, cacheStatusLabels),
      cachedInputTokens: nullableCount(cache.cachedInputTokens), writeInputTokens: nullableCount(cache.writeInputTokens),
      uncachedInputTokens: nullableCount(cache.uncachedInputTokens), totalInputTokens: nullableCount(cache.totalInputTokens),
      observedAt: nullableTimestamp(cache.observedAt), observationId: nullableText(cache.observationId),
      requestContextIndexRevision: nullableDecimal(cache.requestContextIndexRevision), revisionStatus: code(cache.revisionStatus, revisionLabels),
      requestedTtlSeconds: nullableCount(cache.requestedTtlSeconds), effectiveTtlSeconds: nullableCount(cache.effectiveTtlSeconds),
      ttlSupport: code(cache.ttlSupport, ttlSupportLabels), ttlApplication: code(cache.ttlApplication, ttlApplicationLabels),
      requestStartedAt: nullableTimestamp(cache.requestStartedAt), minimumValidUntil: nullableTimestamp(cache.minimumValidUntil),
      minimumValidUntilProvenance: code(cache.minimumValidUntilProvenance, minimumProvenanceLabels),
      minimumValidUntilScope: code(cache.minimumValidUntilScope, minimumScopeLabels), estimatedExpiry: nullableTimestamp(cache.estimatedExpiry),
      serverConfirmedExpiry: nullableTimestamp(cache.serverConfirmedExpiry) } };
}

const label = (labels: LabelMap, value: string | null): string => t(...(value === null ? unknown : labels[value] ?? unknown));
const amount = (value: number | string | null, unit = ''): string => value === null ? t(...unknown) : `${value}${unit}`;

/** Pure body-free metadata view. The caller owns request ordering, session epochs and error classification. */
export function createContextCompositionPanel(options: ContextCompositionPanelOptions): ContextCompositionPanel {
  let threadId: string | undefined;
  try { threadId = text(options.threadId); } catch { threadId = undefined; }
  const { onRefresh, onNext } = options;
  let alive = true;
  let disabled = false;
  let view: View = 'initial';
  // ceiling: one page, at most 100 body-free fragments. No accumulated pages or sourceId deduplication.
  let result: Snapshot | undefined;

  const element = document.createElement('section');
  element.className = 'context-composition';
  const heading = document.createElement('h3');
  const summary = document.createElement('p');
  summary.className = 'context-composition-status';
  summary.setAttribute('role', 'status'); summary.setAttribute('aria-live', 'polite'); summary.setAttribute('aria-atomic', 'true');
  const actions = document.createElement('div');
  actions.className = 'context-composition-actions';
  const refresh = document.createElement('button');
  const next = document.createElement('button');
  for (const button of [refresh, next]) { button.type = 'button'; button.className = 'quiet-button'; }
  actions.append(refresh, next);
  const content = document.createElement('div');
  content.className = 'context-composition-content';
  element.append(heading, summary, actions, content);

  function detail(dl: HTMLElement, key: string, names: Labels, value: string) {
    const term = document.createElement('dt'); term.textContent = t(...names);
    const description = document.createElement('dd'); description.textContent = value;
    description.dataset.contextField = key;
    dl.append(term, description);
  }
  function group(host: HTMLElement, names: Labels): HTMLDListElement {
    const section = document.createElement('section'); section.className = 'context-composition-group';
    const title = document.createElement('h4'); title.textContent = t(...names);
    const dl = document.createElement('dl'); section.append(title, dl); host.append(section); return dl;
  }
  function note(host: HTMLElement, names: Labels) {
    const paragraph = document.createElement('p'); paragraph.className = 'context-composition-note'; paragraph.textContent = t(...names); host.append(paragraph);
  }
  function drawSnapshot(snapshot: Snapshot) {
    const meta = group(content, ['페이지 관측', 'Page observation']);
    detail(meta, 'threadId', ['대화 ID', 'Thread ID'], snapshot.threadId);
    detail(meta, 'snapshotId', ['스냅샷 ID', 'Snapshot ID'], snapshot.snapshotId);
    detail(meta, 'contextIndexRevision', ['현재 컨텍스트 revision', 'Current context revision'], snapshot.contextIndexRevision);
    detail(meta, 'historyVersion', ['기록 버전', 'History version'], snapshot.historyVersion);
    detail(meta, 'asOf', ['페이지 관측시각', 'Page observed at'], snapshot.asOf);
    const metrics = document.createElement('div'); metrics.className = 'context-composition-metrics'; content.append(metrics);
    const sizes = group(metrics, ['UTF-8 직렬화 크기', 'UTF-8 serialized sizes']);
    detail(sizes, 'sizes.residentSerializedBytes', ['전체 상주', 'All resident'], amount(snapshot.sizes.residentSerializedBytes, ' B'));
    detail(sizes, 'sizes.selectedResidentSerializedBytes', ['선택된 상주', 'Selected resident'], amount(snapshot.sizes.selectedResidentSerializedBytes, ' B'));
    detail(sizes, 'sizes.excludedResidentSerializedBytes', ['제외된 상주', 'Excluded resident'], amount(snapshot.sizes.excludedResidentSerializedBytes, ' B'));
    detail(sizes, 'sizes.archiveSerializedBytes', ['아카이브', 'Archive'], snapshot.sizes.archiveSerializedBytes === null
      ? t('미로딩 / 미확인', 'Unloaded / unknown') : amount(snapshot.sizes.archiveSerializedBytes, ' B'));
    const estimates = group(metrics, ['Heuristic token 추정', 'Heuristic token estimates']);
    detail(estimates, 'sizes.selectedBodyEstimatedTokens', ['선택 본문', 'Selected body'], amount(snapshot.sizes.selectedBodyEstimatedTokens));
    detail(estimates, 'sizes.navigationEstimatedTokens', ['탐색', 'Navigation'], amount(snapshot.sizes.navigationEstimatedTokens));
    detail(estimates, 'sizes.baseEstimatedTokens', ['기본', 'Base'], amount(snapshot.sizes.baseEstimatedTokens));
    detail(estimates, 'sizes.toolEstimatedTokens', ['도구', 'Tool'], amount(snapshot.sizes.toolEstimatedTokens));
    detail(estimates, 'sizes.totalEstimatedTokens', ['전체', 'Total'], amount(snapshot.sizes.totalEstimatedTokens));
    detail(estimates, 'sizes.estimateProvenance', ['추정 근거', 'Estimate provenance'], label(estimateLabels, snapshot.sizes.estimateProvenance));
    const limits = group(metrics, ['컨텍스트 한도', 'Context limits']);
    detail(limits, 'limits.requestedContextWindowOverride', ['요청 override', 'Requested override'], amount(snapshot.limits.requestedContextWindowOverride));
    detail(limits, 'limits.effectiveContextWindow', ['유효 한도', 'Effective window'], amount(snapshot.limits.effectiveContextWindow));
    detail(limits, 'limits.provenance', ['한도 근거', 'Limit provenance'], label(limitLabels, snapshot.limits.provenance));
    note(content, ['토큰 수는 heuristic 추정치이며 provider 실측 usage가 아닙니다. 미확인 값은 합산하지 않습니다.',
      'Token counts are heuristic estimates, not provider-measured usage. Unknown values are not added.']);

    const request = snapshot.requestSizes;
    const requestDetails = group(content, ['직전 준비 요청 크기', 'Last prepared request sizes']);
    if (request === null) {
      detail(requestDetails, 'requestSizes.observation', ['준비 관측', 'Preparation observation'],
        t('준비 관측 미확인', 'Prepared request observation unconfirmed'));
    } else {
      detail(requestDetails, 'requestSizes.observationId', ['준비 관측 ID', 'Preparation observation ID'], request.observationId);
      detail(requestDetails, 'requestSizes.recordedAt', ['준비 관측시각', 'Preparation recorded at'], request.recordedAt);
      detail(requestDetails, 'requestSizes.scope', ['준비 관측 범위', 'Preparation scope'], label(requestScopeLabels, request.scope));
      detail(requestDetails, 'requestSizes.provenance', ['준비 크기 출처', 'Preparation size provenance'], label(requestProvenanceLabels, request.provenance));
      detail(requestDetails, 'requestSizes.historyVersion', ['준비 기록 버전', 'Preparation history version'], request.historyVersion);
      detail(requestDetails, 'requestSizes.contextIndexRevision', ['준비 컨텍스트 revision', 'Preparation context revision'], request.contextIndexRevision);
      const revisionsMatch = request.historyVersion === snapshot.historyVersion && request.contextIndexRevision === snapshot.contextIndexRevision;
      detail(requestDetails, 'requestSizes.revisionMatch', ['표시 구성과 revision 비교', 'Revision comparison with displayed composition'], revisionsMatch
        ? t('표시 snapshot과 revision 일치', 'Revisions match the displayed snapshot')
        : t('다른 준비 revision / 현재 구성 일치 미확인', 'Different preparation revision / match to current composition unconfirmed'));
      detail(requestDetails, 'requestSizes.inputSerializedBytes', ['입력 JSON 크기', 'Input JSON size'], amount(request.inputSerializedBytes, ' B'));
      detail(requestDetails, 'requestSizes.baseInstructionsSerializedBytes', ['기본지침 JSON 크기', 'Base instructions JSON size'], amount(request.baseInstructionsSerializedBytes, ' B'));
      detail(requestDetails, 'requestSizes.toolDefinitionsSerializedBytes', ['도구정의 JSON 크기', 'Tool definitions JSON size'], amount(request.toolDefinitionsSerializedBytes, ' B'));
      detail(requestDetails, 'requestSizes.outputSchemaSerializedBytes', ['출력 schema JSON 크기', 'Output schema JSON size'], amount(request.outputSchemaSerializedBytes, ' B'));
      detail(requestDetails, 'requestSizes.componentPayloadBytes', ['Component payload 합계', 'Component payload total'], amount(request.componentPayloadBytes, ' B'));
    }
    note(content, ['네 JSON component만의 byte 관측입니다. provider 정규화·전송 봉투·delta 크기·과금 token이 아니며 추정치·token·캐시와 합산하거나 환산하지 않습니다.',
      'Byte observations cover only four JSON components, before provider normalization. They exclude transport envelopes and delta sizes, are not billable tokens, and are not combined or converted with estimates, tokens or cache counters.']);

    const cache = snapshot.cache;
    const cacheDetails = group(content, ['캐시 집계와 관측', 'Cache aggregates and observation']);
    detail(cacheDetails, 'cache.scope', ['관측 범위', 'Observation scope'], label(scopeLabels, cache.scope));
    detail(cacheDetails, 'cache.status', ['캐시 상태', 'Cache status'], label(cacheStatusLabels, cache.status));
    detail(cacheDetails, 'cache.provenance', ['집계 근거', 'Counter provenance'], label(cacheProvenanceLabels, cache.provenance));
    detail(cacheDetails, 'cache.cachedInputTokens', ['캐시 읽기 token', 'Cache read tokens'], amount(cache.cachedInputTokens));
    detail(cacheDetails, 'cache.writeInputTokens', ['캐시 쓰기 token', 'Cache write tokens'], amount(cache.writeInputTokens));
    detail(cacheDetails, 'cache.uncachedInputTokens', ['미캐시 입력 token', 'Uncached input tokens'], amount(cache.uncachedInputTokens));
    detail(cacheDetails, 'cache.totalInputTokens', ['전체 입력 token', 'Total input tokens'], amount(cache.totalInputTokens));
    detail(cacheDetails, 'cache.observedAt', ['캐시 관측시각', 'Cache observed at'], amount(cache.observedAt));
    detail(cacheDetails, 'cache.observationId', ['관측 ID', 'Observation ID'], amount(cache.observationId));
    detail(cacheDetails, 'cache.requestContextIndexRevision', ['관측 요청 revision', 'Observed request revision'], amount(cache.requestContextIndexRevision));
    detail(cacheDetails, 'cache.revisionStatus', ['관측 revision 상태', 'Observation revision status'], label(revisionLabels, cache.revisionStatus));
    const ttl = group(content, ['캐시 정책과 유효시각', 'Cache policy and validity times']);
    detail(ttl, 'cache.requestedTtlSeconds', ['요청 TTL', 'Requested TTL'], amount(cache.requestedTtlSeconds, ' s'));
    detail(ttl, 'cache.effectiveTtlSeconds', ['유효 TTL', 'Effective TTL'], amount(cache.effectiveTtlSeconds, ' s'));
    detail(ttl, 'cache.ttlSupport', ['TTL 지원 근거', 'TTL support evidence'], label(ttlSupportLabels, cache.ttlSupport));
    detail(ttl, 'cache.ttlApplication', ['TTL 적용 관측', 'TTL application evidence'], label(ttlApplicationLabels, cache.ttlApplication));
    detail(ttl, 'cache.requestStartedAt', ['요청 시작시각', 'Request started at'], amount(cache.requestStartedAt));
    detail(ttl, 'cache.minimumValidUntil', ['최소 유효 하한 (서버 확정 아님)', 'Minimum validity lower bound (not server-confirmed)'], amount(cache.minimumValidUntil));
    detail(ttl, 'cache.minimumValidUntilProvenance', ['하한 근거', 'Lower-bound provenance'], label(minimumProvenanceLabels, cache.minimumValidUntilProvenance));
    detail(ttl, 'cache.minimumValidUntilScope', ['하한 적용 범위', 'Lower-bound scope'], label(minimumScopeLabels, cache.minimumValidUntilScope));
    detail(ttl, 'cache.estimatedExpiry', ['추정 만료시각', 'Estimated expiry'], amount(cache.estimatedExpiry));
    detail(ttl, 'cache.serverConfirmedExpiry', ['서버 확정 만료시각', 'Server-confirmed expiry'], amount(cache.serverConfirmedExpiry));
    note(content, ['집계는 개별 조각에 배분하지 않습니다. 읽기로 TTL이 다시 시작되지 않으며 하한 경과만으로 실제 만료를 판단하지 않습니다.',
      'Aggregates are not attributed to fragments. Reads do not restart TTL; passing a lower bound does not establish actual expiry.']);

    const fragmentsHeading = document.createElement('h4');
    fragmentsHeading.textContent = t(`현재 페이지 조각 (${snapshot.fragments.length}개)`, `Fragments on this page (${snapshot.fragments.length})`);
    const list = document.createElement('ol'); list.className = 'context-composition-fragments';
    list.setAttribute('aria-label', t('현재 페이지 컨텍스트 조각', 'Context fragments on this page'));
    for (const row of snapshot.fragments) {
      const item = document.createElement('li'); const dl = document.createElement('dl');
      detail(dl, 'sourceId', ['출처 ID', 'Source ID'], amount(row.sourceId));
      detail(dl, 'taskId', ['작업 ID', 'Task ID'], amount(row.taskId));
      detail(dl, 'kind', ['종류', 'Kind'], label(kindLabels, row.kind));
      detail(dl, 'sourceOrdinal', ['출처 순서', 'Source ordinal'], amount(row.sourceOrdinal));
      detail(dl, 'residentOccurrence', ['상주 occurrence', 'Resident occurrence'], amount(row.residentOccurrence));
      detail(dl, 'selected', ['선택', 'Selection'], row.selected ? t('선택', 'Selected') : t('제외', 'Excluded'));
      detail(dl, 'required', ['필수', 'Required'], row.required === null ? t(...unknown) : row.required ? t('예', 'Yes') : t('아니요', 'No'));
      detail(dl, 'selectionReason', ['선택 이유', 'Selection reason'], label(reasonLabels, row.selectionReason));
      detail(dl, 'residency', ['상주 상태', 'Residency'], label(residencyLabels, row.residency));
      detail(dl, 'residentSerializedBytes', ['상주 직렬화 크기', 'Resident serialized size'], amount(row.residentSerializedBytes, ' B'));
      detail(dl, 'estimatedTokens', ['Heuristic token', 'Heuristic tokens'], amount(row.estimatedTokens));
      item.append(dl); list.append(item);
    }
    content.append(fragmentsHeading, list);
  }

  function syncDisabled() {
    const blocked = !threadId || disabled || view === 'pending';
    refresh.disabled = blocked; next.disabled = blocked;
    element.setAttribute('aria-busy', view === 'pending' ? 'true' : 'false');
  }
  function draw() {
    if (!alive) return;
    heading.textContent = t('현재 대화 컨텍스트 구성', 'Current-chat context composition');
    element.setAttribute('aria-label', heading.textContent);
    refresh.textContent = t('구성 새로고침', 'Refresh composition');
    next.textContent = t('다음 구성 페이지', 'Next composition page');
    content.replaceChildren();
    const showing = view === 'snapshot' && result !== undefined;
    content.hidden = !showing;
    next.hidden = !showing || result!.nextCursor === null;
    if (!threadId) summary.textContent = t('현재 대화를 확인한 뒤 컨텍스트 구성을 조회해 주세요.', 'Verify the current chat before inspecting its context composition.');
    else if (view === 'pending') summary.textContent = t('컨텍스트 구성 조회 중…', 'Inspecting context composition…');
    else if (view === 'unsupported') summary.textContent = t('이 실행 환경의 컨텍스트 구성 조회 지원을 확인할 수 없습니다.', 'Context composition inspection support could not be confirmed in this runtime.');
    else if (view === 'stale') summary.textContent = t('구성 스냅샷이 변경되었습니다. 새로고침해 주세요.', 'The composition snapshot changed. Refresh to inspect it again.');
    else if (view === 'unavailable') summary.textContent = t('컨텍스트 구성 상태 미확인. 새로고침해 주세요.', 'Context composition is unconfirmed. Refresh to inspect it again.');
    else if (view === 'invalid') summary.textContent = t('컨텍스트 구성 결과를 확인할 수 없습니다. 새로고침해 주세요.', 'Cannot verify the context composition result. Refresh to inspect it again.');
    else if (showing) summary.textContent = result!.fragments.length === 0
      ? t('이 페이지에 조각이 없습니다. 집계 값은 별도로 표시합니다.', 'This page has no fragments. Aggregate values are shown separately.')
      : t('현재 페이지의 구성 메타데이터입니다. 본문은 포함하지 않습니다.', 'Composition metadata for the current page. No message bodies are included.');
    else summary.textContent = t('새로고침으로 현재 대화의 컨텍스트 구성을 조회해 주세요.', 'Refresh to inspect the current chat’s context composition.');
    if (showing) drawSnapshot(result!);
    syncDisabled();
  }
  function clear(nextView: View) { result = undefined; view = nextView; draw(); }
  const refreshListener = () => { if (alive && threadId && !disabled && view !== 'pending') onRefresh(); };
  const nextListener = () => {
    if (alive && threadId && !disabled && view === 'snapshot' && result && result.nextCursor !== null) onNext(result.snapshotId, result.nextCursor);
  };
  refresh.addEventListener('click', refreshListener); next.addEventListener('click', nextListener);
  const unsubscribe = onLanguageChange(draw);
  draw();
  return { element,
    render(value: unknown): boolean {
      if (!alive || !threadId) return false;
      let accepted: Snapshot;
      try { accepted = readSnapshot(value, threadId); } catch { clear('invalid'); return false; }
      result = accepted; view = 'snapshot'; draw(); return true;
    },
    showPending() { if (alive) clear('pending'); },
    showUnavailable(reason) { if (alive) clear(reason === 'unsupported' || reason === 'stale' ? reason : 'unavailable'); },
    setDisabled(value) { if (alive) { disabled = value; syncDisabled(); } },
    dispose() {
      if (!alive) return;
      alive = false; unsubscribe(); refresh.removeEventListener('click', refreshListener); next.removeEventListener('click', nextListener);
      result = undefined; element.replaceChildren(); element.remove();
    },
  };
}
