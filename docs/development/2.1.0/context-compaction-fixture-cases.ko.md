# 구성 조회 후 압축 통합 fixture

[English](context-compaction-fixture-cases.md)

PR Windows source-1 CI43544691413에서 기존 `conversation-integration.spec.ts`의 terminal/conversation 압축 두 사례가 timeout으로 실패했다. 로컬 `context-compression-red-01`에서도2FAIL로 재현했고, 열린 `session-context-dialog`가 배경 compact/toggle 클릭을 차단하는 원시 trace를 확인했다. 새 구성 조회 동선에서는 정상적인 모달 차단이며, 원래 시험의 아이콘=갱신만 가정이 낡았다.

- CF01: 두 기존 startingView 사례를 모두 유지한다. 아이콘 클릭/40% 확인 뒤 구성 모달을 확인하고 실제 닫기 버튼으로 닫는다.
- CF02: 기존 압축1회·같은 id/generation·보기와 입력 소유권·초안 보존·새 세션/send 없음 assertion을 그대로 실행한다. retry/timeout 확대/force click/skip을 사용하지 않는다.
- CF03: 현재 전체 conversation-integration 파일을 실행해 다른 전환/실패/초안 보호 사례를 확인한다. 영향 파일을 표적 CI에도 포함한다.

제품 코드를 시험에 맞춰 바꾸지 않는다. 시험 동선을 수정하므로 이 전후를 같은 바이트의 제품 RED/GREEN으로 표시하지 않는다. 작성 시 수정 후 실행은 NOT_RUN이다. 실제 Core1.1/Native 압축은 이 합성 IPC 시험으로 증명되지 않는다.

부모 실행: `context-compression-flow-02`의 전체 conversation-integration12개가 PASS/18.8초/exit0이다. 두 원실패 사례와 나머지10개를 모두 실행했고 원래 assertion을 유지했다. 실제 Core·Native 또는 전체 PR CI의 통과로 확대하지 않는다.
