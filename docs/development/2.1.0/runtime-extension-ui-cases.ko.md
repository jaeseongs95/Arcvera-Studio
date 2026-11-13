# 현재 대화 MCP 결과 UI 사전 검증 사례

[English](runtime-extension-ui-cases.md) · 한국어

이 문서는 UI 시험 코드와 구현에 앞서 작성한 계약이다. 대상은 `src/mcp-runtime.ts`의 독립 패널이며 설정 편집·실행 검토·권한 판단·네트워크 호출·세대 판정은 부모 manager가 소유한다. 새 확인 대화상자를 만들지 않는다.

공개 API는 `createMcpRuntimePanel({target?, serverName, onReload, onInspect})`이다. `target`은 `{id, generation, threadId, title}`이며 생성 시 복사한다. 반환값은 `element`, `render(unknown): boolean`, `showPending()`, `showUnconfirmed()`, `setDisabled(boolean)`, `dispose()`이다. `onReload`는 기존 재연결 검토 흐름을 열고 `onInspect(cursor?)`만 조회 의도를 전달한다. 자동 조회·자동 페이지 병합·재시도·타이머·worker·상태 DB는 없다.

상태 결과는 native adapter의 좁은 DTO를 따른다. `operation=mcpStatus`, 정확한 `threadId`·`serverName`, boolean `found`, `runtimeStatus`, `authStatus`, `toolsState`, `toolNames`, `nextCursor`가 필요하다. runtime은 unknown/notStarted/starting/connected/authenticationRequired/failed/cancelled/disabled, auth는 unknown/unsupported/notLoggedIn/bearerToken/oAuth, tools는 available/empty/failed/unknown만 허용한다. found=false는 세 상태 unknown과 빈 도구 목록만 허용한다. available은 비어 있지 않은 이름 목록, failed/empty는 빈 목록이어야 한다. native가 toolsError 부재로 unknown과 이름을 함께 반환할 수 있으므로 unknown의 이름도 상한·중복을 검사하지만 표시하지 않는다. 이름 중복·미지 enum·대상 불일치·누락/형식 오류는 이전 결과를 지우고 고정 오류 문구와 false로 반환한다.

thread/id 256 UTF-8 bytes, server 128 bytes, cursor 256 bytes, tool name 256 bytes, toolNames 256개 상한을 적용한다. 이름은 `textContent`로만 출력하고 50개씩 더 보기를 제공한다. 원시 toolsError/schema/env/headers/url과 미지 필드는 저장·출력하지 않는다. nextCursor는 명시적인 다음 페이지 버튼 callback에만 사용하며 화면에는 출력하지 않는다. reload 결과는 `operation=reloadMcp`, 정확한 thread, outcome=queued 또는 unconfirmed(reason=ackMissing/targetMismatch)만 투영한다. queued는 접수, unconfirmed는 미확인으로 표시한다.

대상이 없거나 잘못되면 명시 안내만 제공하며 제어 버튼은 없다. showPending/showUnconfirmed/잘못된 결과는 이전 연결·도구 표시를 지운다. parent의 setDisabled는 별도 bool로 유지되어 언어 전환·render·showUnconfirmed 뒤에도 보존된다. dispose는 DOM, 모든 버튼 listener, 언어 구독과 결과를 해제하며 이후 render는 false이고 다른 메서드/callback은 무효이다.

| 사례 | 요구와 판정 oracle |
| --- | --- |
| 대상 없음/잘못된 대상 | 안내 표시, 제어 버튼 0개, render false |
| 대상과 동작 | 표시 title/server 일치, 키보드 버튼으로 검토/상태조회 callback 각각 1회 |
| queued/미확인 | 접수와 연결 완료를 구분, 미확인 후 이전 연결 상태 제거 |
| runtime/auth/tools | connected·인증 필요·available·empty·failed·unknown 각각 구분 |
| 이름·비공개 필드 | HTML 모양 이름은 문자 그대로, script/img 없음, raw 오류·URL·미지 필드 미노출 |
| 잘못된 DTO | 다른 thread/server, 미지 operation/enum, 중복, 누락, 상한 초과는 false와 고정 문구 |
| 목록과 페이지 | 첫 50개, 더 보기로 50개씩 최대 256개, cursor callback은 클릭 전 0회, 페이지는 교체 |
| parent 잠금과 pending | disabled 및 pending 동안 callback 0회, 언어/상태 변경 뒤 parent 잠금 유지 |
| 실패 후 상태 제거 | connected 후 showUnconfirmed/invalid/pending에 연결·목록이 남지 않음 |
| 언어 전환 | ko/en 안내·상태·aria·버튼 이름 갱신, 도구 이름/목록 위치 유지, callback 증가 없음 |
| dispose와 늦은 결과 | 보관한 버튼을 클릭해도 callback 없음, 언어 변경/render/setDisabled로 DOM 재생성 없음 |

시험은 `tests/mcp-runtime.spec.ts`에서 독립 source fixture를 사용한다. 이 fixture는 실제 Core/CLI/native 호출이 아니며 manager의 세대·busy/exit drain, 실제 MCP 연결, 앱의 시각 배치·성능·실기기 접근성을 증명하지 않는다. 이 경계는 부모 통합/실제 환경 검증으로 보완한다.

실행 상태: **NOT_RUN**. 이 단위 담당은 빌드·시험·의존 설치를 실행하지 않는다. 부모가 1420 포트와 시험 실행을 직렬 조정한다. 작성 순서는 이 문서 → 시험 코드 → 구현이다. 통과 여부와 실제 실행 증거는 부모 결과에 기록한다.

위 NOT_RUN은 작성·인계 시점의 상태다. 부모는 독립 패널 8건, 관리/부모 연결 14건, 기존 확장 22건을 합쳐 `artifacts/mcp-runtime-ui-03/`에서 44 PASS, 실패·skip 0, exit0을 확인했고 TypeScript 검사도 0이다. 같은 대화/서버의 구성 재조회·언어 전환에는 기존 패널과 조회 자료를 보존하고, 다른 대상·창으로 바뀌면 dispose한다. 테스트용 브라우저 화면에서 구성 수정/삭제와 runtime 조회 동작을 분리해 배치하고 긴 목록 앞에 제어를 두었다. 도구 목록 발견을 실제 호출 성공으로 표시하지 않도록 문구도 구분했다. 실제 Tauri·Core 연결/실도구 호출·실기기 접근성·성능은 이 결과에 포함되지 않는다.
