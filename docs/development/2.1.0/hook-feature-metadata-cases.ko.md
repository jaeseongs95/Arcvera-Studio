# 현재 대화 Hook 기능 메타데이터 후보 소비

구현 전 검사 계획. **NOT_RUN**: 이 단위는 빌드·시험·서버를 실행하지 않는다. 부모가 Rust inline fixture, Playwright source fixture와 통합을 직렬 검증한다. fixture는 실제 Core·CLI·네트워크나 실행 증거가 아니다.

## 근거와 계약

Core 후보 `core110-release-20261114/source/codex-rs/app-server-protocol/src/protocol/v2/plugin.rs`의 `HooksListResponse`, `HooksListScope`, `HooksRuntimeInfo`를 직접 대조했다. 생성 schema·컴파일 결과·개발 실행 파일은 미수신이므로 후보 소비 준비를 API 지원 완료로 판정하지 않는다. 기존 inventory 사례를 대체하지 않고 다음 경계만 추가한다.

- 기존 `hooks/list {threadId}` 단일 송신과 thread/cwd 결속, 안전 Hook 파서·4096행·공유 8 MiB frame 경계를 유지한다. MCP 경로·설정/신뢰 mutation·읽기 전용 SHA256 ID를 변경하지 않는다.
- DTO에 `runtime: {featureEnabled:boolean,baseHandlerCount:number} | null`을 추가한다. Native는 항상 runtime을 내보내며 UI는 구버전 DTO의 필드 부재도 수용한다. count는 0 이상 `9007199254740991` 이하 정수이다. `featureEnabled=false`에도 builtin Hook이 동작할 수 있다. count는 설치된 base handler만 세며 per-turn 환경/executor overlay를 제외한다. inventory 행 수와 같다고 가정하지 않는다.
- wire scope가 `currentThread`일 때만 유효한 runtime 객체를 투영한다. scope 부재/null은 레거시로 runtime=null이며, runtime 부재/null도 null이다. 명시 `configuration`·미지 값·잘못된 scope는 고정 안전 오류로 거절한다. 존재하는 non-null runtime은 scope가 없어도 bool/count 형식을 검사한다. malformed를 unknown으로 숨기지 않는다.
- Native는 runtime 미지 필드를 투영하지 않는다. 순수 패널은 정확한 두 runtime 필드만 수용하며 추가 필드를 거부한다. 원문·비밀·명령·진단·미지 필드는 표시하지 않는다.
- `executionGate`와 `applicationState`는 계속 unknown이다. 일반 기능 상태·개별 사용 설정·신뢰·실행 증거를 구분한다. runtime이 없으면 0/false를 만들지 않는다. pending/unconfirmed/invalid는 이전 runtime과 목록을 지운다.
- 공개 패널 API는 그대로다. 부모 disabled, 언어 상태와 dispose 수명을 보존한다. 네트워크·타이머·재시도·변경 버튼·새 구독은 추가하지 않는다.

## 사전 사례와 독립 oracle

Native 진입점은 `exchange_hooks`, UI 진입점은 `createHookRuntimePanel().render`이다. 기존 Rust 메모리 RPC fixture와 `tests/hook-runtime.spec.ts`의 격리 source harness를 재사용한다. 부모 실행 명령은 기존 Rust `runtime_extensions` 필터와 `npx playwright test tests/hook-runtime.spec.ts`이다. 추가 check-plan/빌드 명령도 이 작업자의 실행 금지 범위에 포함한다.

| ID | 경계 | 기대 결과 |
| --- | --- | --- |
| HF01 Native | 레거시 scope/runtime 부재·null, scope 없이 정상 runtime | inventory 유지; runtime=null; gate/application unknown; 송신 정확히 1회 |
| HF02 Native | currentThread false+3, true+0, 최대 safe count | 정확한 두 값만 투영; inventory 행 수와 분리; gate/application unknown |
| HF03 Native | configuration·미지·타입 오류 scope | 고정 안전 오류; raw sentinel 없음; 재송신 없음 |
| HF04 Native | runtime 누락/null, bool/count 누락·오류·음수·소수·unsafe | 누락/null만 unknown; malformed는 실패; sentinel 비노출 |
| HF05 Native | runtime의 큰 불사용 raw/비밀 필드 | 기존 frame 안에서 허용; allowlist 두 필드만 반환; 단일 조회 |
| HF06 UI | 기존 DTO 부재와 명시 null, false+3, true+0 | 미제공은 false/0 대신 미확인; 명시 기능/설치된 base 개수 표시; builtin/overlay 한계와 두 unknown 유지 |
| HF07 UI | malformed bool/count·추가 필드·HTML/비밀·다른 thread | false 반환; 이전 목록/runtime clear; 고정 안내; 원문 비노출 |
| HF08 UI | disabled→언어 전환→재렌더→pending/unconfirmed→dispose | 부모 disabled 유지; 2언어 설명; 이전 runtime clear; 폐기 후 callback/DOM 부활 없음 |

기존 HR/HU assertion을 삭제·skip하지 않는다. HR05의 레거시 미해석 runtime은 이제 명시 runtime=null DTO를 기대하며 원래 gate unknown·sentinel 비노출 oracle은 보존한다. 신규 Native 5개/HF01–05, UI 3개/HF06–08 시험을 작성한다. 실제 Core1.1 같은 후보짝, 생성 schema, Native GUI·busy/epoch·최종 패키지와 독립감사는 부모의 미완 검증 범위다.

부모 후속 실행: `context-hook-ui-03` source/IPC fixture 57 PASS/49.3초/exit0(구성 패널14·부모 컨텍스트25·Hook panel13·manager5), tsc exit0. 새 Native5는 로컬 자원 보호를 유지하며 정확 commit CI에서 검증한다. 현재 실제 Core1.1 증거로 확대하지 않는다.
