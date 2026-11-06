# 설정과 업데이트 확인 시험

사용자 요구: 자동 설치 대신 백그라운드 확인을 설정에서 켜고 끈다. 수동 확인은 항상 제공한다. Desktop과 OpenDIY Codex를 각각 확인하고 새 버전이면 예·아니오로 다운로드/설치를 확인한다. GitHub 바로가기도 설정 안에 둔다.

실행 환경은 기존 Playwright의 격리 native IPC fixture다. 실제 설치, 사용자 설정, 운영 daemon에는 접근하지 않는다. 네이티브 다운로드·서명 검증·Windows 설치는 별도 시험이며 이 결과로 대체하지 않는다.

| ID | 전제·입력 | 절차 | 기대·통과 기준 | 실패 기준 | 실제 결과 |
|---|---|---|---|---|---|
| US01 | 저장값 OFF | 초기화·설정 열기 | 자동 버전 조회 0회, 수동 확인 활성 | 자동 조회 또는 수동 버튼 차단 | PASS (브라우저 IPC fixture) |
| US02 | OFF/ON·저장 오류 | 켜기→재초기화→끄기, 오류 주입 | 저장 확인 후 상태 갱신, 오류 때 원래 값/오류 표시 | 저장 실패를 성공 표시 | PASS (브라우저 IPC fixture) |
| US03 | 최신 또는 새 정식 버전 | 각 제품 수동 확인 | 현재/최신 구분, 새 버전의 예·아니오 표시 | 단순 조회가 다운로드·설치 호출 | PASS (브라우저 IPC fixture) |
| US04 | 새 버전 안내 | 아니오 선택, 다시 수동 확인→예 | 아니오 부작용 0, 예만 정확한 제품/버전/confirmed 전달 | 무확인·중복·다른 제품 설치 | PASS (브라우저 IPC fixture) |
| US05 | 네트워크 실패/미구성/알 수 없는 응답 | 수동 확인 | 최신으로 단정하지 않고 상태 구분 | 오류를 최신/성공 처리 | PASS (브라우저 IPC fixture) |
| US06 | 백그라운드 조회 중 OFF/설정 닫기 | 지연 응답 반환 | OFF 후 백그라운드 알림 소멸, 수동 확인은 독립 | 늦은 응답이 설정을 되살림 | PASS (브라우저 IPC fixture) |
| US07 | 설정 화면 | GitHub 바로가기 클릭 | 고정 HTTPS 계정 URL을 기존 native opener에 1회 전달 | 새 메뉴/쉘/임의 경로 실행 | PASS (1.4 ST06으로 분리) |

버전 계획: 1.4.x 세션 도구·권한 확인, 1.5.x MCP 서버 추가, 1.6.x 업데이트 설정. 배포본 1.3.2는 보존한다. 각 후보의 코드·시험·릴리스·설치를 구분하며 이 문서는 발행/설치 완료 근거가 아니다.

추가 실행 전 사례:

| ID | 전제·입력 | 절차 | 기대·통과 기준 | 실패 기준 | 실제 결과 |
|---|---|---|---|---|---|
| US08 | 두 제품 모두 새 버전 | 백그라운드 ON, Desktop 안내 아니오, Codex 안내 아니오 | 두 안내를 순서대로 제공, 다운로드·설치 호출 0 | 두 번째 안내 누락 또는 설치 호출 | PASS (브라우저 IPC fixture) |
| US09 | 설치 실패 | 예→실패→닫기→수동 재확인→예 | 실패 직후 재설치 불가, 명시 재확인 후에만 재시도 | 같은 후보 자동·중복 설치 | PASS (브라우저 IPC fixture) |

실행: 2026-11-06 `npx playwright test tests/session-tools-layout.spec.ts tests/update-settings.spec.ts --reporter=line`, 종료 코드 0, 전체 14 PASS 중 업데이트 7 PASS. 첫 실행의 저장 실패 복구 시험은 Playwright uncheck가 최종 unchecked를 요구해 정상 복구와 충돌한 오라클 오류였고 click 후 저장 실패/원래 checked를 직접 확인하도록 수정했다. 최초 FAIL 자료는 `test-results/followup-first`에 보존했다. 두 제품 안내 큐와 설치 실패 후 명시 재확인 사례는 실행 전에 추가했다.

`npm run build` 및 `cargo metadata --locked --no-deps --format-version 1 --manifest-path src-tauri/Cargo.toml` 종료 코드 0. metadata는 Rust 컴파일/링크/시험을 하지 않는다. 실제 Desktop 서명 공개키·버전 서명 자산이 미구성이고 Codex 지정 버전 설치 계약이 미지원이라 실제 설치는 NOT_RUN이다. 이를 숨기거나 설치 완료로 처리하지 않는다.

US09 오류 안내 보완(실행 전): native installer가 진행 중 세션 등의 고정된 설명 문자열을 반환하면 팝업에 그 이유를 함께 표시한다. 오류 객체/형식 미확인은 일반 실패 안내로 남긴다. 오류 안내는 textContent로만 표시하고 설치 실패 후 새 명시 조회가 없으면 재시도하지 않는다.

