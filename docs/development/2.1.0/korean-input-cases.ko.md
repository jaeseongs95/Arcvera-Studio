# 2.1.0 한글 조합 입력 시험 사례

범위는 Desktop 키보드 처리다. xterm의 조합 처리와 UTF-8 전송을 유지하고 CLI는 수정하지 않는다. CLI 소유 결함은 다음 버전으로 이관한다. 브라우저 조합 주입은 Windows 물리 IME 시험이 아니다.

| 사례 | 진입점과 입력 | 기대 결과 |
| --- | --- | --- |
| IME01 | 최대화된 터미널에서 isComposing, keyCode229 또는 진행 중인 조합 이벤트와 함께 Escape | 조합 중 배치·포커스를 유지한다. 조합 종료 후 Escape는 격자를 복원한다. |
| IME02 | 조합 중 Ctrl+Shift+F 또는 Ctrl+Tab | 검색을 열거나 세션을 바꾸지 않는다. 조합 종료 후 같은 단축키는 동작한다. |
| IME03 | 터미널 검색에서 조합 중 Enter/Escape | 검색 이동·닫기를 실행하지 않는다. 조합 종료 후 Enter 이동과 Escape 닫기는 동작한다. |
| IME04 | 키보드 조합 플래그가 없는 진행 중 조합에서 터미널 Shift+Enter | CLI 줄바꿈을 보내지 않는다. 조합 종료 후 일반 Shift+Enter는 ESC CR을 한 번 보낸다. |
| IME05 | Chromium 조합을 ㅎ → 하 → 한으로 갱신하고 한을 확정 | 실제 xterm이 제어문자·중복 자모 없이 한을 정확히 한 번 보낸다. |
| IME06 | 기존 Enter, Shift+Enter, Unicode 붙여넣기, 대화창 조합 사례 | 기존 전송·줄바꿈·UTF-8 완전성과 대화창 조합 중 미전송을 보존한다. |
| IME07 | 한글 일치가 두 개인 검색에서 조합 취소 또는 compositionend 뒤 input | 취소 시 선택을 유지하고 변경된 확정 검색은 한 번만 실행한다. 일반 Enter는 한 번 이동한다. |
| IME08 | compositionstart, focusout 뒤 일반 단축키 | 남은 조합 상태를 지우고 일반 단축키가 동작한다. |

제품 수정 전 Vite pre-load hook로 고정 a02913d main 모듈에 IME01–04를 실행하고 실패를 보존한다. 동일 시험 바이트를 후보판에 실행한다. native IPC는 명시적 fixture이므로 실제 CLI 처리·WebView2·물리 IME 성공의 근거로 삼지 않는다.

## 2026-11-17 관측 결과

- 고정 a02913d 제품 모듈은 8실패·8통과, 종료코드 1이었다. 검색 리뷰 전 후보판은 16통과, 종료코드 0이었다.
- 별도 읽기 전용 리뷰에서 조합 취소와 compositionend/input 중복 검색이 기존 선택을 이동시키는 결함을 발견했다. 추가 시험은 실제 xterm 선택 좌표에서 2실패·18통과였다. 수정 후 같은 20개 시험이 모두 통과했다(종료코드 0, 19.7초).
- 최종 SHA256은 main.ts 8610cf3447942cbf58f0fde9994316a2b9b4b430f29ff4438267419aed7a290c, terminal-flow.spec.ts 88323a8bd83c1c20fa530dad3c0f110f45938762170d58956280fe763272c035다.
- TypeScript --noEmit과 Vite 제품 프런트엔드 빌드는 모두 종료코드 0이었다. 기존 큰 청크 경고는 남았다. 별도 읽기 전용 최종 코드 리뷰에서 차단 결함은 없었다. 좁은 변경분 리뷰이며 릴리스 수용 감사는 아니다.
- 로컬 근거는 artifacts/korean-input-20261117/before/{execution.json,report.json,stdout.log}, search-red/{execution.json,stdout.log,results/}, after/{execution.json,report.json,stdout.log}다. 최종 report SHA256은 edd5f12737fe62dbe29251be0a6803eea4c5bcb3a39bd28540bebd2a40ac423c, execution은 a8b8fc6fefca2d116e7bd523d39fecb8bb2cabcbb11e3cde374f8c472272af2e다. 초기 상대 JSON 경로가 config 폴더 아래로 해석되어 기준판·최종 JSON을 원위치에서 복사 보존하고 runner를 절대경로로 고쳤다. 덮어쓴 중간 JSON은 없으며 실제 stdout·실행기록·실패 trace는 보존했다.
- 시험 계획을 실제 workflow task digest와 wrapper 명령에 연결했다. 초기 계획의 직접 Playwright argv와 차이가 있으며 서명 증명이나 check-proof 수용 통과는 주장하지 않는다. 초기 서버 cwd 설정 실패와 중단 실행은 별도 보존하고 제품 근거 수에 넣지 않았다.

