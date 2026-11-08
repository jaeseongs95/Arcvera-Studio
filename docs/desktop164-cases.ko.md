# Desktop TEST 1.6.4 사전 사례

제품 기준은 d044d32530f6abffe37f34e2210ff882862d6a59(1.6.4 focusfix 포함)이다. TEST163의 기존 격리 CI 방식만 재사용하며 이전 d03/TEST4d1 로직이나1.6.3 패키지를 새1.6.4로 오표시하지 않는다. 실제 GUI·singleton·서명/설치·재시작·native helper 성공은 이 준비 단계에서 NOT_RUN이다.

- TC164-IDENTITY: config의 productName/title=`OpenDIY Desktop 164 TEST`, identifier=`io.agentgrid.desktop.test164`만 변경한다. 정확3값을 역치환한 raw SHA는 d044 원config와 같아야 한다. 창배열/width/height/CSP/resources/currentUser/updater 설정은 보존하며 wrongidentifier/누락width를 거부한다.
- TC164-CI: test/desktop164-signed-isolation push 전용 single job이다. 기존 npm/cache/Rust cache/windows shared-key/2jobs 설정과 signed NSIS, portable, packaged PTY self-test, 실패 evidence always upload를 재사용한다. 기존 windows.yml은 변경하지 않는다. 키는 기존 repository env 참조만 사용하고 새로운 키나 로컬 인증파일을 만들지 않는다.
- TC164-MANIFEST: 기존 updater-manifest generator에 합성 TEST164 NSIS3MiB와 무해한 sig를 넣어 정확1.6.4 이름·원바이트·signature·3파일 checksum을 검사한다. production NSIS 이름과 standalone EXE를 TEST installer로 대체하면 거부한다. 서명 원문 보존은 암호학적 검증이 아니다. standalone과 NSIS 내부 installed EXE의 bundle-type 토큰 변환에 따른 SHA 차이는 별도 설치 바이트 판정이 필요하며 standaloneSHA를 installedSHA로 가정하지 않는다.
- TC164-HELPER: 제품 package binding 이후에만 CI가 default-off actual_probe module의 TEST id3토큰을 .test→.test164로 임시 치환한다. 동일module 역치환·pure6case·원 source복원·제품 input/artifact 재대조를 유지한다. 로컬 검사는 실제embedded code를 memory writer stub로 호출하며 Rust/source write/compile은0이다.
- TC164-SCOPE/VERSION: d044 tracked238파일을 초기 SHA와 전후 대조한다. 기존config1파일 세값 외237파일은 동일해야 하며 신규 tracked후보는 workflow/test/docs3개뿐이다. package/config/Cargo는 모두1.6.4다. 원TEST1634파일 및 d03/4d1은 읽기전용 보존한다.

로컬 명령은 `node --test scripts/test164-package.test.mjs scripts/updater-manifest.test.mjs scripts/notices.test.mjs`이다. stdlib와 기존source만 사용하므로 npmci/새 의존성 설치는 불필요하다. 합성 fixture/원시결과는 ignored artifacts/test164-preparation 및 기존 packaging/notices artifacts에 남는다. 실제 CI dispatch/제품 빌드/GUI/CLI/설치/registry/publish/Git commit·push는 root의 후속 단계다. 기존 production/release URL은 변경하지 않고 TEST artifact를 공개 release에 발행하지 않는다.

원TEST163 template은 artifacts/test164-preparation/template163.json에 보존했고 현재 base와 version에 맞춰 pin을 교정했다. 요청 Sol6.1/high 재사용, effective 설정은 미관측이다. Workflow570은 root 소유이며 새 workflow를 만들지 않는다.
