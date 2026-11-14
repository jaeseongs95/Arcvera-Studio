# Runtime preview 단계 진단 계측

[English](runtime-preview-diagnostics.md) · [한국어](runtime-preview-diagnostics.ko.md)

상태: 가설·판정 기준을 먼저 기록한 계측 준비다. 원인 확정이나 수정 효과의 증거가 아니다. 이 단위의 빌드·시험·실제 앱 실행·별도 검증 CLI는 **NOT_RUN**이며 부모가 직렬 검증한다. 사용자 목표는 Desktop 2.1.0 릴리스와 같은 Core 1.1 후보의 실제 통합이다. Core 1.1 최종 schema/실행물 지원을 주장하지 않는다.

## 관측과 가설

명시 공유 기술 결과 `D:/codex/build/avrev-a18/evidence/OPENDIY_DESKTOP-D-0085.json` 및 `native-run/native-input.json`에서 고정 Native 53/ca69와 공개 Core 1.0.1 전체 패키지의 정상 선택 후 preview가 시간 초과했다. 기존 codex 이름 거절과 별개의 실패다. 기록된 검수 구간은 04:16:44~04:17:10이며, 부모가 전달한 파일 조건은 CLI 약 443 MB/server 약 341 MB 및 debug 개발 실행물이다. 어느 checkpoint가 시간 한도를 소진했는지는 아직 미확정이다.

기존 preview의 pair 조회, update 도움말, 패키지 검증과 streaming SHA256은 같은 20초 deadline을 쓴다. 다음 관측은 **같은 파일 bytes·같은 budget·같은 기능/guard**에서 수행한다. profile·파일 크기·캐시의 warm/cold 여부·시스템 부하와 고정 실행물/source 식별은 부모의 별도 기술 receipt로 남긴다. 계측 자체에는 그 식별값을 기록하지 않는다.

| 가설 | 구별할 관측 | 판정 한계 |
| --- | --- | --- |
| H1: pair/help 조회가 budget 대부분을 사용 | `pair_probe`/`update_help` 시간이 길고 이후 `package_probe` 시작 전/초기에 실패 | phase 누락은 0이 아니다. 해당 단계 미도달·프로세스 종료·수집 누락을 구별한다 |
| H2: 비최적화 SHA 비용 | `file_hash` 안의 누적 `file_sha256` 시간이 크고, 같은 조건의 release-profile 비교에서 그 시간이 감소 | wall-clock 시간에는 스케줄링 지연이 포함된다. profile 차이만으로 CPU 원인을 확정하지 않는다 |
| H3: 파일 읽기/기타 I/O | 누적 `file_read`가 크거나 `file_hash` 전체와 read/SHA 합의 차이가 큼 | read는 OS read 호출 구간만이다. 차이에는 open/metadata/checkpoint/기타 비용이 포함되며 저장장치 원인을 특정하지 않는다 |
| 혼합/미확정 | 둘 이상이 크거나 완결된 trace가 없음 | 시간 한도 증가·hash 생략·검사 완화로 판정을 대체하지 않는다 |

## 계측 계약과 기존 의미

`ARCVERA_STUDIO_RUNTIME_TRACE=1`을 명시한 부모의 own fixture에서만 stderr를 사용한다. 미설정·다른 값은 off다. preview 진입에서 한 번 읽고 private 경로로 전달하며, 기존 inline fixture 및 후검증의 기존 함수는 disabled 경로를 사용한다. process-global 환경변수를 시험에서 바꾸지 않는다. 새 파일/큐/스레드/서버/재시도는 없다.

각 완료 record에는 고정 prefix `runtime_preview_trace`, 고정 `phase`, `elapsed_ms`, 고정 `outcome`만 쓴다. 파일 phase는 처리한 `bytes`도 쓸 수 있다. 경로·args·manifest·hash·오류 원문·프로세스/대화/사용자 식별값·CLI 출력은 기록하지 않는다. phase는 `pair_probe`, `update_help`, `package_probe`, `file_hash`, `file_read`, `file_sha256`, `preview_total`뿐이다. `preview_total`의 `blocked`는 `Ok`이지만 `can_apply=false`인 기존 결과이며 성공/허용으로 바꾸지 않는다.

파일의 read/SHA 시간은 각 호출 시간만 누적하고 **파일 완료 시 한 번씩** 기록한다. chunk별 로그는 없다. `file_read` bytes는 반환받은 bytes, `file_sha256` bytes는 실제 hash update에 전달한 bytes다. read 후 checkpoint에서 실패하면 두 값이 다를 수 있다. 시작하지 않은 하위 연산은 `not_run`으로 기록하며 0 ms 정상 측정으로 해석하지 않는다. 전체/중첩 phase는 서로 겹치므로 합산하지 않는다. 정상 full-package standalone preview는 최대 10줄, runtime-only fallback은 최대 28줄이다. 반복 preview는 부모가 각각 직렬로 수집하고 자동 합치지 않는다.