Windows 물리 IME·native WebView2·실제 CLI·최종 Core 연동은 미검증이다. native IPC 대체물은 브라우저 fixture에만 있다. CLI 소스와 바이너리는 바꾸지 않았다. 네이티브 시험 전 새 커밋에 연결된 개발용 실행 파일이 필요하며 이 결과로 설치 승인이나 2.1.0 전체 릴리스 준비를 선언하지 않는다.

CI 후속: 실행 44039888650의 IME07 commit-input-end는 검색 성공 문구가 있지만 xterm 선택이 남지 않아 실패했다. xterm SelectionService는 행 수가 바뀌면 선택을 지운다. 배치 경합 가능성이 있으나 해당 CI trace에 resize를 직접 기록한 것은 아니다. 선택 판정 전에 검색을 열고 fonts.ready 뒤 실제 행·열이 FitAddon.proposeDimensions()와 같은지 확인한다. 첫 결과·다음 결과 좌표와 취소 판정은 유지하고 실패 사례 재시도나 기대값 교체는 하지 않는다.

배치 준비 조건을 적용한 동일 20개 시험에서 고정 a02913d는 11실패·9통과(종료코드 1), 제품 코드가 동일한 수정 후보는 20통과(종료코드 0, 18.6초)였다. 시험 SHA256은 a96fd14e76b63e1d5d1d101a9218784d468ff922f467f364e3a69bfeee158c13이며 TypeScript는 종료코드 0이었다. 별도 읽기 전용 리뷰에서 차단 결함이나 기대 결과 약화는 없었다. 배치 준비 후 조합 검색을 확인하며 이후 resize와의 임의 중첩까지 검증한 것은 아니다. 현재 before/after는 이 비교이고, 원 16개 기준판과 원 20통과는 before-original16·after-original20에 보존했다. CI44039888650은 실패 기록을 유지한다. bundled·source-1·native는 통과했지만 패키징은 차단됐다.


## 검수 후 제품 경계 재작업

앞의 배치 준비 시험 통과는 즉시 검색 경합을 해결한 근거가 아니었다. 추가 IME09에서 검색 열기와 input을 같은 DOM 작업으로 실행하면 첫 선택은 존재하지만 실제 두 animation frame 뒤 선택이 사라졌다. 현재 제품은 검색창 표시 변경과 검색 실행 전에 실제 FitAddon을 호출해 행 수 변경을 먼저 반영한다. IME07의 fonts/행열 대기는 제거했다.

IME09–14는 즉시 검색/resize, 선택 해제·재오픈·세션 전환 후 같은 한글 재조합, 대화 입력/검색의 compositionend 누락 후 focusout 복구, 플러그인·중계·세션 검색의 isComposing/229/sequence 경로다. 8bf0908 제품과 동일 시험 바이트에서 40개 중 12실패·28통과(69.1초, exit1), 첫 제품 교정 뒤 40통과(31.5초, exit0)였다. IME10 재오픈/세션 전환 두 사례의 수정 전 실패는 초기 검색 선택 단계의 resize 경합이므로 그 실행만으로 후속 재검색 경계 실패를 입증하지 않는다. 선택 해제 사례는 재조합 확정 이후의 실제 실패다.

