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