모든 정상 `Result` 복귀는 오류여도 해당 span 완료 record를 남긴다. panic·강제 종료·막힌 OS I/O의 완료를 보장하지 않는다. stderr write 실패는 제품 결과를 바꾸지 않도록 무시하며, 이 경우 관측 누락이지 정상 수치가 아니다. tracing의 timing/stderr 비용은 원래 deadline 안에 포함되며 로그 sink의 지연도 결과에 영향을 줄 수 있다. 별도 스레드나 더 긴 timeout으로 숨기지 않는다.

20초/1 GiB executable/16 KiB manifest 상한, 모든 checkpoint와 오류 문자열·도메인 결과, canonical 경로/closed entrypoint/version/target/hash/helper/fingerprint 검증, 공유 서버 수명과 public API를 유지한다. 동기 I/O 중단을 새로 보장하지 않는다.

## engineering-test-plan

기존 Rust inline fixture를 사용한다. 실제 Core/daemon/앱/HOME/auth를 실행하거나 읽지 않는 단위 시험이다. 사전 문서 다음에 시험을 작성하고 계측을 연결한다. 기존 D85 8개 및 전체 runtime_transition assertion은 삭제·skip·완화하지 않는다.

| 사례 | 요구/Oracle | 수준 |
| --- | --- | --- |
| TR01 | 정확한 env 값 `1`만 활성; 기본/0/true/공백 off | 순수 입력 해석 단위 |
| TR02 | disabled trace는 writer에 0 bytes; enabled는 static phase/outcome/시간/수량만, 도메인 payload 미반사 | 형식/비밀 경계 단위 |
| TR03 | 성공/오류 closure 결과를 그대로 반환하고 각 완료 event의 outcome 구분; 실패 writer도 도메인 결과 불변 | 계측 wrapper 단위 |
| TR04 | 파일 read/hash 성공의 bytes/hash와 취소·만료/누락파일 오류 보존; 하위 연산 미실행은 not_run | 합성 filesystem 단위 |
| TR05 | preview allowed/blocked/error 분류와 기존 반환값 보존; phase 누락을 정상으로 채우지 않음 | 기존 preview fixture 단위 |

부모는 실제 동일 파일의 debug/release-profile 비교에서 전체 trace와 원래 도메인 결과를 함께 보존한다. TRACE off/on의 기존 회귀, 실제 stderr 수집/유한 줄 수/비밀 비노출, pair/help 또는 hash 중 실패 시 완료 record를 직접 확인한다. 합성 fixture만으로 H1/H2/H3를 확정하거나 원래 Native 실패가 해결됐다고 보고하지 않는다.

## 작성 후 인계

부모 실제 관측(e99, 같은 CLI442612736/server340826624 bytes, 원20초, 세션0 read-only): debug는 pair540ms/help69ms/read355ms/SHA19006ms/전체20001ms에서 두번째 파일의 부분 hash 후 deadline 오류였다. release는 pair512/help68/read232/SHA509/전체1327ms로 두 파일 모두 확인한 뒤 canApply=true였다. 두 자체 앱 exit0, 확인된 own Core 시작·종료와 최종stopped, 운영 앱 identity 보존을 확인했다. 표적43562379004 및 전체43562383034 CI도 통과했다. 단위 전환39개 중 새 계측5개가 포함된다.

이 통제 재현에서 주 예산 소모 지점은 debug SHA 구간이다. 각 구간은 wall time이라 스케줄링을 완전히 분리하지 않으며, 원 a18은 idle TUI 상태·다른 파일 사본이라 그 실행의 정확한 단계 시간을 소급하지 않는다. Root는 제품20초·검사·해시를 바꾸지 않고 최종 배포와 같은 최적화 비교품으로 원 GUI 교체·재연결 독립 재검수를 요청했다. 원 Native 수용은 아직 미완이다. 원시 경로는 `desktop210-integration-20261114/preview-comparison.json`, `preview-debug-e99-01`, `preview-release-e99-01`이며 debug lifecycle scope 오기는 원출력과 clarification을 함께 보존했다.

부모 CI는 `build-native-fixture.ps1`의 기존 기본 debug 빌드를 유지하고, 별도 `-FixtureProfile release` 실행물도 만든다. release 비교품은 `release-profile/` 하위의 독립 manifest·resource·config와 `fixture<commit12>release` identifier를 사용한다. 이는 최종 배포품이나 설치가 아니라 같은 소스의 최적화 조건을 확인할 격리 비교품이다. 두 프로필 모두 원래 20초와 검증 guard를 사용하며 수집 시 부모가 같은 CLI/server 파일, 동일 SHA, 부하·캐시 조건과 실제 trace/result를 함께 기록한다. 실행물·프로필 이름만으로 가설을 확정하지 않는다.

고정 phase 계측과 inline 단위 시험 5개를 작성했다. 기존 D85 8개와 나머지 assertion은 유지했다. 정적 diff/범위·UTF-8/LF 및 guard 대조 결과와 최종 파일 SHA256은 Ready 보고에 남긴다. 빌드·시험·실제 trace 수집은 **NOT_RUN**이다. H1/H2/H3는 여전히 가설이며 부모의 같은 조건 관측과 기존 회귀를 기다린다.
