# Runtime 최초 해결 경쟁 — 사전 사례와 검증 계획

대상: `fix/desktop-1.6.1-input-compat`, base `3a2` 계열. 제품 변경 전에 작성했다. 기존 공식 CLI smoke01 FAIL은 그대로 보존한다. 이 문서의 시험은 별도 root CI에서 실행할 예정이며, 작성 시점의 모든 신규 Rust 시험과 RED/GREEN은 **NOT_RUN**이다.

## 관측과 최소 변경

기존 기본 smoke의 동시 두 `Engine.start` 중 하나는 `실행파일 선택이 바뀌었습니다. 다시 시도하세요.`로 거부됐다. `runtime_pair_snapshot_with`는 잠금 밖 resolve 전후 generation을 대조하는데, 최초 memo 채우기도 generation을 증가시키므로 동일 선택의 동시 최초 resolver를 사용자 설정 변경과 같은 것으로 거부한다. 다른 세션의 UUID 미결속 원인은 이 수정의 범위 밖이다.

쓰기 범위는 `session.rs`의 해당 함수, 기존 `session_runtime_source_tests.rs`, 이 문서다. `Engine.write`, frontend, CLI/R38, 운영/인증/GUI는 변경·실행하지 않는다. 새 의존성·새 runner·로컬 Rust 빌드는 만들지 않는다.

최초 진입 상태가 완전히 미해결(모든 memo 필드 None, ready/preparing false)이고, resolve 뒤 정확히 한 세대만 증가했으며 CLI/server/kind/manifest 전체가 현재 memo와 같을 때만 현재 generation에 합류한다. 사용자 설정 변경은 generation 증가와 memo 무효화이므로 변경만 있으면 pair가 다르고, 변경 후 재해결이면 최소 두 세대가 증가한다. 준비 상태·preparation 번호는 합류가 변경하지 않는다. 이미 해결된 snapshot에는 이 예외를 적용하지 않는다. generation mismatch를 일반적으로 무시하지 않는다.

## CS 제약 (cs-engineering)

- RFR-FENCE / CONC-FENCE-002: 사용자 선택 변경, 같은 경로 ABA, 다른 manifest 또는 pair, 기존 memo에서 온 늦은 결과는 현재 값을 덮거나 ready를 되살릴 수 없다. mutex 아래 generation과 전체 pair를 함께 확인한다.
- RFR-LOCK / CONC-LOCK-003: 외부 resolve 및 prepare callback 중 runtime mutex를 잡지 않는다. 신규 시험의 실제 두 OS thread와 mpsc arrival/release barrier로 순서를 고정한다. sleep으로 경합을 추정하지 않는다.
- RFR-CANCEL / CONC-CANCEL-004: resolver 오류/취소와 준비 취소는 현재 유효한 prepared 결과를 오염시키거나 연결을 허용하지 않는다.
- RFR-READY: 같은 최초 해결에 합류해도 ready/preparing/preparation을 수정하지 않는다. 기존 준비 중 거절 정책은 보존한다.

위 제약은 구현 검토 조건이며 통과 선언이 아니다. 정적 변경 범위·락 경계는 직접 확인하고 실제 동시성·취소 결과는 native CI로 확인한다. 현재 CS 동적 검증 상태는 UNVERIFIED다. root workflow 338의 기존 범위를 사용하며 별도 workflow/승인/장부 DB를 만들지 않는다.

## 공통 입력·전제·공개 경계

Windows 기존 native Rust runner의 `session::runtime_source_tests::first_resolution_tests`에서 실행한다. 기존 child test module을 재사용하고 새 module 선언을 만들지 않는다. `Engine::new(:memory:)`, 합성 CLI/server 경로와 source manifest만 사용하며, 실제 CLI·파일 선택·네트워크·모델을 호출하지 않는다. `runtime_pair_snapshot_with`가 문제가 있는 실제 `Engine.start`의 resolution 경계이므로 이 컴포넌트 경계에서 시험한다. 전체 Engine.start/PTy/공식 CLI 정상 사용을 이 결과로 주장하지 않는다.

