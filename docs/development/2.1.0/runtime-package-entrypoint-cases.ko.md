# Runtime 패키지 entrypoint 회귀 사례 — D85

[English](runtime-package-entrypoint-cases.md) · [한국어](runtime-package-entrypoint-cases.ko.md)

상태: 사전 사례/시험 설계. 이번 하위 작업의 compile·시험·실제 Native 실행은 **NOT_RUN**이며 부모가 직렬로 검증한다. 사용자 목표 원문은 “너의 목표는 2.1.0 릴리즈이다. cli 담당과 협업해서 2.1.0 릴리즈에 들어갈 개발 범위를 산정하고 작업하면 된다.”이다.

관측 근거는 공유 기술 결과 `D:/codex/build/avrev-a13/evidence/A13-F01.json`이다. 실제 Native 105d625/a88 + 공개 Core1.0.1의 정상 전체 패키지 교체 preview가 구 `codex.exe` 이름 요구로 FAIL했다. 직접 읽은 공개 `D:/codex/build/desktop210-integration-20261114/core101-baseline/runtime/codex-package.json`은 layoutVersion 1, version 1.0.1, target x86_64-pc-windows-msvc, entrypoint `bin/arcvera.exe`다. 이 자료는 신규 시험 실행 결과가 아니다.

## 최소 수정과 불변조건

`package_evidence`/`replacement_package_evidence`의 기존 API를 유지한다. manifest는 `bin/arcvera.exe`(현행) 또는 `bin/codex.exe`(legacy)의 정확한 문자열만 허용한다. manifest가 결속한 canonical CLI, 같은 canonical bin의 `codex-app-server.exe` 또는 CLI 자체, 전체 버전·target·layout을 확인한다. 경로 순회·임의 실행 파일·다른 bin·manifest와 선택명 불일치는 거절한다. runtime-only ZIP 재사용은 설치된 전체 패키지 manifest가 결속한 CLI 이름 및 CLI/server/`codex-code-mode-host.exe`의 동일 byte를 요구한다. 파일 복사·rename·fallback 완화로 통과시키지 않는다.

교체 후 검증은 preview의 CLI 이름과 파일 hash를 유지하며, runtime-only 재사용에서 확보한 helper hash도 재확인한다. 기존 fingerprint의 경로·manifest·hash·세대/관측 결속, 취소와 전체 deadline, 읽기 크기 상한·변경 감지, 자동 재시도 금지를 유지한다. 전체 패키지 트리 무결성 검증·실제 daemon 교체는 CLI 책임이며 preview의 hash 검사를 그 증거로 확대하지 않는다. 이번 수정은 새 helper 이름을 추정하거나 전체 패키지 preview의 기존 helper 검사 범위를 늘리는 작업이 아니다.

## engineering-test-plan / 실패 oracle

기존 Rust inline native fixture에 아래 사례를 먼저 추가한다. fixture는 UUID own 임시 폴더의 합성 bytes를 사용하며 실제 실행 파일·daemon·HOME·auth를 읽거나 실행하지 않는다. 새 시험은 기존 API만 사용하여 수정 전 모듈에서도 컴파일 가능한 형태로 작성한다. 시간 조건은 직접 취소 flag 및 만료 deadline으로 검사하며 sleep하지 않는다. CS 의무는 manifest→canonical CLI→같은 bin 파일→preview/후검증 신원 결속과 취소/deadline 유지다. 별도 실행·AGS CLI는 이번 배정에서 금지되어 NOT_RUN이다.

