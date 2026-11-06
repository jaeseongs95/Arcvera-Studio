# Desktop 네이티브 업데이트 사례

대상: `desktop_updates.rs`, `session_exit.rs`, 앱 설정 저장. 운영 앱·공유 daemon·실제 인증/설정·다른 세션 로그는 시험 대상에서 제외한다. 구현 전 사례를 고정하며 실제 실행 결과는 별도로 갱신한다.

## 불변조건과 설계 경계

- U1: 백그라운드 ON/OFF는 앱 메타데이터에 저장한다. 수동 조회는 설정과 무관하다.
- U2: 조회는 고정 GitHub stable release metadata만 읽는다. 사용자 확인 전 설치 파일 다운로드·설치는 0회다. renderer에는 상태·검증한 버전·고정 저장소 release URL·지원 여부·일반 오류만 전달한다.
- U3: Desktop 설치는 빌드에 명시한 신뢰 공개 키, 동일 릴리스의 서명 정보 manifest, 서명 및 버전 결속 검증이 모두 필요하다. 키를 만들거나 서명 생략으로 우회하지 않는다.
- U4 (`CONC-CLAIM-001`, `CONC-CANCEL-004`): 업데이트 설치 gate가 이긴 이후 새 세션 생성은 실패한다. 세션 생성이 먼저 이기면 업데이트가 실패하고 세션은 중단되지 않는다. 완료 phase·프로세스 없음·reaper 완료·정리 실패 없음이 모두 필요하다.
- U5 (`CONC-LOCK-003`): updater 직렬 요청 → native exclusive admission → extension admission → runtime 확인(해제) → registry → process/done 순서만 사용한다. 일반 native worker는 native read admission을 작업·정리 전체에 보유한다. 네트워크 중 registry/process/done lock을 잡지 않는다. 실패 시 gate가 소유한 exiting 상태만 복원한다.
- U6: Codex는 선택한 실행 파일의 제한된 `--version`만 조회하며 daemon 시작/전환/설치 명령을 만들지 않는다. release identity 또는 설치 계약 미확인 상태는 unknown/미구성으로 구별한다.
- U7: Windows installer 시작 성공 전 UI cleanup·ExitState 변경은 0회다. 서명 통과 뒤 `ShellExecuteW` 실패나 UAC 거부가 나도 오류와 admission 해제만 수행하며 창·트레이를 먼저 파괴하지 않는다. 재시도는 새로운 release 조회와 승인 버전 일치 검사를 거친다.

조회·서명/호스트 adapter는 신규 모듈에, 기존 세션 admission과 설정 접근은 Engine의 작은 메서드에 둔다. renderer는 파일·명령·엔드포인트를 지정하지 않는다. 별도 범용 provider 프레임워크는 만들지 않는다.

## 실행 전 사례

