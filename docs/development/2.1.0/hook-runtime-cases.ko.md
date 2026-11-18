# 현재 대화 Hook inventory: 기존 wire 소비 사례

이 문서는 구현과 fixture 작성 전에 준비한 검사 계획이다. 상태는 **NOT_RUN**이다. 이 단위에서는 빌드·시험·실제 Core 호출을 실행하지 않는다. 부모 담당이 동일 후보의 native fixture, 호출자 결속, 실제 Core와 GUI를 검증한다.

## 근거와 범위

- Desktop 기준: `50a237e83ce430c7faa7c2994b91f21460283fab`의 `src-tauri/src/codex_runtime_extensions.rs`, `codex_hooks.rs`, `codex_extensions.rs`.
- Core 기준: `d6026cbd`, `app-server-protocol/src/protocol/v2/plugin.rs`의 `HooksListParams`, `HooksListResponse`, `HooksListEntry`, `HookMetadata`; `app-server/src/request_processors/catalog_processor.rs`의 `hooks_list_response`.
- 기존 공개 요청 `hooks/list {"threadId":...}`는 `cwds` 없이 loaded thread를 찾고 `thread.config_snapshot().cwd()`와 `thread.list_hooks()`를 반환한다. 응답은 `data` 한 항목의 `cwd/hooks/warnings/errors`이며 thread ID나 실행 gate 확인 응답은 없다.
- `hooks_to_info`는 builtin을 제외하므로 반환 목록은 전체 실행 handler 수가 아니다. 새 1.1 `scope/runtime/featureEnabled/baseHandlerCount` metadata는 해석하지 않는다. 미해석을 서버 미제공으로 단정하지 않는다. 설정 목록 조회·승인·enablement·mutation·모델·프로세스 setter를 바꾸지 않는다.

## API와 의미

`RuntimeExtensionRequest::HooksStatus {}`는 추가 필드를 거부한다. `validate`는 기존 thread ID의 UTF-8 256바이트·공백/제어문자 검사를 재사용한다. `exchange_hooks(rpc, thread_id, cwd)`는 호출자가 이미 허용한 연결에서 정확히 한 번 조회한다. cwd가 없는 기존 `exchange`에 Hook 요청을 넣으면 송신 없이 실패한다.

반환 DTO는 `{operation:"hooksStatus",threadId,provenance:"loadedThreadHooksList",executionGate:"unknown",applicationState:"unknown",hooks:{items:[{id,eventName,handlerType,source,enabled,isManaged,trustStatus}],warningCount,errorCount}}`이다. item 필드는 기존 파서가 검증한 enum·bool 및 raw key의 SHA256을 `hook-` 접두어와 소문자 hex로 표현한 읽기 전용 id만 명시 허용 목록으로 투영한다. id는 대응·구분용이며 원문 key나 권한·승인 토큰이 아니다. 기존 trust mutation의 key로 재사용할 수 없다. provenance는 사용한 요청 경로의 출처이며 서버가 새 metadata를 확인했다는 뜻이 아니다. 요청 대상 thread와 cwd를 묶고 지연 응답을 거르는 것은 부모 호출자의 책임이다. 기존 wire는 thread ID를 echo하지 않으므로 없는 echo를 만들어 검증하지 않는다. 존재하는 상위 `threadId`가 다른 경우에는 거부한다.

기존 안전 파서로 필수 필드·enum·중복·유한 숫자·절대 sourcePath·정확한 cwd를 확인한다. 이 파서는 검증 입력으로만 사용하며 전체 결과를 복제·직렬화하지 않는다. Core는 key를 source와 event/선언 위치에서 만들고, sourcePath는 원래 선언 파일 경로, currentHash는 선언/플러그인 신뢰 승인용 fingerprint로 반환한다. runtime에 필요한 동작 key가 아니므로 모두 투영에서 제외한다. 원문 name/action ID·pluginId·server/tool·command/matcher/statusMessage·diagnostic 원문/path·미지 필드·불사용 prompt/agent 본문·입력·환경·인증 원문도 반환하지 않는다. warnings/errors는 개수만 투영한다. 기존 설정 화면의 원문 검토 파서는 그대로 둔다.

## 불변조건과 상한

