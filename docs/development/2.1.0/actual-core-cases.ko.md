# 실제 Core baseline 사례

[English](actual-core-cases.md)

공개 Core 1.0.1과 개발 Desktop의 실제 경로를 먼저 검증한다. Core 1.1.0 같은 최종 후보 짝의 통합을 대체하지 않는다. 별도 identifier `io.agentgrid.desktop.integration210`, 새 workspace·WebView profile·HOME을 사용하며 운영 앱·daemon·인증과 다른 세션 자료는 건드리지 않는다.

`actual-core-fixture`는 명시적 Cargo 시험 feature다. 제품 기본 빌드에는 시험 코드가 포함되지 않는다. 실행은 자체 runner의 지정 HOME·Core exe와 기존 공유 서버를 사용하며 mock transport로 대체하지 않는다. 준비값이 없으면 실패하고 통과로 채우지 않는다.

| 사례 | oracle |
|---|---|
| 실제 config/read 투영 | 준비된 사용자 STDIO 서버의 원본과 typed effective 기본값을 구분해 수정 가능 판정. 이름·필드명만 기록, raw 값 비노출 |
| Native/WebView2 대상 | root가 시작한 exe hash·CDP와 별도 identifier를 먼저 확인한 뒤만 userdata IPC 호출 |
| 실제 add/edit/delete | 자체 프로젝트에서 한 사용자 서버 저장·정확 readback. 보호 env 보존·stale version 무변경·전송 전환 삭제 범위·최종 부재 확인 |
| 정리 | 예상 밖 세션이 없을 때만 시험 앱 종료. 운영 앱 PID/시작시각 보존. 준비된 daemon은 소유 HOME에서 별도 정리 |

`actual-mcp-red-01`은 실제 Core read 경로에서 0 PASS/1 FAIL/Cargo101이었다. 다른 키는 `enabled`, `environment_id`, `tool_timeout_sec`이며 기존 완전 일치 guard가 정상 사용자 구성을 거절했다. 수정 후 `actual-mcp-green-02`는 19 PASS/0 FAIL/0 ignored/Cargo0이다. 이 중 하나는 실제 공유 Core 읽기, 18개는 합성 RPC 회귀다.

`tests/native-mcp-manager.mjs`는 명시적 `AGENT_GRID_NATIVE_OUTPUT`의 root launch receipt가 필요하다. actual WebView2/Tauri IPC를 사용하지만 모델 turn·실제 인증·현재 thread reload·최종 installer·provider cache hit를 검증하지 않는다. 첫 `native-core101-02` 실행은 대상·실제 inventory가 통과한 뒤 add 결과의 `editable=false`로 실패했고 시험 앱은 종료했다. 수정 후 `native-core101-03`은 같은 실제 경로의 7개 사례 PASS/오류0/exit0이다. 새 실행 파일 SHA256은 `1c128725789919ca67eccf9922b020f68520d60bd7b18bcba0114f40e16aa142`이며 identifier만 별도인 개발 앱이다. `app-exit.json`의 운영 앱 identity 보존은 true이며, `D:/codex/build/s210c1/stop.json`은 자체 daemon의 stopped를 확인했다.
