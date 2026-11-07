# Desktop TEST 1.6.3 사전 사례

이 CI 시험본은 기존 앱을 종료하거나 운영 PID를 조회하지 않고 root가 fresh own HOME에서 GUI/서명 updater를 검사할 수 있도록 고유 identity를 제공한다. GUI39의 input0·새CDP0·37ms exitclose0 원 FAIL은 보존하며 그 원인을 확정하거나 제품 PASS로 바꾸지 않는다. 동결 제품 d03a00e54e5c72947d2d20705932fb17110f5bd3 로직은 그대로다.

- TC40-IDENTITY: base config의 productName=`OpenDIY Desktop 163 TEST`, identifier=`io.agentgrid.desktop.test163`, app.windows[0].title=`OpenDIY Desktop 163 TEST`의3값만 바뀐다. base 바이트를 이3값만 원문으로 역치환하면 d03 SHA와 같아야 한다. resources-only bundle overlay는 byte동일하며 merge 결과에서3값을 제외한 전체구조, width1400/height900/minWidth800/minHeight540/label/dragDropEnabled/CSP/권한/updater/currentUser/version은 원문과 같다. 잘못된 identifier나 빠진 width는 통과할 수 없다.
- TC40-CI: push branch test/desktop163-signed-isolation 전용 한 packaging job만 준비한다. 기존 setup-node24/npm cache/Swatinem shared-key windows/CARGO_BUILD_JOBS2·incremental0을 재사용한다. npmci→기존 packaging/notices회귀→notices생성→기존Tauri signedNSIS/frontend build→portable/resources/checksums→같은packagedPTYselftest→기존updater-manifest→source/config/inputs/artifacts binding으로 연결한다. 기존 public키152bytes/SHAa8c985f6를 검사하며 repository vars/secrets 이름만 참조한다. private key는 signing step env에만 있고 출력/파일/artifact에 담지 않는다. actualGUI/installer/발행 step이 없어야 한다.
- TC40-MANIFEST: 기존 prepareUpdaterArtifacts 공개함수에 정확 TEST productName/1.6.3의 무해한 synthetic NSIS+sig를 넣어 실제예상 filename과 normalized updater3file/SHA가 연결됨을 확인한다. 원script를 바꾸지 않고 missing pair/multiple/wrongversion 등 기존11case와 notices1case도 재사용한다. fixture는 시험루트 artifacts 안에만 생성하며 실행가능 EXE가 아니다.
- TC40-SCOPE: 전후 tracked source SHA/diff를 대조해 기존 파일 변경은 base config3값뿐, 추가 파일은 이문서/전용workflow/최소static testcase뿐임을 검사한다. Rust/source/lock/permissions/기존 windows.yml/overlay/scripts는 불변이다. source binding은 원commit/실제CIcommit/run/입력핀과 merge검사를 보존한다.

이 로컬 단위는 Node stdlib의 config/source 검사와 기존 synthetic packaging/notices mock만 실행한다. YAML/PowerShell은 가용한 기존 parser로 syntax만 검사하며 새 의존성은 설치하지 않는다. 실제 CI 서명/패키징/packagedselftest/GUI/installer/restart/network/운영조회/registry/Git commit·push/장부는 NOT_RUN 또는0이다. root가 후보를 검토하고 exact branch를 push해 CI접수·실제artifact 확인·로컬freshfixture 실행을 별도로 한다. 기존 production/release 경로·manifest URL을 변경하지 않고 TEST artifact는 publish하지 않는다.

새 기능 구현이 아니라 시험식별자/CI preparation이므로 product RED/GREEN은 not-applicable이다. 정적 검사의 민감도는 잘못된 identity/창배열 항목 누락을 주입한 자체메모리 입력이 거부되는지 확인한다. 이 mock을 실제 singleton/GUI/설치 성공으로 확대하지 않는다. 기존 Sol6.1/high 요청/advisor READY를 재사용하며 host-effective는 미관측이다. workflow524는 root 소유이고 중복 workflow는 만들지 않는다.

## 사전 범위 교정03 — default-off helper 전용 identity

root가 actual_probe::validate_identity(340)와 validate_package(392)를 확인해 기존 ‘ TEST’ suffix를 보존하도록 title/productName을 OpenDIY Desktop 163 TEST로 교정했다. 첫초안은 실제빌드/실행 전 artifacts/test40/initial-draft에 보존한다. identifier는 기존1.6.2 TEST와 분리한 io.agentgrid.desktop.test163을 유지한다.

TC40-HELPER: 제품 패키지/portable/updater/원본불변/selftest/source·package binding 완료 뒤에만 CI가 default-off actual_probe module 안의 정확 identifier token3곳을 .test→.test163으로 임시 결속한다(validator1+pure fixture2). ‘ TEST’ suffix/production-identifier 거부/ownroot/socket103bytes/nonce32/reparse/argc1/transport/phase/nativeverify/Job/installer 의미는 변경하지 않는다. helper만 dev compile하고 기존 pure6case를 실행하며 EXE/source/diff/로그/pin을 별도 helper/에 보존한다. CI finally는 원 source bytes를 복원하고 제품 입력핀·이미고정한 product file핀을 다시 대조한다. 이 helper source declaration은 상품 d03+config3값의 source binding과 분리한다. 로컬 source에 이 임시 Rust변경을 적용하지 않으며 로컬 Rust/actualhelper실행0이다.

사전 Node 검사는 실제workflow 안의 helper source rewrite 코드만 harmless memory/writer-stub로 호출하여 실제src55,854bytes에서3identifier token만 바뀌고 역치환이 원SHA와 같음을 확인한다. 이 source-only 검사는 실제 Rust compile/pure6PASS가 아니다. 새로운 Rust프로젝트/의존성/일반 policy API/키는 추가하지 않는다.

