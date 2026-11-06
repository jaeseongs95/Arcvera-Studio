# 세션 도구 하단 배치 시험

요구: 헤더에는 세션 식별·상태와 최대화·종료를 두고, 세션 관리 도구는 실제 DOM에서 하단으로 이동한다. 기존 클릭 명령·지원 조건·키보드·드래그를 보존한다.

환경: 기존 Playwright/Vite, 합성 Tauri IPC fixture와 demo. 운영 앱·daemon은 변경하지 않는다. 네이티브 실행 파일과 실제 CLI 연결은 이 시험의 범위 밖이다.

| ID | 전제·입력 | 절차 | 기대 결과·통과 기준 | 실패 기준 | 실제 결과 |
|---|---|---|---|---|---|
| ST01 | 정상 지원 세션, 1440×1000 | 헤더/하단 실제 자손과 위치 확인, 하단 예약·대화 전환 클릭 | 헤더는 최대화·종료, 관리 버튼은 하단. IPC가 원래 세션/generation으로 1회 전달 | 잘못된 부모·위치·명령·세션 또는 중복 호출 | PASS (dev fixture) |
| ST02 | 정상 지원 세션 4개, 800×1000, 1440×620 | 4열 선택 및 실제 열 리사이즈, 모든 가시 버튼 경계·겹침 검사 | 버튼이 카드 안에 있고 서로 겹치지 않으며 터미널 높이 양수. 문서 가로 overflow 없음 | 버튼 잘림·겹침·가로 overflow·터미널 소실 | PASS (dev fixture) |
| ST03 | 정상 지원 세션 | 버튼 Tab/Shift+Tab 순서와 Enter/Space, 최대화·Escape, Alt+2 확인 | DOM 순서와 시각 순서 일치, 활성화 1회, 기존 세션 선택·최대화 보존 | 초점 함정·순서 불일치·중복 명령·기존 단축키 실패 | PASS (dev fixture) |
| ST04 | demo 카드 4개 | 실제 드래그 핸들로 카드 재정렬 후 하단 버튼 클릭 | 순서가 바뀌며 dragging 상태 해제, 관리 버튼은 드래그를 시작하지 않음 | 재정렬 실패·dragging 잔류·버튼 조작의 재정렬 | PASS (dev fixture) |

검증 명령 및 결과는 구현 후 아래에 기록한다. 기존 conversation-integration/context-controls/link-drag 회귀 시험을 함께 실행하며, 시험 제목의 기존 '상단' 표현은 동작 assertion과 무관하므로 변경하지 않는다.

첫 실행: 36개 중 35개 통과, ST03 실패. xterm 진입 후 Tab으로 하단에 나갈 것이라는 시험 기대가 기존 CLI Tab 입력 처리와 충돌했다. 제품은 변경하지 않고, xterm 진입 확인 후 하단 첫 버튼에 명시적으로 focus하여 하단 관리 버튼의 Tab 순서를 확인하도록 oracle을 정정했다. 첫 실패 trace는 `test-results/session-tools/session-tools-layout-ST03--a3b09-nd-retains-window-selection/trace.zip`에 보존한다. 이를 제품 결함 red/green 근거로 취급하지 않는다.


최종 실행:

- `CI=1 AGENT_GRID_TEST_OUTPUT=test-results/session-tools npx playwright test tests/session-tools-layout.spec.ts tests/conversation-integration.spec.ts tests/context-controls.spec.ts tests/link-drag.spec.ts --reporter=line`: exit 1, 35 PASS / ST03 oracle 실패 1. 이 실행에서 기존 회귀 시험 31개는 모두 PASS.
- oracle 정정 후 `CI=1 AGENT_GRID_TEST_OUTPUT=test-results/session-tools-final npx playwright test tests/session-tools-layout.spec.ts --reporter=line`: exit 0, 5 PASS (ST02 화면 조건 2개 포함).
- 원시 결과: 각 output 디렉터리의 `.last-run.json`, 최초 실패 trace 및 error-context, ST02 `session-tools.png`. 800px 4열·실제 열 축소 후 screenshot을 직접 열어 하단 줄바꿈·창 제어 배치를 확인했다.
- 제품은 최초 실행 이후 변경하지 않았다. 최종 bundled frontend 및 실제 실행 파일 검증은 통합 단계에서 별도 수행한다.

추가 실행 전 사례:

| ID | 전제·입력 | 절차 | 기대·통과 기준 | 실패 기준 | 실제 결과 |
|---|---|---|---|---|---|
| ST05 | native fixture의 3번째 세션 | 하단 권한 확인 클릭 | 해당 id/generation만 permissionStatus 1회 | 다른 세션 조회 또는 설정 변경 | PASS (fixture) |
| ST06 | native fixture 설정 | GitHub 바로가기 클릭 | 고정 https://github.com/jaeseongs95 를 native opener에 1회 전달 | 별도 메뉴·임의 주소·쉘 실행 | PASS (fixture) |

부모 통합 재검사: `npx playwright test tests/session-tools-layout.spec.ts tests/update-settings.spec.ts --reporter=line`, 2026-11-06, 종료 코드 0, 14 PASS 중 이 파일 7개 PASS. `test-results/followup-additional` 보존. 이 결과는 1.4 분리 전 통합 작업트리의 브라우저 IPC fixture 결과이며 native/실제 설치 검증을 대신하지 않는다.