각 worker는 별도 OS thread에서 실제 resolver에 진입한 뒤 arrival을 전송하고 release 채널을 기다린다. 첫 worker의 arrival을 받은 뒤 둘째를 생성하여, 외부 callback의 try_lock 검사가 다른 worker의 정상 최초 잠금과 경합하지 않게 한다. 두 resolver가 함께 대기하는 조건은 유지한다. main은 두 arrival을 모두 받은 뒤 한 worker만 먼저 확정시키고 다른 worker를 해제한다. 채널 deadline은 5초이며, 실행기 실패/timeout은 assertion RED와 구분한다. 같은 테스트 바이트와 명령을 RED/GREEN에서 유지한다.

| ID / 요구 | 입력·사전조건 | 절차 | 기대 결과 및 pass/fail 기준 | 작성 시 실제 |
|---|---|---|---|---|
| RFR01 / 동일 최초 해결 합류 | 비어 있는 Engine, 같은 CLI/server/kind/manifest를 반환하는 두 resolver thread | 두 arrival barrier → 첫 resolver 확정·prepare 성공 → 둘째 release | 둘 다 같은 generation/전체 pair. generation 증가는 한 번. 기존 prepared/ready/preparation 보존. old 함수에서는 둘째 성공 assertion이 실패해야 올바른 RED | NOT_RUN |
| RFR02 / 사용자 저장 fence | 비어 있는 Engine, 첫 resolver 대기 | 실제 change_runtime_selection_with 성공 → resolver release | 늦은 결과 Err, 새 generation·비어 있는 memo·ready=false 유지 | NOT_RUN |
| RFR03 / 같은 경로 ABA | 첫 resolver가 미해결 세대에서 대기. 다른 resolver가 A 확정 | 실제 설정 변경 두 번 → 같은 A 재해결·prepare → 첫 resolver release | 전체 경로가 다시 같아도 늦은 결과 Err. 최신 generation/ready/prepared 유지 | NOT_RUN |
| RFR04 / 전체 pair 식별 | 최초 resolver 대기; CLI/server/kind/manifest 중 한 필드만 다른 승자 | 다른 pair 확정·prepare → 첫 resolver release. 네 입력 각각 실행 | 모두 Err. 최신 pair/manifest/prepared 유지. manifest 차이를 경로 동일로 덮지 않음 | NOT_RUN |
| RFR05 / 이미 해결된 memo | A가 이미 확정. B를 해석하는 오래된 resolver 대기 | 다른 resolver가 B 확정·prepare → 오래된 B release | 현재 pair와 같고 +1이어도 Err. 최초 빈 memo 합류로 확대하지 않음 | NOT_RUN |
| RFR06 / resolver 취소·오류 | 최초 resolver 대기, 다른 resolver는 같은 pair를 확정·prepare | 취소 flag 설정 후 resolver가 Err 반환 → release 결과 확인 | Err 유지, 최신 generation/ready/prepared 불변. 오류를 합류 성공으로 바꾸지 않음 | NOT_RUN |
| RFR07 / 준비 진행 상태 보존 | 동일 최초 resolver 둘, 첫 pair가 확정되고 prepare callback은 대기 | 둘째 resolver 합류 → 준비 요청 시도 → 첫 prepare release | 합류 자체는 성공하지만 ready=false/preparing=true/preparation 불변. 중복 준비는 기존대로 거부. 첫 prepare 완료 뒤 ready 정상 | NOT_RUN |
| RFR08 / 합류 뒤 취소 fence | RFR01에서 얻은 유효 snapshot/prepared | cancelled=true로 prepare_start_with 호출, prepare/connect callback은 panic sentinel | Err, callback 0회, 기존 ready/prepared는 다른 미취소 조회에 유효 | NOT_RUN |

기존 `runtime_preparation_tests`의 server path/role, manifest 변경, stopped_start, selection_save_and_resolution, 준비·연결 중 선택 변경 및 stop/exit 사례도 그대로 실행한다. 기존 assertion은 약화하지 않는다.

## 실행·증거 계획

고정 명령(후속 root CI): `cargo test --locked --manifest-path src-tauri/Cargo.toml session::runtime_source_tests::first_resolution_tests -- --nocapture --test-threads=1` 및 기존 `session::runtime_preparation_tests` 필터. root 최종 전체 native CI가 추가 회귀를 확인한다. 이 작업자는 위 명령을 실행하지 않는다.