공통 ime.ts는 document 수명의 캡처 이벤트와 WeakSet으로 flag/229/sequence를 판정하고 focusout에서 해제한다. 대화창의 별도 composing 불리언을 제거하고 같은 판정을 적용했다. 검색 중복 억제는 실제 Terminal을 약한 키로 삼아 값과 선택 좌표가 유효할 때만 적용하며, 선택 해제와 세션 변경 뒤 같은 한글도 다시 찾는다. 새 의존성이나 CLI 변경은 없다.

원자료는 artifacts/korean-input-20261117/review-before 및 review-after의 execution/report/stdout/실패 trace다. 최종 대기 제거 및 약한 검색 상태 정리 후 실행은 별도 review-final에 남긴다. 이 브라우저 시험은 실제 Chromium/xterm 제품 핸들러를 실행하며 native IPC만 대체한다. 물리 Windows IME·WebView2·실제 CLI·최종 Core 조합과 전체 릴리스 수용은 여전히 미검증이다.


최종 제품/시험 조합은 대기 없이 40통과(32.9초, exit0, skipped/flaky 0)였다. 최종 report SHA256=d630396dddc1914ddbb6f511d00712ad0149c2c84482ac47bb4d0bd5301b6ab7, execution=b91b832165c1db7b8a2d58f560eb6f9fe0e85a182915a2db163f1b0487561d5d다. TypeScript --noEmit과 Vite 제품 빌드는 exit0이며 기존 large-chunk 경고만 남았다.

## 빈 code의 Windows Process 키 추가 수정 (2026-11-18)

격리한 08310ed WebView2 실행 파일에서 설치된 Windows 한국어 입력기로 `한글` 조합을 확인했다. n17의 확정 키는 `Process`, keyCode 229이며 `code`가 빈 값이었다. `compositionend` 뒤 키 해제 전에 비조합 Enter/13이 발생해 유효한 폼을 한 번 제출했다. 기존 `code: Enter` 재현은 이 실제 순서를 포함하지 않았다. 실패 후 나머지 native 사례 2개는 실행하지 않았다. 앱은 정상 종료했고 소유한 프로세스 세대 7개 모두 부재를 확인했다.

공통 guard는 키를 식별할 수 없는 조합 눌림을 해제까지 추적한다. 눌린 동안 조합이 끝나면 뒤따르는 Enter를 계속 막고, 문자 키가 조합 종료 전에 해제되면 다음 Enter를 막지 않는다. 일반 키 누름·focusout·reset·창 blur에서 해당 상태를 해제하며 약한 참조와 기존 xterm handler를 유지한다. 타이머·의존성·CLI 변경은 없다.

IME19는 확정 Enter의 폼 시작 0건과 새 Enter의 시작 1건, IME20은 실제 xterm의 한글 1회·확정 CR 0회·새 Enter CR 1회를 검사한다. IME21–23은 문자 키 해제·focusout·blur 복구를 보존한다. 격리 작업본의 동일한 시험 바이트와 Playwright 하위 프로세스 인수에서 수정 전 예상 assertion 2실패·3통과, 수정 후 5통과(exit0)를 확인했다. 최초 웹 서버 작업 경로 오류는 시험 0건으로 별도 보존하며 제품 실패로 세지 않는다. 근거는 `artifacts/ime-empty-code-20261118/{red-01,green-01}`에 있다.