US09 보완 실제 실행: `npx playwright test tests/update-settings.spec.ts -g US09 --reporter=line`, 종료 코드 0, 1 PASS. native의 보류 이유 문자열 표시·실패 후 예 비활성·재확인 전 중복 설치 없음·명시 재확인 후 재시도를 확인했다. `test-results/desktop160-install-failure-ui` 보존. 실제 installer 실행은 하지 않았다.

TLS 회귀 사례(구현·실행 전 계획, 2026-11-06): 실제 TEST 앱 stderr에서 reqwest 0.13.5의 provider 미등록 panic과 blocking Client의 후속 panic을 확인했다. updater 2.14.1의 provider 등록은 플러그인 자체 조회 경로 안에 있으므로 앱의 별도 Client 생성에는 선행 등록을 보장하지 않는다. 아래 사례는 기존 Rust 시험 harness와 표준 라이브러리를 사용하며 네트워크 요청을 보내지 않는다.

| ID | 전제·입력 | 절차 | 기대·통과 기준 | 실패 기준 | 실제 결과 |
|---|---|---|---|---|---|
| TLS01 | 기존 시험 실행 파일을 `--exact`로 실행하는 새 자식 프로세스, provider 선등록 없음 | provider가 `None`인지 먼저 assert하고 실제 `request_client()` 생성·drop, 두 번째 생성·drop 뒤 provider identity 비교 | 새 프로세스 완료 표식, 두 Client 정상 생성·해제, 기존 provider 보존 | 초기 provider 존재, panic, `Err`, 자식 비정상 종료, 완료 표식 누락 | NOT_RUN (root Windows CI 실행 예정) |
| TLS02 | 별도의 새 자식 프로세스, provider 선등록 없음, 4개 스레드 | 초기 `None` assert 후 barrier로 동시 시작, 각 스레드가 실제 Client 생성·drop을 3회 반복, provider identity 비교 | 12회 모두 정상 생성·해제, 모든 스레드 join 및 자식 완료 표식 | 등록 경합 panic, 생성 실패, provider 변경, 자식 비정상 종료·완료 표식 누락 | NOT_RUN (root Windows CI 실행 예정) |
| TLS03 | 수정 빌드의 격리 실제 GUI, 두 자체 TEST 세션, 백그라운드/수동 업데이트 조회 | 조회와 각 세션 입력을 병행하고 조회 결과·실제 turn 완료·정확 대상·초안 보존 및 stderr 대조 | 조회 정상 결과 또는 정상 오류, TLS panic 없음, 두 세션 입력 완료와 보존 근거 | TLS panic, 입력 미완료, 대상 혼선, 초안 훼손 | NOT_RUN (수정 빌드 이후 별도 실제 GUI 검증) |

TLS01/02는 자식 내부의 provider 미등록 assertion과 완료 표식을 모두 요구한다. 다른 시험이 부모 프로세스의 provider를 미리 등록해도 새 자식의 상태에는 영향을 주지 않으며, 잘못된 `--exact`로 0개 시험이 실행되면 완료 표식이 없어 실패한다. Client 생성·drop만으로 공개 GitHub HTTPS 통신·인증서 검증·설치를 검증했다고 하지 않는다. TLS03 전에는 입력 정지와 TLS 결함의 인과나 해결을 선언하지 않는다.

결함 탐지 검증 계획: 후보의 격리 사본에서 provider 등록 블록만 제거하고 동일한 TLS01/02 명령을 실행한다. 직접 rustls 의존성과 시험 코드는 그대로 보존하여 컴파일 오류를 RED로 세지 않는다. RED는 새 자식의 초기 provider assertion을 통과한 뒤 reqwest provider 미등록 panic으로 실패해야 한다. GREEN은 동일 시험 코드로 후보를 실행한다. 시험이 제품과 같은 파일에 있으므로 전체 파일 digest 대신 동일 `#[cfg(test)]` 모듈의 byte digest도 대조한다. 두 실행 모두 아직 NOT_RUN이다. 명령·등록 블록·범위는 `artifacts/r38-update-tls-fix/`에 보존한다.

고정 버전 공식 근거: [updater 2.14.1 Cargo 기능](https://docs.rs/crate/tauri-plugin-updater/2.14.1/source/Cargo.toml), [updater의 provider 등록](https://docs.rs/tauri-plugin-updater/2.14.1/src/tauri_plugin_updater/updater.rs.html#578-582), [reqwest 0.13.5 provider 선택](https://docs.rs/reqwest/0.13.5/src/reqwest/async_impl/client.rs.html#720-724), [reqwest provider 미등록 panic](https://docs.rs/reqwest/0.13.5/src/reqwest/async_impl/client.rs.html#2498-2510), [blocking Client의 후속 panic](https://docs.rs/reqwest/0.13.5/src/reqwest/blocking/client.rs.html#1475-1480), [rustls 0.23.46 프로세스 provider 계약](https://docs.rs/rustls/0.23.46/rustls/crypto/struct.CryptoProvider.html#method.install_default). 버전 없는 `latest` 링크는 이 결함의 고정 근거로 사용하지 않는다.
