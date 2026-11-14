# Actual Native composition boundaries

[한국어](native-composition-cases.ko.md)

Use a root-launched separate-identifier, hash-pinned app, own Core HOME and fresh workspace/profile/CDP only. Preserve operational apps and other sessions. Core1.0.1 evidence does not establish1.1 support.

- NC01: Check CI commit/identifier/executable/manifest hashes and actual App identifier/version before Native userdata IPC.
- NC02: Preserve two own real TUI sessions and selected-thread MCP/Hook reads. Bind `contextComposition` to session/generation/expectedThreadId.
- NC03: Explicit unavailable baseline requires supported:false, fixed unsupported/unavailable reason and no snapshot. No invented fragments/zero counts. A subsequent MCP read must succeed, proving the runtime still works.
- NC04: Reject another expectedThreadId. The actual context-icon popup displays unsupported/unconfirmed status and closes normally.
- NC05: Future Core1.1 available mode verifies actual snapshot thread/IDs/u64 strings/page25. Full feature/TTL/tool/hook execution remains separate.
- NC06: Normally close own app/TUIs; stop only the endpoint/version recorded by own Core startup; separately check operational-app identity.

Opt in with `AGENT_GRID_NATIVE_COMPOSITION_EXPECT=available|unavailable` using existing `native-smoke.mjs`/`native-runtime-extensions.mjs`. **NOT_RUN** at authoring; no Core1.1 executable has been received. No model turn, Enter, trust approval or configuration mutation is part of this boundary test.

## Subsequent parent execution

`native-105-core101-context-07/smoke/native-smoke.json`: 13 actual WebView2/Tauri IPC/TUI cases passed, errors0, Node exit0, using source105d625/CI executable a88a0ab8 with own Core1.0.1. Composition returned supported=false/reason=unavailable and no snapshot; another expectedThreadId was rejected. The actual icon popup displayed unconfirmed status and closed normally. The subsequent MCP read still found one tool and both sessions retained their id/generation/thread. This does not establish1.1 API support, model/tool execution, Hook effects or caching.

App exit0/stopReason null, operational PID20076 identity preserved, and normal stop/stopped of the same own Core endpoint/version were separately confirmed. Evidence: `D:/codex/build/desktop210-integration-20261114/native-105-core101-context-07`. Core1.1 available mode(NC05) remains unverified.
