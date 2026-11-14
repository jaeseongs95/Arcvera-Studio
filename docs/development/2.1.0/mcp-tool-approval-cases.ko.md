# MCP 도구 승인 사례 (D-0087)

[English](mcp-tool-approval-cases.md)

fixture와 구현 전에 작성한 계획이다. 기준은 Desktop 커밋 `1471e6adb86f612693e713491dd7f19227077391`이며 e99 release fixture와 제품 입력이 같다. 공개 Core 1.0.1의 `mcpServer/elicitation/request`를 기존 대화 화면·질문함 투영·Native 응답 검사가 미지원으로 처리한다. 수정 전 collector는 보존하며 GUI 왕복 완료 근거로 사용하지 않는다.

`mode=form`, `_meta.codex_approval_kind=mcp_tool_call`, 유효한 원래 thread/turn/server와 완전한 도구 이름·인수를 갖춘 도구 승인만 지원한다. Native pending 요청 식별자와 GUI 입력 소유권을 재사용하고 없는 `callId`는 만들지 않는다. 응답은 정확히 `{action:'accept'|'decline'|'cancel',content:null,_meta:null}`만 허용한다. 요청의 저장 선택지를 행동으로 표시하거나 응답에 복사하지 않는다. 일반 form·URL·OAuth/인증·알 수 없는 elicitation은 미지원으로 유지한다.

| 사례 | 공개 경계와 판정 기준 | 예정 근거 |
|---|---|---|
| MTA-01 | 유효한 도구 요청의 서버·도구·완전한 인수를 두 GUI 화면에 표시하고, 명시적 일회 허용 클릭으로 정확한 응답을 한 번 보낸다. | Native WebSocket 요청/응답 fixture, 대화·질문함 Playwright 시험. |
| MTA-02 | 거절·취소는 해당 action의 정확한 응답만 보낸다. 세션/항상 허용 선택지는 없다. | 동일 공개 응답·GUI 경계. |
| MTA-03 | 잘못된 mode/kind, 누락·잘못된 대상/도구 필드, 다른 thread, 상세 참조·검토 불가능한 과대한 payload는 승인 컨트롤이나 지원 응답을 얻지 못한다. | Native 부정 요청 행렬, GUI 미지원 사례. |
| MTA-04 | 추가 응답 필드, 저장/세션 action, 알 수 없는 action, 필수 null 필드 누락, non-null content/meta는 송신·pending 소비 전에 거절한다. | 기존 연결을 통한 Native 부정 응답 행렬. |
| MTA-05 | session/generation/thread·외부 owner·GUI lease·이미 응답·중복/늦은 응답·취소·응답소실 보호를 보존한다. | Native 소유 연결 fixture와 기존 보호 시험, GUI 닫기/재열기·지연 응답 시험. |
| MTA-06 | HTML과 오해를 유도하는 인수도 텍스트로 표시한다. 한도 안의 내용은 전체를 검토할 수 있고 숨김·잘림·과대한 내용에는 승인 버튼을 주지 않는다. | Playwright DOM 검사, Native 크기·형식 검사. |
| MTA-07 | 기존 command/fileChange decision 선택지와 질문 answers payload를 보존한다. | 기존 Native·Playwright 시험과 집중 회귀 assertion. |
| MTA-08 | 실제 GUI 입력→로컬 합성 provider→MCP 한 도구→명시적 GUI 일회 응답→같은 thread/turn의 도구·보이는 assistant 완료, 자체 정리·설정 불변. | 제품 fixture 재빌드 뒤 부모가 단독 수행하는 격리 Native 시험. |

기존 버튼·텍스트 노드·응답 상태·초안·결과 불명 처리를 재사용한다. 서버·도구·인수를 먼저 보여주고 일회 행동을 배치한다. 읽기 전용·오래된 상태·메타데이터 미확인 요청은 활성 응답 컨트롤을 제공하지 않는다. 작성자는 정적 읽기·쓰기만 수행한다. Rust·TypeScript·Node·Playwright 실행은 이 단위에서 모두 **NOT_RUN**이며 부모가 담당한다. RED/GREEN·실통합 완료·독립감사·릴리스 통과를 주장하지 않는다. collector bytes가 달라지면 동일 RED/GREEN 시험으로 간주하지 않는다.

