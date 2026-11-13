# MCP 사용자 base 수정·삭제 검증 계획

[English](mcp-cases.md) · 한국어

대상은 `codex_mcp.rs`의 `list`/`edit`/`remove` helper이다. 기존 add 계약은 보존한다. RPC fixture는 실제 Core 저장, 연결, 현재 thread reload, 도구 발견의 증거가 아니다. 빌드·시험 실행은 부모의 직렬 조정 대상이며 현재 NOT_RUN이다.

## 계약과 사례

| ID | 요구 / entry point | 입력·기대값·실패 판정 | 수준 / 결과 |
|---|---|---|---|
| E01 | stale review / edit, remove | inventory의 user base configVersion과 현재 version 불일치 시 write 0회 | inline native RPC fixture / NOT_RUN |
| E02 | 정확한 한 서버 CAS / edit | `mcp_servers."server.name"`, backend에서 읽은 filePath, expectedVersion, replace 한 번. env secret·미지 필드·sibling 보존 | fixture / NOT_RUN |
| E03 | 유지·설정·삭제 / edit | omitted 유지, args[]/env{} 명시 비움, clearEnv/clearBearerTokenEnvVar 키 삭제. set+clear 동시 입력 거부 | fixture / NOT_RUN |
| E04 | 전송 변경 / edit | STDIO↔HTTP 필수 command/url 확인; 반대 전송의 Core 거부 키만 제거; 공통·미지 필드 보존 | fixture / NOT_RUN |
| E05 | 범위 / list, edit, remove | 사용자 base 단독·effective 일치만 editable. project/profile/managed/disabled/중복/누락 또는 effective 불일치면 write 0회 | fixture / NOT_RUN |
| E06 | 삭제 / remove | 해당 quoted key에 null/replace CAS. 사용자/effective/다른 layer 모두에서 대상 없음 readback; sibling 보존 | fixture / NOT_RUN |
| E07 | 결과 불명 / edit, remove | CAS conflict/readonly/ACK 유실/override/불완전 응답/readback 실패·불일치·다른 layer 재등장 시 실패, 자동 재시도 없음, 오류 secret 미노출 | fixture / NOT_RUN |
| E08 | 기존 add / add | 기존 inline fixture와 GUI baseline 회귀 확인. GUI 56 PASS는 부모 보고이고 이 문서의 새 helper 시험 실행 근거가 아님 | 부모 직렬 실행 대기 |

기존 runner: `cargo test --manifest-path src-tauri/Cargo.toml codex::extensions::mcp::tests -- --test-threads=1`. 추가 runner/의존성/인증 저장소는 만들지 않는다. fixture의 write payload를 독립적인 명세 기대값과 비교하며, sibling은 write payload에 포함되지 않음을 확인한다. red/green·mutation·실제 Core 통합은 NOT_RUN이다. helper 신설은 기존 실패 수정으로 소급하지 않는다. 계획 검사 CLI도 실행 허가 범위 밖이므로 NOT_RUN이다.

## 전송 필드 정본과 삭제 안내

Core `d6026cbd6157fa18d966031792c537ac383377ac`의 `codex-rs/config/src/mcp_types.rs:391`, `:513`의 raw 타입 및 실제 transport 검증을 대조했다.

- HTTP→STDIO: `url`, `bearer_token_env_var`, `bearer_token`, `http_headers_helper`, `http_headers`, `env_http_headers`, `oauth`, `oauth_resource`, `auth` 제거. 기존 HTTP 인증·헤더·OAuth 설정 삭제를 검토 화면에 설명해야 한다.
- STDIO→HTTP: `command`, `args`, `env`, `env_vars`, `cwd` 제거. 기존 실행·인수·환경·작업폴더 설정 삭제를 설명해야 한다.
- `scopes`, `environment_id`, timeout, 도구 정책, 미지 필드는 보존한다. 같은 transport에서는 명시 patch 외 필드를 보존한다. 원문 secret은 metadata·review·오류로 반환하지 않는다.

## IPC 연결 계약

