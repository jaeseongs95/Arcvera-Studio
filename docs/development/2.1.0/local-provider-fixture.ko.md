[English](local-provider-fixture.md) | [한국어](local-provider-fixture.ko.md)

# 현재 thread 검증용 로컬 provider 준비물

2026-11-14 KST. **준비물 작성 상태이며 실행은 NOT_RUN**이다. Desktop 2.1/Core의 실제 thread 생성·MCP/hook 조회 경계를 검증하기 위한 test-only 준비다. Core1.1의 미동결 metadata RPC를 추가하지 않는다. 원본 Core, 실제 HOME/config/auth와 운영 앱·daemon은 변경하지 않는다. 아래 실행·구성 작성은 부모가 별도 자체 환경에서 수행할 절차이며, 이 문서를 작성한 단위에서는 수행하지 않았다.

## HTTP 없이 가능한 범위

catalog만으로 인증 정책을 바꾸지는 못한다. 새 자체 custom provider에 `requires_openai_auth=false`를 지정하고 env_key/token/인증 설정을 넣지 않은 상태에서 로컬 `model_catalog_json`을 지정한다. 이 경로는 catalog를 `StaticModelsManager`로 사용하고 TUI login 화면을 건너뛰므로, 모델 추론 없는 thread 생성·조회에는 model HTTP 서버가 필요 없는 소스 경로가 있다. catalog가 모델 turn을 대신하지는 않는다. 실제 TUI가 생성한 threadId가 있어야 하며 빈 화면이나 프로세스 시작만으로 thread 생성 성공을 선언하지 않는다. 다른 시작 동작의 외부 통신까지 없음을 증명한 결과도 아니다.

정본 소스는 `D:/codex/build/core110-context-20261113/source`의 다음 파일이다. 검토 기준은 배정된 d602 계열 소스이며 배포 Core1.0.1 실행 호환성은 아직 미검증이다.

| 근거 | 확인한 계약 |
| --- | --- |
| `codex-rs/core/src/config/mod.rs:2350–2378` | catalog를 ModelsResponse로 읽고 빈 models는 거절 |
| `codex-rs/model-provider/src/provider.rs:549–559` | 명시 catalog는 StaticModelsManager, provider catalog HTTP 조회 경로를 선택하지 않음 |
| `codex-rs/model-provider-info/src/lib.rs:219–229,521–540` | requires_openai_auth=false, env_key 미지정이면 해당 provider API key 불필요 |
| `codex-rs/model-provider/src/provider.rs:879–894` | 비OpenAI provider의 scoped auth가 비어 있는 auth header를 반환하는 기존 시험 |
| `codex-rs/tui/src/lib.rs:2525–2531` | 인증을 요구하지 않는 provider는 login 화면 생략 |
| `codex-rs/tui/src/app_server_session.rs:1123–1184` | TUI의 명시적 thread 시작 요청 경로; 모델 turn과 구분 |
| `codex-rs/app-server/tests/common/config.rs:136–150` | 기존 mock provider의 base_url, Responses wire, retry=0 구성 |
| `codex-rs/app-server/tests/common/mock_model_server.rs:68–83` | created → assistant message → completed 단일 SSE 응답 |
| `codex-rs/core/tests/common/responses.rs:782–819,850–860,1114–1118` | event/data framing, 메시지/완료 payload, text/event-stream |
| `codex-rs/codex-api/src/endpoint/responses.rs:123–152`, `sse/responses.rs:386–391,451–507` | POST /responses, SSE 수신·메시지·완료 해석 |
| `codex-rs/protocol/src/openai_models.rs:414–528,963–1063` | ModelInfo, models wrapper, 기존 base_instructions를 instruction template로 승격; template 누락 거절 |
| `codex-rs/config/src/loader/mod.rs:133–134`, `codex-rs/cli/src/main.rs:504–505` | 사용자 base와 별도 `<name>.config.toml` profile, --profile |

## 파일과 고정 계약

- `tests/fixtures/core-local-catalog.json`: 실제 모델이 아닌 `desktop-local-fixture` 하나의 합성 metadata. text 입력, shell disabled, 합성 context window 32768, high 설정을 광고한다. 기존 ModelsResponse의 `base_instructions` 계약 한 가지를 사용하며 실제 모델 성능·컨텍스트 용량을 뜻하지 않는다.
- `tests/fixtures/core-local-provider.mjs`: Node 내장 http/crypto만 사용한다. `127.0.0.1`에만 bind하고 기본 port=0, maxRequests=1이다. GET /health와 POST /v1/responses만 제공하며 /models·다른 endpoint·WebSocket은 제공하지 않는다. 외부 접속·자식 프로세스·파일 저장이 없다.
- 요청 본문은 버퍼로 모으거나 JSON으로 해석하지 않고 wire byte 수/sha256만 계산한다. 원문 prompt/context, 요청 header, token을 로그에 쓰지 않는다. Authorization/Proxy-Authorization/Cookie가 붙은 model 요청은 값 출력 없이 거절한다. 이는 요청 내용의 의미나 모델 선택을 검증하는 서버가 아니다.
- 모델 응답은 기존 Core fixture와 같은 세 event이며 고정 메시지는 `desktop-local-provider-fixture`다. completed usage의 0과 null은 **test-only 합성값**이다. 실제 토큰 사용량·cache hit·provider cache·모델 추론·MCP 도구 호출 증거가 아니다. cached_tokens/TTL은 만들어 넣지 않는다.
- ceiling: 요청 wire 상한 8 MiB, 동시 model 요청 1개, 연결 상한 4개, headers timeout 5초, request timeout 10초, keep-alive timeout 1초다. `--max-requests`는 1..16만 명시 허용한다. 한도를 넘으면 413/429, 다른 경로는 404, 인증 header/잘못된 content-type은 400/415로 거절한다. 같은 응답을 무제한 재생하거나 실패 후 다른 형식을 시도하지 않는다.

