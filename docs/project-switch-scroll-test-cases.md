# 프로젝트 전환 스크롤 시험 사례

사전 계획: 실제 프로젝트 탭 클릭과 실제 xterm/구조화 대화 DOM을 Playwright로 실행한다. Tauri RPC만 합성 fixture로 대체하며 실제 CLI·네이티브 앱·사용자 대화는 사용하지 않는다.

| 사례 | 요구 및 진입점 | 독립 oracle |
| --- | --- | --- |
| PS-1 | Beta→Alpha 탭, 새로 보이는 터미널 | 실제 스크롤 컨테이너가 최하단이며 프로젝트 탭 초점 유지 |
| PS-2 | 동일 Alpha 재선택, 일반 render/state 갱신 | 과거 읽기 위치 유지 |
| PS-3 | 구조화 대화 Alpha→Beta→Alpha, 늦은 items 응답 | 응답 렌더 완료 후 최하단, 초안·탭 초점 유지 |
| PS-4 | 지연 응답 중 Alpha→Beta→Alpha | 숨은 뷰에 이전 요청이 적용되지 않으며 현재 선택만 갱신 |
| PS-5 | 이력 지연 중 세션 inactive 관측→닫기 | 늦은 요청이 카드를 복원하거나 초점을 탈취하지 않음 |

실행: `npx playwright test tests/project-switch-scroll.spec.ts --reporter=line`. 기존 playwright.config.ts와 Vite 1420 사용, 병렬 워커 1. 종료 코드와 원시 출력은 root에 전달한다. 브라우저 격리 응답에서 표적 mutation을 적용해 같은 시험이 결함을 탐지하는지 확인한다. 네이티브 프로세스·실제 모델과 실제 사용자 부하는 검증 범위 밖이다.


## 확인한 구현과 변경 범위

- `git show b431157 -- src/main.ts`: 2026-10-29 추가된 `terminal.write('', callback)` → `fit.fit()` → `scrollToBottom()` 경로는 현재 소스에도 남아 있다. 이 확인 범위에서는 기능이 삭제됐다는 근거가 없다.
- 실제 확인한 결함은 같은 프로젝트 탭을 재선택해도 과거 읽기 위치를 최하단으로 덮어쓰는 경계다. 전환 전 실제 표시 카드 집합을 보관하고 새로 표시되는 세션에만 기존 helper를 적용한다. 선택 버전·터미널 identity·generation·hidden 가드를 유지한다.
- 명시적인 세션 이동 완료는 기존 탐색 의미를 유지하기 위해 `scrollExisting=true`를 전달한다. 기존 `project-tabs.spec.ts`의 same-project 사례는 탭 재선택이 아니라 동일 프로젝트로 이동한 사례이므로 기대값을 변경하지 않는다.
- 구조화 대화는 `setForeground` → `loadPage` → `loadItems` → `renderItems`의 기존 nearEnd 동작으로 정상 복귀했다. 추가 강제 이동을 제거한 브라우저 변이에서도 PS-3가 통과했다. 재현하지 못한 결함을 추정해 제품 코드를 추가하지 않았다.
- root 요청으로 설정 진입 문구를 `플러그인·스킬·Hook` / `Plugins, skills & hooks`로 수정했다. 스크롤 수정과 별개의 UI 문구 변경이다.

## 실행 결과와 검증 한계

- 최종 `npx playwright test tests/project-switch-scroll.spec.ts --reporter=line`: **5 passed**, 종료 0, 6.0초. 출력 디렉터리 `test-results/project-scroll-final`.
- 같은 최종 시험 파일에 `PROJECT_SCROLL_MUTATION=same-project`와 `--grep PS-2`를 적용한 격리 브라우저 응답에서는 기존 무조건 스크롤을 복원했다. `LATEST_MARKER`가 화면에 나타나 PS-2 assertion **FAIL**, 종료 1. 출력·trace: `test-results/project-scroll-red`. 제품 파일은 되돌리거나 덮어쓰지 않았다.
- 최종 시험 파일 SHA256: `8105be280f7cb478f8de1563fa612fbb660848367e5d71c347104f8f3f17d8fc`. RED와 마지막 GREEN은 같은 파일을 사용한다. GREEN은 5개 전체, RED는 PS-2 선택이므로 argv 범위가 다르다.
- 최초 terminal oracle은 `.xterm-viewport` DOM scrollHeight를 읽어 2개가 실패했다. 설치된 xterm 6의 실제 UI는 별도 scrollable element를 사용함을 소스와 기존 시험에서 확인하고, `Shift+PageUp`과 실제 `.xterm-rows`에 나타난 메시지로 교체했다. 이 최초 실패를 제품 결함 재현으로 세지 않는다.
- 최초 mutation은 스크롤 callback이 끝나기 전 negative assertion이 통과했다. 출력 seq2 ACK 뒤 2 animation frame의 렌더 barrier를 두어 실제 완료 후 검사하도록 보강했다. 이후 동일 최종 시험에서 RED/GREEN을 확인했다. 고정 sleep이나 timeout 증가는 사용하지 않았다.
- 중간 후보 `npm run build`는 최초 visibilitychange 인자 타입 오류 후 수정하여 종료 0이었다. 최종 후보의 통합 build와 기존 명시 move 회귀 시험은 root가 실행한다. 브라우저 합성 RPC 검사이며 실제 앱·모델·사용자 부하는 NOT_RUN이다.
