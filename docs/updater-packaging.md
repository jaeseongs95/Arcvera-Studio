# Windows updater 패키지 준비

## English

The renamed Arcvera-Studio client consumes `arcvera-latest.json` and accepts only
the canonical `jaeseongs95/Arcvera-Studio` release URL. The same package also
contains `latest.json` for existing OpenDIY Desktop clients, whose strict URL
validator requires the previous repository name. Its old URL relies on GitHub's
repository-rename redirect and points to the identical installer and signature.
Do not reuse the old repository name. If that redirect stops working, use the
approved manual installation path rather than weakening the client validator.

Set the repository variable `ARCVERA_STUDIO_UPDATER_PUBLIC_KEY`; the existing
`OPENDIY_UPDATER_PUBLIC_KEY` remains a compatibility alias when the new variable
is absent. Private-key secret names remain unchanged. A signed updater package
has five files: the installer, its signature, both manifests and `SHA256SUMS.txt`
with four entries. Publish all five together only after separate release approval.
Normal push CI does not enable signing. Fixture tests do not verify a real
signature, installation or restart. See [rename compatibility](rebranding.md).

## 한국어

기존 push/PR CI는 일반 NSIS·포터블 빌드를 유지한다. 수동 workflow 실행에서 `updater_package=true`를 선택한 경우에만 updater 서명 패키지를 만든다. 자동 릴리스 게시·태그·운영 설치는 수행하지 않는다. 현재 시험 후보 `0ec19d8eb52c77b0fd1e901d9fcbece0c3f553a0`은 보존한다.

## 사전 검증 사례

| ID | 전제·입력 | 절차 | 기대 결과·실패 기준 | 실제 |
|---|---|---|---|---|
| UP01 | 기본 push/PR 또는 수동 옵션 false | 기존 CI 확인 | 일반 build 명령 유지, updater signing secret 참조 단계는 실행하지 않음 | NOT_RUN |
| UP02 | 수동 옵션 true, 공개키 또는 개인키 미입력 | CI runner의 서명 빌드 단계 | 누락 항목 이름만 표시하고 빌드 전 명시 실패. 값 출력·일반빌드 fallback 금지 | NOT_RUN |
| UP03 | 정확 현재 버전 NSIS 1개와 같은 파일의 .sig | manifest 생성기 실행 | NSIS·.sig bytes 보존, 새 저장소의 arcvera-latest.json과 기존 클라이언트용 latest.json 생성, 두 manifest의 서명·설치 파일 동일 | PASS (Node 파일 fixture 1개, 실제 서명 아님) |
| UP04 | NSIS 없음/복수/다른 버전, .sig 없음/공백/과대 | 같은 생성기 실행 | 명확 오류, updater 출력 생성 안 함 | PASS (Node 반례 7개, 빈 installer 포함) |
| UP05 | package와 Tauri 버전 불일치 또는 prerelease/경로형 버전 | 같은 생성기 실행 | manifest 생성 거부, 외부 URL·다른 tag 사용 불가 | PASS (Node 반례 3개) |
| UP06 | 기존 updater 키 쌍을 사용자 CI 설정에 입력 | 수동 옵션 true로 기존 Windows CI | 키를 내장한 exe와 NSIS+.sig/latest.json 산출, 모든 기존 검사 통과; 로컬 fixture만으로 실제 서명 성공 판정 금지 | NOT_RUN |
| UP07 | 키 내장 구버전·더 높은 버전의 실제 서명 NSIS/manifest, 격리 Windows TEST | 실제 확인→예→서명 검증→설치·재시작; 잘못된 서명도 대조 | 정확 새 버전 설치·재시작과 서명 실패 거부 확인. 별도 설치 승인 필요 | NOT_RUN |

UP03~05의 Node 사례는 manifest/파일 결속만 검증한다. 실제 서명 키·Windows 설치·GitHub 배포·업데이트 활성화 검증을 대체하지 않는다. private key/암호/secret 값은 테스트에 사용하지 않는다.

