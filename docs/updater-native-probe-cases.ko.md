# Updater native probe — 실행 전 사례

## 추가 실제 loopback 다운로드 사례 — 2026-11-07, 구현·실행 전

기존 PN01~05의 helper source4784와 CI42337313905/local03 실제5PASS/exit0 결과를 보존한다. 아래 두 사례는 새 helper 후보의 별도 경계다. 기존 offline 사례의 network 금지를 해제하지 않는다. 제품4f·원 latest/installer/.sig·공개키·Cargo.lock은 바꾸지 않는다. helper-only CI에서 기존 cache로 컴파일만 한 뒤 이 PC에서 실행한다.

| ID | 입력·절차 | 기대 및 실패 기준 |
|---|---|---|
| PN06 | 같은 고정 서명 NSIS를 자체 Node HTTP 서버의 `127.0.0.1:<새 포트>/<own nonce>/valid.exe`로 제공한다. MockRuntime에 별도 updater builder를 만들고, proxy를 끄고 5초 timeout을 둔다. 원 metadata/signature를 restore한 Update의 download_url만 자체 loopback으로 바꾸고 실제 `Update.download`를 호출한다. | 정확한 원 전체 바이트/SHA, 실제 chunk 총합·Content-Length 및 finish callback, valid GET 정확1회. 원 파일 불변, beforeExit0/install0/restart0. byte 검사를 mocked 반환으로 대체하지 않는다. |
| PN07 | PN06과 같지만 서버가 메모리의 installer byte0만 XOR1한 전체 길이의 payload를 제공한다. 원 signature와 version은 그대로 둔다. 실제 `Update.download`를 호출한다. | 전체 전송·finish callback 뒤 `Error::Minisign`이어야 한다. helper는 바이트를 설치하지 않고 beforeExit0/install0/restart0, tampered GET 정확1회. IO/timeout/HTTP 오류를 서명 거부로 판정하지 않는다. |

서버는 새 loopback 포트와 nonce의 두 경로만 허용한다. 인증 헤더가 있으면 거부하고 헤더 원문을 기록하지 않는다. 최종 helper exit0·7case의 실제 결과·서버 요청2개·오류0 및 소켓/리스너/프로세스 종료를 확인한다. 실패·timeout은 원자료를 남기며 자동 재실행하지 않는다. 원 PN01~05의 실제 verify 경계도 그대로 실행해 offline client panic 방어가 보존됐는지 확인한다.

updater-v2.14.1 `Update.download`는 body 수신→finish callback→verify 순서다. finish callback은 서명 통과나 설치 진입 증거가 아니다. HTTP loopback은 전송 fixture이며 TLS·실제 GitHub 공개 다운로드·사용자 제품의 정상 offer→download→install→자동 재시작을 대체하지 않는다. metadata check·외부 endpoint·GUI·설치·재시작·privatekey 접근0을 유지한다. HTTP를 허용하려고 제품 보안 설정이나 전역 proxy/인증서를 바꾸지 않는다. 실제 케이스 결과는 새 실행 뒤 별도 artifact로 기록한다.

아래 원 준비 기록의 network0·NOT_RUN은 당시 PN01~05/준비 단계의 범위와 시각을 나타낸다. 추가된 두 download 사례에 그대로 적용하거나 과거 원결과를 덮어쓰지 않는다.

대상은 동결 제품 source `4f2a33cf8adcc731048b06804c56aca805e29584`, updater crate `2.14.1`, 최종 CI `42331873361`의 artifact `13558466303`이다. 이것은 helper-only 시험 후보이며 제품·릴리스 후보가 아니다. 제품 src/버전/기존 windows.yml/Cargo.lock/최종 asset 변경0, signing secret/private key 사용0, GUI/설치/재시작0이다.

입력 ZIP SHA256 `59887ad85ed2f3c93a6dce0758d6fca85768ef6b4d9dd330d1c04590c8335525`를 먼저 확인한다. ZIP 내부 실제 updater `latest.json`과 installer/.sig 원본을 읽기만 한다. manifest/endpoint/comparator를 가짜 offer로 바꾸지 않는다. `restore_update`는 저장된 실제 metadata를 native plugin에 복원하는 공개 API다. `check/download/install`은 호출하지 않는다.

공개키는 repository variable `OPENDIY_UPDATER_PUBLIC_KEY`를 환경으로 전달한다. source-default empty pubkey는 final trust key가 아니다. 원 UTF-8 bytes SHA256은 `a8c985f62b8182b7fc54d892fb36a27140c3f27e590ae5e891d42ff494b01803`과 직접 비교하며 trim하지 않는다. 확인된 공개 원파일은152bytes/BOM0/앞뒤공백0/끝개행0이다. private key나 signing secret은 읽지 않는다.