실제 공개 필드는 `params._meta.tool_name`, `params._meta.tool_params`다. 인수 JSON은 object이며 직렬화한 UTF-8이 16KiB 이하, 중첩 깊이가 64 이하이고 `__detail`·`truncated`·`hasMore`가 true인 상세 참조·잘림 표시가 없어야 한다. 대상과 이름은 기존 식별자 한도인 비어 있지 않은 256byte 이하·제어문자 없음 조건을 사용한다. Native와 GUI에 같은 한도를 적용한다. 대화 패널은 Native의 `mcpToolApprovalAvailable=true`도 요구해 언어별 JSON 숫자 재직렬화 차이로 UI만 허용되는 경계를 막는다. 이 boolean은 기존 64KiB wire 검사 뒤에 추가하며 인수 사본을 만들지 않는다. 질문함은 Native의 한정된 `mcpToolApproval` 투영을 사용하고 metadata 조회·완전성 미확인 시 MCP 행동을 잠근다. 원래 요청·응답 한도 64KiB는 유지한다.

구현/fixture 인계: `src/mcp-tool-approval.ts`는 두 GUI가 사용하는 무상태 검토 parser·정확 응답 helper다. Native 의무는 `mcp_tool_approval_exact_actions_send_once_and_preserve_resolution_boundary`, `mcp_tool_approval_rejects_non_tool_and_unreviewable_requests`, `mcp_tool_approval_invalid_response_never_sends_or_consumes_pending`, `mcp_tool_approval_wrong_current_thread_and_cancellation_do_not_consume_pending`, `mcp_tool_approval_external_owner_and_stopped_connection_never_send`, `mcp_tool_approval_admission_rejects_stale_generation_and_stopping_before_transport`로 연결한다. 기존 `conversation.spec.ts`, `question-center.spec.ts`에 추가한 `D0087` 시험은 실제 GUI 컴포넌트의 공개 host 경계를 사용하지만 합성 fixture이며 실제 Core/모델 통합 근거가 아니다. 이 작성 단위에서는 모두 NOT_RUN이다. 기존 assertion은 보존하고 시험 host에 새 정확 응답 command 및 Native pending 소비 뒤 응답소실을 표현하는 경로만 추가했다.

CS 불변조건: 같은 pending 식별자에서 소유자와 세대를 유지하고, 미지원 응답은 송신 시도 슬롯을 소비하기 전에 거절한다. 실제 응답 시도는 transport 쓰기 전에 소비하며 자동 재전송·저장을 추가하지 않는다. ACK를 해결·turn 완료로 간주하지 않는다. 기존 취소·기한·큐·byte 한도를 보존한다. Native 요청 분류/투영 한 곳을 두 GUI가 사용하고 기존 command/question 계약과 MCP action 응답을 구분한다.

## 부모 실행 근거

후보 통합 뒤 TypeScript 검사는 exit 0, D-0087 기준 GUI source fixture 30개는 26.5초에 통과했다. D-0089 보완 뒤 같은 두 GUI 파일의 관련 사례 48개는 39.0초에 통과했다. 이는 합성 IPC source fixture이며 실제 Native 승인·도구 호출 완료 근거가 아니다. 원결과는 `desktop210-integration-20261114/d87-ui-01`, `d87-ui-02`에 보존한다.

기존 e99 실행물 관측 시도 `s210g1/case01`은 Windows canonical cwd 식별 오류로 전송 전에 중단됐다. `case02`는 GUI 전송 한 번과 합성 provider 첫 요청까지 관측했지만 collector 읽기 단계에서 실패했다. 둘 다 승인 0회이며 D-0087 실제 재현이나 수정 후 성공으로 계산하지 않는다. 자체 Core stopped와 운영 앱 identity 보존은 확인했으나 case01은 자체 앱 강제 종료, case02는 정상 exit 요청 ACK/page close와 별개로 부모 process exit 필드 누락을 명시한다. 원결과를 덮어쓰지 않는다.