`McpServerPatch { transport?: McpTransportPatch, enabled?: boolean }`; transport는 `type` tagged union이다. STDIO는 optional command/args/env와 clearEnv, HTTP는 optional url/bearerTokenEnvVar와 clearBearerTokenEnvVar이다. omitted/null optional은 유지, 명시 args[]/env{}는 비움, clear flag는 키 삭제이며 set+clear는 거부한다. transport 변경에는 새 command/url을 명시해야 한다.

`edit(directory,name,expected_version,&patch,rpc)`와 `remove(directory,name,expected_version,rpc)`는 저장과 정확한 readback만 확인한다. `list`는 configVersion과 editable을 반환한다. renderer가 raw keyPath/filePath를 지정하지 않는다. 현재 thread reload/status는 별도 부모 소유 구현이다.

## UI 사례 (구현·실행 전 정의)

- U01: 사용자 단독 서버의 수정 화면은 보호된 기존 값을 반환하거나 채우지 않는다. 비운 필드는 유지하고 명시 JSON 배열/객체만 patch에 넣는다. 검토한 name/configVersion을 전송한다.
- U02: 전송 변경 검토는 삭제될 기존 인증·헤더·OAuth 또는 실행·인수·환경·작업폴더 종류를 설명하고 새 URL/인수/환경변수 원문은 요약에 노출하지 않는다.
- U03: 삭제는 한 name/version을 검토·확인하며 취소 때 mutation은 0회다. 설정 삭제를 현재 thread의 도구 정리 완료로 표시하지 않는다.
- U04: 읽기 전용/상위 계층 서버에는 수정·삭제를 제공하지 않는다. 편집 중 재조회 version이 바뀌면 입력을 보존하고 전송을 막는다.
- U05: 변경 응답 대기 중 중복 요청을 막고 exit drain에 포함한다. 창 닫기는 입력 secret을 지우되 이미 보낸 요청을 재전송하거나 취소 완료로 오판하지 않는다.

위 UI 시험도 합성 IPC 경계이며 실제 Core CAS 저장이나 현재 thread 반영의 대체 근거가 아니다.

## 진행 중 실행 결과

2026-11-14 KST 부모 직접 실행: `mcp-native-01`은 필수 THIRD_PARTY_NOTICES 생성 누락으로 테스트 전 build 단계 Cargo101로 중단했다. 원시 기록은 보존했다. 정식 고지 생성기와 작업 전용 offline Cargo 설정으로 준비한 `mcp-native-02`는 표적 15 PASS, 실패·ignored 0, Cargo0이다. 원시 로그·실행 인자·source-sha256은 `D:/codex/build/desktop210-integration-20261114/mcp-native-02/`에 있다. 기존 Rust 경고와 새 unused import 한 건도 기록됐으며, 새 import는 제거했다. 경고가 없었다고 보고하지 않는다.

UI의 최초 새 기능 사례는 아직 없는 수정 버튼을 기다리다 timeout으로 실패했으며 `artifacts/mcp-ui-red/`에 보존했다. 구현 후 같은 UI 사례를 포함한 기존 추가·실패·exit와 신규 수정·삭제·전송 변경·stale/읽기 전용·secret 정산 10건이 `artifacts/mcp-ui-green/`에서 PASS, exit0이다. 이는 기능 추가 전의 부재와 구현 후 동작을 확인한 것으로, 제품에 이미 존재하던 결함을 소급 선언하지 않는다. TypeScript 검사도 exit0이었다. 이후 현재 thread UI 모듈 연결 변경은 다음 영향 회귀 실행에서 다시 확인한다. 실제 Core 저장·변경 후 도구 발견은 계속 NOT_RUN이다.

이후 부모 연결·영문/한글 전환·조회 자료 보존·배치 보완을 포함한 `artifacts/mcp-runtime-ui-03/`의 관리/결과 패널/기존 확장 영향 회귀는 44 PASS, 실패·skip 0, exit0이고 TypeScript 검사도 0이다. 이는 여전히 합성 IPC 검증이며 실제 Core CAS·재로드·도구 호출 완료를 뜻하지 않는다.
