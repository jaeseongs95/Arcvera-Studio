# 로컬 MCP 도구 호출 provider fixture

[English](tool-provider-fixture.md) · [한국어](tool-provider-fixture.ko.md)

상태: **test-only fixture, Node 시험 60 PASS / 0 FAIL, 보완 fixture의 실제 Core/Native 통합 NOT_RUN**. 제품 코드가 import하지 않는 독립 loopback fixture다. 사용자 목표 원문은 “너의 목표는 2.1.0 릴리즈이다. cli 담당과 협업해서 2.1.0 릴리즈에 들어갈 개발 범위를 산정하고 작업하면 된다.”이다. 이 fixture는 실제 모델·과금·provider cache의 증거가 아니다.

## 정본과 최소 계약

공개 소스 `D:/codex/build/core110-release-20261114/source`의 `codex-rs/core/tests/common/responses.rs:1000`/`:1012`는 `response.output_item.done`의 function_call 및 namespace 형태를 정의한다. 같은 파일 `:1498`은 function_call 완료 뒤 후속 요청에 assistant 완료를 반환하는 두 단계 fixture다. `codex-rs/core/src/tools/handlers/mcp.rs:546`/`:573`, `codex-rs/tools/src/responses_api.rs:59`/`:76`에 따라 광고는 `tools:[{type:"namespace",name:"mcp__<server>",tools:[{type:"function",name:"<tool>"}]}]`이다. `codex-rs/protocol/src/models.rs:2484`/`:2564`는 정상 MCP text를 `output:[{type:"input_text",text:"desktop-local-fixture"}]`로 변환한다. `local-mcp-server.mjs`의 정상 반환을 근거로 이 정확한 출력만 허용한다. flat 광고·문자열 출력·추측 fallback은 지원하지 않는다. 생산자 소스는 변경 중인 후보이며 생성 schema/exe 대조는 남아 있다.

첫 JSON 요청은 명시한 도구가 정확히 한 번 직접 광고됐고 function_call_output이 없는지 확인한다. 그 후 고정 call_id `desktop-fixture-call-1`, namespace/name, arguments 문자열 `{}`의 function_call SSE를 보낸다. 두 번째 요청은 같은 call_id의 출력이 정확히 한 개이고 위 정상 배열 또는 아래의 정확한 timing-prefix 배열과 일치할 때만 고정 assistant message와 completed를 보낸다. 두 응답의 usage는 **합성 0**이다. canned assistant만으로 도구 성공을 선언하지 않는다. 실제 MCP 호출 영수증·Core/Native 연동은 부모 검증 범위다.

## 사전 engineering-test-plan / CS 조건

공개 경계는 CLI child + 실제 own loopback HTTP이며 Node 내장 `node:test`를 사용한다. Node는 기존 2.1 환경의 허용 도구다. 새 의존성·외부 서비스·Core·GUI는 없다. 도구 광고와 결과 oracle은 위 Core 정본 및 local MCP 고정 text에서 가져온다. 검증할 불변조건은 stage 0→1→2만 성공, 동시 승인 최대 1, POST 예산 2, 본문 8MiB 이하, 유한 request/lifetime, 원문 비저장·비출력이다. JSON 파싱에는 이 본문 크기에 비례한 추가 메모리가 필요하며 전체 프로세스 메모리 8MiB를 주장하지 않는다. 정형 AGS plan/proof 파일과 CLI는 소유 허용 4파일 밖이므로 만들거나 실행하지 않는다. 이 문서의 사례는 시험·구현 전에 작성했다.

| ID | 요구/입력 | 실패 oracle / 기대값 |
| --- | --- | --- |
| TP01 | namespace 광고→정상 동일 call_id 출력 | 첫 SSE는 정확 function_call만, 둘째만 assistant; usage 합성0, 정상 자동종료 |
| TP02 | 미광고/flat/중복/deferred 도구 | 고정 422, function_call·assistant 없음 |
| TP03 | 첫 요청 출력/둘째 누락·다른 call_id·오류·다른 text/형태 | 고정 422, 최종 assistant 없음 |
| TP04 | Authorization/Proxy-Authorization/Cookie, 경로·content-type·JSON 오류 | 고정 400/404/415/422; 입력 원문 비노출 |
| TP05 | 8MiB 초과·동시 요청·3번째 POST | 413/429; 성공 응답 상한2·동시1 |
| TP06 | 느린 본문·유한 lifetime·소유 child 종료 | 408 및 서버 종료, own port 재접속 불가, 무관 PID 접근 없음 |
| TP07 | 요청 본문/args/credential/CLI 민감 sentinel | stdout/stderr/HTTP 오류에 원문 없음; 로그는 byte/hash/stage/확인 bool만 |
| TP08 | 잘못된 CLI·미지/중복 option | 고정 오류, child 비정상 종료, 원문 option 비노출 |