로컬 Rust는 디스크 하한 때문에 NOT_RUN이다. CI의 별도 같은 응답 검증 witness는 구버전 allowlist 거절과 수정 후 응답 형식 수용만 비교하고, 수치 mutation은 원래 정수 검토 분류만 검사한다. 실제 CI 결과는 원로그 확인 전까지 미검증이다. 최종 Core 1.1 동작·생성 schema·실행물·동일 후보 통합과 독립감사는 남아 있다.

## D-0089 숫자 검토 한도 — fixture 작성 전 계획

미커밋 D-0087 후보는 GUI에서 유한한 숫자를, Native에서 모든 JSON number를 허용했다. JavaScript 안전정수 범위를 벗어난 Native `u64`와 음수 0은 JavaScript 변환·문자열화 뒤 사람이 보는 값이 달라질 수 있다. 미커밋 후보의 결함이며 배포 제품의 사고 근거로 표현하지 않는다. 확인된 Native JSON number와 JavaScript Number 사이의 경계 때문에 draft 규칙 `LANG-NUMERIC-002`를 적용하며 더 넓은 검증 상태를 주장하지 않는다.

| 사례 | 판정 기준과 공개 경계 |
|---|---|
| MTA-N01 | Native에서 원래 `9007199254740993`을 정확한 `u64`로 파싱하고 GUI 변환·transport 쓰기 전에 pending 검토 불가 표시와 응답 거절을 확인한다. |
| MTA-N02 | Native `u64`/`i64`의 양쪽 안전 경계는 허용하고 바로 바깥의 값과 정수 타입 최댓값·최솟값은 거절한다. |
| MTA-N03 | 범위 안의 유한 float, 일반 0과 양수·음수 소수는 지원한다. 큰 유한 float와 `-0`, `-0.0`은 거절한다. |
| MTA-N04 | 두 실제 GUI 컴포넌트에서 범위 밖 양방향 값·큰 유한 float·음수 0은 버튼을 잠그고 안전 경계·0·유한 소수는 명시적 일회 허용을 유지한다. Native availability flag 요구도 보존한다. |

숫자 검토 한도는 유한하고 절대값이 `Number.MAX_SAFE_INTEGER`(`9007199254740991`) 이하이며 음수 0이 아닌 경우다. Native는 원래 `serde_json::Number`의 정수·float 표현을 검사하고 GUI는 `JSON.stringify` 전에 검사한다. 중첩 object·array에도 같은 조건을 적용한다. 요청된 유한 소수 동작을 보존하는 안전 한도이며 모든 소수 표기·JSON 숫자 원문의 보존을 약속하는 것은 아니다. 기존 byte·깊이·상세 참조 한도, 소유권, 정확 응답, 저장 거절은 유지한다. 새 D-0089 사례는 이 작성 단위에서 **NOT_RUN**이며 모든 실행은 부모가 담당한다. 앞선 D-0087 결과로 새 수치 사례를 검증했다고 판단하지 않는다.

작성 결과 인계: `mcp_tool_approval_numeric_review_ceiling_checks_original_number_variants`에 독립적인 수치 기대값 18개를 두고 원래 숫자 파싱·Native 요청 분류·정확 응답 검사·pending availability flag를 확인하도록 작성했다. `mcp_tool_approval_numeric_rejection_never_sends_or_consumes_pending`는 기존 WebSocket 경계에서 수치 거절 3개와 정상 소수 허용 1개를 검증한다. 기존 GUI spec마다 `D0089 ... numeric review ...` 사례 9개를 추가하며, 음수 0은 브라우저 안에서 파싱해 검토 helper에 전달한다. 기존 Native 시험 6개와 D-0087 GUI assertion 30개는 보존했다. 부모는 기준 D-0087 GUI 30 PASS/26.5초와 TypeScript exit 0을 보고했으며 작성자가 실행한 검사는 아니다. 새 D-0089 검사·Rust 컴파일·실제 Native 통합·독립감사는 이 단위에서 NOT_RUN이다.
