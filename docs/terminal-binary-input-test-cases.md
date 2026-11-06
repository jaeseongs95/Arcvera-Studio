# 터미널 binary 입력 시험

## 수정 범위와 계약

xterm 6.0은 DEFAULT 마우스 보고를 `onBinary`, SGR 보고를 `onData`로 보낸다. 기존 앱은 `onData`만 연결했다. 새 연결은 기존 `TerminalInputQueue`를 공유하며 native 호출은 `session_write { id, generation, data, binary?: true }`다. text 호출에서는 `binary`를 생략한다.

`binary: true`의 `data`는 각 code unit이 0x00–0xFF인 byte-string이다. UTF8 텍스트로 재인코딩하지 않는다. native 구현은 JSON에서 복원한 각 값을 같은 u8로 쓰고 범위·크기·세대·입력 소유권을 다시 검사해야 한다.

다음 불변조건을 유지한다.

- text와 binary는 하나의 FIFO를 사용한다. 진행 중 요청은 최대 하나다.
- 진행 중 항목을 포함하여 128개·256KiB, 개별 입력은 64KiB까지 받는다. binary는 실제 바이트 수, text는 기존 UTF8 바이트 수로 계산한다.
- 잘못된 byte-string과 용량 초과는 입력 전체를 거절한다. 받아들인 입력을 자르거나 중복 전송하지 않는다.
- 프로젝트·세대 변경과 terminal 폐기는 대기 중인 두 채널을 함께 취소한다. 이미 실행 중인 native 요청은 기존 소유권 규칙을 유지한다.

## 사례와 판정 기준

기존 `tests/terminal-flow.spec.ts`의 fixture와 실제 Edge/xterm을 사용한다. native IPC는 합성이므로 실제 PTY 바이트 보존의 검증과 구분한다.

| 사례 | 실제 진입점 | 독립 판정 기준 |
| --- | --- | --- |
| B01 DEFAULT | CSI로 mouse tracking을 켠 실제 xterm에 좌클릭 | public `onBinary`의 down/up 값과 같은 byte-string·`binary: true`가 기존 IPC에 도착한다. 낮은 좌표에서도 binary 채널이다. |
| B01 SGR | SGR mouse tracking을 켠 실제 xterm에 좌클릭 | public `onData` 및 기존 SGR 문자열을 유지하고 `binary` 필드를 붙이지 않는다. |
| B02 높은 좌표 | 실제 DEFAULT mouse의 96·223열 좌클릭 | 전송된 X 좌표 바이트가 각각 0x80·0xFF이며 0xC2/0xC3 UTF8 확장으로 바뀌지 않는다. |
| B03 순서 | native 쓰기를 보류하고 text→mouse down/up→text | 순서가 x→binary down→binary up→z이고 동시 native 쓰기는 하나다. |
| B04 바이트·용량 | 실제 큐 모듈의 공개 enqueue | 0x00–0xFF 전체를 포함한 64KiB payload 4개를 원형 그대로 받는다. 256KiB 상한, 전체 거절, invalid U+0100, 65,537바이트 거절 및 정상 drain을 확인한다. |
| B05 세대 | mouse가 대기 중일 때 세대 교체 | 예전 mouse를 취소하고 새 세대의 text만 후속 전송한다. |
| B05 프로젝트 | mouse가 대기 중일 때 다른 프로젝트 선택 | 숨겨진 프로젝트로 대기 중인 mouse를 전송하지 않는다. |
| B05 폐기 | mouse가 대기 중일 때 terminal 폐기 | 대기 중 mouse를 취소하고 이미 진행 중인 요청만 완료한다. |

기존 128개 입력 상한, Unicode 용량, 입력 재개, 취소·폐기 참조 해제, startup parse/ACK, 복사·붙여넣기 및 프로젝트 회귀 시험도 유지한다.

## 실행 근거

환경 변수는 `CI=1`, `PLAYWRIGHT_CHANNEL=msedge`로 고정했다. 기존 Vite dev 서버를 시험 runner가 소유하며, 설치·production 빌드·native GUI·Codex CLI·모델 실행은 하지 않았다.

제품 수정 전 명령:

```text
node node_modules/@playwright/test/cli.js test tests/terminal-flow.spec.ts --grep 'B01|B02' --reporter=line
```

`artifacts/terminal-binary-red.log`: DEFAULT의 실제 public onBinary payload는 정확했지만 IPC 전달 assertion 2개가 실패했고 SGR 1개는 통과했다. 종료 코드는 1이다. 최초에는 로그 디렉터리가 없어 명령 자체가 실행되지 않았으며, 그 호출은 RED 근거에 포함하지 않는다.

수정 후 B01–B05 명령:

```text
node node_modules/@playwright/test/cli.js test tests/terminal-flow.spec.ts --grep 'B01|B02|B03|B04|B05' --reporter=line
```

`artifacts/terminal-binary-green.log`: 8개 통과, 종료 코드 0이다. `node node_modules/typescript/bin/tsc --noEmit`도 종료 코드 0이며 근거는 `artifacts/terminal-binary-typecheck.log`다.

관련 전체 회귀 명령:

```text
node node_modules/@playwright/test/cli.js test tests/terminal-flow.spec.ts tests/project-tabs.spec.ts --reporter=line
```

`artifacts/terminal-binary-regression.log`: 위 새 사례를 포함하여 59개 통과·1개 skipped, 종료 코드 0이다. skipped는 기존 `TERMINAL_PERF_REPLAY=1`로만 켜는 비교 측정이며 실행했다고 보고하지 않는다. `git diff --check`로 담당 소스·시험 파일의 공백 검사도 종료 코드 0을 확인했다.

## 검증 한계

이 결과는 DEFAULT binary 전달 누락의 브라우저 재현과 frontend 수정 근거다. FCS02 당시 실제 mouse mode는 prefix 미관측으로 여전히 미확인이다. 이 component 결과를 FCS02의 실제 child 전환 성공으로 바꾸어 보고하지 않는다.

최종 native 결속에는 root 담당 Rust 구현의 `binary: true` raw byte 변환, 같은 native guard 통과·거절 및 실제 PTY 도착 바이트 검증이 필요하다. 기존 native API가 binary 표시를 무시하면 0x80–0xFF가 UTF8로 확장될 수 있으므로 frontend 시험만으로 native 호환 완료를 선언하지 않는다.
