1.7.1 D0053 후속 시험 보강 범위다. main664a8efe에서 시작하며 제품의 비시험 실행 동작은 바꾸지 않는다. 과거 간헐 실패 원인은 미확정으로 유지한다.

기존 12be611/CI42477403292에서 종료 누락은 정확한 대상 wait assertion으로 검출됐지만 Drop-owned process handle을 mem::forget하는 변이는 살아남았다. 이번에는 Job 집계와 별도로 해당 핸들의 실제 해제를 관측한다. 과거 원 receipt는 verification/drop-sensitivity-165/first-fail/files/runs에 보존한다.

기존 공개 경계 spawn_command_in_job_suspended → token/whole-launch Drop을 사용한다. cfg(test)에서만 원 pending handle 값을 읽고, 독립 observer와 같은 kernel object임을 Drop 전에 확인한다. Drop 후 원 slot이 같은 object를 가리키면 실패한다. slot 재사용으로 다른 object를 가리키는 경우와 invalid handle만 허용하며 다른 조회 오류는 실패한다. 캡처한 raw handle을 닫거나 소유권을 재구성하지 않는다.

Microsoft의 [CompareObjectHandles 계약](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-compareobjecthandles)은 object identity 비교이며 Windows10/Server2016 이상을 대상으로 한다. 사용은 기존 Windows 테스트 모듈에 한정한다. [Job accounting 계약](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information)의 집계와 개별 owner handle 해제 관측을 구분한다.

통과 조건은 선작성 test-plan.json의 두 변이가 동일 test bytes/argv로 각각 정확한 assertion을 내고, 수정 없는 control이 token-only/whole-launch 각8회 통과하는 것이다. 실제 target signaled/nonzero exit/entry marker 부재/observer 종료 뒤 기존10초 zero-job 조건을 유지한다. 컴파일·spawn·timeout 오류는 검출 성공으로 세지 않는다.

기존 runner를 재사용하며 Windows GitHub Actions에서 세 변형을 순차 실행한다. 변형별 완전한 crate 사본과 별도 target, CARGO_BUILD_JOBS=2, CARGO_INCREMENTAL=0, test-threads=1, 실행당300초를 사용한다. 로컬 Rust/Cargo, 공동시험, 기존 daemon/auth/사용자 DB/설치는 실행하지 않는다. 최종 근거의 독립감사는 완결된 업무 단위에 한 번 적용한다.