- 단일 송신, 자동 retry·추가 RPC·새 연결·자원 소유권 없음. 오류/빈 결과를 실행 성공이나 gate 상태로 바꾸지 않는다.
- 공유 Rpc의 8 MiB frame 상한과 기존 Hook 파서의 4096개 목록, diagnostics 128개, command/matcher/statusMessage 64 KiB 등 필드별 상한을 유지한다. 문자열 상한의 단위는 UTF-8 바이트이다.
- MCP 전용 256개 배열/8 KiB 문자열/1 MiB DTO 제한을 Hook에 적용하지 않는다. 사용하지 않는 큰 원문은 공유 frame 상한 안에서 허용하고 투영하지 않는다. Hook DTO에는 별도 8 MiB 직렬화 상한을 적용한다.
- 이 단위는 캐시/큐/재시도/동시성 제어를 추가하지 않는다. CS의 ACK 의미 분리와 유한 입력 경계를 적용하며, 실제 수명·역순 epoch·busy/exit 검증은 부모 통합 범위이다.

## 사전 사례와 oracle

진입점은 별도 표기가 없으면 `exchange_hooks`이고, 기존 Rust `#[test]`와 메모리 WebSocket fixture를 사용한다. fixture는 실제 Core/CLI/네트워크 호출이 아니며 실제 loaded thread 보장은 별도 검증해야 한다.

| ID | 시나리오 | 독립 기대값/실패 판정 |
| --- | --- | --- |
| HR01 | 정상 command/mcpTool/prompt/agent | 송신은 정확한 `hooks/list {threadId}` 한 번; cwds 없음; inventory와 provenance, 두 unknown 상태; command 등 원문 없음 |
| HR02 | 잘못된 thread·cwd 입력, cwd 없는 `exchange` | 송신 0; 고정 안전 오류; HooksStatus 추가 필드 역직렬화 실패 |
| HR03 | 다른 cwd·중복/복수 project·상위 다른 thread echo | 실패, 다른 프로젝트 내용 투영 없음; 송신 1 |
| HR04 | data/hooks/warnings/errors 누락·형식 오류, 사용 필드/enum/숫자/절대경로 오류·중복 key | 실패; 성공 빈 목록으로 대체하지 않음 |
| HR05 | 레거시 empty | 성공 빈 inventory, executionGate/applicationState는 unknown; enabled/disabled 추정 없음 |
| HR06 | 257개 및 4096개 Hook | 목록을 자르지 않고 전부 반환; 4097개는 실패; MCP 배열 상한과 분리 |
| HR07 | 불사용 원문 >8 KiB·큰 배열/넓은 객체, 사용 command 64 KiB 및 초과 UTF-8 | 불사용 원문 허용·미노출; 경계 command는 검사 후 삭제; 초과 사용 필드는 실패 |
| HR08 | key/hash/sourcePath/pluginId/server/tool·warnings/errors·command/matcher/statusMessage·unknown fields에 secret sentinel | 성공 결과와 실패 문자열에 sentinel 없음; diagnostics 개수만 유지; 읽기 전용 id와 enum/bool 허용 필드만 존재 |
| HR09 | 지원 안 됨·transport·timeout·연결 종료·취소 | 안전한 실패 분류, 원문 오류 없음, 송신 1·자동 retry 없음 |
| HR10 | raw frame >8 MiB 또는 합성 Hook DTO >8 MiB | 각각 기존 Rpc와 Hook projection byte 경계가 거부; MCP 기존 계약/fixture는 보존 |
| HR11 | 같은 key 재조회, 다른 key·동일한 event/type/source | 같은 key의 id는 안정적, 다른 key는 구분; 알려진 SHA256 벡터와 일치; 원문 key 없음 |

## 실행 인계와 한계

사전 계획 → fixture → 구현 순서로 변경한다. 부모가 기존 Rust 테스트 runner로 `runtime_extensions_hooks_` 필터와 MCP 회귀를 실행한다. 테스트 명령·결과·후보 hash는 실행 담당의 근거에 남긴다. 이번 문서와 fixture 준비를 실제 Core 동작·native GUI·gate 반영·2.1 릴리스 완료의 증거로 쓰지 않는다.