| ID | 경계 | Oracle |
| --- | --- | --- |
| EP01 | 현행 full/CLI 모드 | arcvera manifest와 선택 CLI가 일치하면 증거/hash 확보; CLI 모드는 server hash=CLI hash |
| EP02 | legacy full/CLI | codex manifest와 선택 CLI 정상 계약 보존 |
| EP03 | manifest/선택명 불일치 | 양쪽 실행 파일이 존재하고 동일 bytes라도 거절 |
| EP04 | 경로 순회·미지원 이름·다른 bin | `../`, backslash/절대/미지원 entrypoint, 다른 bin server/CLI 모두 거절 |
| EP05 | 현행 runtime-only | 설치된 arcvera 전체 패키지와 3개 파일 bytes가 같을 때만 설치 패키지 재사용 |
| EP06 | runtime-only 변조·누락·다른 manifest 이름 | CLI/server/helper 각각 다른 bytes 또는 helper 누락, 설치 manifest의 CLI 이름 불일치 거절 |
| EP07 | 교체 후 동일 신원 | 같은 이름/bytes 성공; CLI/server/helper 변조 및 동일 bytes의 다른 CLI 이름+manifest 전환 거절 |
| EP08 | 취소·만료 | package/replacement 조회가 성공하지 않음; 기존 fingerprint/세대 결속 시험도 유지 |

신규 시험을 붙인 변경 전 모듈을 `D:/codex/build/desktop210-integration-20261114/d85-entrypoint/before-with-regression.rs`에 먼저 보존한다. 부모는 같은 inline 시험 본문·fixture·명령으로 RED/GREEN을 실행하고 compile 실패를 제품 RED로 세지 않는다. 기존 assertion을 삭제·skip·완화하지 않는다. 동일성은 before와 수정 후의 `#[cfg(test)] mod tests` bytes/hash를 대조한다.

준비·정적 대조 결과와 digest는 하위 작업 Ready 보고에 남긴다. 수정 효과·실제 공개 패키지 preview·교체/취소/세대 회귀·독립감사 통과는 부모의 실제 실행 전까지 미검증이다.

## 작성 후 인계 상태

생산 코드 수정과 `d85_` 회귀 시험 8개를 작성했다. 공개 API/직렬화 DTO의 변경은 없으며 private `manifest_matches`는 검증한 entrypoint 문자열을 반환하고 private 교체 신원에 CLI 이름을 보관한다. 기존 assertion은 유지했다. 변경 전 사본과 수정 후의 전체 inline 시험 본문이 동일함을 UTF-8 SHA256 `ba5b6dca58a9f28292020c35305c45ceb9ec7434c74208a541540157a9996bfe`로 정적 대조했다. 변경 전 사본 전체 SHA256은 `d77058683f9ce0064d06e99ee6e601c84d94d0bbdc5f14cd6a7186af84d522f4`이다.

부모가 같은 시험을 RED/GREEN으로 실행할 준비가 됐다는 의미다. 이 하위 작업에서 compile·시험·실제 Native 실행·설치/배포·별도 AGS CLI 검사는 모두 **NOT_RUN**이다. 현재 수정 파일의 최종 digest는 Ready 보고에 기록한다.

## 부모 실행 환경

로컬 D: 여유 23.17 GiB가 기존 Native runner의 24 GiB 사전 하한보다 작아 로컬 compile/RED/GREEN은 **NOT_RUN**으로 유지한다. 하한을 낮추거나 기존 근거를 지우지 않는다. 기존 Windows CI의 격리 checkout에서 관측 baseline `916417d6b4c93fe253be3f94412fd05328c42936`의 생산 코드에 현재의 동일 inline 시험 본문만 결합한다. 새 8개 사례 중 EP01·EP05·EP07에 해당하는 정확한 3개 assertion 실패를 요구하며 compile/환경 오류는 RED로 인정하지 않는다. finally에서 수정 파일의 원래 bytes를 복원·해시 대조한 뒤 같은 8개 사례가 모두 통과해야 한다. baseline을 직접 부모로 가진 변경에서만 이 쌍을 실행하며 이후 commit에서도 일반 `runtime_transition` 전체 회귀는 계속 실행한다. 원시 before/fixed 로그와 source/test digest receipt는 Native adapter CI artifact에 보존한다.

이 CI 검증은 Windows 파일시스템·합성 실행 파일 bytes의 경계다. 실제 공개 Core 패키지를 선택한 Native preview·교체 및 기존 사용자 세션 보존은 수정 실행물을 고정한 뒤 동일 실제 사례로 별도 확인한다.
