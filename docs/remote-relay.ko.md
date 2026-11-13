# OpenDIY Remote 연결

## 2.0.0 이후 AGPL 소스 제공 / Source offer from 2.0.0

2.0.0과 이후 모든 버전은 AGPL-3.0-or-later입니다. 중계기는 원격 이용자 모두가
볼 수 있는 위치에 **Arcvera-Studio — AGPL — 소스 다운로드**를 표시해야 합니다.
호스트의 `X-Arcvera-License`와 `X-Arcvera-Source-Url` HTTP 헤더에서 값을 받거나,
원격 허용 목록에 `software_license`를 추가하여 같은 이름의 읽기 전용 명령 결과를
조회하세요. `sourceUrl`은 실행 중인 버전의 대응 소스 압축 파일입니다.
헤더·명령을 받기만 해서는 화면 안내가 되지 않습니다. 공개 전 실제 중계기 UI에서
로그인한 이용자가 링크를 볼 수 있고 해당 소스를 다운로드할 수 있는지 확인해야 합니다.
기존 중계기에는 이 UI가 없을 수 있으므로 중계기 구현을 갱신해야 합니다.

Version 2.0.0 and every later version use AGPL-3.0-or-later. A relay must
prominently show **Arcvera-Studio — AGPL — Download source** to every remote user.
Read `X-Arcvera-License` and `X-Arcvera-Source-Url` on the host's HTTP connection,
or allow and invoke the read-only `software_license` command. Display its
`sourceUrl` as a visible link. Receiving metadata alone does not display an offer.
Update older relays and verify the visible link and matching download before
publication. See [SOURCE.md](../SOURCE.md).

Desktop 1.7.0의 설정에서 **OpenDIY Remote 연결**을 연다. 별도로 준비한 개인 중계기의 HTTPS 주소와 그 중계기의 HostSecret을 입력하고 연결을 누른다. `wss://`도 HTTPS로 정규화한다. 로컬 시험에는 루프백 HTTP를 사용할 수 있다. 외부 평문 HTTP, URL 사용자 정보, redirect는 허용하지 않는다.

‘중계기 연결됨’은 중계기의 실제 등록 응답을 확인했다는 뜻이다. 원격 브라우저에서 그 중계기의 웹 주소를 열어 브라우저용 토큰 또는 설정된 인증으로 로그인한다. HostSecret과 브라우저 로그인 토큰은 서로 다른 값이다. Desktop은 HostSecret을 파일에 저장하지 않으며, 입력 직후 화면에서 지운다. 앱 재실행 뒤에는 재입력해야 한다.

원격 요청은 이 Desktop의 기존 프로젝트·세션·대화·질문·예약 관리 기능으로 연결된다. 기존 세션 세대와 권한 검사를 유지한다. 원격에서 Desktop 종료, 실행 파일 선택·교체, PC 브라우저 열기는 허용하지 않는다. 경로 선택은 원격 화면에서 입력한 절대 경로를 확인하며 PC에 파일 선택창을 띄우지 않는다.

잘못된 비밀키(401/403), 다른 호스트 점유(409), 인증 잠금(429)은 자동 재시도하지 않는다. 원인을 확인하고 연결 해제 후 다시 연결한다. 일시 네트워크 단절은 같은 호스트 ID로 재접속하되 명령을 자동 재실행하지 않는다. 연결 요청의 IPC 응답이 불명확하면 상태 확인을 누르고 결과를 확인한 뒤 조작한다.

연결 해제는 새 원격 요청과 재접속을 중단한다. 이미 수락한 명령은 완료될 수 있으며, 연결 해제가 세션이나 진행 중 작업의 종료를 의미하지 않는다. 로컬 연결을 닫은 뒤 중계기의 해제 응답까지 확인했는지 별도로 표시한다. 출력 큐 상한이나 프로토콜 오류로 연결이 중단되면 다시 연결하고 세션을 새로고침한다. 최근 요청 캐시는256개/16MiB, 진행 요청은4개/16MiB, 이벤트 대기는4MiB로 제한한다.

이 기능은 기존 세션 도구의 ‘모바일 원격 제어’와 별개다. Codex 자체 모바일 페어링을 켜거나 중계기를 자동 설치하지 않는다. 실제 개인 중계기 접속과 실물 휴대전화 검증은 CI의 격리 서버·브라우저 시험과 구분해 기록한다.