## 부모 재현 준비

Node의 `server.closeAllConnections()`가 있는 Node 18.2 이상이 필요하다. 현재 배포 Core exe의 경로·SHA와 별도 Desktop identifier를 부모가 확인한다. 새 자체 HOME/project/profile을 만들고 상위 경로의 AGENTS/프로젝트 설정도 시험 대상임을 확인한다. process별 환경의 ARCVERA_HOME, HOME/USERPROFILE 등 경로를 자체 디렉터리로 고정하고 실제 auth 저장소나 API key 환경을 상속하지 않는다. 전역 환경을 바꾸거나 실제 인증 자료를 읽어 정리하지 않는다. 설정은 `cli_auth_credentials_store="file"`로 자체 HOME만 사용하며 토큰/키 파일을 만들지 않는다. 조직 정책을 우회하지 않고 정책이 허용하지 않으면 중단한다.

실제 turn을 하지 않는 첫 단계에는 HTTP fixture를 시작하지 않아도 된다. 모델 요청이 발생하면 실패하도록 base_url을 부모가 확인한 미사용 loopback port로 두고, 자신의 모델·provider·catalog 선택과 threadId를 먼저 확인한다. 외부 provider로 fallback하지 않는다.

실제 turn이 명시 허용된 뒤의 HTTP fixture argv는 shell 조합 대신 아래 배열을 process API로 전달한다. nodeExe는 확인된 Node 절대 경로이며 script는 이 작업본의 절대 경로다.

```json
[
  "<nodeExe>",
  "D:/codex/build/desktop210-core110-20261114/tests/fixtures/core-local-provider.mjs",
  "--port", "0",
  "--max-requests", "1"
]
```

listening JSON의 실제 port를 아래 TOML의 `<port>`에 넣는다. 다음은 부모의 **새 자체** `desktop-local-fixture.config.toml`에 넣을 내용이다. `<catalogAbsolutePath>`는 위 JSON 파일의 절대 경로를 forward slash로 표시한다. 이 샘플을 실제 HOME에 쓰지 않는다.

```toml
model = "desktop-local-fixture"
model_provider = "desktop_local_fixture"
model_catalog_json = "<catalogAbsolutePath>"
model_reasoning_effort = "high"
approval_policy = "never"
sandbox_mode = "read-only"
cli_auth_credentials_store = "file"

[model_providers.desktop_local_fixture]
name = "Desktop loopback test fixture"
base_url = "http://127.0.0.1:<port>/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
request_max_retries = 0
stream_max_retries = 0
```

TUI argv 초안은 `["<verifiedCoreExe>","--profile","desktop-local-fixture","--cd","<ownProjectAbsolutePath>"]`이다. positional prompt를 넣지 않는다. 실제 Core1.0.1 help와 profile 선택 지원을 부모가 먼저 대조한다. Desktop이 사용하는 자체 app-server도 같은 effective provider/catalog/profile이어야 한다. 서로 다른 profile/generation이면 중단하고, 실제 endpoint와 SessionInfo.threadId를 확인한 뒤 해당 thread의 조회만 수행한다. daemon 시작/종료 argv는 부모가 이미 확보한 정확한 버전별 계약을 재사용하며 이 문서는 새 명령을 추정하지 않는다. Windows background 실행은 hidden window로 하고 자기 프로세스 handle만 보존해 종료한다.

## 검증 oracle과 남은 한계

1. 무turn 단계: auth/token 없이 올바른 합성 모델·provider·catalog와 자체 threadId가 관측된다. HTTP fixture 요청은 0회다. MCP/hook 현재thread 조회는 그 threadId/generation만 대상으로 하며 실제 상태를 관측한다. 이는 hook 실행이나 도구 호출 검증이 아니다.
2. 명시 허용된 단일 합성 turn: health가 0에서 1로 바뀌고, byte 수/sha256과 고정 세 event 메시지·Core turn 완료를 서로 대조한다. health의 completedRequests는 본문 수신 후 응답을 시도한 횟수이며 Core 완료 ACK가 아니다. 실패를 재전송으로 숨기지 않는다.
3. 요청이 외부로 향하거나 auth header가 필요하거나 다른 wire/endpoint를 요구하면 실패로 남긴다. 이 준비물에 추정 response 형식을 추가하지 않는다. Core1.0.1과 d602 계열의 차이는 실제 실패 근거로 별도 확인한다.
4. fixture 종료 뒤 자기 listener/연결·자식 자원만 회수하고 운영 앱·daemon·auth 보존을 확인한다. 정상 사용·성능·메모리·실provider cache·실모델 품질·최종 Core1.1 통합은 이 준비물의 범위 밖이다.

이 단위는 파일 작성과 소스 직접 대조만 수행했다. syntax check, JSON parser/schema 검증, 서버 시작, HOME/config 생성, catalog 로드, 실제 Core/TUI/Desktop thread와 turn, MCP/hook 조회는 모두 **NOT_RUN**이다.