RED: base의 원 함수 + 새 테스트 파일 그대로를 격리 CI 후보에 결합. RFR01의 같은 generation 합류 성공 assertion이 `실행파일 선택이 바뀌었습니다` Err 때문에 실패해야 한다. 컴파일 실패/환경 오류는 유효한 RED가 아니다. GREEN: 최소 함수 수정 + 동일 테스트 바이트·argv. mutation/old 코드로 사용자 작업 트리를 reset하지 않는다.

로컬 확인: 새 checkout의 `target`, `src-tauri/target`이 모두 부재이고 D 드라이브 여유 약 175.9GB였다. 재사용 가능한 exact native 캐시를 확인하지 못했으며 새 Rust 로컬 빌드 금지에 따라 로컬 RED/GREEN 및 native 실행은 NOT_RUN이다. 모델/CLI/GUI 실행도 NOT_RUN이며 이 범위에서는 수행하지 않는다.
## 구현 후 정적 확인과 인계

- 제품 수정은 `runtime_pair_snapshot_with` 안의 18줄 변경뿐이며 외부 resolve는 기존처럼 잠금 밖이다. 새 상태 필드·선택세대·retry·프로세스·대기 큐를 추가하지 않았다. `Engine.write`와 다른 함수는 이 작업자가 수정하지 않았다.
- 기존 `session_runtime_source_tests.rs`에 신규 7개 시험으로 RFR01~08을 연결했다. RFR08 취소 admission은 `concurrent_first_resolution_joins_only_the_identical_pair` 안에 있다. RED에서 RFR01과 RFR07은 기존 generation mismatch 거절을 잡아야 하며, 나머지 fence 시험은 그대로 통과해야 한다. 실제 실행 전 예측이며 RED/GREEN 결과가 아니다.
- 고정 시험 파일 SHA256: `2F90F8B77F599A7F7305DE34E1F8AAB29BA3FA2213A72403D1D5416D4FF362F2`.
- 원 함수부터 다음 `verify_runtime_selection` 직전까지 UTF-8 LF 바이트 SHA256: `DB83EB3BDAEC27AE78567B189F80469F8A5F351F53796B5F50B25682B7D18E16`. 수정 함수 같은 구간 SHA256: `DBD5F57E4AA02EB502BAD84699B5A2617EF28F83163E8E7BCD4D1AF92B3F2E7E`.
- `git diff --check` 실제 exit 0. 수정 함수 diff, 신규 시험의 lock 해제 순서, 세 파일의 UTF-8 no BOM/LF 보존을 직접 확인했다. 컴파일·Rust 시험·실제 동시성 RED/GREEN·공식 CLI 재실행·GUI는 전부 **NOT_RUN**이다. rustfmt나 신규 build도 실행하지 않았다.
- CS 검토: `RFR-FENCE`, `RFR-LOCK`, `RFR-CANCEL`, `RFR-READY`의 구현 조건은 정적으로 반영했지만 동적 의무는 아직 UNVERIFIED. 기존 준비 중 거절 정책은 보존하므로 모든 동시 `Engine.start` 성공을 보장하는 변경이 아니다.
- root가 동일 시험 바이트로 원 함수 RED와 수정 함수 GREEN 및 기존 `runtime_preparation_tests`/전체 native 회귀를 실행한다. 원 공식 smoke01 FAIL과 새 결과를 혼합하지 않는다. 이 인계는 제품 완료·설치·배포 승인이나 독립 감사 PASS를 뜻하지 않는다.

독립 코드 검토 후 두-worker 생성 순서를 위처럼 고쳤다. 기존 f7f6321 CI의 native success는 원 시험 바이트의 결과이며, 이전 시험 SHA는 이 변경 뒤의 시험 바이트와 다르다. 최초 RED 18ce65b도 이전 시험 바이트로 실행 중인 별도 근거다. 후속 CI는 새 바이트와 대상 commit으로 구분한다. 제품 함수는 이 시험 수정에서 바뀌지 않았다.