## Timing metadata oracle 보완 사전 사례

관련 시험 oracle 결함: `OPENDIY_DESKTOP-D-0086` (TST/P2, 부모 등록 OPEN v1). 여기의 fixture 보완과 단위시험은 해당 결함의 수정 근거이며 독립검수 완료나 실제 Core 최종 turn 성공 판정은 아니다.

고정 Core commit `d6026cbd6157fa18d966031792c537ac383377ac`의 `codex-rs/core/src/tools/context.rs:235`~`:247`은 `Duration::as_secs_f64()`를 `Wall time: {wall_time_seconds:.4} seconds\nOutput:`로 포맷하고 content items 앞에 `input_text` 한 개를 삽입한다. 같은 commit의 `context_tests.rs:220`~`:234`에도 이 header가 별도 항목인 기대값이 있다. 기존 단일 정상 payload는 계속 허용한다. 추가 허용 형식은 정확히 두 항목이며, 첫 항목은 키가 `type,text`뿐인 timing-only `input_text`, 둘째는 기존 고정 정상 payload다. 숫자는 부호·지수·선행 0 없이 정수부 1~10자리(0 허용), 소수부 정확히 4자리인 유한 비음수로 제한한다. 이는 시험용 허용 부분집합이며 Core의 모든 Duration 범위를 지원한다고 주장하지 않는다. header를 결과 text와 합치거나 임의 오류 text를 성공으로 취급하지 않는다.

| ID | 요구/입력 | 실패 oracle / 기대값 |
| --- | --- | --- |
| TP09 | 동일 call_id, 정확 timing header + 기존 정상 결과 | 고정 assistant/completed, 합성 usage 0, 두 receipt만, 원문 비노출·own 서버 정상 종료 |
| TP10 | 잘못된 소수 자릿수/부호/지수/비유한값/11자리 정수/선행 0/단위/제어문자/추가 text | 고정 422, toolResultConfirmed 없음 |
| TP11 | metadata 추가 키/잘못된 type/원문값, 누락·중복·역순·초과 항목, 다른 call_id·결과·추가 field | 고정 422, toolResultConfirmed 없음 |

사례·회귀를 fixture 수정 전에 추가한다. 기존 28개 시험 및 assertion은 유지하며 같은 Node 명령으로 비교한다. 부모가 보고한 별도 `s210t1/a2` 관측은 정확한 일회성 bound 승인 뒤 MCP item 완료와 최종 turn 실패다. 원래 `never` 정책 사례 및 a2 실패는 보존한다. 이 변경은 확인된 Core metadata framing에 맞춘 시험 oracle 보완이며 제품 RED→GREEN이 아니다. 보완 fixture로 실제 Core/Native 재실행은 부모 소유이며 현재 **NOT_RUN**이다. 승인 guard·hint·remember/persistent 권한은 변경하지 않는다.

보완 실행 결과: 동일 시험 SHA256 `b66acc7036db9cfaf57d8dab0cfe4b13a136b7e88d5ed3a08e135d0175b3a322` 및 아래 명령으로 수정 전 **exit 1 / 56 PASS / 4 FAIL**(duration_ms 15217.1735), 수정 후 **exit 0 / 60 PASS / 0 FAIL / 0 cancelled / 0 skipped / 0 todo**(duration_ms 13289.432)를 확인했다. 수정 전 실패는 TP09의 3개 정상 framing에서 `422 !== 200` 및 그 상위 시험 실패다. 기존 28개는 두 실행에서 모두 통과했다. 추가분은 상위 2개와 하위 30개(정상 3개·거절 27개)다. child/동적 loopback은 실행별 소유 격리했으며 별도 소스 사본·정형 AGS CLI 검증은 소유 범위 밖이라 **NOT_RUN**이다. 동일 작업본에서 fixture를 수정하기 전과 후에 시험 bytes·입력·명령을 유지해 비교했다. Core/Native·서비스·제품 빌드·전체 회귀는 이 단위에서 실행하지 않았다.

## 실행 준비

### 선택 wire ID와 실제 경계 확인

원시 내용을 저장하지 않은 `s210t1/a4` HTTP 관측은 정확한 timing/고정 결과와 함께 선택 `id` 키가 있음을 확인했다. 고정 d602의 `ResponseItem::FunctionCallOutput.id`는 `Option<ResponseItemId>`이고 ID는 문자열이다. 이 ID는 `call_id`와 별개이며 호출 권한을 대신하지 않는다. 미제공은 그대로 허용하고, 제공은 비어 있지 않은 UTF-8 256바이트 이하·제어/format 문자가 없는 문자열만 허용한다. null·다른 타입·과도한 값은 거절하고 원값을 로그나 응답에 반사하지 않는다. 다른 call_id·본문·미지 키는 계속 거절한다.

