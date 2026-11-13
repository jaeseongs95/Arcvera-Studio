# Hook 현재 대화 조회 검증

[English](hook-verification.md)

이 문서는 준비물 작성 시점의 NOT_RUN 기록 이후 부모가 수행한 결과다. 기존 `hooks/list {threadId}`만 사용하며, Core1.1의 미동결 runtime metadata는 구현하지 않았다. 전체 실행 handler 수·gate·적용 완료는 미확인으로 남긴다.

| 실행 범위 | 결과·근거 |
|---|---|
| Native runtime adapter | `hook-runtime-native-01`: 27 PASS/0 FAIL/0 ignored/Cargo0. 기존 MCP16 + 새 Hook11의 메모리 WebSocket fixture |
| 대화 authority/lifetime 회귀 | `hook-conversation-native-02`: 124 PASS/0 FAIL/0 ignored/Cargo0. 새 Hook expectedThreadId·추가필드 거절 포함 |
| 새 panel·부모·MCP source fixture | `artifacts/hook-runtime-ui-01`: 34 PASS/2 FAIL. HP03/M210에서 늦은 close가 새 창 상태를 지우는 D82 확인 |
| 동일 실패 조건 수정 후 | `artifacts/hook-runtime-ui-02`: 36 PASS/0 FAIL. 변경 전 `extensions-before.ts` SHA449344fd 보존. 재열기 전에 옛 view를 정리하고, 이미 열린 새 dialog에는 늦은 close 정리를 적용하지 않음 |
| 기존 확장 회귀 | 표준 직접 Playwright 명령, exec session17903/chunk56f54c: 22 PASS/exit0. 복합 shell wrapper는 AGS 경로판정으로 실행 전 차단됐고 직접 프로젝트 test 명령으로 실행함 |
| 배치·최소 화면 | HP01/HP05 직접 Node Playwright: 2 PASS/exit0, `artifacts/hook-layout/results`. 앞선 npm.cmd 정규식의 pipe 전달 실패에서는 시험이 실행되지 않았으며 직접 Node 진입점으로 보완 |
| TypeScript | `npm exec -- tsc --noEmit` exit0 |
| 비시험 개발 앱 | `actual-app-build-04` exit0, exe SHAd5aa9d5d402df020049351bb4ad6e6726158fe7c6b6938085569e53241d8b464. 별도 identifier fixture이며 최종 패키지가 아님 |

실제 실행 근거의 공통 경로는 `D:/codex/build/desktop210-integration-20261114`다. own Core1.0.1에 로컬 custom provider/catalog를 적용하고 실제 TUI thread 두 개를 생성했다. `runtime-core101-04`의 Native14건은 조회·세션 보존·언어·재개 오류·크기/순서·합성 한글 입력·종료 경계를 확인했다. `runtime-core101-05`는 실제 확장 팝업의 버튼과 표시를 추가해 14 PASS/오류0/exit0이다.

MCP 도구 한 개의 발견과 Hook inventory 0건을 실제 선택 thread에서 관측했고 stale expectedThreadId는 거절했다. Hook 0건을 기능 비활성으로 추정하지 않았다. runtime DTO는 key/command/matcher/path/diagnostic 원문 대신 읽기 전용 hash ID와 허용 metadata만 투영한다. 실제 GUI의 세션 ID/generation/thread와 운영 앱 identity는 보존됐고 자체 앱·Core는 종료됐다. provider의 최종 health는 요청0/inFlight=false이며 자체 프로세스도 종료됐다.

이 결과는 모델 turn·실제 인증·MCP 도구 호출·Hook handler 실행·설정 변경 후 현재 thread reload·provider cache hit·Core1.1·최종 installer·장기 부하·독립감사 완료를 뜻하지 않는다. source fixture와 실제 경로의 수량을 합쳐 모두 실제 Core 시험이라고 표현하지 않는다.
