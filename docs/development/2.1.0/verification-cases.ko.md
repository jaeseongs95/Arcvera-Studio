# Desktop 2.1.0 사전 검증 사례

2026-11-14 KST. 실행 전 정의한다. 모든 사례는 현재 NOT_RUN이며, CLI 담당의 계약·실행물 전달 뒤 구체 입력과 예상 wire를 고정한다. 기존 runner와 시험을 재사용하고, 모의 경계와 실제 Core·GUI 실행을 분리해 기록한다.

| ID | 요구·실제 진입점 | 입력·실패 경계 | 통과 조건·증거 |
|---|---|---|---|
| P01 | S04, 공유 서버 준비·연결 | Core1.0.1 기준과 1.1 개발 후보의 실제 제품 접두사·실행 경로·최초/기존 준비 | 선택한 정확한 파일·버전·세대와 연결됨. 기존 D70/71 회귀 없음. 원본 설정·운영 daemon 불변 |
| P02 | S01, 세션 도구의 컨텍스트 설정 | 두 thread의 서로 다른 크기, 기본값 복원, 늦은 응답·잘못된 ACK·거절·응답 유실 | 정본 요청/유효 크기와 UI 일치. 다른 thread 변경 없음. 결과 불명 보존·중복 송신 없음 |
| P03 | S01, 실제 압축 요청·이벤트·이력 | 압축 정상·실패·취소·연결 유실, 구세대 완료 이벤트 | 실패를 한 번 표시하고 실행 완료·문맥 보존을 오판하지 않음. 실제 모델 요청/계보는 Core 증거와 대조 |
| P04 | S02, MCP 관리 | STDIO/HTTP 추가·변경·삭제, 기존 이름·CAS 충돌·정책 덮어쓰기·재조회 불일치 | 정확한 사용자 계층과 좁은 요청만 변경. 저장과 연결을 구별하고 임의 secret/config를 renderer에 노출하지 않음 |
| P05 | S02, 지원되는 thread reload | 추가/변경/삭제 전후 실제 도구 목록·호출, reload 실패·비지원·진행 작업·응답 유실 | 합의한 적용 범위에서만 성공 표시. 신규/기존 thread 차이와 실제 tool discovery를 확인 |
| P06 | S03, hook 목록·검토·변경 | feature off/on, trusted/untrusted/modified/managed, 비활성 유지, 표시 후 hash 변경·정책 거절 | 현재 hash만 신뢰 승인. 신뢰와 enabled·feature를 혼동하지 않음. 자동 권한 확대 없음 |
| P07 | S03, 실제 hook 적용 | 설정 변경 뒤 새/current thread의 무해한 fixture 실행·실패·중복·연결 유실 | 합의된 범위의 실제 실행과 표시가 일치. 재조회만으로 실행 성공을 주장하지 않음 |
| P08 | S04/S05, 취소·재접속·닫기 | 진행 요청 중 취소·proxy EOF·시간 초과·관측 연결 종료·늦은 이벤트 | 취소 접수와 효과 종료 구별. 구세대 상태 미반영. 소유 연결·프로세스 회수, 공유/무관 프로세스 보존 |
| P09 | S01/T78, context_index 도구 출력 | 실제 live 및 history에서 제공되는 공통 tool 항목과 필요한 상태 | 현재 정본 계약으로 보존·표시되는지 확인. 전용 UI를 임의로 추가하지 않음 |
| P10 | S06, 최종 후보 | 개발 후보 공동 시험 후 최종 Windows 패키지·문서·CI·감사 | 실제 패키지와 버전·해시 확인, 정확 commit CI, 영한 README/릴리스노트·라이선스 보존. 공개 단계는 별도 승인 |

P06-H1 (D77, 실행 전 추가): 기존 extensions Playwright fixture에서 `enabled=true`이고 `trustStatus=untrusted/modified`인 Hook을 조회한다. 목록·상세는 “사용 설정됨”과 신뢰 차단을 각각 표시하고 “사용 중”으로 표현하지 않아야 한다. 빈 Hook 목록에는 조회 범위에 항목이 없다는 사실만 표시하고 기능 활성·실행 가능 여부가 확인됐다고 추정하지 않아야 한다. 기존 fixture가 대신하는 IPC와 실제 Core gate 검증은 구분한다. 같은 새 시험을 수정 전 RED와 수정 후 GREEN에 사용한다.

환경·실행 계획: `D:/codex/build` 아래 이 작업 전용 경로와 자체 HOME/TEMP/프로젝트를 사용한다. 실제 인증이나 타 세션 원시 대화·개인 로그는 읽거나 복사하지 않는다. 1420 포트의 기존 Playwright runner는 root가 직렬 사용하며 기존 서버가 다른 작업 소유인지 먼저 확인한다. Cargo는 기존 도구 체인·캐시와 현재 자원을 확인한 뒤 자기 명령에만 한도를 적용한다. 최종 Rust release 빌드는 양쪽 개발 실행물의 통합 결과를 확인한 뒤 수행한다.

결과에는 후보 commit·실행 파일 해시·명령·종료 코드·tests/failures/errors/skips·원시 근거 위치·미검증 경계를 기록한다. 잘못된 fixture나 실행기 오류는 제품 RED와 구분하고, 일부 통과로 전체 완료를 만들지 않는다.

## 진행 중 단위 검증 근거

아래는 합성 IPC를 사용하는 기존 Playwright UI 경계의 결과이며, 위 P01~P10 전체나 실제 Core 통합 완료를 뜻하지 않는다. 경로는 작업본의 `artifacts/` 기준이다.

| 대상 | 실제 결과 | 남은 경계 |
|---|---|---|
| 기존 context·extension 기준 | `baseline-ui/run.log`, exit 0, 56 PASS, 실패·skip 0 | Core/Tauri 실행 경계는 fixture로 대체 |
| D77 / P06-H1 수정 전 | `d77-red/run.log`, exit 1, 1 assertion FAIL. untrusted Hook에 “사용 중” 표시를 실제 DOM에서 관측 | 실행기 실패가 아닌 기대 동작 불일치 |
| 같은 시험·같은 입력의 수정 후 | `d77-green/run.log`, exit 0, 1 PASS | 구성·신뢰 표시와 빈 목록 설명만 확인 |
| 확장·MCP 영향 회귀 | `d77-regression/run.log`, exit 0, 27 PASS, 실패·skip 0. `typecheck.exit.txt` 0 | 실제 feature gate·현재 thread 적용·MCP discovery·캐시 구성 조회·최종 패키지는 미검증 |

`frontend.yml`은 현재 2.1 작업 브랜치의 frontend 변경을 검증하며 Rust/Core·최종 Windows 패키지 검증을 대신하지 않는다. private 저장소에서는 수동 실행만 허용한다. 최종 후보는 기존 전체 Windows 검증도 별도로 확인한다.
