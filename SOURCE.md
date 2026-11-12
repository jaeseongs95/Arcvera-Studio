# License and Corresponding Source / 라이선스와 대응 소스

## English

Arcvera-Studio version 2.0.0 and every subsequent version are licensed under
the GNU Affero General Public License, version 3 or any later version
(`AGPL-3.0-or-later`). Copyright (c) 2026 Agent Grid contributors.
You may redistribute and modify this program under those terms. This program
is provided WITHOUT ANY WARRANTY; see [LICENSE](LICENSE).
The original MIT notice remains in [LICENSE-MIT-LEGACY](LICENSE-MIT-LEGACY).
Previously granted MIT rights remain valid. Third-party components retain
their own licenses in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

For every published binary, download `arcvera-studio-<version>-source.zip`
from the **same version's release**, free of charge:
https://github.com/jaeseongs95/Arcvera-Studio/releases
The application's Settings → License and source opens that release.
The source archive contains the exact Git revision (`SOURCE-REVISION`), tracked
source, build and installer scripts, vendored Cargo dependency sources, and
integrity-checked npm dependency archives. `source-manifest.json` binds the
revision and dependency archive hashes. Compare `SHA256SUMS.txt` and the binary's
`build-info.json` with that revision. A moving branch or a different version's
archive is not the Corresponding Source for that binary.

Build on Windows with Node.js/npm, Rust and the MSVC Windows build tools
(general-purpose build tools are not included). In the extracted directory:

```powershell
# Populate npm's content cache from the included, integrity-checked archives.
node scripts/restore-source-dependencies.mjs
npm ci --offline --cache .source-npm-cache
# The source archive includes .cargo/config.toml for its Cargo vendor directory.
$env:CARGO_NET_OFFLINE = 'true'
npm run tauri -- build --config src-tauri/tauri.bundle.conf.json --bundles nsis
```

NSIS/WebView2 tooling may require separate installation or download as described
in the Windows workflow. Updater signing keys are private credentials and are
not included; an ordinary build does not require them. A modified distributor
must update its source links to its own matching source and keep the notices.
AGPL does not by itself license projects merely opened or edited with this app.

When providing remote access through a relay or a modified version, prominently
display **Arcvera-Studio — AGPL — Download source** to every remote user, linking
to the matching release's source archive. Do not hide this behind operator-only
settings. Relay implementations must expose the host's source information in
their user interface; see [remote integration](docs/remote-relay.ko.md).
An operator must keep the source download available for the binary it serves.

## 한국어

Arcvera-Studio 2.0.0과 이후 모든 버전에는 GNU Affero General Public License
제3판 또는 이후 판(`AGPL-3.0-or-later`)을 적용합니다.
Copyright (c) 2026 Agent Grid contributors. 이 조건으로 재배포·수정할 수 있으며,
프로그램은 무보증으로 제공됩니다. [LICENSE](LICENSE)를 참고하세요.
[기존 MIT 고지](LICENSE-MIT-LEGACY)와 이미 부여된 MIT 권리는 유지됩니다.
타사 구성 요소에는 [각자의 라이선스](THIRD_PARTY_NOTICES.md)가 적용됩니다.

공개 실행 파일의 대응 소스는 위 저장소에서 **동일 버전의 릴리스**에 첨부된
`arcvera-studio-<버전>-source.zip`으로 무료 제공합니다.
앱의 설정 → 라이선스·소스에서 해당 릴리스를 열 수 있습니다.
소스에는 정확한 Git 리비전(`SOURCE-REVISION`), 소스 코드, 빌드·설치 스크립트,
Cargo 의존성 원본과 무결성을 확인한 npm 의존성 압축 파일을 포함합니다.
`source-manifest.json`의 리비전·해시를 `SHA256SUMS.txt` 및 실행 파일의
`build-info.json`과 대조하세요. 다른 버전이나 이동하는 브랜치는 대신 사용할 수 없습니다.

Windows에서 Node.js/npm, Rust, MSVC Windows 빌드 도구를 준비하고 위 명령을
실행하세요. 일반 빌드 도구와 NSIS/WebView2 도구는 별도 준비가 필요할 수 있으며
Windows 워크플로를 참고하세요. 개인 서명 키는 포함하지 않으며 일반 빌드에는
필요하지 않습니다. 수정본 배포자는 자신의 정확한 대응 소스로 링크를 변경하고
고지를 보존해야 합니다. 앱에서 열거나 편집한 프로젝트에 AGPL이 자동 적용되지는 않습니다.

중계기나 수정본으로 원격 서비스를 제공할 때는 모든 원격 이용자가 볼 수 있는
위치에 **Arcvera-Studio — AGPL — 소스 다운로드**를 표시하고, 실행 중인 버전의
소스 압축 파일로 연결해야 합니다. 운영자 전용 설정에만 숨겨서는 안 됩니다.
중계기 구현은 호스트의 소스 정보를 사용자 화면에 표시해야 합니다.
제공 중인 실행 파일에 맞는 소스 다운로드를 계속 유지하세요.
