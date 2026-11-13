# 현재 대화 MCP runtime adapter 사전 사례

[English](runtime-extension-cases.md) · 한국어

## 범위와 정본

Desktop 2.1.0의 기존 대화 연결에서 MCP 설정 reload와 서버 상태 한 페이지를 요청한다. 새 프로세스, daemon 조작, 설정 파일 변경, OAuth 로그인, 임의 RPC는 제공하지 않는다. 소유 세션·generation·실행 파일·연결 기한·watch·종료 정리는 호출자의 기존 ConversationConnection 경계가 맡는다.

구현 전 기준은 Core `d6026cbd6157fa18d966031792c537ac383377ac`의 다음 source이다.

- `codex-rs/app-server-protocol/src/protocol/v2/mcp.rs:51–109`: status 요청과 응답, nullable runtimeStatus, toolsError, cursor.
- 같은 파일 `288–309`: reload의 선택 thread와 reconnectQueuedForThreadId. 이 응답은 captured runtime의 queue 접수만 확인하며 ready·현재 registry 소속·이전 client 정리를 보장하지 않는다.
- `codex-rs/app-server/src/request_processors/mcp_processor.rs:174–215`: loaded thread를 reload 전에 capture하고 registry를 재검사한 뒤 reconnect를 enqueue한다.
- 같은 파일 `485–509,604–703`: threadId와 serverName이 모두 있으면 해당 thread의 서버 connection을 사용한다. status page는 서버 단위이며 tool 페이지가 아니다.
- `codex-rs/app-server-protocol/src/protocol/v2/tests.rs:2836–2905,4132–4161`: 구형 runtimeStatus 누락과 빈 reload 응답의 직렬화 계약.

## 요구와 불변조건

| ID | 요구·판정 경계 |
| --- | --- |
| RX1 | 두 RPC는 호출자가 검증한 정확한 threadId를 명시한다. status는 정확한 serverName도 명시한다. 응답에 threadId가 있으면 같은 값인지 검사한다. 다른 서버·중복 서버를 반환하지 않는다. |
| RX2 | reload는 한 번만 전송한다. 정확한 reconnectQueuedForThreadId만 outcome=queued이다. 구형 `{}`와 다른 thread의 ack는 unconfirmed이며 ready·도구 발견·cleanup 완료로 승격하지 않는다. |
| RX3 | status의 runtimeStatus 누락/null은 unknown이다. toolsError가 문자열이면 본문 없이 toolsState=failed이다. toolsError 누락과 tools 누락은 unknown이다. 명시적인 toolsError=null과 빈 tools 객체만 empty catalog로 표시한다. 어느 경우도 runtime 연결 성공으로 추론하지 않는다. |
| RX4 | renderer에는 고정 operation·threadId·serverName·found·연결 enum·auth enum·toolsState·toolNames·nextCursor만 반환한다. 임의 RPC 오류/서버 진단·환경변수·header·OAuth·URL·tool schema·설명은 반환하지 않는다. |
| RX5 | status 한 페이지와 서버 한 개만 허용한다. cursor는 자동 순회하지 않는다. 중단·연결 단절·응답 유실에 자동 재전송하지 않는다. |
| RX6 | 사용하는 입력·응답 필드와 좁은 출력 DTO의 바이트·행·중첩 상한을 초과하면 오류로 거부하며 잘라낸 성공 응답을 만들지 않는다. 사용하지 않는 raw 설명·schema는 기존 Rpc 8 MiB 운송 경계 안에서 허용하며 반환하지 않는다. |

요청 API는 `RuntimeExtensionRequest::{ReloadMcp {}, McpStatus { server_name, cursor }}`, 연결 전 `validate(thread_id, &request)`, `exchange(&mut Rpc<S>, thread_id, &request)`이다. exchange도 같은 validate를 호출하므로 호출자가 사전 검사를 생략하더라도 전송 전에 같은 검사를 수행한다. enum의 serde operation은 `reloadMcp`/`mcpStatus`이며 알 수 없는 요청 필드는 거부한다. ReloadMcp는 빈 struct variant로 선언하여 internally tagged unit variant가 추가 필드를 무시하는 Serde 경계를 피한다.

정확한 wire는 다음과 같다. cursor가 없으면 해당 키를 생략한다.

```json
{"method":"config/mcpServer/reload","params":{"threadId":"thread-a"}}
{"method":"mcpServerStatus/list","params":{"threadId":"thread-a","serverName":"server-a","detail":"toolsAndAuthOnly","limit":1,"cursor":"1"}}
```

