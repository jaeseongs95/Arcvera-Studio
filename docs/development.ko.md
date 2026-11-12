# 개발 및 검증

[English](development.md) · [처음으로](../README.ko.md) · [사용 가이드](user-guide.ko.md)

저장소 변경 전에 [AGENTS.md](../AGENTS.md)를 읽으세요.
Windows에서 Node.js 24, Rust stable, MSVC 빌드 도구와 WebView2가 필요합니다.
Microsoft 런타임은 사용자가 공식 경로에서 직접 설치합니다.

```powershell
npm ci
npm test
npm run build
node scripts/notices.mjs
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri -- dev
```

NSIS 패키지를 빌드하려면 다음 명령을 사용합니다.

```powershell
npm run tauri -- build --config src-tauri/tauri.bundle.conf.json --bundles nsis
```

Desktop 2.0.0은 작업 프로세스를 Windows Job에서 직접 실행합니다. 별도 Desktop
도우미 실행 파일은 없습니다. Core와 셸은 별도 프로세스입니다. 수정한 portable-pty는
`src-tauri/vendor/portable-pty`에 있습니다. 네이티브 컴파일 전에 타사 고지를 생성하세요.

로컬 소스 사본·의존성·중간 산출물은 `D:/codex/build/<project-task>/`에 둡니다.
디스크 여유와 기존 사본 재사용 가능성을 먼저 확인하고, 소스·필수 근거를 보관한 뒤
본인이 만든 불필요한 임시 산출물만 삭제합니다. 한글 경로로 도구가 실패하면 해당
build 경로의 별도 체크아웃을 사용합니다.

## 2.0.0 CI 및 릴리스

[Windows 워크플로](../.github/workflows/windows.yml)는 소스 fixture 2개 shard와
번들 브라우저 시험, 네이티브·portable-pty·updater 시험, 소스·라이선스 검사,
NSIS 필수 구성 요소·바로가기 검사와 패키지의 PTY 자체 시험을 수행합니다.
공통 모델·패키징 단위 검사는 한 번만 실행하고, 브라우저 작업에서는 번들 작업만
프런트엔드를 빌드합니다. 브라우저 실패 추적 자료를 보존합니다.
과거 1.8.0 관측, 전체 자원 스냅샷과 디버그 전용 도우미 빌드·업로드는 제외합니다.
별도 PTY 변이 민감도 워크플로는 수동으로만 실행합니다.

main·develop push와 PR은 일반 검증·패키징을 실행합니다. 작업 브랜치 push는 PR
실행을 중복하지 않습니다. 업데이트 서명 산출물을 준비하려면 release 브랜치의 `windows.yml`을
`updater_package=true`로 수동 실행합니다. 두 검증 작업이 모두 통과해야 패키징합니다.
기존 저장소의 서명 설정을 보존하며, CI는 릴리스를 게시하지 않습니다.

릴리스에는 Windows x64 portable ZIP, NSIS 설치기와 SHA-256 목록을 포함합니다.
실행 파일 자체는 코드 서명되지 않았으며 업데이트 산출물의 서명과 구분합니다.
실제 결과·남은 제한은 정확한 커밋과 실행 파일에 결속해 릴리스 노트에 기록합니다.

## 시험 범위

```powershell
arcvera-studio.exe --self-test --duration-seconds 60 --report D:/codex/build/pty-result.json
arcvera-studio.exe --codex-smoke --sessions 2 --cwd D:/projects/already-trusted --report D:/codex/build/core-result.json
```

첫 명령은 로컬 셸 fixture로 실제 PTY 격리·출력 상한·취소·반복 생성과 종료를 확인합니다.
장시간 PTY 부하는 `--duration-seconds 14400`으로 실행합니다. 두 번째 명령은 실제 Core
서버와 TUI를 시작하지만 모델 요청이나 신뢰·승인 화면 응답은 수행하지 않습니다.
`--cwd`에는 기존의 신뢰된 경로를 지정하고 결과는 그 프로젝트 밖에 저장합니다.
인증·설정·외부 대화·진행 작업을 보존하세요. GUI·실제 IME 검증을 대신하지 않습니다.

시험 전에 입력·전제·절차·기대 결과·판정 조건을 정하고 정확한 후보의 결과를
기록하세요. fixture, 코드 검토, CI, 네이티브 진단과 실제 GUI 근거를 구분합니다.
다른 버전의 통과나 빌드 성공만으로 2.0.0의 장시간 안정성·전체 호환성을 판단하지 않습니다.
과거 결과는 [검증 기록](validation.md)의 명시된 버전에만 적용합니다.

## 기여 및 라이선스

관련 범위의 브랜치에서 작업하고 무관한 변경을 보존합니다. 공급자 프로토콜은 Codex
adapter 안에 두고 실행 환경이 보고한 모델·기능을 사용합니다. UI 상한으로 모델 문맥을
잘라내지 않습니다. Claude의 소스·자산·문구를 복사하지 않습니다.
2.0.0부터 [AGPL-3.0-or-later](../LICENSE)를 적용합니다. [기존 MIT 고지](../LICENSE-MIT-LEGACY),
타사 고지와 동일 버전의 무료 [대응 소스](../SOURCE.md)를 보존합니다.
