# 세션 권한 확인 사례

범위: 로드된 현재 세션의 후속 turn용 기본 설정과 관측된 승인 요청을 읽기 전용 팝업으로 확인한다. `sandboxPolicy`는 permission profile의 legacy 호환 요약이며, 현재 action이 캡처한 권한이나 OS 전체의 실제 enforcement를 나타내지 않는다. 설정 변경, GUI attach, 입력 소유권 취득, 새 서버 시작, 다른 adapter fallback은 제외한다.

계약 근거: CLI 담당의 [소스 계약 답변](../../r38-0167-20261106/desktop-contract-review-01/answer.ko.md)을 직접 읽고 SHA256 `dff4c1ad619a448636ad1d86d612d1006707239dd6a2cabcbdac846bd0cc7b32` 일치를 확인했다. 조회값을 실행 중인 작업·운영체제의 전체 실효 권한으로 표시하지 않는 안내를 PERM06의 UI assertion으로 연결한다. 이 답변은 실제 운영 실행 결과가 아니다.

공통 전제: Rust는 기존 socket fixture, UI는 `permission-status.ts` 직접 import fixture를 사용한다. 실행 앱·공유 daemon·인증 파일에는 접근하지 않는다. 실제 세션 검증은 별도로 남긴다.

| ID | 입력·사전조건 | 절차 | 기대 결과·합격 기준 | 실제 결과 |
|---|---|---|---|---|
| PERM01 | 확인 ACK와 승인·reviewer·sandbox·프로필 필드 | 권한 조회 1회, wire·응답·소유 상태 검사 | initialize experimentalApi=true 후 threadId만 조회. 필요한 필드만 반환. 설정 쓰기·attach·resume 0건, 입력 소유 불변 | 미실행 |
| PERM02 | 공식형 `{}` 또는 RPC -32601 | 권한 조회 1회 | supported=false와 noAuthoritativeSettings/methodUnsupported 구분. 값 추정·fallback·재시도 0건 | 미실행 |
| PERM03 | settingsApplied 누락/false/문자열, 필드 누락·잘못된 enum·잘못된 sandbox 구조 | 각 응답으로 조회 | 확인 미제공과 invalidPermissionFields 구분. 권한값을 정상 기본값으로 채우지 않음 | 미실행 |
| PERM04 | granular 정책, sandbox 네 종류, nullable 프로필 | DTO 정규화 검사 | 확인한 allowlist 필드만 보존. 잘못된 타입 거부. 미지 필드·설정 전체·비밀 sentinel 미반환 | 미실행 |
| PERM05 | 승인 3종, 일반 질문, 중복 ID, 불완전 관측 | 로컬 snapshot 포함 조회 | 승인 종류·ID·metadataAvailable만 반환. 질문·명령 본문 제외. 관측 불완전 시 unknown이며 0건 확정 금지 | 미실행 |
| PERM06 | UI 정상 snapshot, 제목에 HTML 모양 문자열 | 팝업 open | 정확한 id/generation/permissionStatus 호출 1회. 후속 turn 기본 설정·샌드박스 호환 요약·실효 권한 경계 안내 확인. 읽기 전용 필드·제목 textContent. 승인·설정 입력 없음 | 안내 assertion 추가 후 미재실행 |
| PERM07 | 미지원·형식미확인·통신실패 | 팝업 open 각각 실행 | 서로 다른 안내. 통신 오류 원문/비밀 sentinel 미표시. 다른 호출·자동 재시도 없음 | 미실행 |
| PERM08 | A 조회 보류 후 B 세대 open 또는 닫기/dispose | A 늦은 응답 해제 | A 응답이 B·닫힌 팝업을 덮지 않음. dialog 제거·리스너 정리 | 미실행 |
| PERM09 | Escape·닫기·재열기·수동 새로고침, 다른 화면에도 새로고침 버튼 존재 | 권한 dialog 범위의 버튼으로 키보드/버튼 조작 | 접근 가능한 dialog 이름·초점, 닫기 후 제거. 명시 새로고침만 재조회 | 첫 통합 실행 locator strict mode 실패 후 범위 수정, 미재실행 |
| PERM10 | 연결 중단·RPC 서버 오류·기한 경과 | 단일 조회 후 wire·결과 확인 | 통신 실패를 supported=false로 바꾸지 않음. 비밀을 포함할 수 있는 원문은 팝업에 출력하지 않음 | 미실행 |

실행 경계: 부모 통합 전 포트 1420/e2e와 Cargo 실행은 금지되어 있다. 작성한 사례·코드와 실제 실행 결과를 구분해 부모에게 전달한다.

초기 작성 후 담당이 실행한 정적 검사: `npx tsc --noEmit` 종료 코드 0. Rust 3개 변경 파일의 `git diff --check` 종료 코드 0. 동작 사례 실행은 부모가 담당한다.

부모가 전달한 첫 UI 통합 결과는 권한 fixture 6개 중 5 PASS, 1 FAIL이다. PERM09의 페이지 전체 새로고침 locator가 주간 사용량과 권한 팝업의 버튼을 함께 찾아 strict mode에서 실패했으며, 이번에 권한 dialog 범위로 수정했다. 원 실패 자료 `test-results/followup-first/permission-status-PERM09-동일한-진행-중-조회는-합치고-명시-새로고침만-다시-조회한다/error-context.md`와 `trace.zip`의 존재를 직접 확인했고 변경하지 않았다. 이 보완 후 e2e·Cargo는 부모의 직렬 실행을 기다리며 재실행하지 않았다.

부모 최종 브라우저 실행: `npx playwright test tests/session-tools-layout.spec.ts tests/permission-status.spec.ts --reporter=line`, 종료 코드 0, 13 PASS 중 권한 팝업 6개 PASS. `test-results/desktop140-glyph-final` 보존. 원 e49e4af CI42179391322의 native job은 success로 직접 조회했으며 실제 공식/OpenDIY 세션 응답이나 설치 후 검증으로 확대하지 않는다. 위 표의 사전 미실행 및 최초 실패 기록은 이 실행 이력과 구분한다.
