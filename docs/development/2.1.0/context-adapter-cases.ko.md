# 컨텍스트 구성 Native 소비 사례

Core 1.1의 `ThreadContext*` 타입 후보를 소비하는 준비 단계다. 생성 schema, 컴파일 결과 및 개발 실행물은 아직 수신하지 않았다. 이 문서는 실제 API 지원이나 릴리스 완료 근거가 아니다.

사용자는 현재 대화의 컨텍스트 아이콘에서 조각 선택, 직렬화 크기, 추정 토큰, 캐시 관측과 TTL의 출처를 확인한다. 기존 세션 크기 설정과 압축 기능을 유지한다. 다른 세션의 원문이나 인증 자료를 읽지 않는다.

| ID | 입력·실패 경계 | 판정 |
|---|---|---|
| CA01 | 선택한 Native thread, 첫 페이지 | `thread/context/read` 1회, limit25, 원문·변경 RPC 없음 |
| CA02 | 후속 cursor와 snapshot, 응답 대상 불일치 | snapshot 없이 후속 페이지 전송 금지, thread/snapshot 불일치 거절 |
| CA03 | 미지원 -32601, stale -32600/data.reason, 기타 오류 | unsupported/stale/unavailable 구분, 원문 오류 비노출·재시도0 |
| CA04 | nullable 값과 실제 0, 미래 상태 문자열 | null·0 보존, 미지 문자열은 표시 계층에서 미확인 |
| CA05 | u64 revision/ordinal, 수량 정밀도 | 식별자는 정확한 decimal string, 수량은 safe integer 범위 밖 거절 |
| CA06 | 64KiB, page25, 문자열·잘못된 필드 | 상한 및 사용 필드 검사, 미사용 필드 비전달 |
| CA07 | refresh/다음 페이지/닫기/다른 thread/늦은 응답 | Native 세대·thread 결속, 화면 epoch로 잘못된 대화에 표시하지 않음 |
| CA08 | 연결 실패·취소·종료 | 기존 deadline/proxy 정리·설정 mutex 재사용, GUI lease·서버 자동시작 없음 |
| CA09 | 전체 팝업: 데스크톱 1440px·좁은 360px·긴 25조각 목록 | 넓은 화면에서 실제 폭700px 초과, 좁은 화면 가로 넘침 없음, 스크롤 전후 닫기 버튼 접근·정상 종료 |
| CA10 | 후보 optional requestSizes 없음/null/0/관측 객체·잘못된 합계/정밀도 | 미관측을0으로대체하지않음,두revision exact문자열·독립관측시각 유지,4 JSON성분합계만검증·본문비전달 |

정의 후 Native 메모리 WebSocket/투영 회귀와 기존 conversation 검증을 수행한다. 패널 fixture는 표시·수명만 증명한다. Core 1.1의 같은 후보짝 실제 호출, 최종 WebView/패키지 검증은 별도이며 현재 **NOT_RUN**이다. 후보 변경 시 생성 schema와 대조해 수정한다.

## 준비 단계 실행 근거

- `context-composition-native-01`: Native 메모리 WebSocket/투영 6 PASS, 0 FAIL/ignored, Cargo0. 정확한 read 요청, structured 오류, 대상·snapshot 불일치, 정수·null·상한 검증이다. 실제 Core 호출이 아니다.
- `context-composition-ui-01`: source fixture 38 PASS/55.2초/exit0(패널 14, 부모 연결 3, 기존 컨텍스트 21). 부모의 thread 결속·미지원 설정과 독립 조회·닫기/재열기 늦은 응답을 포함한다. 실제 Native IPC나 Core를 호출한 시험이 아니다.
- `tsc --noEmit`: exit0.
- 기존 대화 전체 Native 회귀는 resource floor 사전 검사에서 중단됐다. Cargo를 시작하지 않았으며 **NOT_RUN**이다. 보호 한도를 낮추지 않는다. 같은 commit의 CI에서도 기존 conversation과 비시험 adapter 경로를 검증한다.

근거 위치: `D:/codex/build/desktop210-integration-20261114` 및 source fixture 실행의 도구 콘솔. Core 1.1 API 지원·실행물·같은 후보짝 실통합·최종 패키지 검증은 계속 미완이다.

Core 후속 후보는 `requestSizes`에 직전 준비 요청의4 JSON성분 bytes를 제공한다. `prepared_sampling_prompt`/`utf8_json_components_before_provider_normalization`이며 전송 봉투·provider 정규화·delta bytes·과금 token과 구분한다. top snapshot과다른준비revision을현재값으로승격하지않고원문은투영하지않는다. CA10 추가 Native3개는 작성시 NOT_RUN이다.

후속 부모 실행: `request-sizes-native-01`은 새3개를 포함한 Native memory-WebSocket/투영9PASS/0FAIL/0ignored/Cargo0이며, `artifacts/request-sizes-ui-01`은 패널20·부모컨텍스트25·대화연결12 합계57PASS/1.1분/exit0, tsc exit0이다. source/IPC와메모리fixture 범위로서 Core1.1 실제 값·합계·관측revision의 확정은 미검증이다.
