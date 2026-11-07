# Windows 서명 업데이트 설치 인계 회귀

기준 제품은 0e0d98a7edfd6b99c2bead67857895febc1b2f0e(1.6.2)다. 기존 서명 후보와 actual14/19 FAIL은 보존한다. 이 패치는 다음 1.6.3 후보이며 아직 발행·설치한 버전이 아니다.

실제19는 원서명·변조 거부·native `/D` 관측 뒤 helper exit0였지만 설치·재시작을 관측하지 못했다. 무해한22 비교에서 ShellExecute 자식은 부모의 정확한 KILL_ON_JOB_CLOSE Job에 속했고 부모 종료 뒤 사라졌다. 이것은 Windows 수명 경계의 재현이며 실제19 NSIS의 Job 소속을 사후 확정하는 근거는 아니다.

| 사례 | 실제 경계·입력 | 기대·판정 | 실행 위치 |
| --- | --- | --- | --- |
| H01 | 동일 own Job에서 무해한 자식을 명시적 CREATE_BREAKAWAY_FROM_JOB로 생성 | 정확 Job membership=false, 부모 정상 종료 뒤 자식 marker와 정상 exit0. Win32 error·timeout은 실패로 보존 | 이 PC의 별도 fixture23, 각 사례 한 번 |
| H02 | 업데이터 NSIS 실행 함수에 존재하지 않는 절대 EXE 전달 | Io error가 호출자에게 반환되고 시험 process가 계속 살아 있음. ShellExecute fallback·성공 exit 없음 | CI Windows Rust unit |
| H03 | 기존 updater2.14.1 verify/install 거부 및 NSIS/MSI 인자 escape corpus | 원서명 검증·변조 거부·signed-version·기존 quoting 사례 유지 | CI Windows, vendored crate 기존 unit |
| H04 | 원 TEST NSIS60581·원서명·manifest·key 및 새 helper로 기존 실제 runner 실행 | 다운로드 원bytes, native verify, 1byte 변조 거부/hook0, 정상 설치·정확 설치 EXE SHA, 재시작 GUI 및 정리까지 기존 oracle 모두 통과. 60초 설치 상한과 실패 기록 보존 | 소스 동결·독립 사전 감사 후 이 PC |
| H05 | 제품 기존 native admission·실패 후 permit release 및 frontend 회귀 | 새/진행 중 세션 보호와 기존 사례 유지. 버전만 1.6.3으로 일치 | 기존 Windows CI |
| H06 | exact registry crate와 vendored source 비교 | Windows NSIS launch와 그 테스트·주석 외 verify/extract/parameters/MSI/비Windows 코드 불변. 라이선스 고지 포함 | 원crate SHA 및 diff 직접 검사 |

설계 경계: 새 서명 구현이나 별도 상주 helper를 만들지 않는다. 설치를 소유한 기존 업데이터의 NSIS launch만 Windows 표준 CommandExt로 바꾸고, 이미 생성된 parameters는 raw_arg로 전달한다. 현재 제품은 currentUser NSIS만 번들한다. MSI는 기존 ShellExecute 경로를 유지한다. CREATE_BREAKAWAY는 현재 APP_JOB의 BREAKAWAY_OK를 사용하며 다른 자식의 KILL_ON_JOB_CLOSE·per-session Job 정책을 바꾸지 않는다. OS가 breakaway를 거절하면 오류로 반환하고 재시도하지 않는다.

OS-LIFETIME-001(draft)을 실제 Job 마지막 handle 수명 문제에 한정해 적용한다. 설치 자식은 서명 검증 뒤 성공한 spawn에서 Windows installer로 수명을 넘긴다. 부모가 설치 완료를 기다리면 실행파일 교체와 교착할 수 있어 기다리지 않는다. launch 오류는 반환하고 기존 UI/permit을 보존한다. 정상·오류·취소 경로는 기존 다운로드 상한/timeout/서명·세션 gate로 검사하며, OS error/획득 직후 실패 주입이 빠지면 UNVERIFIED로 남긴다.

원22는 과거 control/treatment 근거이고 새 H01의 red/green 쌍으로 재포장하지 않는다. 실제19→H04도 helper/dependency가 바뀐 후속 실행으로 구분한다. H04 미실행 상태에서 CI·무해한 marker 통과만으로 실제 업데이트 완료를 선언하지 않는다.

금지: 운영 PID·daemon 조회/중단/게이트, 인증·다른 세션 로그 열람, 실제 GUI/installer/daemon 강제 종료, 로컬 Rust 빌드, 전역 Job SILENT/KILL 플래그 변경, 자동 재시도·uninstall·등록 삭제. 테스트 실행과 제품 발행·PC 설치는 각각 별도 근거로 판정한다.