같은 최종 시험 bytes SHA256 `ad22b53e6d81afd088a0cb0a3a5052e47c321f7a8e988faad1e191c51b702320`로 격리한 원본53 provider는 **69 PASS / 8 FAIL, Node exit1**, 수정 provider는 **77 PASS / 0 FAIL / 0 cancelled / 0 skipped, exit0**이다. 원본 fixture SHA256은 `378261ca0dd2f3de262f0cfe66942cfd2d8791f8014515d37e6b691baa10f034`이며 원시 TAP와 receipt는 `D:/codex/build/s210t1/proof-before/`에 보존한다. 이는 fixture 판정 경계의 비교이며 제품·Core의 RED/GREEN이 아니다.

부모의 `s210t1/a5`는 공개 Core1.0.1 고정 app-server에서 명시한 로컬 모델·thread, 정확한 일회성 정상 MCP 승인, 실제 MCP 완료 및 같은 turn 완료, provider 두 단계의 정확한 반환 확인을 실행해 통과했다. own config hash 불변·Core EOF0·provider exit0·listener 닫힘과 관측한 direct child3개의 종료를 확인했다. 승인에는 기억/영구 메타데이터를 보내지 않았고 미확인 요청은 거절한다. 원 never/a2/a3/a4 결과를 보존한다. 이 사례는 실제 Core/stdIO/MCP 경로와 합성 모델 응답의 연동이며 Core1.1·Desktop GUI·실제 추론/과금/cache·모든 하위 프로세스의 일반적 회수 증거가 아니다.

저장소 루트에서 argv 배열 `['node','tests/fixtures/core-tool-provider.mjs','--tool-name','mcp__desktop_probe.desktop_local_probe_v1']`를 사용한다. 실제 Node 실행 파일 경로는 부모가 확인하며 shell 문자열 조합을 하지 않는다. 추가 option은 `--lifetime-ms`(500..60000, 기본30000), `--request-timeout-ms`(100..10000, 기본5000)이다. 127.0.0.1의 **동적 port만** 사용하며 시작 JSON의 origin에 `/v1`을 붙여 별도 own HOME/project/profile의 provider base_url로 지정한다. 기존 local catalog/provider 설정은 부모가 별도로 준비한다. 외부 URL/token/API key, Authorization/Cookie, 실제 HOME/config는 사용하지 않는다.

사전조건: Node 내장 test/fetch 사용 가능, own loopback 허용, 직접 광고된 정확 namespace/tool, 무인자 MCP 도구, Responses HTTP/SSE(압축 요청 미지원), 두 요청이 같은 fixture로 전달됨. WebSocket·도구 검색/defer_loading·평문 flat 도구 광고는 지원하지 않는다. 요청 본문은 검증 중 메모리에서만 처리하며 파일/로그에 저장하지 않는다. stdout은 testOnly 표시의 시작/종료와 byte 수·SHA256·stage·광고/결과 확인 bool만 기록한다. 시작 JSON에는 안전한 loopback origin과 maxRequests도 포함된다. `/health`는 stage/attempts/maxRequests/inFlight만 조회하며 POST 예산을 쓰지 않는다. `/v1/responses` POST는 인증·형식 오류와 동시 거절까지 최대2회 예산에 포함한다. 단일 inflight, Content-Length와 chunked 모두 8MiB 본문 상한, request/lifetime deadline과 자체 연결 정리를 가진다. 실패를 자동 재시도하지 않는다. 정상 두 단계 완료 또는 lifetime/SIGINT/SIGTERM에서 자체 서버와 연결을 닫는다.

검증 명령: `node --test --test-reporter=spec tests/core-tool-provider.test.mjs`. 최초 fixture 실행은 **exit 0, 28 PASS / 0 FAIL / 0 cancelled / 0 skipped / 0 todo**(상위 10개 + 하위 18개, duration_ms 8037.8354)였다. 첫 26개 통과 뒤 인증·형식 오류의 요청 예산과 chunked 초과 본문 두 경계 사례를 추가해 28개를 다시 실행했다. 시험은 실제 child/loopback으로 TP01~TP08을 확인하고 요청 원문·자격증명 sentinel의 stdout/stderr/오류 비노출 및 정상 종료 뒤 own port 접속 실패를 확인했다. 최신 보완 결과는 위 60개 실행이며 최초 기록을 대체하지 않는다. 새 fixture라 기존 제품 결함의 RED→GREEN 증거를 주장하지 않는다. 이 시험 실행 자체에는 실제 Core/Native/MCP 통합·실제 도구 호출 영수증이 없으며 물리 cache/LLM 동작도 판정하지 않는다. 설치·빌드·기존 회귀·최종감사는 실행하지 않았다.
