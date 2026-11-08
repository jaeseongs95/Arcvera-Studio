# 실제 Desktop 원격 연결 시험 계획

대상: CI42480861393, PR7 HEAD29cf53f / merge40d0253. 실행 파일 SHA2564643876993028c7bc66808b267d2a2dbf6e05d984b0c08ec709ff7bdd600de66, PE1.7.0. 로컬 Rust 빌드는 실행하지 않는다.

실행 전 정의한 사례:

1. 패키지의 SHA256SUMS 전 항목과 portable ZIP 안 실행 파일을 직접 대조한다. CI self-test237/0, 실제 실행 파일/보고서 해시·exit0 결속을 확인한다.
2. 자체 UUID 시험 폴더에 workspace DB, CODEX_HOME, APPDATA, WebView 프로필, TEMP를 격리한다. AGENT_GRID_CODEX를 이 폴더의 존재하지 않는 파일로 고정해 CLI·운영 daemon 실행을 차단한다. 기존 사용자 DB·인증·프로세스는 조회하지 않는다. 새 네이티브 창은 시험 중에만 사용하고 complete_exit로 정상 종료한다.
3. 고정 Remote 서버를 loopback 임시 포트에서 별도 생성한다. Desktop 설정의 원격 연결 버튼을 클릭해 URL과 합성 HostSecret을 입력한다. 실제 registered 뒤 연결됨과 비밀 입력칸 비움을 확인한다.
4. 브라우저 API로 create_project를 전송하고 실제 Tauri 어댑터→Engine 결과를 local list_projects와 대조한다. list_directory의 currentPath·entries 및 원격 종료 거부를 확인한다. CLI/TUI·모델 실행 성공으로 확대하지 않는다.
5. 설정 연결 해제를 클릭하고 releaseConfirmed=true, disconnected 및 새 호스트 수락 가능 상태를 확인한다. 잘못된 비밀로 연결하면401/error이며 명시 해제 전 자동 재접속하지 않는다.
6. 실제 창 스크린샷·사례별 결과·소유 자식의 exit를 보존한다. 실패와 시간초과를 성공으로 바꾸지 않는다. 실물 휴대전화·운영 중계기/TLS 연결·CLI 세션 경로는 미실행이다.

상한: 한 번에 Desktop1개와 자체 relay Node1개, 기존 프로세스 전체 열람/종료 없음. CDP는 자체 예약한 loopback 포트만. 각 대기 최대15초, 최종 정상 종료10초. 결과 저장 후 자체 child에만 최후 종료를 사용하며 이 경우 normal-exit 사례는 FAIL이다. 시험 폴더는 근거로 남긴다.

첫 CI42483127165는 창의 CDP 포트가 열리지 않아 FAIL이며 native 기능은 미실행이다. Tauri 공식 API E2E 문서와 wry#1782는 Runtime150 이후 상승 프로세스에서 환경변수 CDP 인자를 무시한다고 명시한다. 후속 시험은 같은 CI 사용자의 runas /trustlevel:0x20000으로 권한을 낮춘 Node에서 실행하며, 실제 medium integrity SID 확인을 필수 전제로 추가한다. 제품·UAC·레지스트리·허용 정책은 변경하지 않는다. 같은 원시 실패는 보존한다.
