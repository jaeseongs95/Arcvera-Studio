# 격리 runtime 전환 시험 진입점

`--runtime-transition-test`는 GUI 초기화 전에 실행되며 기본 실행과 `--codex-smoke`에 섞을 수 없다. 동일 제품의 `:memory:` Engine과 실제 runtime preview/apply API를 호출한다. 새 backend, 고정 lifecycle 응답, 모델 턴은 사용하지 않는다. 이 문서 작성 시 R40 실제 통합시험은 아직 NOT_RUN이다.

호출자는 전용 fixture root를 준비하고 그 안에 `.opendiy-runtime-test-owned` 파일을 정확한 UTF-8 바이트 `opendiy-runtime-test-v1` + LF로 만든다. 이 표식은 실수 방지용이며 인증이나 외부 프로세스 소유 증명이 아니다. 선택한 CLI와 app-server 및 기존 관리 패키지는 fixture 안의 복사본이어야 한다. 전체 패키지 manifest와 helper·정적 자원은 CLI 담당자가 제공한 동일 후보를 보존한다.

프로세스 환경의 `CODEX_HOME`, `HOME`, `USERPROFILE`, `APPDATA`, `LOCALAPPDATA`, `TEMP`, `TMP`를 fixture 안의 기존 디렉터리로 명시한다. 기존 운영 HOME, 인증·DB·개인 로그를 읽거나 복사하지 않는다. `--report`는 fixture의 하위 보고서 디렉터리에 있는 새 파일이어야 한다. 기존 파일은 덮어쓰지 않으며 생성 실패 때 실행 파일을 호출하지 않는다.

```powershell
& $desktopExe --runtime-transition-test --root $fixtureRoot --cli $ownCli --server $ownServer --action preview --report $newPreviewReport
```

preview의 `result.preview.fingerprint`를 보존하고 같은 HOME·선택 실행본으로 `--action apply --fingerprint <정확한 64자리 소문자 hex>`를 한 번만 호출한다. 명시적 서버 교체 정책은 preview와 apply 양쪽에 `--replace-existing`을 함께 준다. `cancel`은 Engine에 confirmed=false를 전달하고 정확한 확인 거부를 검사한다. `stale`은 적용 가능한 preview와 다른 fingerprint를 받아 Engine의 정확한 stale 거부를 검사한다. 실제 패키지 bytes가 preview 후 바뀌는 별도 시험에는 원 fingerprint로 apply를 호출하고 실제 거부와 daemon 부작용 부재를 외부 관측한다. harness가 fixture 파일을 변조하지 않는다.

종료 코드 0은 해당 관측 또는 예정한 거부가 성립했다는 뜻이다. preview의 `canApply=false`를 적용 성공으로 읽지 않는다. apply reply의 unknown/실패는 코드1이며 재시도하지 않는다. 옵션·경로·환경·보고서 생성 거부는 코드2다. 보고서의 `observationCompleted`와 `actionAccepted`, 실제 preview/reply/error를 함께 읽는다. 중단돼 미완성인 보고서는 통과가 아니다.

보고서는 Desktop·선택 실행본의 경로/SHA, 관측자 PID, preview/실제 reply와 선택 실행본의 전후 SHA를 남긴다. `observerPid`는 Desktop 자신의 PID다. 실제 daemon PID·생성시각·이미지/SHA·endpoint 일치는 CLI 담당자의 own fixture 관측과 추가 결속해야 한다. 이 진입점만으로 TUI 재첨부, MCP 도구 호출, busy draft 보존, context/memory 게시, 전체 IT01–05를 통과시키지 않는다.

공유 daemon은 기존 앱 Job에서 명시적으로 분리될 수 있다. `appJobProcesses`는 daemon 회수 증거가 아니다. 호출자가 별도의 own Job/HOME 소유권과 종료 절차를 관리하며 운영·기존 GUI는 건드리지 않는다. Desktop은 이 진입점에서 세션을 만들거나 daemon 종료 명령을 별도로 보내지 않는다. 설치·공개 배포는 이 작업에 포함하지 않는다.
