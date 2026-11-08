# MCP 서버 추가 요구 및 검증 사례

대상: 기존 확장 관리의 MCP 탭. 선택한 CLI의 사용자 base config에 새 이름만 추가한다. 운영 설정·인증·공유 daemon·다른 세션은 접근하지 않는다. 공식 0.167.0과 OpenDIY CLI의 `config/read`, `config/value/write` 공통 계약을 사용한다. 설정 저장과 실행 중 세션 반영·연결 성공은 구분한다.

## Engineering test plan

- R1: STDIO(command/개별 args/선택 env)와 Streamable HTTP(URL/선택 bearer 환경변수 이름)를 추가할 수 있다.
- R2: 서버 이름을 임의 keyPath로 해석하지 않는다. 동일 이름이 어느 반환 계층에라도 있으면 쓰지 않는다. 사용자 base layer의 정확한 절대 경로와 expectedVersion으로 새 key 하나만 쓴다.
- R3: 기존 서버·미지 필드·비밀값을 재직렬화하지 않는다. renderer에는 이름·전송·활성 상태·설정 범위만 보낸다. RPC 진단 원문·URL·command·args·env·header·OAuth secret을 목록/오류로 반환하지 않는다.
- R4: readonly, 충돌, 미지원, timeout, 연결 실패 및 저장 후 검증 실패를 성공으로 바꾸거나 자동 재시도하지 않는다.
- R5: 기존 IPC/admission/실행파일 고정/프로세스 소유권을 유지한다. 저장 후 새 세션에서 확인하도록 안내하며 기존 세션 reload를 주장하지 않는다.

공통 사전조건: native 단위시험은 기록된 fixture 응답만 사용한다. 실제 CLI 통합시험은 별도 `D:/codex/build/<id>`와 짧은 격리 CODEX_HOME/TEMP를 사용하고 실제 인증을 읽거나 복사하지 않는다. UI 시험은 기존 Playwright runner/1420 포트를 root가 직렬 실행한다. mock 시험은 실제 CLI 저장·연결의 증거가 아니다.

| ID / 요구 | 입력·사전조건 | 절차 / 공개 경계 | 기대 결과 및 pass/fail 기준 | 실제 결과 |
|---|---|---|---|---|
| M01 / R1,R2 | 빈 fixture config, `server.name`, STDIO와 공백·따옴표·빈 문자열 args | `mcp::add` → CAS write → readback | 정확히 한 quoted keyPath, args 배열이 문자 그대로 저장되고 readback 일치. shell 호출 없음 | NOT_RUN |
| M02 / R1 | HTTP fixture, bearer 환경변수 이름 | UI HTTP 폼 → review → 기존 extensions_mutate | URL/환경변수 이름이 typed payload에만 들어가고 review에는 비밀값 없음 | NOT_RUN |
| M03 / R2,R3 | 기존 사용자/프로젝트/비활성 layer 동일 이름과 secret sentinel | 추가 요청 | write 0회, 기존 config 및 비밀값 보존. 다른 이름 추가는 sibling을 쓰기 payload에 포함하지 않음 | NOT_RUN |
| M04 / R2 | user layer 없음/중복/상대 경로/profile만 존재 | 추가 요청 | write 0회, 정확한 실패. 임의 경로·기본 홈 추정 없음 | NOT_RUN |
| M05 / R2,R4 | 조회 뒤 fixture CAS conflict | 추가 요청 | 한 write만 시도, 충돌 오류, 자동 재시도 없음 | NOT_RUN |
| M06 / R3 | config에 URL query/command/args/env/header/OAuth sentinel, RPC 오류에도 sentinel | list 및 실패 요청 | serialized inventory·오류에서 sentinel 없음. 이름/전송/활성/범위만 노출 | NOT_RUN |
| M07 / R4 | readonly, method not found, timeout, disconnect | 각 실패 주입 | 실패 종류 구분, 원문 진단 미노출, 결과 불명은 refresh 안내 | NOT_RUN |
| M08 / R4,R5 | write okOverridden, 잘못된 filePath, readback 값 불일치/실패 | 추가 요청 | 저장 가능성을 명시하되 성공 반환 없음. reload 호출 없음 | NOT_RUN |
| M09 / R1,R2 | 잘못된 이름, transport 혼합/미지 필드, 잘못된 args/env/URL | deserialize/추가 요청 | RPC 전에 거부. 불명확한 입력을 다른 의미로 바꾸지 않음 | NOT_RUN |
| M10 / R1,R3 | UI env 비밀 sentinel 입력 | 폼 → review → apply → 닫기/다시 열기 | review 텍스트에 env/URL/args 없음, payload 정확, 저장·닫기 후 입력 삭제 | NOT_RUN |
| M11 / R4,R5 | section 조회 실패 또는 mutation pending | 탭/refresh/close/exit 경쟁 | MCP 추가 불가 상태·오류 표시, 중복 제출 없음, 기존 exit drain 유지 | NOT_RUN |
| M12 / R1,R5 | 공식/OpenDIY 실제 실행파일, 새 격리 config, fixture MCP | 각각 UI 저장 → 실제 config/API readback → 새 세션 discovery | 두 CLI에서 신규 서버 정확 저장·도구 발견. 운영 config 무변경. 저장만 성공하면 연결 성공으로 보고하지 않음 | NOT_RUN |

