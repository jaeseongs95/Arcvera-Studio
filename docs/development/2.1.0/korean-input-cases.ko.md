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
