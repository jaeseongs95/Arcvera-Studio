# OpenDIY Windows NSIS handoff patch

Upstream: tauri-plugin-updater 2.14.1, crates.io archive SHA-256
`d957abbcb3f9f6c76d22a3a0f012f30880640d0bccb5634d2b12a9937e4c872b`.
The MIT and Apache-2.0 licenses are retained with the complete packaged source.

`src/updater.rs` changes only the Windows NSIS launch boundary. It passes the
existing NSIS parameters to `CommandExt::raw_arg` and explicitly requests
`CREATE_BREAKAWAY_FROM_JOB`. OpenDIY's app Job already permits explicit
breakaway; ordinary children still inherit its kill-on-close policy. A spawn
error returns without a ShellExecute fallback or process exit. The application
keeps its existing no-op before-exit callback so launch failure preserves UI.
The GUI installer receives null standard streams so it does not retain the
exiting application's redirected pipes.

Signature/version verification, extraction, parameter construction, MSI launch,
and non-Windows installation remain upstream code. The additional Windows
feature exposes the creation flag. A focused missing-file regression checks the
error return. See `docs/updater-job-handoff-cases.ko.md` in the application for
the actual Job/installer tests and the limits of the evidence.