## 실행 명령 및 경계

- 준비: `npx tsc --noEmit` (독립 정적 검사 허용)
- root 직렬 실행: `cargo test --manifest-path src-tauri/Cargo.toml codex::extensions::mcp::tests -- --test-threads=1`
- root 직렬 실행: `npx playwright test tests/mcp-manager.spec.ts tests/extensions.spec.ts`
- 실제 CLI 통합(M12), 빌드/번들, 운영 GUI 반영은 이 단위의 mock/native 단위시험과 별도로 확인한다.
- 현재 결과: 모든 동작시험 NOT_RUN. 서버/Cargo 실행은 root의 직렬 검증 단계까지 보류한다. red/green·표적 변이는 미실행이며 정적 타입 검사로 대체하지 않는다.

## 구현 및 정적 검사 결과

- 구현: `codex_mcp.rs`의 `list`/`add`를 기존 `extensions_list`/`extensions_mutate`에 연결했다. IPC 작업은 `addMcpServer`, 입력은 `name`과 tagged `transport`이다. STDIO는 `{type:"stdio",command,args,env}`, HTTP는 `{type:"streamableHttp",url,bearerTokenEnvVar}`이다. 목록은 `mcpServers:{items:[{name,transport,enabled,configuredIn}],userFile}`만 반환한다.
- `npx tsc --noEmit`: 종료 0.
- `node --experimental-strip-types --check tests/mcp-manager.spec.ts`: 종료 0. 시험 파일 문법 검사이며 브라우저 동작시험이 아니다.
- `git diff --check -- src/extensions.ts src-tauri/src/codex_extensions.rs`: 종료 0.
- `rustfmt --edition 2021 --emit stdout src-tauri/src/codex_mcp.rs | Out-Null`: 종료 1. PATH에 rustfmt가 없어 실행되지 않았다. Rust 문법·타입·동작 검증은 NOT_RUN이다.
- root 연결 사항: 기존 IPC 재사용으로 lib/session/main 변경 불필요. `playwright.config.ts`의 sourceFixtures에 `mcp-manager`를 넣어 source module fixture를 bundle 시험에서 제외해야 한다.
- HTTP URL은 HTTP(S) absolute URI이며 userinfo/fragment를 거부한다. OAuth 로그인 UI와 기존 서버 편집·삭제·기존 세션 재로드는 이번 추가 기능의 구현 범위가 아니다.

## 첫 UI 실행 실패와 fixture/oracle 수정

- root 전달 첫 실행 결과는 전체 43개 중 34 PASS / 9 FAIL, 종료 1이다. 이 집계는 root 보고이며 여기서 별도로 전체를 재집계하지 않았다.
- MCP 5개는 앱이 이미 만든 extensions dialog를 남긴 채 isolated host fixture가 manager를 하나 더 만들어 중복 ID strict mode에서 실패했다. 직접 확인한 원문은 `test-results/followup-first/mcp-manager-M01-M10-STDIO--320d4-crets-and-save-clears-input/error-context.md`의 `#extensions-tab-mcpServers resolved to 2 elements`이다. 제품 실패로 바꾸거나 원 FAIL 산출물을 지우지 않는다.
- fixture는 정확히 하나의 닫힌 기존 app dialog만 확인해 제거한 뒤 isolated manager를 만든다. `.first()`로 중복 ID를 무시하거나 locator strict 검사를 완화하지 않는다.
- 기존 `extensions.spec.ts`의 네 탭 End 기대값과 영어 제목은 MCP 기능 추가로 달라진 명세를 반영한다. 시험명을 four에서 five로 바꾸고 skills/hooks/marketplaces/MCP/plugins 전체 순회, End/Home, 뒤로 wrap, 추가 catalog 조회 없음 assertion을 유지·보강했다. 언어 전환 시험은 제목만 MCP 포함으로 바꾸며 기존 초안·한 건 보류 mutation·터미널 보존 assertion을 그대로 유지했다.
- 수정 후 Cargo/브라우저 재실행은 NOT_RUN이다. root의 1420 직렬 재실행을 기다린다. 제품 코드는 이 fixture/oracle 수정에 포함하지 않는다.
