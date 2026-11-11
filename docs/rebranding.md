# Arcvera-Studio naming and upgrade compatibility

[한국어](rebranding.ko.md) · [Home](../README.md)

The product previously called OpenDIY Desktop is Arcvera-Studio from the 1.8.0 candidate. The npm/Cargo package and Windows executable are `arcvera-studio` and `arcvera-studio.exe`. Source and releases use [jaeseongs95/Arcvera-Studio](https://github.com/jaeseongs95/Arcvera-Studio). This is a development candidate; preparing packages does not install or publish them.

## Existing installations and settings

The stable application identifier remains `io.agentgrid.desktop`. It identifies the existing WebView data and application database; it is not the displayed product name. No database, project, conversation, authentication file or live process is moved by this source change.

The NSIS template retains the existing uninstall registry key ending in `OpenDIY Desktop` and install-location key `Software\agentgrid\OpenDIY Desktop`. Windows displays the new product name. An upgrade restores the prior install directory, and the upstream installer removes the previous main binary using its recorded name. Exact legacy desktop/Start Menu shortcuts are migrated only when they target that installation; conflicting user shortcuts are preserved. A running installed application or an unsuccessful process check aborts the installer instead of terminating work. Existing custom shortcuts/scripts pointing directly to `opendiy-desktop.exe` must be updated to `arcvera-studio.exe` after an approved upgrade. Portable users can select the new executable while keeping their existing application data.

`ARCVERA_STUDIO_WORKSPACE_DIR` and `ARCVERA_STUDIO_CODEX` are the preferred environment names. Existing `AGENT_GRID_WORKSPACE_DIR` and `AGENT_GRID_CODEX` remain aliases. The new name takes priority even when its value is empty or invalid; validation reports the error instead of silently selecting another path. Saved executable preferences still take priority over environment discovery.

The browser language cache reads `arcvera-studio-language` first, then `agent-grid-language` when the new key is absent. New selections write the new key and retain the legacy value. The native database remains the durable language authority. Retaining the previous executable/package and these unchanged data locations provides a rollback path; do not delete application data when uninstalling an old build.

## Updates

The build-time public key is `ARCVERA_STUDIO_UPDATER_PUBLIC_KEY`; the previous `OPENDIY_UPDATER_PUBLIC_KEY` remains an alias. CI uses the existing repository variable when the new one is absent. Existing private signing secrets are unchanged. No keys are regenerated or printed.

New clients read `arcvera-latest.json` and accept only installer URLs under the exact renamed repository and release tag. `latest.json` remains a compatibility manifest with the old repository URL for existing clients whose validator requires that path. GitHub's existing rename redirect leads to the same installer and signature. Publish both manifests, the installer, signature and checksums together only after separate release approval. If the old repository redirect stops resolving to this repository, suspend the compatibility update route and provide the new package manually; do not relax client URL checks.

## Names intentionally retained

OpenDIY Codex and OpenDIY Remote are separate products. Their executable names, capabilities, `opendiy/asyncQuestion`, TUI pipe prefix, launcher query flag and relay headers retain their external contract names. Versioned diagnostic schemas, ownership markers, test environment switches and historical verification documents retain their original names so existing evidence collectors and safety guards remain usable. The attachment cache's existing directory is also preserved. These are compatibility identifiers, not current product branding.

The installer template is based on [Tauri CLI 2.12.4](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi), with its MIT license in `src-tauri/installer/TAURI-LICENSE-MIT`. The local changes preserve upgrade records, add a non-terminating running-app check and create shortcuts only at absent destinations. Existing links, including unreadable links, remain untouched by creation; exact owned legacy targets may still be migrated. Reconcile these changes with upstream before changing the pinned CLI version.

## Validation boundaries

Node packaging tests use synthetic installer/signature bytes; browser tests substitute the native bridge. Windows compilation and headless PTY checks run in CI. They do not verify an actual installation, production WebView session, real CLI history deletion, physical phone control or long-duration stability. Historical PTY failures remain separate investigations and are not resolved by a later successful build.

The Windows CI shortcut regression executes the production NSIS function bodies and migration hook with real COM shortcuts in temporary directories. Its 80 scenarios cover absent, owned, foreign and corrupt links, both names together, configured/root Start Menu locations and finish/passive/update/no-shortcuts/WiX modes. A targeted mutation removes all three creation guards and must fail the same byte-preservation assertions. Shell directory variables are redirected; this does not run the full installer or MUI pages. Concurrent hostile replacement of a checked destination is not covered.
