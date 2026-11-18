# 고정 소스의 Native 개발 실행물

[English](native-fixture-artifact.md)

기존 `3830a87e` 개발 실행물의 원 manifest는 일부 Native 파일만 담았다. 전체 checkout·frontend와 정확한 commit의 결속으로 확대하지 않는다. 추가 Native 검수와 Core1.1 공동 시험에는 CI에서 생성하는 새 실행물과 manifest를 함께 사용한다.

CI는 표적 단위시험과 비시험 `cargo check` 뒤에 현재 checkout의 별도 identifier 개발 앱을 `--debug --no-bundle --ci`로 빌드한다. 설치·서비스 시작·실제 앱 실행은 하지 않는다. identifier는 `fixture.json`의 값으로 실제 앱에서도 검증한다. 운영 제품 identifier와 다르며 같은 fixture를 동시에 실행하지 않는다.

수용 사례:

- AF01: tracked source가 HEAD와 다르면 거절한다. 빌드 전후 검사를 하고 40자리 commit 및 모든 tracked 파일 hash를 보존한다.
- AF02: 앱 hash·identifier·실제 build argv·생성 frontend/notice/config hash를 기록하고 실제 resource map에서 sidecar를 복사한다. checkout/app 밖 경로는 거절한다.
- AF03: 빌드 실패·실행물 누락은 성공 manifest를 만들지 않는다. 기존 app 증거를 덮지 않는다.
- AF04: 압축 해제 후 실행물과 manifest hash를 대조하고 own HOME/프로젝트/profile·고유 CDP port로 실행해 identifier를 확인한다. 실행은 검수/공동시험의 별도 단계다.

artifact `desktop-native-adapter-validation`에는 `fixture.json`, `source-manifest.json`, `generated-manifest.json`, `app/` 및 단위시험 로그가 들어간다. 새 경로의 실제 CI 빌드·AF01~03 실행 결과와 AF04 Native 실행은 작성 시점 **NOT_RUN**이다. CI syntax 검사나 hash만으로 실제 Core1.1 통합·최종 패키지·릴리스 승인을 선언하지 않는다.
