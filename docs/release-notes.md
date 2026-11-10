# Release notes

## 1.8.0 — Unreleased

### English

Arcvera-Studio is the new product name. Application titles, package names, the
`arcvera-studio.exe` executable, Windows artifacts and current repository links
use the new name. Existing application data and installation records retain their
stable identifiers. See [compatibility and recovery](rebranding.md).

The subagent manager adds search and status filters, an expanded detail view and
a confirmed deletion flow for eligible completed subagents. Deletion uses the
selected runtime's supported cleanup contract and preserves uncertain outcomes.

Validation is in progress. Browser fixtures cover the interface and language
cache compatibility; they do not demonstrate native installation or live cleanup.
This candidate has not been installed, deployed, tagged or released. Historical
PTY failures remain under investigation and are not resolved by later passing runs.

### 한국어

제품 이름을 Arcvera-Studio로 변경했습니다. 앱 제목, 패키지 이름,
`arcvera-studio.exe` 실행 파일, Windows 산출물과 현재 저장소 링크에 새 이름을
사용합니다. 기존 앱 데이터와 설치 기록의 영속 식별자는 유지합니다.
[호환 및 복구 안내](rebranding.ko.md)를 참고하세요.

서브에이전트 관리에 검색·상태 필터, 상세 보기와 삭제 가능한 완료 항목의 확인
절차를 추가했습니다. 삭제에는 선택한 실행 환경이 지원하는 정리 계약을 사용하며,
결과가 불확실한 경우에는 확정된 결과로 표시하지 않습니다.

검증은 진행 중입니다. 브라우저 시험 화면으로 UI와 언어 캐시 호환을 확인했으며,
이 결과는 네이티브 설치나 실제 작업의 삭제 검증을 대체하지 않습니다. 이 후보는
설치·배포·태그·릴리스를 하지 않았습니다. 과거 PTY 실패는 원인 조사 중이며,
이후 시험의 성공으로 해결된 것으로 간주하지 않습니다.

## Entry format / 작성 형식

Each release uses `## <SemVer> — <YYYY-MM-DD or Unreleased>`, followed by
`### English` and `### 한국어` in the same entry. Describe user-visible changes,
compatibility or recovery steps, verification and material limitations. Mark a
candidate Unreleased until publication is authorized and completed.

각 릴리스는 `## <SemVer> — <YYYY-MM-DD 또는 Unreleased>` 아래에 `### English`와
`### 한국어`를 함께 작성합니다. 사용자에게 보이는 변경, 호환·복구 절차, 검증과
중요한 제한을 기록합니다. 게시 승인과 실제 게시가 끝나기 전에는 Unreleased로 표시합니다.