adapter의 출력 DTO ceiling은 JSON 1 MiB, 일반 배열 256개, 객체 512개 필드, 중첩 32단계, JSON 값 총 8192개, 문자열 8192바이트·객체 key 256바이트이다. 투영 시 사용하는 raw 필드에는 data 1행, tools 256개와 정확한 thread/server·enum·도구 map key와 Tool.name 일치 검사를 적용한다. 별도로 thread ID 256바이트, server 이름 128바이트, tool 이름 256바이트, 입출력 cursor 256바이트를 제한한다. 길이는 UTF-8 바이트이며 제어문자·공백뿐인 식별자·빈 cursor는 거부한다. 기존 Rpc가 전체 text frame의 8 MiB를 검사한 뒤 JSON을 파싱한다. adapter는 대상·사용 필드를 검증하고 좁은 DTO를 만든 뒤 그 DTO에 1 MiB·shape 상한을 검사한다. 이 상한은 운송 계층의 최초 할당 한도를 낮추는 기능이 아니다.

## 사전 사례와 oracle

시험 entry point는 가능한 한 `exchange`이며 기존 Rust `#[test]`와 메모리 WebSocket frame fixture를 재사용한다. 실서버 연결 없이 outbound frame을 해석하여 정확한 method/params와 전송 횟수를 독립 oracle로 확인한다. 응답 DTO의 고정 필드와 enum은 위 정본·요구에서 수작업으로 지정한다.

| Case | 요구 | 입력·상황 | oracle |
| --- | --- | --- | --- |
| RX01 | RX1,RX2 | reload ack가 thread-a | 정확한 reload params, outbound RPC 1개, outcome=queued. ready·cleanup 등의 성공 필드 없음 |
| RX02 | RX2 | legacy `{}`, ack=null, 다른 queued thread | unconfirmed와 ackMissing/targetMismatch 구분. 다른 thread 문자열은 반환하지 않음 |
| RX03 | RX1,RX6 | 잘못된 thread/server/cursor, 요청에 임의 필드 | 전송 전에 거부, outbound 0개 |
| RX04 | RX1,RX5 | server-a status, cursor 없음/있음 | detail=toolsAndAuthOnly·limit=1·threadId/serverName 고정, cursor 왕복. 자동 다음 페이지 전송 없음 |
| RX05 | RX3 | connected·명시적 toolsError=null·도구 존재 | runtimeStatus=connected, toolsState=available, 이름만 반환 |
| RX06 | RX3 | connected·명시적 toolsError=null·빈 도구 | toolsState=empty. 별도 연결 enum 보존 |
| RX07 | RX3 | runtimeStatus 없음/null, toolsError 없음, tools 없음 | 해당 값 unknown. 빈 정상/ready로 보정하지 않음 |
| RX08 | RX3,RX4 | toolsError에 secret sentinel, failed/authenticationRequired/starting/disabled 등 | 원문 비노출, enum 보존, toolsState=failed. 도구 오류를 성공한 빈 목록으로 바꾸지 않음 |
| RX09 | RX1,RX6 | malformed data/row/enum/도구 값, 중복·과다 서버, 다른 서버, 응답의 다른 threadId | 고정 안전 오류. 부분 정상 결과 반환 없음 |
| RX10 | RX5,RX6 | nextCursor, 다음 수동 요청, 반복 cursor | 한 요청당 한 페이지. 자기 자신인 nextCursor는 오류. 서로 다른 호출 간 무한 순회는 만들지 않음 |
| RX11 | RX4,RX6 | 사용하는 도구 수·도구 이름 UTF-8 상한 초과 및 출력 DTO 제한기 자체의 1 MiB/문자열/배열/필드/중첩/값 수 상한 초과 | 실패, truncation 없음. 출력 제한기 사례는 bounded_response 직접 시험이며 미사용 raw의 거절 oracle로 사용하지 않음 |
| RX12 | RX2,RX4,RX5 | reload 송신 후 RPC error 또는 read 단절 | 고정 안전 오류·결과 미확인, outbound RPC 1개. 자동 재시도 없음 |
| RX13 | RX4 | env/header/OAuth/URL query/server 진단/raw tool schema에 sentinel | 성공 DTO와 오류 문자열에 sentinel 없음. 이름과 opaque cursor 외 임의 원문을 투영하지 않음 |
| RX14 | RX1,RX3 | data=[] | found=false, 연결/auth/tools 모두 unknown. 도구 0개를 정상 catalog로 보고하지 않음 |
| RX15 | RX4,RX6 | 정상 서버 1개·도구 1개의 미사용 description >8192B, inputSchema enum >256개, schema >512필드·>8192값·총 raw >1 MiB이면서 8 MiB 이내 | exchange 성공, 정확한 이름·상태 DTO만 반환, 설명/schema/sentinel 없음. outbound 1개 |
| RX16 | RX4,RX6 | RX15의 미사용 큰 데이터에 잘못된 thread/server/cursor/tool name·enum·도구 수를 조합 | 기존 사용 필드 거절 유지, 안전 오류, outbound 1개. raw 크기로 대상·이름 검증을 우회하지 않음 |
| RX17 | RX4,RX6 | 미사용 설명을 포함한 raw RPC text frame >8 MiB | 기존 Rpc::read가 JSON 파싱 전에 거절, adapter는 고정 안전 오류, outbound 1개. 운송 상한 변경 없음 |

