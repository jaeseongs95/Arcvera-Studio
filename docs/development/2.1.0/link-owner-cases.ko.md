# 터미널 링크 클릭 소유권

## 명시적 host 소유권의 xterm 회귀 사례

HX01–HX05는 실제 xterm의 none/X10/1000/1002/1003 각각에서 plain/OSC8 링크와 무수정·Ctrl·Meta·Alt·Shift·Ctrl+Shift·Ctrl+Alt를 확인한다. Shift 선택은 opener 0, 나머지는 Desktop opener 1이다. 실제 SGR Down/Up 보고도 함께 확인하며 이 보고를 차단하지 않는다. HX06은 누른 뒤 mouse mode 또는 modifier가 바뀌어도 opener가 중복되거나 없어지지 않는지 확인한다. 이 사례는 명시적 host 계약의 Desktop 소비자 경계이며 실제 Core dispatch/OS 창 개수 증거는 아니다. Core 실행물과 같은 후보 검증은 별도 유지한다.

## 준비 조회와 종료 경쟁 회귀

전체 CI 43600272698의 종료 경쟁 시험은 새 `resume --help` 준비 조회를 합성 CLI가 실제 TUI 실행으로 처리하여 관측 proxy 연결 전에 실패했다. 합성 CLI에 옵션이 없는 정상 resume 도움말 응답을 추가한다. 제품 기한이나 종료·재전송 방지 assertion은 변경하지 않는다. 동일 `shared_reaper_handoff_during_observer_join_reports_unknown_for_stop_and_shutdown`의 stop/shutdown 두 경로를 다시 검증한다. 이는 시험 CLI 계약 수정이며 실제 Core 링크 중복 해결의 증거가 아니다.

수정 뒤 위 기존 시험은 로컬 Windows에서 1 PASS/0 FAIL/0 ignored, Cargo exit 0으로 두 경로를 통과했다. 근거: `desktop210-integration-20261114/d93-resume-help-green/{stdout.log,result.json}`. 전체 CI의 최초 574 PASS/1 FAIL/7 ignored 결과는 보존하고 후속 전체 회귀와 구분한다.

[English](link-owner-cases.md)

현재 상태: 아래 모드 기반 규칙은 원인 판별용 prototype으로만 보존했다. xterm만 감지하고 Core는 인식하지 못하는 URL에서 열기가 사라질 수 있어 제품 변경을 원복했으며, 해결 완료로 판단하지 않는다. Core와 세션별 `--terminal-link-owner=tui|host` 지원 확인 및 HTTP(S) 마우스 링크 단일 소유 계약을 검토한다. Core standalone과 비HTTP·비마우스 인증 요청은 보존해야 한다. prototype과 41개 source 시험 결과는 `desktop210-integration-20261114/d93-mode-prototype` 및 `d93-green-01`에 남긴다.

현재 소비자 초안은 모드 추정을 사용하지 않고 실제 resume 도움말 선언으로 host 옵션을 전달한다. 별도로 제스처 이동 이력을 보존해 마우스 보고 중 되돌아온 drag가 링크 click으로 실행되지 않도록 했다. 같은 drag 사례는 수정 전 1 FAIL, 수정 후 기존 링크·선택 시험과 함께 13 PASS/TypeScript exit0였다. Native 새 협상 시험과 실제 지원 Core 통합은 아직 NOT_RUN이며 D93은 미해결 상태다.

합의한 소비자 경계: 선택한 동일 CLI의 `resume --help`를 기존 기한·취소 가능 명령 경로로 조회한다. 실제 옵션 선언 열에 `--terminal-link-owner`가 있을 때만 resume 명령에 `--terminal-link-owner host`를 추가한다. 버전 숫자나 설명 문구로 지원을 추정하지 않는다. 미선언 CLI에는 기존 argv를 유지하고, 잘못된 help나 조회 실패는 세션을 만들기 전에 오류로 처리한다. Core의 default tui 및 HTTP(S) 외 링크와 비마우스 인증은 보존한다. source11 개발 패키지에는 이 옵션이 아직 없다.

Native 사전 사례 B01~B05: (1) 실제 옵션 선언이면 host argv 1개, (2) 미선언이면 기존 argv 동일, (3) 설명 문구·접미 이름은 지원 아님, (4) 잘못된 help/UTF-8 거절, (5) 동일 CLI·thread/socket/cwd·무토큰 경계를 유지한다. 새 옵션을 가진 실제 Core와의 합계1회 검증은 별도이며 단위 argv 결과로 대체하지 않는다.