| ID / 요구 | 입력·전제 | 절차 / 실제 경계 | 기대·pass/fail 기준 | 실제 |
|---|---|---|---|---|
| PN01 / 실제 plugin 정상 검증 | 위 ZIP·공개키·실제 latest/installer/.sig pins | mock_builder+MockRuntime로 plugin 등록, 실제 원metadata restore_update, Update::verify 원installer | Ok, on_before_exit counter0, 파일 byte불변. 입력 pin 실패는 검증통과가 아님 | NOT_RUN |
| PN02 / installer변조 거부 | PN01 같은 원본 및 in-memory byte0 XOR1 | 실제 Update::verify 호출 | Error::Minisign, callback0. 다른 parse/IO 실패를 서명거부 PASS로 쓰지 않음 | NOT_RUN |
| PN03 / signature변조 거부 | 실제 sig의 innerpacket byte10 XOR1, 올바른 base64로 재인코딩 | clone Update의 signature만 메모리변조, 실제 verify 원installer | Error::Minisign, callback0. 원artifact/.sig 변경0 | NOT_RUN |
| PN04 / trustedcomment변조 거부 | 실제 sig의 trustedcomment 뒤 X를 메모리추가 | clone Update signature 재인코딩 뒤 실제 verify | Error::Minisign, callback0. 원byte불변 | NOT_RUN |
| PN05 / targetversion semantic 거부 | 원payload/signature 그대로, 원manifest의 메모리 variant version만1.6.2 | 실제 restore_update 뒤 Update::verify 원installer | Error::SignedVersionMismatch {signed:1.6.1,announced:1.6.2}, callback0. 원latest.json 불변, fake offer/network0 | NOT_RUN |

Mock 범위는 Tauri 앱/runtime과 no-op on_before_exit counter뿐이다. signature parser/minisign/global signature/signed-version 경계는 실제 upstream plugin이다. `configure_client`는 panic하도록 하여 예기치 않은 network를 거부한다. 기본 mock version0.1.0에서 실제1.6.1 metadata를 복원하며 custom comparator는 없다. restore_update가 require_signed_version을 강제한다.

CI는 기존 Windows Rust cache의 shared-key `windows`를 재사용하고 integration helper만 선택한다. 최종 installer 재빌드/NSIS/package/npm build는 하지 않는다. CI는 --no-run으로 helper만 컴파일하고 실제case실행0을 기록한다. native helper exe·최종fixture·SHA·공개키만 upload하여 root가 이PC에서5tests/exit0을 확인한다. 비밀키는 포함하지 않는다. `--locked`로 Cargo.lock 변경을 거부한다. 로컬 준비 단계에서는 Rust compile/실제시험을 하지 않는다.

ShellExecute/UAC/설치·재시작은 NOT_RUN이다. upstream은 extract→on_before_exit→ShellExecuteW→실패Err/성공process-exit 순서이므로 hook counter0을 ShellExecute 실패로 확대할 수 없다. ownTEMP 단일파일 execute-deny 방식은 별도 권한실험이 필요해 이 helper에 넣지 않았다. 기존 native18 injected installer error는 admission/permit 복원 근거이며 actual plugin ShellExecute 실패 근거가 아니다. UI mock7도 실제 설치/재시작을 증명하지 않는다. 공개 stable/latest에서 실제offer가 가능해진 뒤 별도 승인된 실제설치/재시작으로 종결해야 한다.

로컬 재실행(다운로드된 native helper exe, 기존 final updater artifact 디렉터리, 공개키만 사용):
```powershell
$env:OD_UPDATER_ARTIFACT_DIR='<downloaded-helper-artifact>/fixture'
$env:OD_UPDATER_PUBLIC_KEY=[IO.File]::ReadAllText('<downloaded-helper-artifact>/public-key.pub')
& '<downloaded-helper-artifact>/native/updater_native_probe.exe' --nocapture --test-threads=1
```

참조: upstream updater-v2.14.1 `src/updater.rs`의 restore_update/verify 및 offline_tests, Tauri2.12.5 test::mock_builder/mock_context/noop_assets. 테스트 민감도 red/green과 actual plugin 동작 결과는 실제 CI/로컬실행 전까지 NOT_RUN이다.

실행 위치 정정: 사용자 로컬시험 지시에 따라 GitHub job의 실제case 실행은0이다. CI는 기존cache로 --no-run compile 및 입력 묶음 준비만 한다. root의 정확 test/desktop161-updater-native branch push만 최초 CI를 시작하며 optional workflow_dispatch는 초기등록 전 의존하지 않는다. root만 commit/push/dispatch하며 준비자가 수행하지 않는다. CI artifact는 helper전용이며 release asset이 아니다.

