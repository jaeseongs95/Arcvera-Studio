# 컨텍스트 구성 표시 패널의 사전 사례

상태: 사례·소스 fixture 준비용. 빌드, Playwright, 실제 Core/Native/전체 dialog 검증은 **NOT_RUN**이다. 생성 schema·컴파일 결과·개발 실행 파일이 없는 타입 후보를 소비하는 준비이며, API 지원 완료를 뜻하지 않는다.

정본은 `core110-release-20261114/source/codex-rs/app-server-protocol/src/protocol/v2/thread.rs`의 `ThreadContextReadResponse` 및 관련 타입(1915–2032행 후보)이다. `core/src/context_manager/history_composition.rs`, `core/src/session/context_composition.rs`, `core/src/session/context_cache.rs`, `history/src/context_index.rs`에서 추정치·이유·관측 범위를 대조했다. 대화 본문·개인 로그·인증 자료는 입력하지 않는다.

## 공개 경계와 책임

`createContextCompositionPanel({ threadId, onRefresh, onNext(snapshotId, cursor) })`는 `element`, `render(unknown): boolean`, `showPending()`, `showUnavailable('unsupported' | 'stale' | 'unavailable')`, `setDisabled(boolean)`, `dispose()`를 반환한다. 순수 DOM 표시와 명시 callback만 제공한다. 부모는 Native 조회, 현재 session/generation/thread 결속, 응답 순서, stale 오류 분류, 권한, 종료, dialog, sourceFixtures 설정을 소유한다. 이 패널은 원문 오류를 파싱하지 않는다.

한 번에 검증된 현재 페이지 0–100행만 보관하고 새 결과로 교체한다. 같은 sourceId의 서로 다른 residentOccurrence와 null 식별 행을 합치지 않는다. 다음 페이지는 현재 snapshotId와 nextCursor를 명시 callback에 전달한다. 자동 조회·페이지 누적·타이머·네트워크·변경/신뢰 버튼은 없다. 언어 변경은 현재 상태와 부모 disabled를 보존한다. 폐기 시 언어 구독·버튼 listener·DOM·결과를 해제하며 지연 render는 false이다.

## 입력과 표시 계약

- camelCase 후보 필드를 사용한다. Native가 u64 식별값(contextIndexRevision/historyVersion/cache.requestContextIndexRevision/fragments.sourceOrdinal/residentOccurrence)을 정확한 decimal string 또는 원래 null로 전달한다. canonical `0` 또는 선행 0 없는 양의 십진수, u64 최댓값까지이며 숫자로 변환하지 않는다.
- 양/크기/cursor는 0..MAX_SAFE_INTEGER 정수이며, 요청/effective context window 한도는 null 또는 양의 safe integer(0 불가)이다. nullable는 명시 null이다. fragment.selected는 boolean, required는 boolean|null이다. 필수 필드 누락·잘못된 타입·다른 thread·101행 이상은 false와 고정 안내로 이전 목록을 지운다. 부모는 Native의 `-32600`, `data.reason=staleContextSnapshot`을 분류해 `showUnavailable('stale')`를 호출한다.
- thread/snapshot/source/task/observation ID는 256 UTF-8 byte 이하의 공백만 아닌 문자열이다. 제어문자·bidi 제어·잘못된 surrogate는 거부한다. 시각은 64byte 이하 RFC3339와 유효한 시각이다. 필드 allowlist 외의 본문/auth/schema/진단 등은 거부하며 원문을 반사하지 않는다. 허용된 ID도 textContent로만 표시한다.
- String status/provenance/selectionReason는 유한 256byte 문자열이다. 확인된 상수만 번역하고 미래값은 고정 미확인으로 표시하며 원래 값은 보관·표시하지 않는다. 고정 enum kind/residency의 미지값은 거부한다. 비밀 문자열을 추론해 가리는 기능이 아니므로 Native가 원문 없는 DTO를 만들어야 한다.
- resident/selected/excluded UTF-8 직렬화 byte와 archive를 서버 값 그대로 표시한다. archive null은 미로딩/미확인이다. 선택본문·탐색·base·tool·전체 heuristic token과 provenance 및 요청/effective 한도는 provider 실측 usage와 분리한다. 합계를 계산하거나 null을 0으로 만들지 않는다.
- cache는 응답 집계 읽기/쓰기/미캐시/전체 수치와 provenance, 관측시각·revision·current/stale/unknown을 보여 준다. 현재 revision과 관측 revision을 비교해 상태를 재계산하지 않는다. 요청/effective TTL, support/application, minimumValidUntil 하한·범위·근거, estimatedExpiry, serverConfirmedExpiry를 별도 항목으로 표시한다. 하한은 서버 확정 만료가 아니며 읽기로 TTL을 재시작하지 않는다. 하한을 지나도 실제 만료를 추정하지 않고 집계를 조각에 배분하지 않는다.

