# F01 권한 버튼 실제 지원 확인

고정 원본: `bacea878474678472a10d85617c4d147344ffe5f`. 원 결함은 실제 main이 확인된 권한 기능 지원과 무관하게 버튼을 생성·노출한 것이다. 공식형 `{}`를 받는 실행 환경에서도 비활성화만 하거나 팝업을 열게 해서는 안 된다.

요구: 브랜드·버전·공통 메서드 존재로 지원을 추정하지 않는다. 실제 현재 세션의 authoritative permission 응답만 노출 근거로 삼는다. 미지원·미확인·오류는 구분해 보존하고 버튼은 숨긴다. 준비 전 실패를 확정 미지원으로 바꾸지 않도록 기존 contextStatus의 실제 연결 확인 뒤에 조회한다. 보이는 active 세션의 확인된 thread/세대마다 최대 한 번, 전역 동시 한 번만 기존 읽기 RPC를 보낸다. 새 polling·retry·서버·attach·resume는 만들지 않는다.

공통 전제: 기존 Playwright/Vite runner에서 실제 `main.ts`를 실행하고 Tauri IPC만 fixture로 대체한다. 실제 CLI·운영 daemon·인증·설치·배포는 시험하지 않는다. 실패 assertion과 원시 출력은 수정 전 격리 사본에 보존하고 같은 테스트 파일·명령으로 수정본과 대조한다.

| ID | 입력·사전조건 | 절차 | 기대 결과·합격 기준 | 실제 결과 |
|---|---|---|---|---|
| PV01 | 연결된 실행 세션, `{}`/미지원/필드 미확인/통신 오류 | main 부트스트랩 후 권한 버튼과 기본 조작 검사 | 권한 비노출. 최대화·종료·ID 복사는 보존 | PASS, 입력 fixture 4개 |
| PV02 | 서로 다른 id/generation의 authoritative 응답 | 지원 확인 후 둘째 카드 권한 열기 | 확인된 카드만 노출, 자동 확인은 대상당 1회, 팝업은 선택 세대의 명시적 최신 조회 | PASS |
| PV03 | threadId는 있으나 기존 연결 미확인 | 렌더·기존 폴링 후 연결 확인, 숨김/복귀·재렌더 | 준비 전 권한 RPC 0회. 확인 뒤 한 번. 기존 폴링·복귀로 추가 확인 없음 | PASS |
| PV04 | 세 세션, 첫 권한 응답 보류 | 전역 숨김, 대기 세션 중단, 응답 해제·복귀 | 최대 동시 1. 숨김 중 대기 조회·버튼 노출 없음. 같은 세대 응답은 복귀 시 재조회 없이 정상 사용 | PASS |
| PV05 | gen1 응답 보류 중 같은 id의 gen2 연결 | 이전 응답과 새 응답을 순서대로 해제 | gen1은 gen2를 노출하지 않음. gen2는 자기 authoritative 응답으로만 표시 | PASS |
| PV06 | 삭제 전 권한 응답 보류 | 세션 종료 관측 뒤 카드 삭제, 늦은 응답 해제 | 삭제된 카드·지원기록·팝업을 되살리지 않음 | PASS |
| PV07 | 같은 id/generation에서 threadId 교체 | 이전 응답 해제 후 새 thread 확인 응답 보류·해제 | 이전 지원·팝업 폐기. 기존 contextStatus의 새 threadId와 일치한 뒤 자기 응답으로만 노출 | PASS |

네트워크 경계: 최초 자동 확인은 id/threadId/generation 대상마다 지원 발견용으로 한 번 수행하고, 팝업 열기/새로고침은 최신 상태를 원하는 명시적 조회다. 숨겨진 대기 확인은 보내지 않으며 이미 전송한 IPC 결과는 같은 대상에만 보존하고 숨김 중 표시하지 않는다. 복귀 시 같은 결과를 재사용한다. 진행 중 하나가 끝나기 전 다음 권한 조회는 보내지 않는다. 기록은 현재 카드 수명에만 존재하고 삭제·세대·thread 교체 때 해제한다. 최초 확인의 unknown/error에는 자동 재시도를 하지 않는다.

thread 경계 근거: `src/model.ts:55`의 updateSession은 동일 generation의 threadId 변경을 거부하지 않으며 `src-tauri/src/session.rs:245`는 ThreadId 이벤트를 SessionInfo에 대입한다. 따라서 thread 불변을 가정하지 않고 권한 확인 기록과 표시만 새 대상으로 무효화한다. 기존 대화 기능의 소유권·재개 경로는 변경하지 않는다.

검증: 같은 최종 테스트 파일·명령으로 격리된 bacea878 원본은 10 FAIL(exit 1), 수정 후보는 10 PASS(exit 0)였다. PV01은 실제 main의 권한 버튼이 visible이라 `toBeHidden` assertion에서 실패했다. 기존 권한 팝업·세션 도구·컨텍스트 회귀 37개 PASS, `npm run build` exit 0, test-engineering proof는 CONSISTENT다. 세부 argv·stdout/stderr·hash·receipt·한계는 [검증 보고](../verification/desktop-permission-visibility-fix-20261106/summary.md)에 보존한다. fixture로 대체한 native IPC 및 실제 CLI/daemon의 미검증 경계를 유지한다.