이름과 cursor는 명시적으로 허용된 protocol 데이터이다. 임의 서버가 이 필드에 비밀을 넣은 경우 문자열 내용만으로 비밀 여부를 판별할 수 없으며, 이 adapter가 보장하는 것은 허용 필드 밖 원문의 비노출이다.

## 2026-11-14 projection 경계 보완 사전 계획

Core `mcp.rs:13,93`의 tools는 원본 `McpTool` map이다. `ToolsAndAuthOnly`는 도구 이름만을 반환하는 계약이 아니므로 도구의 미사용 description/inputSchema가 긴 경우도 정상 입력이다. 기존 exchange는 `bounded_response(raw)`를 먼저 실행하여 이 데이터를 출력 DTO의 8192B 문자열·256개 배열·512개 필드·8192개 값 제한으로 잘못 거절했다. 이는 raw를 renderer에 노출하거나 운송 상한을 높여 해결할 문제가 아니다. 기존 same_thread와 status/reload 투영을 거친 DTO에만 같은 제한기를 적용한다.

CS 불변조건은 (1) Rpc의 8 MiB text frame 경계 유지 (2) 사용 필드와 정확한 대상 검증 유지 (3) renderer에는 유한한 허용 DTO만 반환 (4) 한 요청·한 응답이며 재시도 없음이다. 불사용 schema를 다시 직렬화하거나 순회할 필요가 없으며 raw JSON 파싱 비용은 기존 Rpc 경계에 남는다. 성능·전역 메모리 개선은 측정하지 않았으므로 주장하지 않는다.

시험은 기존 Rust inline MemoryStream fixture와 runner를 유지한다. RX15–17을 exchange에서 검사하고, RX11의 도구 수·이름 거절은 exchange에 보존한다. 기존 미사용 diagnostic/ignored의 raw-size 거절 사례는 출력 제한기 직접 시험으로 분리한다. 출력 제한기의 일반 shape 사례는 현재 고정 DTO로 도달할 수 없는 방어 경계임을 명시한다. 이 oracle 변경은 Core tool 원본과 좁은 DTO의 계약 분리에 따른 보완이며 기존 사용 필드 거절을 통과로 바꾸지 않는다.

변경 전 source/doc은 `D:/codex/build/desktop210-integration-20261114/runtime-native-projection-boundary/before/`에 보존했다. 부모는 변경 후 동일 RX15 회귀 시험 bytes를 변경 전 제품 코드에 적용한 격리 사본과 수정 후보에서 실행하여 RED/GREEN을 수집한다. 원본 작업 트리를 rollback하지 않는다. 이 계획은 코드 변경 전에 저장하며 빌드·시험·check-plan은 이 하위 작업에서 NOT_RUN이다. 현재 문서의 이전 실행 근거는 이전 후보에만 해당한다.

## 검증 상태와 한계

사례 문서를 제품 adapter와 inline fixture 작성 전에 저장했다. 이번 하위 작업은 빌드·시험·check-plan 실행이 금지되어 해당 검사는 NOT_RUN이다. 부모가 `cargo test --manifest-path src-tauri/Cargo.toml runtime_extensions`를 직렬 실행하고 실제 출력과 최종 후보 digest를 연결해야 한다. 별도 시험 runner·의존성을 추가하지 않는다.

부모 실행 보고: RuntimeNative03은 adapter SHA256 `26100D4A5D1B0E19DD197991F42F2FAAF89DAACC9DBF0FA5DA967724CD33724A`에서 컴파일 후 11개 중 10 PASS/1 FAIL, Cargo exit 101이었다. 원본은 `D:/codex/build/desktop210-integration-20261114/runtime-native-03/`에 보존한다. 실패 assertion은 `{operation:"reloadMcp",method:"arbitrary"}` 역직렬화 거부이며, internally tagged unit variant가 추가 필드를 허용한 결함을 잡았다. 기대값을 유지하고 ReloadMcp를 빈 struct variant로 수정했다. 이 수정 후보의 빌드·재시험은 아직 NOT_RUN이다.

fixture는 RPC framing·요청 결속·응답 의미·비밀값 투영·재전송 금지의 단위 근거이다. 실제 Core build, 네트워크, MCP handshake/tool discovery, UI, 살아 있는 thread generation·deadline/watch·cleanup, 이전 client 해제, 진행 중 tool call 보존을 증명하지 않는다. 해당 통합 근거는 부모의 별도 검증 범위이며 fixture 통과로 대체하지 않는다.
# 부모 연결 및 실행 근거

