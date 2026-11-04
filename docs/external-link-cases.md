# 외부 링크 검증 사례 (구현 전 작성)

기준: 22551c6. 범위: 질문·대화·터미널의 명시 클릭으로 HTTP(S) URL을 기본 브라우저에 전달한다. 자동 열기, 임의 프로토콜, shell 문자열 실행은 금지한다. 기존 Playwright runner와 Rust 단위 테스트를 사용한다. 브라우저 시험의 native invoke는 stub이며 실제 OS 브라우저 실행을 증명하지 않는다.

| 사례 | 입력·사전조건 | 절차 | 기대 결과·통과 기준 | 실제 결과 |
| --- | --- | --- | --- | --- |
| EL-01 | 일반 본문 `https://community.openai.com/c/codex/37`, native invoke stub | 렌더링, hover, pointer click, keyboard Enter | 렌더링·hover 호출 0회, 명시 활성화마다 정확한 URL 1회, 페이지 이동 없음 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-02 | xterm OSC 8 label과 HTTP(S) target, native invoke stub | 출력 후 label hover·click | 자동 열기 0회, 클릭 1회, WebView popup 없음, target tooltip | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-03 | 일반 URL이 xterm 열 폭을 넘어 wrap, CJK·emoji prefix, 후행 `).,` | 첫·이어진 줄의 URL hover·click | 전체 URL 한 번 전달, 괄호·문장부호 제외, 잘못된 셀 링크 없음 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-04 | Unicode host/path, query `?a=1&b=한글#부분`, balanced `(b)` | 본문·터미널에서 클릭 | URL 내용 보존·정규화, query·괄호 손실 없음, 표시 원문 보존 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-05 | javascript/file/data/mailto, leading spaces, literal/percent-encoded controls, backslash, malformed host | 공개 openExternalUrl 및 Rust validator 호출; OSC 8 disallowed target 클릭 | native launch 0회, 오류 반환·표시; 허용된 HTTP(S)만 전달 | PASS: TS/렌더러; Rust 단위 테스트 작성, CI 실행 대기 |
| EL-06 | native launch가 거절하는 stub | 본문·터미널 링크 클릭 | 예외가 오류 callback/기존 toast에 전달, 성공으로 처리·자동 재시도하지 않음 | PASS: TS/렌더러; Rust 단위 테스트 작성, CI 실행 대기 |
| EL-07 | terminal link controller | 출력·provider 확인 후 dispose, stale activation 호출 | provider 해제, 기존 handler 복원, stale callback 외부 실행 없음 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-08 | 대화 fenced code URL·본문 URL·HTML 같은 문자열 | 대화 렌더링·검색·코드 복사 | 코드 URL은 code text 유지, 본문 URL만 링크, HTML 실행 없음, 검색·복사 원문 보존 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-09 | radio label 내부 링크 | label 내부 anchor 클릭 | 외부 열기 1회, option 선택·form submit·dialog 닫힘 없음 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-10 | Windows native 앱, disposable HTTP(S) test URL, 사용자 명시 클릭 | 실제 패치 앱 링크 클릭 후 기본 브라우저 확인 | 실제 기본 브라우저의 요청 URL 확인, 실패시 오류 표시 | NOT_RUN (별도 실제 호스트 검증 필요) |
| EL-11 | 실제 대화의 구조화 질문 label/option/description URL | 질문 poll로 표시한 후 각 URL 클릭 | 질문·option 보조링크 native 전달, select 값·초안·응답 호출 횟수 보존 | PASS: Playwright 공개 경계 (native invoke stub) |
| EL-12 | main.ts 실제 세션 카드와 terminal-output 이벤트, native invoke stub | 실제 카드 URL pointer click 후 native 실패를 주입하고 재클릭 | 전체 제품 경로의 URL 전달·기존 오류 toast 확인; baseline main.ts는 같은 클릭 assertion 실패 | PASS: Playwright 공개 경계 (native invoke stub) |

판정: 각 사례의 모든 기대 결과가 충족되어야 PASS. assertion 실패는 FAIL, runner/환경 오류는 ERROR, 실행하지 않은 경계는 NOT_RUN으로 남긴다. 자동 시험은 실제 브라우저 launch 경계를 대체하지 않는다. SOLID 경계: 렌더러의 텍스트/셀 링크화와 호스트의 URL 검증·OS 실행을 두 함수/모듈로 분리하며, 사용자 본문은 innerHTML에 전달하지 않는다.

## 실행 결과와 증거

- Windows, Node/Playwright/Chromium; fixture는 자체 DOM과 stub Tauri invoke만 사용한다. 기존 사용자 세션·인증·DB·운영 서비스를 열거나 변경하지 않았다.
- `npx playwright test tests/external-links.spec.ts --reporter=list`: 종료 0, 6개 PASS. 로그 `artifacts/external-links/final-playwright.log`.
- `npm run build`: 종료 0. 38개 module, 기존 500kB chunk warning 유지. 로그 `artifacts/external-links/final-build.log`.
- `npx playwright test tests/external-links.spec.ts -g EL-12 --reporter=list`: `EXTERNAL_LINK_BASELINE=1`일 때 작업트리 대신 Playwright route가 provider/OSC 8 설치만 제거한다. 종료 1, native 호출이 기대 URL 대신 빈 배열인 assertion으로 검출. 동일 argv의 정상 후보는 종료 0, 1개 PASS. 로그 `missing-provider-red.log` / `missing-provider-green.log`. 이는 원본 전체 checkout 실행이 아닌 표적 mutation이다.
- 첫 실행의 x10 기대 대 x9 실제값 실패는 xterm 기본 Unicode 6의 emoji 1셀 폭을 반영한 oracle 교정이다. 두 번째 실패는 Windows clipboard CRLF readback에 LF만 기대한 oracle 오류다. 실제 `writeText` payload의 LF 원문은 exact assertion으로 보존하고 readback만 CRLF를 정규화했다. 제품 복사 코드와 URL 셀 구현은 이 두 실패 때문에 바꾸지 않았다. 원시 실패 trace와 설명을 `artifacts/external-links/`에 보존했다.
- Rust 3개 단위 테스트는 HTTP(S)/Unicode 재정규화, 금지 URL, 검증 후 launch·실패 전달을 검사한다. 로컬 대형 Rust 빌드 제한에 따라 여기서는 실행하지 않았다. CI 실행 책임은 root에게 인계한다.
- 실제 기본 브라우저의 OS 실행과 patched native 앱의 클릭은 EL-10 NOT_RUN. 프런트엔드 invoke stub은 그 경계를 증명하지 않는다. 전문 CLI check-plan/check-proof receipt는 발급하지 않았으며, 위 자연어 계획·실행 로그·표적 mutation 근거를 root의 수용/독립 감사에 전달한다.