| ID | 요구 | 입력·사전조건 | 절차 | 기대 결과·합격 기준 | 실제 결과 |
| --- | --- | --- | --- | --- | --- |
| NU01 | U1 | 격리 임시 SQLite, 설정 없음 | 조회 → false 저장 → 닫기/재열기 → true 저장 | 기본 true, false/true 정확 지속 저장, 무효 저장값 오류 | NOT_RUN |
| NU02 | U2 | 메타데이터 fixture: stable 1.7.0, 설치 키 없음 | 현재 1.6.0 및 1.7.0과 각각 비교 | available/current 구별, 설치 미구성, 조회에서 artifact download 0 | NOT_RUN |
| NU03 | U2 | prerelease/draft/잘못된 tag/과대 또는 잘못된 JSON | 공개 parser 경계 호출 | 최신으로 오인하지 않고 오류, 외부 본문·URL 반사 없음 | NOT_RUN |
| NU04 | U3 | 확인 false, stale version, 신뢰키 없음 | 설치 준비 경계 호출 | 설치·artifact 다운로드 전에 오류 | NOT_RUN |
| NU05 | U3 | 고정 repo 외 URL, 서명 없음, version 불일치 manifest | manifest 검증 | 모두 거부, renderer가 URL/키를 지정할 수 없음 | NOT_RUN |
| NU06 | U4 | 완료된 격리 Engine | update gate 획득 → 세션 allocate → gate drop → allocate | lease 동안 allocate 실패, drop 후 성공 | NOT_RUN |
| NU07 | U4 | 격리 Engine의 신규 또는 cleanup_failed 또는 reaper 미완료 세션 | gate 획득 시도 | 실패, stop_requested false 보존, exiting 복원 | NOT_RUN |
| NU08 | U4/U5 | extension permit 또는 이미 exiting Engine | gate 획득 시도 | 작업/종료를 침범하지 않고 실패, 기존 exiting true 보존 | NOT_RUN |
| NU09 | U4 | 빈 격리 Engine, barrier 동시 allocate/gate 시도 | 결정적 상호 경쟁 | gate 승리 시 allocate 실패, allocate 승리 시 gate 실패, 세션 중단 0 | NOT_RUN |
| NU10 | U6 | official CLI/plain SemVer 및 R37/R38 문자열 | Codex 버전 비교 | OpenDIY identity 없는 CLI unknown, build metadata 차이 무시 금지, 설치 미지원 | NOT_RUN |
| NU11 | U2/U5 | 동시 조회/설치, 느린/과대 HTTP | 직렬 요청 및 제한 reader 경계 | 중복 요청 거부, 20초 deadline·메타데이터 256KiB 상한 | NOT_RUN |
| NU12 | U3/U4 | 서명된 fixture installer·명시 키·Windows 격리 GUI | 사용자 예 → 다운로드 → gate → plugin install | 실제 서명 실패 거부 및 성공 installer 실행/재시작 확인; 운영 설치 제외 | NOT_RUN (root 직렬 시험 필요) |
| NU13 | U4/U5 | 공통 native operation read permit 보유 | update exclusive permit 획득 시도 | 설치 gate 진입 전 실패, 기존 조회·설정·선택창 작업 중단 0 | NOT_RUN |
| NU14 | U4/U5 | update exclusive permit 보유 | 다른 thread의 native operation 시작 시도 | callback 실행 0, 명확 busy; exclusive drop 후 정상 진입 | NOT_RUN |
| NU15 | U4/U5 | runtime.preparing 또는 runtime.changing | Engine update gate 호출 | 설치 callback 실행 0, 기존 runtime 상태 보존, exiting false 복원 | NOT_RUN |
| NU16 | U6 | `codex-cli 0.166.0+native-r38.20261106` 및 stable/alpha Cargo base 허용형 | 순수 CLI 버전 parser 호출 | 양수 int32 R·8자리 숫자 날짜·전체 tag 보존; 날짜/alpha 순서 추정 없음 | NOT_RUN |
| NU17 | U6 | plain 공식 CLI·다른 custom suffix·R0·int32 초과·잘못된 base/date | parser 및 metadata-only 상태 함수 호출 | unknown/unsupported_runtime, 설치 미지원; 다른 CLI를 OpenDIY로 오인하지 않음 | NOT_RUN |
| NU18 | U6 | 로컬 R38/tag와 동일 feed R38/tag | 순수 comparator 및 상태 DTO 호출 | current, installSupported=false, 전체 tag 정확 일치 필요 | NOT_RUN |
| NU19 | U6 | 로컬 R38 대비 feed R39; base/date가 더 낮아 보이는 R39 포함 | comparator 및 상태 DTO 호출 | available candidate, installSupported=false; R 숫자 상승만 판정 | NOT_RUN |
| NU20 | U6 | 같은 R38이나 feed base·날짜·alpha가 다른 경우 | comparator 및 상태 DTO 호출 | unknown/release_identity_mismatch, 설치 미지원 | NOT_RUN |
| NU21 | U6 | 로컬 R38 대비 feed R37; base/date가 더 높아 보이는 R37 포함 | comparator 및 상태 DTO 호출 | unknown/downgrade_refused, 설치 미지원 | NOT_RUN |
| NU22 | U6 | 잘못된 feed tag·prefix 부재·다른 custom·정수 경계 | feed parser와 comparator 호출 | unknown/invalid_release_identity, 잘못된 identity를 current/available로 오인하지 않음 | NOT_RUN |
| NU23 | U7 | 서명 통과 뒤 ShellExecute 실패/UAC 거부가 가능한 실제 installer 경계 | 순수 fixture는 production native admission 내부의 설치 시작 Err만 주입하고 반환 뒤 native operation 재진입; 실제 UAC는 별도 사례 | 순수 범위: 오류 반환·native admission 해제. callback의 cleanup0은 소스 대조. 실제 서명·UAC·창·트레이 보존은 미검증 | NOT_RUN |
| NU24 | U7 | 첫 시도 승인 1.7.0/최신 1.7.0의 installer 실패; 재시도 직전 feed 1.8.0 | 동일 승인으로 재시도하여 새 release 조회와 승인 버전 검사 확인 | 새 조회 1회, stale 승인 거부, installer 호출 0회, 새 확인 필요 | NOT_RUN (현재는 production 제어 흐름 소스 대조만 수행) |

기본 runner: `cargo test --manifest-path src-tauri/Cargo.toml desktop_updates -- --test-threads=1` 및 `cargo test --manifest-path src-tauri/Cargo.toml update_gate -- --test-threads=1`. 로컬 Cargo/GUI는 root가 직렬 실행한다. 테스트 코드 작성과 실호스트 서명 installer 검증을 구분한다. 원본 없는 신규 기능이므로 무의미한 red/green은 만들지 않으며 회귀 민감도는 반례 fixture와 경쟁 사례로 확인한다.

