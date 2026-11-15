# Public document format / 공개 문서 형식

## English

The main `README.md` is English and links to `README.ko.md`. Keep the two editions aligned in meaning. Use the existing section order: product summary, language links, release status, download and guide links, labelled preview, runtime compatibility, quick start, workflow guide, limitations, development and licensing.

At each release, update the version, supported runtime, relevant migration steps, verification scope and remaining limitations in both editions. Distinguish the source version from its publication status; GitHub Releases is the publication record. Check it before naming the latest published version. A dated development note is not publication evidence.

Use the entry format at the end of `release-notes.md`: one version entry contains both `### English` and `### 한국어`, covering user-visible changes, compatibility or recovery, actual verification and material limitations. Identify synthetic or isolated checks, and do not present them as authenticated provider use, long-duration stability or installation validation.

## 한국어

기본 `README.md`는 영문으로 작성하고 `README.ko.md`를 연결합니다. 두 문서의 의미를 일치시킵니다. 제품 소개, 언어 링크, 릴리스 상태, 다운로드·가이드 링크, 명시적으로 구분한 미리보기, 실행 환경 호환성, 빠른 시작, 작업별 안내, 제한, 개발·라이선스의 기존 순서를 유지합니다.

릴리스마다 두 문서의 버전, 지원 실행 환경, 필요한 이전 절차, 검증 범위와 남은 제한을 갱신합니다. 소스 버전과 게시 상태를 구분하며 실제 게시 기록은 GitHub Releases에서 확인합니다. 최신 공개 버전을 적기 전에 이를 조회합니다. 날짜가 있는 개발 기록만으로 공개를 주장하지 않습니다.

릴리스 노트는 `release-notes.md` 마지막의 작성 형식을 따릅니다. 한 버전 항목에 `### English`와 `### 한국어`를 함께 두고 사용자에게 보이는 변경, 호환·복구, 실제 검증과 중요한 제한을 기록합니다. 합성·격리 시험을 구분하고 실제 인증 계정의 공급자 사용, 장시간 안정성이나 설치 검증으로 확대하지 않습니다.
