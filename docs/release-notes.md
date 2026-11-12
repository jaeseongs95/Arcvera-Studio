# Release notes

## 2.0.0 — Unreleased

### English

Desktop now requires the stable Arcvera Core 1.x contract. Official Codex, earlier Core
versions and per-session server fallback are no longer supported. Every shared
connection includes the exact validated server version, including build metadata,
in its initialization capabilities. Core rejects a mismatched contract.

Existing settings, authentication and conversations are retained. Select the
Core CLI and matching server in Executable settings. An unsupported saved path
is reported without clearing it or starting a replacement server. Install Core
separately before migrating; see the [user guide](user-guide.md#runtime).

### 한국어

Desktop은 Arcvera Core 1.x 정식 버전의 계약을 요구합니다. 공식 Codex, 이전 Core 버전과
개별 서버 대체 실행은 지원하지 않습니다. 모든 공유 연결은 빌드 메타데이터를
포함한 검증된 서버의 전체 버전을 초기화 기능 정보에 전달합니다. Core는 계약이
일치하지 않는 연결을 거부합니다.

기존 설정·인증·대화는 유지됩니다. 실행 파일 설정에서 Core CLI와 같은 패키지의
서버를 선택하세요. 저장된 경로가 미지원이면 경로를 지우거나 대체 서버를 실행하지
않고 오류를 표시합니다. 전환 전에 Core를 별도로 설치하세요.
[사용 가이드](user-guide.ko.md#runtime)를 참고하세요.

## 1.8.0 — 2026-11-11

### English

Arcvera-Studio is the new product name. Application titles, package names, the
`arcvera-studio.exe` executable, Windows artifacts and current repository links
use the new name. Existing application data and installation records retain their
stable identifiers. See [compatibility and recovery](rebranding.md).

The subagent manager adds search and status filters, an expanded detail view and
a confirmed deletion flow for eligible completed subagents. Deletion uses the
selected runtime's supported cleanup contract and preserves uncertain outcomes.

Shortcut creation preserves existing destinations, including unrelated or
unreadable links, during renamed upgrades and reinstalls. Isolated Windows
checks execute the production NSIS shortcut functions against 80 cases; removing
the three creation guards makes the same checks detect overwritten links.

Windows CI covers native regressions, source and bundled browser suites and the
packaged headless PTY suite. Browser fixtures substitute the native bridge;
transport peers substitute actual CLI cleanup. Full installation, native GUI,
live cleanup and long-duration operating effects remain unverified. Historical
browser and PTY failures retain unconfirmed causes. Publication and downloads
are shown on [GitHub Releases](https://github.com/jaeseongs95/Arcvera-Studio/releases).

### 한국어

제품 이름을 Arcvera-Studio로 변경했습니다. 앱 제목, 패키지 이름,
`arcvera-studio.exe` 실행 파일, Windows 산출물과 현재 저장소 링크에 새 이름을
사용합니다. 기존 앱 데이터와 설치 기록의 영속 식별자는 유지합니다.
[호환 및 복구 안내](rebranding.ko.md)를 참고하세요.

서브에이전트 관리에 검색·상태 필터, 상세 보기와 삭제 가능한 완료 항목의 확인
절차를 추가했습니다. 삭제에는 선택한 실행 환경이 지원하는 정리 계약을 사용하며,
결과가 불확실한 경우에는 확정된 결과로 표시하지 않습니다.

이름 변경 업그레이드와 재설치 시 기존 바로가기를 보존합니다. 다른 프로그램을
가리키거나 읽을 수 없는 바로가기도 덮어쓰지 않습니다. 격리된 Windows에서 실제
NSIS 바로가기 함수를 80개 사례로 확인하며, 세 생성 경로의 보호 코드를 제거한
대조 실행에서는 같은 시험이 덮어쓰기를 검출합니다.

Windows CI는 네이티브 회귀, 소스·번들 브라우저 시험과 패키지의 화면 없는 PTY
시험을 수행합니다. 브라우저 시험은 네이티브 연결을, 전송 시험은 실제 CLI 삭제를
대체합니다. 전체 설치·네이티브 GUI·실제 작업 삭제·장시간 운영 효과는 미검증입니다.
과거 브라우저·PTY 실패의 원인은 미확정 상태로 유지합니다. 게시 상태와 다운로드는
[GitHub Releases](https://github.com/jaeseongs95/Arcvera-Studio/releases)에서 확인할 수 있습니다.

## Entry format / 작성 형식

Each release uses `## <SemVer> — <YYYY-MM-DD or Unreleased>`, followed by
`### English` and `### 한국어` in the same entry. Describe user-visible changes,
compatibility or recovery steps, verification and material limitations. Mark a
candidate Unreleased until release preparation is authorized; a dated entry
describes that release, while GitHub Releases records actual publication.

각 릴리스는 `## <SemVer> — <YYYY-MM-DD 또는 Unreleased>` 아래에 `### English`와
`### 한국어`를 함께 작성합니다. 사용자에게 보이는 변경, 호환·복구 절차, 검증과
중요한 제한을 기록합니다. 릴리스 준비 승인 전에는 Unreleased로 표시하며, 날짜가 있는
항목은 해당 릴리스를 설명합니다. 실제 게시 상태는 GitHub Releases에 기록합니다.