공식 근거: [Tauri updater](https://v2.tauri.app/plugin/updater/), [updater 2.14.1 API](https://docs.rs/tauri-plugin-updater/2.14.1/tauri_plugin_updater/struct.Updater.html). `restore_update`는 원시 manifest를 다시 네트워크 조회하지 않고 adapter에서 제한 검증한 값으로 Update를 구성하며, 서명이 버전에 결속돼 있어야 한다.

## 구현 중 확인한 경계

- 공개 GitHub 최신 메타데이터(2026-11-06 조회): Desktop `v1.3.2`, Codex `v0.166.0-r37.20261104`, 둘 다 draft/prerelease=false. Codex의 정식 릴리스는 SemVer prerelease 필드에 패치 식별자를 넣으므로 GitHub stable 플래그와 전체 버전을 함께 보존한다.
- `updater 2.14.1`의 Windows `install()`은 `on_before_exit` 후 `ShellExecuteW`가 성공하면 직접 `std::process::exit(0)`한다. 정상 IPC 반환을 설치 완료 증거로 사용하지 않는다.
- session gate는 PTY 생성과 extension/weekly 작업의 admission을 막는다. lib.rs의 공통 native worker는 read permit, installer는 exclusive permit을 작업 전체에 보유해 catalog/external/runtime 진단·준비·설정·선택창의 늦은 진입을 막는다. Engine은 runtime.preparing/changing도 확인한다. 단일 프로세스 개수 snapshot으로 경쟁을 해결했다고 주장하지 않는다.
- CLI 담당이 확인하고 root가 대조한 계약에 따르면 `codex update --check`는 ZIP 다운로드·추출을 하며, `update`는 feed를 다시 조회하고 expectedTag/archiveSHA pin API가 없다. native는 이 두 명령을 호출하지 않는다. 계약 공백이 해소되기 전 Codex 앱 내 설치는 미구성이다.
- metadata-only 비교 근거: 공개 커밋 [`4a64004a26b393af5c0764bf97bdd2c309da7de0`의 `tools/windows-cli/update.ps1`](https://github.com/jaeseongs95/opendiy-codex/blob/4a64004a26b393af5c0764bf97bdd2c309da7de0/tools/windows-cli/update.ps1). CLI prefix `codex-cli ` 제거 후 native 버전의 tag는 `v` + version의 `+native-r`를 `-r`로 바꾼 전체 문자열이다. R 숫자만 순서에 사용하고 같은 R은 정확한 tag 일치가 필요하다. 날짜는 8자리 숫자 형식만 검사하며 실제 달력·최신성을 추정하지 않는다. alpha의 의미와 base 버전 순서도 추가하지 않는다. 기존 일반 SemVer 비교를 Codex 판정에 사용하지 않는다.
- `git diff --check`는 종료 0이다. Cargo/서명 installer/GUI 실행은 root 직렬 실행 전까지 NOT_RUN이다.
- 후속 metadata-only 변경은 `06f2af78e980ae13e1f72978fe4cab864a88ccb0`에서 두 파일만 수정했다. NU16~22용 Rust 순수 parser/comparator/상태 DTO 테스트 5개를 추가했다. Codex metadata intake는 일반 SemVer parser를 거치지 않으므로 `00000000` 같은 8자리 숫자 형식에 임의의 달력·SemVer 선행 0 제약을 추가하지 않는다. Cargo·GUI·CLI·다운로드/설치 실행은 이 후속 작업에서 NOT_RUN이며 컴파일·실행 성공을 주장하지 않는다.
- `94236e1` 고정 source review의 원 finding: 앱 callback이 먼저 `cleanup_before_exit`를 호출했지만 upstream installer는 그 뒤의 `ShellExecuteW` 실패에서 Err로 돌아올 수 있었다. ExitState=false 복원은 이미 파괴한 창·트레이를 복구하지 못한다. 원 finding과 기존 실패 기록은 보존한다.
- [`updater-v2.14.1/lib.rs`](https://github.com/tauri-apps/plugins-workspace/blob/updater-v2.14.1/plugins/updater/src/lib.rs#L111)에서 `UpdaterExt::updater_builder()`가 기본 cleanup callback을 주입하는 것도 직접 확인했다. 따라서 callback override 자체를 지우면 결함이 남는다. 기존 builder의 설정·실행 인수는 보존하고, UI cleanup과 ExitState 접근이 없는 빈 callback으로 기본 동작을 덮는다. [`updater.rs`](https://github.com/tauri-apps/plugins-workspace/blob/updater-v2.14.1/plugins/updater/src/updater.rs#L941)의 성공 후 direct exit은 유지한다. 성공 시 OS의 UI 자원 회수와 실제 설치 완료 여부를 별개로 취급한다.
- NU23 보강은 기존 `desktop_updates_install_operation_excludes_inflight_and_late_native_workers`에서 production `with_install_operation` 안의 Err→반환 후 read admission 재진입을 검사한다. callback 단독 호출이나 소스 문자열 assertion은 추가하지 않았다. 이 시험은 원 조기 cleanup 결함을 재현하거나 실제 UI 보존을 증명하지 않는다. 원래 Engine gate의 실패 후 복원 시험도 보존한다. NU24는 `install_desktop_update`가 매번 새 client/deadline→release 조회→승인 버전 비교→manifest/다운로드/설치 순서를 따르는 소스 근거이며 실제 네트워크 재시도 시험은 NOT_RUN이다. 이번 수정의 Cargo·GUI·UAC·서명 installer 실행도 NOT_RUN이다.
