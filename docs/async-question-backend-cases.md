# 비동기 질문 백엔드 요구 사례

구현 전 작성. 기준: Desktop base `22551c6`, R36 protocol `1b441423baf713d144647d5be37525f7d9cc03c0`.
실행 범위는 합성 metadata fixture이며 운영 세션, 원시 대화, 인증 자료를 읽지 않는다.
공개 경계는 observer 알림 → pending snapshot → Engine 질문함 DTO이다. pure 상태 시험은 이 경계에서 쓰는 동일 adapter 모듈을 직접 실행한다.

| ID | 입력 / 사전조건 | 절차 | 기대 결과 / 합격 조건 | 실제 결과 |
|---|---|---|---|---|
| AQ1 | 진행 turn의 async AgentMessage, 질문 2개 | item/completed를 관측하고 snapshot 조회 | 질문 2개, 각 stable id는 `["request_user_input_async","call-1",index]` JSON 문자열, 원문/선택지 보존 | NOT_RUN |
| AQ2 | 동일 item started/completed 및 중복 completed | 반복 관측 | 같은 stable id가 한 번만 나타남 | NOT_RUN |
| AQ3 | ordinary UserMessage의 공식 답변 envelope | 한 질문 답변 후 원본 item 재생 | 해당 질문만 해결되고 재생으로 복원되지 않음 | NOT_RUN |
| AQ4 | 답변 UserMessage가 원본 질문보다 먼저 도착 | 답변 → 질문 관측 | 해결된 stable id는 pending에 나타나지 않음 | NOT_RUN |
| AQ5 | 진행 질문과 turn/completed, 뒤늦은 같은 turn item | 종료 후 item 재생 | 종료 turn 질문이 복원되지 않음; 다른 현재 turn은 보존 | NOT_RUN |
| AQ6 | 진행 turn 하나의 bounded history, 질문과 답변 혼합 | 구독 시 최신 turn/list, 현재 turn item/list만 조회 | 미답변만 복원; 완료 turn은 pending으로 취급하지 않음 | NOT_RUN |
| AQ7 | history 조회 실패/미지원/상한/단절 | 기존 질문 이후 실패 | 마지막 항목 보존, complete=false, metadataAvailable=false; 확정 0건으로 표시하지 않음 | NOT_RUN |
| AQ8 | 다른 thread 및 새 connection/session generation | 잘못된 thread 알림과 새 state 조회 | 다른 thread 무시, 이전 세대 질문 유출 없음 | NOT_RUN |
| AQ9 | oversized 질문/옵션/고유 id 반복 | 설정 상한을 넘기는 metadata 투입 | bounded state와 명시 진단 유지, 조용한 정상 빈 목록 금지 | NOT_RUN |
| AQ10 | 기존 blocking request/approval | 기존 observer와 GUI request fixture | 기존 requestId/응답 소유권/응답 검증 보존 | NOT_RUN |
| AQ11 | async 답변 제출, TUI busy/기존 draft/modal | 기존 일반 입력 안전 계약 조사 및 synthetic 검증 | RPC 응답으로 보내지 않음; 안전 API가 없으면 직접 응답 불가 이유를 명시 | NOT_RUN |

CS 조건: DIST-ORDER-004(중복/순서/turn 세대), DIST-ACK-001(전송과 해결 구분), PERF-BOUND-002(항목/바이트/조회 페이지 상한), CONC-LOCK-003(I/O 중 상태 잠금 미보유). 판정은 위 사례와 실제 실행 근거에 결속한다. 전체 Rust/실제 TUI 검증은 통합 담당의 승인된 환경에 남긴다.

## 구현과 검증 근거

