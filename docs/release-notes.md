# Release notes

## 2.0.1 — 2026-11-13

### English

Recognize the exact `arcvera ` prefix reported by the Arcvera Core CLI. Desktop
2.0.0 could reject Core 1.0.1 during shared-server preparation and classify its
version as unsupported during update checks. Both consumers now accept the
published CLI format while retaining strict version syntax, stable Core 1.x
requirements and full build-metadata identity checks.

First-time preparation also recognizes the current `arcvera.exe` managed package
path. Existing package metadata, inaccessible state and broken links continue to
select the preserving startup mode; installation is rechecked under Core's lock.

After updating Desktop, select the Core package's `bin/arcvera.exe` under
Executable settings and retry server preparation. Explicit server replacement
continues to require a matching CLI/server package and the existing confirmation
and activity checks. Existing settings, authentication and conversations are
retained.

Windows verification covers strict parsing and preservation regressions, plus
isolated preparation, two-session binding, reuse and normal shutdown with the
released Core 1.0.1 package. Broader control checks in fresh profiles stop at
sign-in or folder-trust onboarding; these prompts were not answered. Real-account
sign-in, model turns, native GUI interaction and long-duration use remain
unverified by this hotfix's isolated checks.

### 한국어

Arcvera Core CLI가 출력하는 정확한 `arcvera ` 접두사를 인식합니다. Desktop
2.0.0은 Core 1.0.1의 공유 서버 준비를 거부하거나 업데이트 검사에서 지원하지 않는
버전으로 판정할 수 있었습니다. 두 소비 경로가 공개 CLI의 출력 형식을 수용하도록
수정했으며, 엄격한 버전 문법·Core 1.x 정식 버전 요구·빌드 메타데이터를 포함한 전체
식별값 검사는 유지합니다.

최초 준비에서도 현재 관리 패키지의 `arcvera.exe` 경로를 인식합니다. 기존 패키지
메타데이터·접근 불가 상태·끊어진 링크가 있으면 보존 시작 방식을 유지하며,
설치 여부는 Core의 잠금 안에서 다시 검사합니다.

Desktop 업데이트 후 실행 파일 설정에서 Core 패키지의 `bin/arcvera.exe`를 선택하고
서버 준비를 다시 시도하세요. 명시적으로 서버를 교체할 때는 기존과 같이 일치하는
CLI·서버 패키지와 확인 절차·진행 작업 검사가 필요합니다. 기존 설정·인증·대화는
유지됩니다.

Windows 검증에는 엄격한 버전 해석과 기존 상태 보존 회귀, 공개 Core 1.0.1 패키지의
격리된 준비·두 세션 바인딩·재사용·정상 종료가 포함됩니다. 새 프로필의 확장 제어
검사는 로그인 또는 폴더 신뢰 안내에서 대기했으며 해당 질문에 답하지 않았습니다.
실제 계정 로그인·모델 요청·네이티브 GUI 조작·장시간 사용은 이번 격리 검사에서
검증하지 않았습니다.

## 2.0.0 — 2026-11-12

### English

Microsoft runtime DLLs and installers are no longer bundled, downloaded or installed
by Desktop. Missing or outdated WebView2 stops the installer before installation
changes, including during updates. Obtain prerequisites directly from Microsoft;
see [Windows prerequisites](../src-tauri/installer/RUNTIME.md). Core's separate
package documents the Visual C++ runtime needed by its voice helper. Existing
open-source dependencies and notices are retained.

Version 2.0.0 and every subsequent version use AGPL-3.0-or-later. The earlier
MIT notice and third-party licenses are preserved. Settings includes the
license text and a link to the matching release's free Corresponding Source.
See [source and redistribution instructions](../SOURCE.md). The validation
counts below identify their tested candidate and scope; local source checks do
not establish validation of the final rebuilt package.

Desktop now requires the stable Arcvera Core 1.x contract. Official Codex, earlier Core
versions and per-session server fallback are no longer supported. Every shared
connection includes the exact validated server version, including build metadata,
in its initialization capabilities. Core rejects a mismatched contract.

Abandoned and failed proxy connections now use the same bounded cleanup as
explicit completion. Cleanup terminates the proxy's own process Job before
waiting for its reader, preventing descendants that inherit stdout from keeping
the reader blocked. The shared Core server remains outside that Job.

The updated source passed 485 native tests on the local Windows PC, with seven
existing opt-in tests ignored. Two unchanged regression cases fail on the prior
cleanup and pass on the fix, checking inherited stdout, real reader completion,
owned Job cleanup and original process handles. A delayed WebSocket handshake
case also passed after separating fixture setup from RPC polling. The earlier
full run's three fixture handshake failures remain recorded.