Host 소유 전환에서도 실제 drag를 click으로 오인하면 안 된다. 추가 사전 사례는 마우스 보고 ON에서 plain/OSC8 링크를 끌어 시작점으로 돌아와 놓았을 때 opener0, 그 뒤 새 click은1이다. DOM selection 유무만으로 판단하지 않고 해당 제스처의 이동 이력을 유지한다. mouseup/blur/dispose에 추적 listener를 회수하며 이벤트 전파나 PTY 보고를 막지 않는다.

D93: 사용자 이중 창 보고와 a5ea3845의 소스를 대조했다. Desktop의 링크 activate는 직접 opener를 호출하며, xterm은 같은 제스처를 PTY로 보낸다. Core는 일반 정지 클릭의 Up뿐 아니라 Ctrl/Super Down에서도 링크를 열 수 있다. 단순 preventDefault나 modifier 예외는 단일 소유를 보장하지 않는다.

원칙: 실제 xterm 보고와 Core 클릭 계약으로 소유를 정한다. none과 X10은 Desktop이 연다. X10은 Up을 보내지 않고 Ctrl 등 수정키도 제거한다. vt200/drag/any에서 Ctrl Down은 Core가 이미 열 수 있고, 무수정 Down/Up 쌍도 Core가 연다. 이때 Desktop은 추가 opener를 실행하지 않는다. Alt-only는 Core가 열지 않아 Desktop 동작을 유지한다. Shift 선택과 document mouseup 정리, Core standalone 동작을 보존한다.

사전 사례:
- LO01: 실제 xterm에서 none/X10/1000/1002/1003 × plain/OSC8 클릭을 실행한다. none은 Desktop opener 1/PTY mouse 0, X10은 Desktop opener 1/Down만 보고, 나머지는 Desktop opener 0/실제 mouse Down·Up을 확인한다. 이는 실제 Core opener·OS 창 개수의 증거가 아니다.
- LO02: release 보고 모드의 Ctrl 클릭은 Desktop 추가 opener 0이며 실제 수정키 mouse 보고를 보존한다. X10은 수정키가 제거돼 Desktop이 열고, Alt-only도 Core가 열지 않아 Desktop 처리를 유지한다. Shift 선택은 추가 열기와 mouse 보고가 없다.
- LO03: Shift 드래그와 일반 선택, 링크에서 이동 후 mouseup 정리를 기존 LD 시험으로 확인한다. LD03의 후속 Desktop 클릭은 명시적으로 보고를 끈 뒤 실행한다. 기존 one-open assertion을 제거하지 않으며, 활성 모드의 소유는 LO01/02가 별도 검사한다.
- LO04: Ctrl Down 뒤 보고를 꺼도 이미 열린 링크를 Desktop이 다시 열지 않는다. 무수정 Down 뒤 보고를 끄면 Up이 전달되지 않아 Desktop이 처리한다. none에서 시작한 Down 뒤 보고를 켜도 Desktop 소유를 유지한다.
- LO05: 같은 후보의 실제 Core TUI 링크 인식 위치에서 양쪽 opener 합계 1을 확인한다. 실제 동작·modifier/drag·선택 정리 검증 전에는 사용자 문제의 해결을 선언하지 않는다.

기존 Playwright source fixture와 실제 xterm public API를 사용한다. 새 의존성, PTY 데이터 필터, stopPropagation, 전역 마우스 모드 변경을 제품에 추가하지 않는다. 수정 전 LO01의 활성 모드 반례와 수정 후 같은 assertion을 대조한다. 실제 Core/OS 창 검증은 아직 NOT_RUN이다.

Core D53과 같은 원인으로 연결한다. xterm `CoreMouseService.X10.restrict`가 수정키와 Up을 제거하고, Core `pointer_down`이 Ctrl/Super에서 즉시 열며 일반 Up은 양쪽 무수정 상태를 요구하는 소스를 대조했다. 첫 RED는 기존 8개 중 6 FAIL/2 PASS였다. 보고 패킷 assertion을 먼저 확인하도록 보완한 두 번째 RED는 실제 Down·Up 각각 1개가 통과한 뒤 Desktop 추가 opener 1개에서 실패했다. 두 원결과를 `d93-red-01`, `d93-red-02`에 보존한다.