- 라이브 `item/completed.agentMessage`의 `delivery=async`, 질문별 call id/index를 보존한다. `UserMessage`는 R36의 완전한 답변 envelope와 동일한 단일 text/skill/mention 제외 규칙으로만 해결한다. 완료 turn과 이전 세대 state는 재생으로 되살리지 않는다.
- 구독 이력은 최신 진행 turn 하나, 32개씩 최대 4페이지/128개/1MiB다. I/O 중 `Events` 잠금을 보유하지 않는다. 관련 라이브 알림과 경합하면 같은 전체 RPC 기한 안에서 한 번만 다시 읽는다. 두 번 모두 경합하거나 프로토콜·읽기·상한 오류가 나면 마지막 라이브 항목과 `complete=false`를 보존한다. 이후 새 turn 시작·완료 또는 재구독이 상태를 다시 확정한다.
- `metadataAvailable`은 관측 연결 여부이고 `complete`는 전체 미응답 metadata 확인 여부다. 두 값은 별개다. 직접 응답은 GUI 입력 소유권/연결, 관측 연결, 완전한 현재 state, 이전 입력 결과 불명 없음까지 확인한다.
- async `requestId`와 `questions[0].id`는 stable questionItemId다. 별도 RPC 질문 응답을 만들지 않고 기존 Respond 요청을 `turn/steer`의 정확한 `expectedTurnId`와 일반 사용자 text envelope로 바꾼다. `{turnId}`는 입력 수락까지만 뜻하며, UserMessage 알림 이전에는 `responded=true`와 pending을 유지한다. 유실·잘못된 성공 응답은 결과 불명으로 보존하며 자동 재전송하지 않는다.
- 고정 R36 소스 `app-server/src/request_processors/turn_processor.rs`의 `turn_steer_inner`, `app-server-protocol/src/protocol/v2/turn.rs`의 `TurnSteerParams`, `tui/src/async_question_reply.rs`, `context-fragments/src/answered_question.rs`를 `git show 1b441423baf713d144647d5be37525f7d9cc03c0:<path>`로 확인했다. `-32600`은 handler의 입력/권한/expected-turn/비수락 검증 거절이며 알려진 미수락으로 처리한다. `-32603`과 전송 유실은 미수락으로 간주하지 않는다.

| 사례 | 실제 실행 / 판정 |
|---|---|
| AQ1~AQ5 | 동일 production 상태 모듈의 Rust 시험 `live_async_questions_preserve_identity_and_deduplicate`, `user_answer_before_or_after_question_prevents_replay`, `completed_turn_cannot_reopen_or_clear_new_turn`: 로컬 PASS. 공개 observer 통합은 CI NOT_RUN. |
| AQ6 | 공개 WS 시험 `async_public_observer_subscribe_restores_only_active_turn_then_merges_live_answers`, `async_public_history_live_race_retries_once_without_replacing_newer_notifications`: 작성, CI NOT_RUN. |
| AQ7 | pure `restore_failure_and_live_race_preserve_last_metadata`: 로컬 PASS. 공개 WS의 unsupported/unmaterialized/cursorCycle: 작성, CI NOT_RUN. |
| AQ8 | 새 state/이전 turn 차단 pure 시험: 로컬 PASS. 공개 observer의 다른 thread 무시와 기존 Engine stale generation 경계는 CI NOT_RUN. |
| AQ9 | pure `bounds_are_explicit_and_bad_envelopes_do_not_resolve`: 로컬 PASS. WS 전체 페이지/바이트 포화 시험과 실사용 장기 부하는 NOT_RUN. |
| AQ10 | 기존 sync request/approval fixture 보존. 구독 응답에 상태가 없었던 observer 전용 fixture 두 곳은 명시 idle snapshot으로 수정하고 기존 무추가RPC assertion을 유지. 기존 전체 Rust 시험 CI NOT_RUN. |
| AQ11 | pure `answer_attempt_is_not_resolution_and_cannot_be_resent`: 로컬 PASS. 공개 WS steer exact request, 수락/미지원/expected mismatch/유실/wrongTurn, ownership 미확인 무송신 시험: 작성, CI NOT_RUN. 실제 TUI/daemon 미실행. |

로컬 작은 runner는 기존 MSVC/Rust 환경 exporter를 재사용하며 새 dependency는 제품에 추가하지 않았다. 실행 명령은 `node D:/codex/build/odAsyncProbe1104/run.cjs`이며 실제 Cargo 인수는 `test --offline --manifest-path D:/codex/build/odAsyncProbe1104/Cargo.toml -- --nocapture`다. 결과 원문은 해당 경로의 `result.json`에 있다. 전체 Tauri crate 및 공개 WS 시험을 실행한 것으로 확대하지 않는다. 기존 결함 상태의 같은 시험 실행은 하지 않았으므로 red/green 증거는 NOT_RUN이다.

처음 작은 runner를 작업 트리 `artifacts/async-backend-probe` 안에서 실행했을 때, 통합 담당의 Vite watcher가 MSVC 임시 linker 파일의 EBUSY로 종료했다. 이는 제품 assertion 실패와 다르다. 통합 담당이 원본 로그를 보존하고 runner를 작업 트리 밖 `D:/codex/build/odAsyncProbe1104`로 옮겼으며 이후 빌드는 그 위치만 쓴다. 원래 시험 실패를 재실행 성공으로 덮지 않는다.