별도 소스 검토에서 다른 문자 키 해제와 보조 키 누름이 이미 확정된 Enter 차단을 푸는 경계를 확인했다. IME24–26은 3실패·보존 사례 1통과를 재현했고, 식별 전 키 해제와 확인된 Enter 해제를 분리하며 보조 키에서 차단을 유지한 뒤 4개 모두 통과했다. 최종 영향 경로 회귀는 48통과·skipped/flaky 0·exit0이며 TypeScript와 Vite도 exit0다. 기존 Vite 청크 크기 경고는 남아 있다. 교차 키 경계는 브라우저에서 직접 재현했으며 실제 Windows 입력기에서의 발생 빈도는 미검증이다.

브라우저 native IPC는 명시적인 fixture다. 새 실행 파일의 Windows 입력기 동작이나 Core 통합을 검증한 결과가 아니며, 새 소스에 결속된 개발용 실행 파일과 native 사례 실행이 남아 있다. 최종 패키징·설치는 이번 범위 밖이다.

## 실제 Windows 조합 Enter 추가 수정

격리한 980f143 Windows WebView2 실행 파일에서 실제 한국어 입력기로 `gksrmf`를 입력해 `한글` 조합을 확인했다. 유효한 새 세션 폼에서 조합 확정 Enter는 keyup 전에 `Process/Enter/229`, `compositionend`, `Enter/13` 순서로 발생했고, 마지막 이벤트가 폼을 한 번 제출했다. 유효하지 않은 폼에서는 이 결함이 가려졌다. 명시적인 missing-CLI 시험 설정으로 Core 시작 전 오류가 났으며, Desktop에서 확인한 결함이다.

IME15는 이 순서를 유효한 폼에 재현해 시작 0건, keyup 뒤 새 Enter에서 시작 1건을 검사한다. IME16은 keyup 없는 focusout 뒤 복구, IME17은 중복·반복 Enter 차단과 명시적 버튼 동작을 검사한다. 동일한 세 시험은 980f143에서 2실패·1통과, 공통 guard 수정 뒤 3통과였다. 조합 Enter는 keyup·다른 키·focusout·reset·창 blur까지 유지하며 약한 참조를 쓰고 타이머를 추가하지 않는다. 뒤따르는 브라우저 동작만 막고 확정된 입력은 검색과 마우스 동작에 제공한다. 기존 터미널 시험에는 다음 독립 Enter 전에 합성 Enter의 keyup을 보완했으며 원래 assertion은 모두 유지했다.

관련 시험 합계 38건이 통과했다(28.7초, exit0). 이는 native IPC를 대체한 Chromium 핸들러 회귀 시험으로, 새 실행 파일의 실제 입력기 검증을 대신하지 않는다. 재빌드 뒤 유효한 폼에서 같은 native 사례를 실행해야 해당 경계를 검증했다고 판정할 수 있다. CLI 코드 변경은 없으며 최종 빌드·설치 요청은 아니다.

별도 검수에서 xterm은 문서의 `preventDefault` 뒤에도 후행 Enter를 변환하는 경로가 확인됐다. IME18은 88f01a3에서 기대 `['한글']` 대신 `['한글', '\r']`를 재현했다(exit1). 기존 xterm custom handler에도 조합 Enter 차단을 적용한 뒤 동일 시험은 한글 1회·확정 Enter의 CR 0회·키를 놓고 새 Enter의 CR 1회를 확인했다. IME15–18 합계 4통과(exit0)다. 실제 xterm과 브라우저 입력기 API를 사용하고 native IPC는 대체했으므로, 실제 터미널/Core 통합은 허용된 native 의존성이 있는 시험에서 확인해야 한다.

이전 980f143 후보의 별도 읽기 전용 검수는 제품 6개·시험 5개(11개 모두)를 검토했고 차단 결함 0개였다. 검수자는 시험을 실행하지 않고 원자료·소스·해시를 직접 대조했다. 해당 후보의 final 실행 뒤 시험 5개에서 EOF LF 1개만 제거했으며, 실행 당시 바이트는 review-final/frozen-tests에 SHA 일치로 보존했다. 당시 제품 6개의 바이트는 해당 후보의 final snapshot과 동일했다.
