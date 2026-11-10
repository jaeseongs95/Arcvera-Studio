# Arcvera-Studio 명칭과 업그레이드 호환

[English](rebranding.md) · [처음으로](../README.ko.md)

기존 OpenDIY Desktop의 제품명은 1.8.0 후보부터 Arcvera-Studio입니다. npm/Cargo 패키지는 `arcvera-studio`, Windows 실행파일은 `arcvera-studio.exe`입니다. 소스와 릴리스의 저장소는 [jaeseongs95/Arcvera-Studio](https://github.com/jaeseongs95/Arcvera-Studio)입니다. 이 변경은 개발 후보이며 패키지 생성이 설치나 게시를 뜻하지 않습니다.

## 기존 설치와 설정

앱의 고정 식별자 `io.agentgrid.desktop`은 유지합니다. 기존 WebView 데이터와 앱 데이터베이스의 위치를 결정하는 값이며 화면에 표시하는 제품명이 아닙니다. 이번 소스 변경으로 데이터베이스·프로젝트·대화·인증 파일이나 실행 중인 프로세스를 이동하지 않습니다.

NSIS 템플릿은 `OpenDIY Desktop`으로 끝나는 기존 제거 프로그램 레지스트리 키와 설치 위치 키 `Software\agentgrid\OpenDIY Desktop`을 유지하고 Windows 표시명을 바꿉니다. 업그레이드는 기존 설치 폴더를 복원하며, 공식 설치 코드가 기록된 이름으로 이전 실행파일을 정리합니다. 바탕화면과 시작 메뉴의 기존 바로가기는 대상이 해당 설치의 실행파일과 정확히 일치할 때만 이전합니다. 같은 새 이름을 사용 중인 사용자 바로가기는 보존합니다. 설치된 앱이 실행 중이거나 실행 상태를 확인하지 못하면 작업을 종료시키지 않고 설치를 중단합니다. `opendiy-desktop.exe`를 직접 지정한 사용자 바로가기·스크립트는 승인된 업그레이드 후 `arcvera-studio.exe`로 수정해야 합니다. 포터블 사용자는 기존 앱 데이터를 유지하고 새 실행파일을 선택할 수 있습니다.

권장 환경변수는 `ARCVERA_STUDIO_WORKSPACE_DIR`과 `ARCVERA_STUDIO_CODEX`입니다. 기존 `AGENT_GRID_WORKSPACE_DIR`과 `AGENT_GRID_CODEX`도 계속 읽습니다. 새 값이 있으면 비어 있거나 잘못된 값이어도 우선하며, 다른 경로로 조용히 바꾸지 않고 검증 오류를 표시합니다. 앱에 저장한 실행파일 경로는 환경변수보다 우선합니다.

브라우저 언어 캐시는 `arcvera-studio-language`를 먼저 읽고, 키가 없을 때만 `agent-grid-language`를 읽습니다. 새 선택은 새 키에 저장하며 기존 값은 남깁니다. 네이티브 데이터베이스가 언어 설정의 영속 정본입니다. 이전 실행파일·패키지와 유지된 데이터 위치를 보관해 복구할 수 있으며, 이전 버전을 제거할 때 앱 데이터 삭제를 선택하지 않아야 합니다.

## 업데이트

빌드 공개키 이름은 `ARCVERA_STUDIO_UPDATER_PUBLIC_KEY`이며 기존 `OPENDIY_UPDATER_PUBLIC_KEY`도 호환합니다. CI는 새 저장소 변수가 없으면 기존 변수를 사용합니다. 개인 서명키의 기존 secret 설정은 유지합니다. 키를 재발급하거나 출력하지 않습니다.

새 앱은 `arcvera-latest.json`을 읽고 새 저장소·동일 릴리스 태그의 설치파일 URL만 허용합니다. `latest.json`은 기존 앱의 고정 경로 검사와 호환하도록 이전 저장소 URL을 담습니다. GitHub의 기존 이름 변경 리디렉션을 통해 같은 설치파일과 서명으로 연결됩니다. 별도 릴리스 승인 후 두 manifest·설치파일·서명·체크섬을 함께 게시해야 합니다. 이전 저장소 리디렉션이 같은 저장소를 가리키지 않게 되면 호환 업데이트 경로를 중단하고 새 패키지를 수동 제공해야 하며 URL 검사를 완화하지 않습니다.

## 유지하는 이름

OpenDIY Codex와 OpenDIY Remote는 별도 제품입니다. 실행파일명·capability·`opendiy/asyncQuestion`·TUI 파이프 접두사·launcher 조회 플래그·relay 헤더는 외부 계약을 유지합니다. 버전이 붙은 진단 schema, 소유권 marker, 시험 환경변수와 과거 검증 문서는 기존 수집기·안전 검사를 위해 원래 이름을 유지합니다. 첨부 캐시의 기존 폴더도 유지합니다. 이 값들은 호환 식별자이며 현재 제품 표시명이 아닙니다.

설치 템플릿은 [Tauri CLI 2.12.4](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)를 기반으로 하며 MIT 라이선스는 `src-tauri/installer/TAURI-LICENSE-MIT`에 있습니다. 변경 부분은 업그레이드 기록 보존과 실행 중 앱을 종료하지 않는 검사입니다. 고정 CLI 버전을 바꿀 때 공식 템플릿과 다시 대조해야 합니다.

## 검증 범위

Node 패키징 시험은 합성 설치파일·서명 bytes를 사용하고 브라우저 시험은 native bridge를 대체합니다. Windows 컴파일과 headless PTY 검사는 CI에서 수행합니다. 실제 설치·운영 WebView·실제 CLI 대화 삭제·물리 휴대전화 제어·장시간 안정성을 검증하지는 않습니다. 과거 PTY 실패는 별도 조사 대상으로 유지하며 후속 빌드 성공으로 해결됐다고 보지 않습니다.