준비 단계 실제 정적결과: git 변경 경로는 승인된4개만 확인했고 product tracked source/version/Cargo.lock/windows.yml 변경0이다. Python tomllib로 Cargo.toml을 파싱하여 dev-dependencies tauri(test) 외 구조변경0을 대조했다. Rust test함수5개·실제 .verify callsite5개, install/download/check/run/파일쓰기 호출0을 소스 검사했다. CI test실행0·정확branch trigger·공개키원bytes pin·artifact ZIP pin을 대조했다. workflow의 embedded PowerShell run4개 AST parse error0, git diff --check exit0이다. 이환경 PATH에서 rustfmt/cargo가 확인되지 않아 Rust구문/타입컴파일은 NOT_RUN이며 GitHub compile-first 단계가 확인해야 한다. YAML은 공개 upstream js-yaml4.1.0 bundle을 메모리에서만 로드하여 파싱했고 branch/permissions/run4개/--no-run/cache구조 대조 exit0이다(설치·새파일0, parser SHA256 c738b0020702aa631abd8de23a902a3079d99d3060e44166955cb51f570881c4). 실제5case·GUI·설치·재시작은 모두 NOT_RUN이다.

서명 변형의 base64 처리에는 이미 활성화된 windows-sys/Win32_Security_Cryptography의 메모리 codec 두 함수만 재사용한다. 새 direct base64 의존성은 추가하지 않는다. Tauri upstream의 test feature는 빈 feature이므로 새 dependency package를 추가하지 않는다. Windows cryptographic key/provider/file API는 호출하지 않으며 최종서명 검증은 이 codec과 별개로 actual updater plugin이 수행한다.

PN00 / MockRuntime 초기화 전제(실행 전): Tauri2.12.5 mock_context는 plugins를 Default 빈 HashMap으로 만든다. upstream updater offline_tests는 mock_app 뒤 private UpdaterBuilder::new(app.handle(), Config{pubkey,...})를 직접 호출하여 plugin등록/PluginApi config역직렬화 경계를 우회한다. 외부 helper는 공개 UpdaterExt를 쓰므로 그 방식을 그대로 가정할 수 없다. Tauri plugin::initialize는 Config를 serde_json::from_value로 역직렬화하고 updater Config는 pubkey:String을 요구한다. 따라서 helper의 Context::config_mut().plugins.0에 {updater:{pubkey:""}}를 넣고 build(context)한다. 기대: plugin등록은 유효한object를 받고 초기화되며, 실제trustkey는 이후 기존exactSHA 고정 .pubkey(public)로만 주입된다. source-default empty키를 trustkey로 사용하지 않는다. 실제build/5case는 여전히 NOT_RUN이며 CI컴파일 성공만으로 이실행초기화가 검증됐다고 하지 않는다.

NP01 / 실제 notices 준비 재현(수정 실행 전 사례): root가 직접 확인한 helper-only CI42336731225/head28b503415773a2bf4ff12327aa9e0007cc421186의 completed FAILURE는 기존 build.rs가 `..\THIRD_PARTY_NOTICES.md`를 찾지 못한 준비오류다. 원FAIL을 유지하며 verifier 실패나 제품결함으로 바꾸지 않는다. 전제·입력은 같은 동결제품 및 새 helper-only 수정커밋, 기존 windows.yml/native의 setup-node24/cache npm→npm ci→node scripts/notices.mjs다. 절차는 이 실제생성 단계를 compile 앞에 재사용하고 기존 --no-run compile을 수행한다. 기대는 실제 notices 생성 후 해당resource 누락 없이 helper compile 단계 진행이며 실제caseCI실행0이다. 빈/가짜notices·frontendbuild·NSIS·제품src/lock/version/finalasset 변경은 금지다. 준비·compile 중 실패는 원오류와 종료코드를 보존하고 통과로 바꾸지 않는다. 실제 새후보 CI: NOT_RUN(준비자는 commit/push/dispatch0, root만 수정commit/push로 시작; 동일원후보 rerun금지).

자원 설정 대조: 기존 windows.yml/native는 CARGO_BUILD_JOBS=2, CARGO_INCREMENTAL=0이다. helper workflow에는 둘다 없었으므로 동일값을 job env에 최소추가한다. RUST_TEST_THREADS는 compile-only job에 실제case실행이 없어 추가하지 않는다. 기존 Rust cache shared-key windows/save-if false는 보존하며 첫CI 시간3분대나 compile캐시 완전재사용 여부로 cache hit를 단정하지 않는다. 새후보의 notices 생성/compile 결과와 cache관측은 실제CI 뒤 root가 별도로 기록한다.

NP01 준비 정적결과: 수정workflow YAML parse 및 기존native와 setup-node24/npm cache·CARGO_BUILD_JOBS2/CARGO_INCREMENTAL0 일치 대조 exit0, npm ci→실제notices script→--no-run compile 순서 대조 exit0이다. 전체 PowerShell run6개 AST parse오류0, git diff --check exit0이다. 새후보 CI/notices 실제생성/compile/로컬5case는 NOT_RUN이며, 수정경로는 workflow와 이사례문서2개뿐이다.