회귀 민감도는 격리 사본에서 manifest의 고정 저장소만 다른 저장소로 바꾼 표적 mutation으로 확인한다. 같은 테스트/명령의 UP03이 정확 URL 불일치로 RED여야 하며 실제 생성기 사본으로 복원한 뒤 GREEN이어야 한다. 없는 구현의 import 실패를 RED로 세지 않는다.

## 사용자가 입력할 CI 설정

저장소 Settings → Secrets and variables → Actions에서 아래 이름을 설정한다. 이미 보유한 updater 키 쌍을 사용한다. 이 작업은 키를 생성하거나 secret 존재/값을 조회하지 않는다. 개인키나 암호를 대화·저장소·명령 인자에 넣지 않는다.

| 종류 | 이름 | 내용 |
|---|---|---|
| Repository variable | `ARCVERA_STUDIO_UPDATER_PUBLIC_KEY` | updater 공개키 파일의 내용. 새 이름이며 기존 `OPENDIY_UPDATER_PUBLIC_KEY`는 새 변수가 없을 때 호환용으로 사용 |
| Repository secret | `TAURI_SIGNING_PRIVATE_KEY` | 위 공개키와 짝이 되는 기존 updater 개인키 내용. CI runner 빌드 단계에서만 참조 |
| Repository secret, 키가 암호화된 경우 | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 기존 키 암호. 암호가 없는 키는 미설정 가능 |

수동 실행 시 Windows validation and package workflow에서 대상 작업 브랜치를 선택하고 `updater_package=true`를 선택한다. GitHub는 기본 브랜치에 해당 workflow_dispatch workflow가 존재해야 한다. 대상 ref와 입력을 명시하려면 `gh workflow run windows.yml --repo jaeseongs95/Arcvera-Studio --ref feat/desktop-updater-signing-ci-20261107 -f updater_package=true`를 사용한다. 이 명령은 사용자 CI 설정 후 root가 실행할 명령이며 여기서 실행하지 않는다. 이번 작업은 기본 브랜치를 병합하지 않는다. push/PR 또는 옵션 false는 기존 일반 CI 경로다. 옵션 true에서 공개키/개인키가 없으면 명시 실패하며 일반 패키지로 대체하지 않는다.

공개키는 native의 build-time `ARCVERA_STUDIO_UPDATER_PUBLIC_KEY`와 Tauri CLI의 `plugins.updater.pubkey` 양쪽에 같은 값으로 주입한다. native는 새 변수가 없을 때 기존 `OPENDIY_UPDATER_PUBLIC_KEY`를 읽으며 CI도 기존 repository variable을 호환한다. 공개키와 createUpdaterArtifacts=true만 runner 임시 JSON에 쓴다. 개인키/암호는 파일에 쓰지 않는다. Tauri CLI 2.12.4는 키 쌍 불일치를 경고만 하므로 이 모드는 해당 경고를 실패로 처리한다. 실제 서명 검증·설치는 UP07의 별도 근거가 필요하다.

## 산출물과 승인된 배포의 연결

일반 산출물 `arcvera-studio-windows-x64` 안의 기존 exe·포터블 ZIP·NSIS·루트 SHA256SUMS는 유지한다. 옵션 true일 때만 추가로 아래 하위 폴더를 만든다.

```text
artifacts/updater/
  Arcvera-Studio-<version>-windows-x64-setup.exe
  Arcvera-Studio-<version>-windows-x64-setup.exe.sig
  arcvera-latest.json
  latest.json
  SHA256SUMS.txt
```

생성기는 bundle/nsis의 정확 `<productName>_<version>_x64-setup.exe` 한 개와 그 파일 이름에 `.sig`를 붙인 쌍만 받는다. package.json과 tauri.conf.json은 같은 stable x.y.z여야 한다. installer와 sig는 bytes를 그대로 복사하고 manifest에는 sig 파일의 서명 내용을 넣는다. 출력 폴더가 이미 있으면 보존하고 실패한다. 생성기는 개인키를 받거나 서명을 생성·검증하지 않는다.

