# Native updater1.6.3 검증 사례

동결 제품 d03a00e54e5c72947d2d20705932fb17110f5bd3 및 CI42414088993의 실제 서명1.6.3 패키지를 사용한다. 원 test/desktop161-updater-native의 PN01–PN05를 재사용하고 artifact 핀·버전만 바꾼다. 제품 파일·Cargo.lock·서명 키·원 실패를 수정하지 않는다. CI는 integration-test 실행 파일을 컴파일하고 이 PC에서 다섯 사례를 실행한다. CI의 라이브러리/시험 target 컴파일과 제품 패키지 재생성을 구분하며 새 release/NSIS 패키지는 만들지 않는다.

|사례|기대 결과|
|---|---|
|PN01|실제 installer7a858ba1와 sigec042928·manifest21753af1를 actual Update.verify가 수락|
|PN02|installer 첫1byte를 메모리에서 XOR1하면 Minisign 거부|
|PN03|signature packet 메모리 변조를 Minisign 거부|
|PN04|trusted comment 메모리 변조를 Minisign 거부|
|PN05|원sig 그대로 announced1.6.4이면 SignedVersionMismatch(signed1.6.3, announced1.6.4)|

모든 사례에서 beforeExitCalls0, 네트워크 client callback은 panic, 원 입력파일3개/공개키SHA a8c985f6 불변을 확인한다. MockRuntime에 실제 pinned vendored updater2.14.1을 연결하며 GUI/다운로드/설치/재시작/운영PID·daemon/auth 접근0이다. native 실제 실행 전은 NOT_RUN이고 Node crypto 결과·CI 컴파일·이전1.6.1/TEST1.6.2 결과로 대신하지 않는다.

ZIP SHA6f6268e28a9486f7d8df96d8186cd8c9ad2efe32ddc78c0801285b86be90c605, artifact13591620469/run42414088993. 변경파일은 native test·helper-only workflow·본 사례 문서뿐이며 Cargo.lock·일반 제품source 및 동결후보는 그대로다. 원 비교 기준은 git test/desktop161-updater-native:src-tauri/tests/updater_native_probe.rs이다.
