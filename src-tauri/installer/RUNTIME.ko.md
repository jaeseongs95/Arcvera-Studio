# Windows 필수 구성 요소

[English](RUNTIME.md)

Arcvera-Studio 패키지는 Microsoft 런타임 DLL과 설치기를 동봉하지 않습니다.
앱과 설치 프로그램이 해당 구성 요소를 자동으로 다운로드·설치·갱신하지 않습니다.
필요한 구성 요소는 Microsoft 공식 경로에서 직접 받아 설치하세요.

- **Desktop:** 창을 표시하려면 Microsoft Edge WebView2 Evergreen Runtime이 필요합니다.
  Microsoft의 [WebView2 다운로드 안내](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)를 이용하세요.
  WebView2가 없거나 설정된 최소 버전보다 오래된 경우 Desktop 설치 프로그램은
  업데이트를 포함해 설치 변경 전에 중단합니다. 런타임을 직접 설치하거나 갱신한 뒤
  Desktop 설치 프로그램을 다시 실행하세요. 포터블 버전도 실행 전에 같은 구성 요소가 필요합니다.
- **Core 음성 도우미:** 음성 초기화에서 `VCRUNTIME140.dll` 등의 Visual C++ 런타임
  누락을 보고하면 Microsoft의
  [지원되는 Visual C++ Redistributable 안내](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)를 이용하세요.
  x64 Core 도우미에는 x64 패키지를 선택하고, 선택한 Core 패키지의 `RUNTIME.md`에
  명시된 버전 조건을 따르세요. 비공식 다운로드 사이트나 다른 설치본에서 DLL을 복사하지 마세요.
  Core는 별도로 설치합니다.

기존 Windows 런타임의 갱신은 해당 공급자의 정책을 따릅니다. 이 배포 정책은 기존
런타임·사용자 설정·인증정보를 삭제하지 않습니다. 재배포 가능한 오픈소스 구성 요소와
그 라이선스 고지는 각 패키지에 유지합니다.

이 릴리스의 로컬 fixture 시험은 런타임이 없는 새 Windows 환경, Desktop 전체 설치와
해당 환경의 Core 음성 기능을 검증하지 않았습니다. 시험 범위는 릴리스노트를 참고하세요.
