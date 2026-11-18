# 현재 대화 Hook 조회 panel 사전 사례

상태: **NOT_RUN**. 이 계획은 fixture와 구현 전에 작성했다. 부모 담당이 실행을 직렬화하며, 이 단위에서는 빌드·시험·실 Core/CLI/native 호출을 실행하지 않는다.

## 대상과 계약

- 대상은 선택한 현재 대화의 Hook inventory를 확인하는 사용자이다. 기존 MCP 순수 panel, `quiet-button`, native button/list/dl, i18n의 `t`/`onLanguageChange`를 재사용한다. 새 CSS·dialog·변경·신뢰 승인 동작은 없다.
- `createHookRuntimePanel({target?:{id,generation,threadId,title},onInspect})` → `{element,render(unknown):boolean,showPending(),showUnconfirmed(),setDisabled(boolean),dispose()}`. target은 복사하여 고정한다. 실제 현재 대상/generation/exit·응답 epoch 결속은 부모 소유이다.
- `render`는 native의 `hooksStatus` projection만 받는다: `threadId`, `provenance:"loadedThreadHooksList"`, `executionGate:"unknown"`, `applicationState:"unknown"`, `hooks:{items:[{id,eventName,handlerType,source,enabled,isManaged,trustStatus}],warningCount,errorCount}`. 원문·미지 필드는 안전한 고정 안내와 false로 거부한다.
- item id는 `hook-`+소문자 SHA256 hex 64자이다. raw key가 아니며 조회 목록 구분용이다. 권한·신뢰 mutation key로 사용하지 않는다. UI의 callback은 인자 없는 `onInspect` 하나뿐이다.
- enum은 native의 기존 검증 목록을 그대로 확인한다. 새 1.1 metadata 필드/의미는 추정하지 않는다. 사용 설정 `enabled`와 신뢰 `trustStatus`, 실행 gate·적용 미확인을 나눠 표시한다. enabled/trusted/빈 목록은 실행 확인을 뜻하지 않는다.
- loaded-thread inventory는 builtin과 매 turn overlay를 포함한 전체 실행 handler 목록이 아니라는 짧은 안내를 항상 표시한다. diagnostics는 0–128의 개수와 고정 안내만 표시한다.
- DTO에는 미지원 enum이 없다. 일반 미확인과 성공 빈 목록은 panel에서 구분하고, Core 미지원의 분류·안내는 부모의 안전 오류 경계에서 제공한다. 원문 오류를 받아 미지원 상태로 추정하지 않는다.

## 수명과 입력 상한

- target이 없거나 부적합하면 안내만 표시하고 제어 버튼을 만들지 않는다. target id/threadId는 공백/제어문자 없는 UTF-8 256바이트, title은 1024바이트, generation은 0 이상의 safe integer이다.
- DTO는 현재 thread와 일치해야 한다. item은 최대 4096개, 중복 id 금지, id 형태·enum·bool·diagnostic 개수를 전부 검사한 뒤 작은 새 snapshot만 보관한다. 다른 thread·누락·형식·상한 오류는 이전 성공 결과도 지우고 고정 안내/false를 반환한다.
- 처음 50개를 표시하고 명시적인 더 보기 버튼마다 50개를 추가한다. 추가 조회·자동 paging·network/timer/retry/state DB는 없다.
- pending은 이전 목록을 지우고 버튼을 막는다. 부모의 disabled 값은 결과 렌더와 언어 전환 뒤에도 유지한다. 미확인은 이전 목록을 지운다. 언어 전환은 결과·진행 상태·표시 개수를 보존한다.
- dispose는 언어 구독·버튼 listener와 snapshot을 해제하고 DOM을 비운 뒤 제거한다. 폐기 뒤 retained button/늦은 render/언어 전환/다른 메서드로 callback·DOM을 되살리지 않는다.

## fixture 사례와 oracle

Playwright의 기존 isolated source harness를 사용한다. 이 fixture는 실제 Core/CLI/native wire 또는 generation gate의 증거가 아니다. 진입점은 공개 panel 메서드와 DOM/키보드 동작이다.

| ID | 사례 | 기대 결과 |
| --- | --- | --- |
| HU01 | 대상 없음·잘못된 대상 | 명시 안내, 버튼 0개, render false |
| HU02 | 표시된 대상과 조회·target 외부 변경 | 제목을 textContent로 표시, 키보드 조회 callback 1회, 고정 target thread 유지 |
| HU03 | 정상 inventory·같은 event/type/source와 다른 id | 모든 행을 구분해서 표시, enabled·신뢰·관리·출처를 별도 문구로 확인, 두 실행 상태 미확인 |
| HU04 | 성공 empty·showUnconfirmed·pending·다시 조회 | 빈 목록은 조회 성공으로 표시하되 실행 여부 미확인, 실패/미확인은 이전 목록 clear, pending callback 금지 |
| HU05 | 다른 thread·operation/provenance/gate·enum·누락·중복/불량 id·bool·개수 경계 | 고정 안전 안내/false, 이전 결과와 원문 모두 미노출 |
| HU06 | HTML·secret가 들어 있는 raw key/command/path/diagnostics/unknown fields | false, payload 원문·HTML node·secret를 DOM에 반사하지 않음; target title HTML은 단순 텍스트 |
| HU07 | 51개/4096개/4097개 목록과 50개씩 더 보기 | 무조회로 50개씩 증가, 마지막 페이지 focus 유지, 4096개는 손실 없음·4097개 거부 |
| HU08 | disabled와 언어 전환·결과 재렌더 | busy/pending/결과/표시 개수 보존, disabled는 parent가 해제할 때까지 유지, callback 증가 없음 |
| HU09 | dispose 뒤 지연 응답·retained button·언어 전환·반복 dispose | render false, DOM 비움/분리 유지, callback 증가 없음 |
| HU10 | warnings/errors 개수와 unknown vs unsupported | 개수·고정 안내만; panel의 미확인/성공 empty 구분, 미지원 안내는 부모 오류 입력 없이는 생성하지 않음 |

## 부모 통합 사전 사례

`hook-runtime-manager.spec.ts`는 합성 host로 HP01 빈 구성에서도 Native session/generation/expectedThreadId 결속, HP02 같은 대상의 언어/metadata 갱신 후 조회 보존, HP03 닫기/다른 thread 재열기 뒤 늦은 결과 거절, HP04 종료 시 수락된 조회 drain 및 중복 요청 없음의 네 경계를 검사한다. 실제 Native/Core 실행 증거가 아니며 실행 전 상태는 NOT_RUN이다.

## 실행 인계

작성 순서는 사전 사례 → source fixture → 구현이다. 부모가 `hook-runtime`을 Playwright sourceFixtures 목록에 추가하고 기존 runner로 실행한다. 타입 검사, 실제 앱 배치·키보드·좁은 화면·언어 동작, 실제 Core inventory 및 독립 감사는 부모 검증에 남는다. 준비 상태를 제품 PASS나 릴리스 완료로 표현하지 않는다.
