# 실제 Native 구성 조회 경계

[English](native-composition-cases.md)

Root가 시작한 별도 identifier·고정 SHA 개발 앱, 자체 HOME의 Core, 빈 workspace/profile/CDP만 사용한다. 운영 앱과 다른 세션은 접근하지 않는다. 1.0.1 검증을 1.1 지원 완료로 표현하지 않는다.

- NC01: CI `fixture.json`의 commit·identifier·exe/manifest hash와 실제 App identifier/version을 확인한 뒤에만 Native IPC를 호출한다.
- NC02: 자체 실제 TUI 두 개와 선택 thread의 MCP/Hook 조회는 유지된다. 새 `contextComposition`도 session/generation/expectedThreadId에 결속한다.
- NC03: 명시 unavailable baseline에서는 `supported:false`, 고정 unsupported/unavailable reason, snapshot 부재를 요구한다. 조각·0 수치를 만들어내지 않는다. 이후 MCP 재조회 성공으로 서버가 계속 동작함을 확인한다.
- NC04: 잘못된 expectedThreadId는 거절된다. 실제 컨텍스트 아이콘의 팝업에서도 미지원/미확인을 표시하고 닫을 수 있다.
- NC05: 후속 Core1.1 available 모드에서는 실제 snapshot의 thread·ID·u64 문자열·page25를 확인한다. 전체 기능·TTL·도구/Hook 실행 검증은 별도다.
- NC06: 자체 앱/TUI를 정상 종료하고 Core 시작 receipt와 같은 endpoint/version만 중지한다. 운영 앱 identity를 별도 확인한다.

새 opt-in은 `AGENT_GRID_NATIVE_COMPOSITION_EXPECT=available|unavailable`이다. 기존 `native-smoke.mjs`/`native-runtime-extensions.mjs`를 재사용한다. 작성 시 실행 **NOT_RUN**이며 현재 Core1.1 실행물은 미수신이다. 모델 turn·Enter·신뢰 승인·설정 변경은 이 경계시험에 포함하지 않는다.

## 부모 후속 실행

`native-105-core101-context-07/smoke/native-smoke.json`: 고정 source105d625/CI exe a88a0ab8와 자체 Core1.0.1에서 실제 WebView2/Tauri IPC/TUI 13 PASS, errors0, Node exit0. 구성 조회는 supported=false/reason=unavailable/snapshot 부재였고, 다른 expectedThreadId는 거절됐다. 실제 아이콘 팝업에서 미확인을 표시하고 닫은 뒤 MCP도구1개 재조회 및 두 세션의 id/generation/thread 보존을 확인했다. 이는1.1 API 지원·모델/도구 실행·Hook/캐시 효과를 증명하지 않는다.

앱 exit0/stopReason null/운영 PID20076 시작시각·경로 보존true, 동일 자체 Core endpoint/version의 정상 stop과 stopped를 별도 확인했다. 근거는 `D:/codex/build/desktop210-integration-20261114/native-105-core101-context-07`이다. 실제1.1 available 모드(NC05)는 미검증이다.
