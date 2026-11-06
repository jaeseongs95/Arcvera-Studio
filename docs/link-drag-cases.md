# 링크 클릭·드래그 시험

기준: `6e453bf`, Windows Edge Playwright, 기존 `playwright.config.ts`와 port 1420. 실제 운영 CLI·인증·세션은 사용하지 않는다. 외부 opener만 호출 기록용 Tauri stub으로 대체하며 브라우저 DOM/xterm/세션 카드 이벤트는 실제 제품 코드를 실행한다.

## 실행 전 사례

| 사례 | 입력·사전 조건 | 절차 | 기대 결과·판정 기준 | 실제 결과 |
| --- | --- | --- | --- | --- |
| LD-01 | 본문 및 radio label 안 HTTP(S) 링크 | 링크에서 실제 pointer를 누르고 같은 링크 안 35px 이동 후 놓음. 이후 클릭·Enter | 취소되지 않은 native anchor drag 0회, 세션 `.dragging` 0개, 정상 클릭·Enter당 opener 정확히 1회, radio 미선택 | baseline FAIL: A native dragstart 1회. 수정 PASS |
| LD-02 | 링크 옆 일반 본문과 radio | 일반 본문을 실제 pointer로 선택하고 radio 직접 클릭 | 선택 원문 일치, radio 선택 가능, opener 0회 | baseline/수정 PASS |
| LD-03 | 실제 xterm plain URL과 OSC8 링크. Shift 사례는 TUI mouse reporting 활성, 일반 사례는 비활성 | 링크 안에서 Shift+drag 및 일반 drag 선택, 이후 새 단순 클릭 | 선택 텍스트 존재, 선택으로 opener 호출 없음, 새 클릭 정확히 1회 | baseline FAIL: Shift 선택 후 opener 1회. 수정은 plain/OSC8 × Shift/일반 4조합 PASS |
| LD-04 | 제품 demo 세션 카드 두 개 | 실제 `.drag-handle`에서 두 번째 카드로 drag | 세션 drag 시작과 카드 순서 변경, 종료 후 `.dragging` 0개 | baseline/수정 PASS |

`tests/link-drag.spec.ts`의 동일 테스트와 명령으로 baseline과 수정 후보를 비교한다. baseline은 작업 트리를 되돌리지 않고 Playwright 응답을 `git show 6e453bf:src/external-links.ts`의 소스로 대체하는 격리 경계에서 실행한다. 원래 실패와 시험기 오류는 구분한다. 사용자 화면의 특정 링크·드래그 종류는 아직 확인되지 않아 이 두 재현을 사용자 관측의 유일한 원인으로 단정하지 않는다.

## 관측·최소 수정·결과

첫 baseline 실행은 LD-01에서 실제 native anchor drag를 확인했고 LD-02/04는 통과했다. LD-03은 mouse reporting 비활성 상태에서 Shift가 기존 선택을 연장하는 xterm 규칙 때문에 선택 전제부터 실패했다. 이 실행(`artifacts/link-drag/red.json`)은 결함 red/green 쌍으로 쓰지 않는다. Shift 사례에 TUI mouse reporting 전제를 추가한 뒤 같은 시험으로 다시 비교한다.

보정된 baseline(`red2.json`, exit 1)은 LD-01의 취소되지 않은 native anchor dragstart와 LD-03의 선택 후 opener 호출을 확인했다. LD-02/04는 통과했다. 첫 수정(`green.json`, exit 1)은 LD-01/02/04가 통과했고, 단순 `terminal.hasSelection()` 차단이 기존 선택이 남는 mouse-reporting 상태에서 다음 정상 클릭까지 차단하는 회귀를 검출했다. 이 실패도 보존한다.

최종 수정은 링크 생성 시 `draggable=false`와 해당 anchor의 `dragstart` 취소만 추가한다. xterm에는 terminal element의 capture `mousedown` 좌표를 기억하고, 활성화 시 비주버튼·Shift·현재 제스처의 이동 선택을 거절한다. 과거 선택이 남아 있어도 새 단순 클릭은 열리며, listener는 terminal link owner의 `dispose()`에서 제거한다. 전역 pointer 차단, CSS 선택 금지, 세션 drag 변경, 타임아웃 증가는 없다.

최종 실행(`artifacts/link-drag/green2.json`, exit 0)은 기존 외부 링크 7개와 LD-01~04 4개 합계 11개가 PASS이며 skipped/flaky/unexpected는 모두 0이다. `npx tsc --noEmit`, 해당 3파일의 `git diff --check`도 exit 0이다. baseline과 최종의 `tests/link-drag.spec.ts`는 같은 바이트지만 최종 명령은 기존 회귀 spec도 함께 실행했다. 이 결과를 서명된 호스트 영수증이나 native WebView2 통합 시험으로 해석하지 않는다.

실행 명령(PowerShell, 작업사본 루트):

```powershell
$env:PLAYWRIGHT_CHANNEL='msedge'
$env:LINK_DRAG_BASELINE='1'
$env:AGENT_GRID_TEST_OUTPUT='artifacts/link-drag/red2-browser'
npx playwright test tests/link-drag.spec.ts --reporter=json > artifacts/link-drag/red2.json

Remove-Item Env:LINK_DRAG_BASELINE -ErrorAction SilentlyContinue
$env:AGENT_GRID_TEST_OUTPUT='artifacts/link-drag/green2-browser'
npx playwright test tests/link-drag.spec.ts tests/external-links.spec.ts --reporter=json > artifacts/link-drag/green2.json
```

남은 경계: 운영 앱/CLI 및 사용자 데이터에는 접근하지 않았다. native OS 브라우저 실제 실행, WebView2 설치본, 사용자가 지칭한 링크의 정확한 종류, 터미널 포인터의 모든 경로를 이 브라우저 시험만으로 보장하지 않는다. 세션 카드 이동이 링크 클릭으로 시작하는 현상은 이 재현에서 관측되지 않았고 정상 handle 이동은 보존됐다.