승인된 릴리스 단계에서는 `artifacts/updater/`의 다섯 파일을 **같은** GitHub 릴리스 `v<version>`의 자산으로 게시해야 한다. 새 앱용 `arcvera-latest.json` URL은 `https://github.com/jaeseongs95/Arcvera-Studio/releases/download/v<version>/Arcvera-Studio-<version>-windows-x64-setup.exe`로 고정되고 플랫폼은 windows-x86_64다. 기존 앱용 `latest.json`은 같은 자산의 `jaeseongs95/opendiy-desktop` URL을 사용하여 기존 클라이언트의 엄격한 URL 검사를 유지한다. 이미 변경된 GitHub 저장소의 redirect에 의존하므로 이전 저장소 이름을 재사용하면 안 된다. redirect가 사라지면 수동 설치 경로를 사용하고 URL 검사 완화로 우회하지 않는다. 이 하위 SHA256SUMS는 installer·signature·두 manifest의 네 항목이며 기존 루트 SHA256SUMS는 일반 파일용이다. 루트 NSIS와 updater NSIS는 같은 빌드 파일의 복사본이다. 이 CI는 Actions artifact 업로드만 하며 release 게시 권한·동작을 추가하지 않는다.

앱은 GitHub stable latest만 조회한다. prerelease 단계 자산 준비와 실제 앱의 stable latest 소비는 별개이므로, prerelease를 앱이 자동 소비한다고 간주하지 않는다. 같은 버전도 새 업데이트로 제공하지 않는다. 키 없이 이미 빌드한 앱은 런타임 설정으로 활성화되지 않으며, 키를 내장한 배포본을 먼저 설치해야 그 이후 더 높은 버전의 서명 업데이트를 받을 수 있다. 이번 1.6.0 키 내장 빌드는 이후 버전을 위한 기준 배포본이며, 기존 키 없는 빌드에 대한 bootstrap 설치는 별도 승인 경계다.

`build-info.json`의 기존 `signed:false`와 문서의 unsigned는 Windows Authenticode 서명을 가리킨다. updater `.sig`는 별도의 payload 신뢰 검증이다. 이 변경은 인증서·Authenticode 서명을 추가하지 않으며 updater 서명이 Windows 평판 경고를 없애지 않는다. Codex 업데이트 설치 계약은 이번 변경 범위 밖이다.

## 검증과 제한

가벼운 검증 명령은 `node --test scripts/updater-manifest.test.mjs`, `node --check scripts/updater-manifest.mjs`, YAML 구조/PowerShell AST 확인과 `git diff --check`다. 실제 Windows 빌드는 기존 CI에서 수행하며 이 PC에서 수행하지 않는다. 키 없는 일반 CI 성공과 manifest fixture 성공은 UP06/UP07 통과가 아니다. 실제 updater 서명·다운로드·설치·재시작은 NOT_RUN을 유지한다.

로컬 결과: Node 11개 PASS(exit0). 별도 사본의 고정 repo URL mutation은 같은 테스트에서 10PASS/1FAIL(exit1), 원본 생성기 사본은 11PASS(exit0)이며 test-proof CONSISTENT다. YAML parser errors0, 기존 native/frontend/Windows 단계 보존 대조, PowerShell run block 23개 AST errors0, Node syntax와 diff whitespace 검사를 수행했다. 이 결과의 scope는 CI 구성과 manifest 생성기이며 실제 CI run 결과는 root의 작업 브랜치 push 이후 확인한다. 원시 결과는 ignored `artifacts/updater-packaging/`에 보존한다.

공식 근거: [Tauri updater 빌드·서명과 static manifest](https://v2.tauri.app/plugin/updater/), 고정 [CLI 2.12.4의 config 병합](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/src/build.rs#L47), [동일 버전을 서명 comment에 묶고 key mismatch를 경고하는 경계](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/src/bundle.rs#L290), [NSIS 이름 생성](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs#L622), [GitHub workflow_dispatch 실행 조건](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow).
