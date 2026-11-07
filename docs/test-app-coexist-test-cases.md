# Desktop 1.6.1 TEST 앱 공존 — 구현 전 사례

고정 기준: `4f2a33cf8adcc731048b06804c56aca805e29584`. 전용 작업 브랜치 `test/desktop161-local-coexist`. 일반 배포 동결본은 변경하지 않는다. 기존 `npm run tauri -- build --config src-tauri/tauri.bundle.conf.json --bundles nsis` 경계를 사용해 TEST override만 바꾼다. 버전1.6.1, Rust/TS/HTML·Cargo·dependency·workflow·plugin·runtime 동작은 보존한다. 설치·발행·실제 GUI·운영 Desktop7400·서비스·auth·R38는 이 작업에서 다루지 않는다.

요구: TEST identifier `io.agentgrid.desktop.test`, productName 및 main title `OpenDIY Desktop TEST`. windows 배열 override는 원 main 객체의 모든 필드를 그대로 포함하고 title만 바꾼다. 기본 app data namespace가 달라져야 하며 실제 시험에서는 별도 own fixture 환경으로 한 번 더 격리한다.

| ID / 입력·전제 | 절차 / 공개 경계 | 기대 및 실패 기준 | 실제 결과 |
|---|---|---|---|
| TA01 / base tauri.conf.json + 기존 bundle override | RFC7396 merge 규칙으로 병합하고 기준 병합과 모든 leaf를 대조 | 변경 leaf는 productName/identifier/app.windows[0].title뿐. window label/main, dragDropEnabled=false, 1400x900/min800x540와 나머지 bundle/security/build/plugins 보존. version1.6.1 유지 | 선작성 시 NOT_RUN; 정적 receipt에 결과 기록 |
| TA02 / candidate4f tracked source | Git blob tree와 diff를 후보 기준으로 대조 | 허용 config와 이 사례 문서 외 tracked 변화0. Rust/TS/HTML·Cargo·workflow·baseconfig byte동일. 기존 소스 수정 발견 시 중단 | 선작성 시 NOT_RUN; 정적 receipt에 결과 기록 |
| TA03 / default path source | lib.rs setup의 app.path().app_data_dir()와 Tauri identifier contract 대조 | workspace override가 없으면 Windows app data는 config identifier별 namespace. TEST io.agentgrid.desktop.test와 production io.agentgrid.desktop가 다름. 실제 filesystem/GUI를 보지 않은 정적 결론은 실제 app 확인으로 표현 금지 | source 확인; 실제 runtime NOT_RUN |
| TA04 / own AGENT_GRID_WORKSPACE_DIR | 실행 담당이 새 own 절대경로와 격리 APPDATA/LOCALAPPDATA/HOME/TEMP 및 CODEX_HOME를 지정한 TEST fixture에서 app 시작 | Desktop DB는 own override/workspace.sqlite3. override는 app metadata만 바꾸고 Codex home/auth를 격리하지 않으므로 CODEX_HOME 등도 별도 own 환경이어야 함. 운영 데이터·기존 auth 채택 금지 | source 확인; 실제 app/DB/env 확인 NOT_RUN |
| TA05 / invalid override | 기존 workspace_directory 경계에서 empty/relative 입력 검사 | empty/relative는 오류이며 default나 운영 경로로 fallback하지 않음. source상 현재 조건 보존 | 기존 source 보존; Rust 실행 NOT_RUN |
| TA06 / single instance 공존 | root가 향후 격리 TEST GUI 차례에서 정확한 TEST app identity/default path를 확인 | 기존 single-instance plugin 보존. TEST 재실행/기존 production와의 공존 결과는 실제 관측 필요. identifier 차이만으로 실제 운영 공존 PASS 주장 금지. 이 작업은 운영7400을 실행·조작하지 않음 | NOT_RUN |
| TA07 / 기존 CI | 기존 workflow의 ordinary build --config 경계와 dependency/native gate 검토 후 root가 정확 branch dispatch | workflow 변경0, native 및 Windows artifact는 root CI로 확인. test/** push는 기존 push branch filter에 포함되지 않으므로 root의 workflow_dispatch 필요. signing 관련 추가 config/키 참조 없음 | source 검토; CI는 root 담당/NOT_RUN |

완료 판정: 이 작업은 사례·최소 override·정적 merge/source 동일성 및 지정 작업브랜치 commit/push까지다. 실제 CI artifact에서 TEST name/id/title/app data path·own environment·single instance·GUI 검증은 root의 후속 실행 근거로 별도 판정한다. 로컬 Rust 빌드0, 신규 dependency0, GUI/app/model/CLI runtime 실행0이며 syntax나 Git hash를 실제 앱 PASS로 바꾸지 않는다.

정적 검증 근거는 ignored `artifacts/testapp-coexist01/`에 저장한다. Tauri CLI는 --config를 JSON Merge Patch로 병합하며 배열은 완전 교체되므로 원 windows 전체 값을 유지한다. 참고: https://v2.tauri.app/reference/cli/ 및 https://docs.rs/tauri/latest/tauri/path/struct.PathResolver.html#method.app_data_dir . 소규모 설정 결속으로 제품 결함 회귀나 red/green은 추가하지 않고 병합 결과 전체 대조를 사용한다.

직접 대조한 CLI2.12.4 구현은 https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/src/helpers/config.rs 의 load_config에서 json_patch::merge를 적용한다. 실제 Rust dependency는 Cargo.lock의 Tauri2.12.5이며 위 PathResolver2.12.5 contract와 대조했다. TA01/TA02 정적 검사는 exit0: merged leaf 변경3개만 존재, 원 main의 title 외 전체 값 보존, override 제외 기존 tracked209파일 blob동일. 이 결과는 실제 Tauri build/schema검증이나 app 실행을 대신하지 않는다.