UI 연결 사례를 실행 전에 정의한다. 저장된 구성의 관리와 native session ID/generation/expectedThreadId에 묶인 runtime 요청은 서로 다른 경로다. ① 재연결은 검토·접수와 준비 완료를 구분하고 config mutation을 보내지 않음 ② 특정 server의 상태 조회만 수행 ③ 창 닫기 후 다른 대화를 열어도 늦은 응답을 표시하지 않고 기존 요청은 추적 ④ app exit는 수락된 runtime 요청을 한 번 drain한다. 합성 IPC 시험은 실제 Core 준비/도구 호출·네트워크 회수의 근거가 아니며 native/live 시험을 별도로 수행한다.

추가 UI 경계: 같은 대화·같은 서버에서 구성 목록만 새로 읽거나 언어를 바꾸면 마지막 runtime 조회 자료와 목록 위치를 보존한다. 다른 server/대화/세대/창으로 바뀌면 기존 panel을 dispose한다. 도구명 발견과 실제 도구 실행 성공은 구별한다. 브라우저 fixture의 화면 확인에서 설정 수정·삭제 동작과 runtime 조회 동작의 위치를 분리하고, 긴 도구 목록 앞에도 조회 제어를 배치한다.

부모는 `conversation_request`의 native session ID/generation 조회와 실제 `SessionInfo.threadId`를 사용한다. renderer의 expectedThreadId는 권한이 아니라 검토 대상의 변경을 감지하는 조건이며, native 값과 다르면 proxy를 열기 전에 거절한다. 같은 ConversationConnection endpoint와 기존 deadline/watch/cleanup을 사용하고 metadata 요청은 기존 세션 잠금으로 유한하게 직렬화한다. GUI/CLI 입력 소유권을 새로 얻거나 terminal을 재시작하지 않는다. 수동 reload는 관측된 idle·질문/승인 없음·연결 확인 조건을 요구하며, 미확인/진행 중 상태는 명시 차단한다. 이 소비 정책은 CLI 담당에게 확인을 요청한다.

`runtime-native-03`의 10 PASS/1 FAIL(Cargo101)은 unit enum variant가 추가 필드를 허용한 사례다. 실패한 입력·거절 assertion은 유지하고 empty struct variant로 수정했다. Rust variant 구성식 동기화도 포함되므로 파일 전체의 바이트가 동일한 RED/GREEN이라고 표현하지 않는다. 새 IPC의 expectedThreadId 필수·추가 method/threadId/cwd 거절 사례를 포함한 `runtime-native-04`는 12 PASS, 실패·ignored 0, Cargo0이다. 원시 결과·source-sha256은 `D:/codex/build/desktop210-integration-20261114/runtime-native-04/`에 있다. 이후 stale expectedThreadId의 공개 요청 거절 assertion을 보강했으며 그 추가 경계는 다음 실행 전까지 NOT_RUN이다. 실제 Core·네트워크·GUI 성공을 의미하지 않는다.

후속 `conversation-native-05`는 그 stale expectedThreadId 공개 요청 거절 assertion을 포함해 124 PASS, 실패·ignored 0, Cargo0이다. 미사용 큰 설명/schema의 정당한 반례는 이전 handler와 같은 새 inline 시험을 사용하는 `projection-red-06`에서 실제 1 FAIL/Cargo101로 재현했다. 원본 작업 코드는 되돌리지 않고 로컬 `cfg(test)` 역사 모듈로만 실행했으며 임시 모듈 등록은 실행 뒤 제거했다. 새 코드의 `projection-green-07`은 16 PASS, 실패·ignored 0, Cargo0이다. before-with-regression과 after의 inline 시험 본문 SHA256은 `4aef86558e3b91ffd2debd5bffff47bd1ac0a0bd9d1b5352630e8e25d45da5ec`로 같다. 제품 전체 파일의 바이트 동일성이나 실제 Core 서비스를 검증한 결과로 표현하지 않는다.

원시 경로는 `D:/codex/build/desktop210-integration-20261114/{conversation-native-05,projection-red-06,projection-green-07}/`이다. RED source는 `runtime-native-projection-boundary/before-with-regression.rs`에 보존했다. 일괄 source-copy/after-snapshot shell 명령은 AGS가 경로를 확인하지 못해 거절하여 미실행이다. 기존 시험 runner 안의 명시적 `cfg(test)` 역사 모듈로 반례를 실행했고 source/test digest를 직접 대조했다. 운송 상한 8 MiB와 사용 field의 범위 검사는 유지했다. 기존 Rust 경고를 숨기거나 경고 0으로 보고하지 않는다.