## engineering-test-plan

수준: 기존 Playwright/Vite source fixture의 공개 DOM/API 경계. `tests/context-composition.spec.ts`가 Core/CLI/Tauri를 호출하지 않는 합성 fixture를 넣는다. 예상값은 위 후보 타입과 부모 배정의 표시 의미에서 정한다. 구현의 합계·TTL 계산을 oracle로 복제하지 않는다. 실행 명령 후보는 부모 전용 `npx playwright test tests/context-composition.spec.ts`; 실행·check-plan 도구 호출은 위임 범위 밖이므로 NOT_RUN이다. 새 runner·의존성은 추가하지 않는다.

| 사례 | 요구/상황 | 기대값·실패 oracle |
| --- | --- | --- |
| CC01 | 초기/유효하지 않은 thread | 초기 안내와 새로고침; 무효 대상은 callback 불가 |
| CC02 | 정상 페이지·상충하는 합계·null | 서버의 각 byte/heuristic 값을 그대로 표시; 합계 재계산·null→0 없음 |
| CC03 | u64 식별/동일 source 중복 occurrence/null 행 | MAX_SAFE_INTEGER 초과 식별을 정확한 문자열로 보존; 행 누락·합치기 없음 |
| CC04 | 캐시 관측·TTL·과거 하한 | 관측/current 상태와 provenance·하한/추정/확정이 분리; 하한 경과로 만료 문구·TTL 재시작 없음 |
| CC05 | 미래 String 상태/근거/이유 | render true, 미확인 표시; 미래 원값/secret sentinel 반사 없음 |
| CC06 | 빈 페이지·미지원·stale·미확인 | 서로 다른 고정 안내; stale 포함 실패 후 이전 목록/상세 제거 |
| CC07 | pending/부모 busy/언어 전환 | 버튼 disabled·aria-busy; 재렌더·언어 변경 후 부모 busy 유지; callback 차단 |
| CC08 | 명시 다음 페이지·교체 | snapshot/cursor 정확히 1 callback; 자동 요청 없음; 새 페이지로 이전 행 제거 |
| CC09 | 다른 thread/누락/형식·숫자·u64·시각 경계 | render false, 고정 안내, 목록/상세 clear, 원payload 비출력 |
| CC10 | 페이지100/101·중복 sourceId | 100행 허용·101 거부; sourceId 중복만으로 합치거나 거부하지 않음 |
| CC11 | HTML ID·raw/비밀 추가필드·객체 accessor | HTML은 textContent, script/img 생성 없음; 비계약 필드/accessor 거부·비노출 |
| CC12 | dispose·지연 응답·보관 버튼·언어 변경 | render false; DOM 부활·callback·언어 구독 반응 없음 |
| CC13 | 영한 전환과 실제 값 | 숫자/식별/page 상태 보존, 문구 전환만; callback 증가 없음 |
| CC14 | 좁은 viewport·키보드 | 고유 버튼 이름/기본 키보드 활성화·focus 표시; 360px 패널 가로 넘침 없음 |

실제 통합에서 남은 것: 생성 schema/Native projection과의 필드·상한 대조, 실제 Core 결과, 실제 thread/generation/종료 경계, 전체 dialog 크기·화면 관측, Windows 접근성/부하. sourceFixtures는 부모가 연결했다. fixture 통과도 실제 통합 경계를 증명하지 않는다.

## 부모 실행 근거

작성 뒤 부모가 `context-composition-ui-01`에서 이 패널의 CC01–CC14 14개와 부모 연결 3개·기존 컨텍스트 21개를 실행해 38 PASS/exit0를 확인했다. `tsc --noEmit`도 exit0다. 이는 source/IPC fixture이며 실제 Core·Native·최종 앱 실행 검증을 뜻하지 않는다. Native 어댑터 메모리 WebSocket 6 PASS는 `context-adapter-cases.ko.md`에 구분했다. 생성 schema·같은 Core1.1 후보짝 실통합·실제 WebView/패키지는 미완이다.