Existing settings, authentication and conversations are retained. Select the
Core CLI and server in Executable settings. Each must be a stable 1.x version;
their versions may differ. Explicit server replacement requires a matching package.
An unsupported saved path
is reported without clearing it or starting a replacement server. Install Core
separately before migrating; see the [user guide](user-guide.md#runtime).

Validation on the local Windows PC includes four failing/passing regression pairs
for initialization binding, rejection and deadlines, 482 native unit checks
(seven existing opt-in checks remain ignored), and real PTY isolation and cleanup.
The 60-second PTY workload passed 236 checks; the short entrypoint passed seven.
These use isolated peers and local shell fixtures. Windows CI also passed the
native integration harness and 591 browser fixture checks, with one existing
opt-in browser check skipped.

Additional local browser coverage uses a mocked native bridge and engine. Eight
cases passed in the original run; the two timed-out cases passed after disabling
the test server's file watcher, with the same 94 frontend, test and configuration
inputs. This is combined evidence from two runs, not one ten-case passing run.

Native GUI, physical IME, authenticated model turns, physical phone control and
long-duration T62 workloads remain unverified. The earlier CI
`ERR_NO_BUFFER_SPACE` failures retain an unconfirmed cause; a subsequent clean
run does not establish a fix. Version 2.0 CI retains the complete source and
bundled browser suites and failure traces, runs shared unit checks once, and
removes historical official-CLI/public-1.8.0 observation workflows, resource
snapshots and debug-only executable uploads. PTY mutation sensitivity remains
available manually. The clean-checkout NSIS fixture creates its output directory.

### 한국어

Desktop은 Microsoft 런타임 DLL·설치기를 동봉하거나 자동으로 다운로드·설치하지
않습니다. WebView2가 없거나 오래된 경우 업데이트를 포함해 설치 변경 전에
중단합니다. 필요한 구성 요소는 Microsoft에서 직접 설치하세요.
[Windows 필수 구성 요소](../src-tauri/installer/RUNTIME.ko.md)를 참고하세요. Core 음성
도우미에 필요한 Visual C++ 런타임은 별도 Core 패키지의 안내를 따릅니다.
기존 오픈소스 의존성과 고지는 유지합니다.

2.0.0과 이후 모든 버전에는 AGPL-3.0-or-later를 적용합니다. 기존 MIT 고지와
타사 라이선스는 보존합니다. 설정에서 라이선스 전문을 읽고 동일 버전의 무료
대응 소스 릴리스를 열 수 있습니다. [소스·재배포 안내](../SOURCE.md)를 참고하세요.
아래 검증 수치는 각 시험 대상과 범위를 구분한 결과이며, 로컬 소스 검사가 최종
재빌드 패키지의 검증을 뜻하지는 않습니다.

Desktop은 Arcvera Core 1.x 정식 버전의 계약을 요구합니다. 공식 Codex, 이전 Core 버전과
개별 서버 대체 실행은 지원하지 않습니다. 모든 공유 연결은 빌드 메타데이터를
포함한 검증된 서버의 전체 버전을 초기화 기능 정보에 전달합니다. Core는 계약이
일치하지 않는 연결을 거부합니다.

중도 반환하거나 오류가 발생한 proxy 연결도 명시적 종료와 동일한 제한 시간 내
정리 절차를 사용합니다. reader를 기다리기 전에 해당 proxy가 소유한 프로세스
Job을 종료하여, stdout을 상속한 자손 프로세스로 인해 reader가 계속 대기하는
경로를 수정합니다. 공유 Core 서버는 해당 Job에 포함되지 않습니다.

수정된 소스는 이 PC의 Windows 환경에서 네이티브 시험 485개를 통과했으며,
기존 opt-in 시험 7개는 제외 상태를 유지합니다. 동일한 회귀 시험 두 개가 이전
정리 코드에서 실패하고 수정 코드에서 통과하여 stdout 상속, 실제 reader 종료,
소유 Job 정리와 원래 프로세스 핸들을 확인했습니다. 시험 준비 단계와 RPC 폴링을
분리한 뒤 지연된 WebSocket 핸드셰이크 시험도 통과했습니다. 이전 전체 실행에서
발생한 fixture 핸드셰이크 실패 세 건은 기록에 보존합니다.

기존 설정·인증·대화는 유지됩니다. 실행 파일 설정에서 Core CLI와 서버를 선택하세요.
각각 1.x 정식 버전이면 버전이 달라도 사용할 수 있습니다. 명시적으로 서버를 교체할 때는
같은 패키지가 필요합니다. 저장된 경로가 미지원이면 경로를 지우거나 대체 서버를 실행하지
않고 오류를 표시합니다. 전환 전에 Core를 별도로 설치하세요.
[사용 가이드](user-guide.ko.md#runtime)를 참고하세요.

이 PC의 Windows 환경에서 초기화 버전 바인딩·거절·기한에 대한 회귀 시험 네 쌍의
수정 전 실패와 수정 후 통과, 네이티브 단위 검사 482개 통과를 확인했습니다.
기존 opt-in 검사 7개는 제외 상태를 유지합니다. 실제 PTY의 격리·종료를 확인했으며,
60초 부하는 236개, 짧은 진입점은 7개 검사를 통과했습니다. 이 시험에는 격리된
통신 상대와 로컬 셸 fixture를 사용합니다. Windows CI에서도 네이티브 통합 harness와
브라우저 fixture 검사 591개가 통과했고, 기존 opt-in 브라우저 검사 1개는 제외했습니다.

추가 로컬 브라우저 시험은 네이티브 연결과 엔진을 모의합니다. 최초 실행에서
8개가 통과했고, 시간 초과한 2개는 시험 서버의 파일 감시기를 끈 뒤 통과했습니다.
프런트엔드·시험·설정 입력 94개는 동일합니다. 이는 두 실행을 결합한 근거이며,
10개가 한 실행에서 모두 통과한 결과는 아닙니다.

네이티브 GUI, 실제 IME, 인증된 모델 요청, 실제 휴대전화 제어와 장시간 T62 부하는
미검증입니다. 과거 CI의 `ERR_NO_BUFFER_SPACE` 실패 원인은 미확정이며, 후속 실행의
통과를 원인 해결로 판단하지 않습니다. 2.0 CI는 전체 소스·번들 브라우저 시험과
실패 추적 자료를 유지하고 공통 단위 검사를 한 번만 실행합니다. 과거 공식 CLI·공개
1.8.0 관측 워크플로, 자원 스냅샷과 디버그 전용 실행 파일 업로드는 제거합니다.
PTY 변이 민감도 시험은 수동으로 실행할 수 있습니다. 깨끗한 체크아웃에서도 NSIS
시험이 출력 디렉터리를 생성합니다.

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
