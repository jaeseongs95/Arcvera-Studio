# Native context composition consumption cases

[한국어](context-adapter-cases.ko.md)

This is preparation against the Core 1.1 `ThreadContext*` candidate. Generated schemas, compilation evidence and a development executable have not yet been received. This document does not establish API availability or release readiness.

The context icon will show the selected conversation's fragments, serialized sizes, estimated tokens, cache observations and TTL provenance. Existing size settings and compaction remain available. No other session's raw conversation or authentication data is read.

| ID | Boundary | Oracle |
|---|---|---|
| CA01 | Native-selected thread, first page | One `thread/context/read`, limit25; no body or mutation RPC |
| CA02 | Subsequent cursor/snapshot; mismatched response | No later page without snapshot; reject different thread/snapshot |
| CA03 | -32601, -32600/data.reason stale, other error | unsupported/stale/unavailable; no raw error or retry |
| CA04 | Null, zero, future status string | Preserve null/zero; the display labels unknown states |
| CA05 | u64 identities and quantity precision | Decimal-string identities; reject unsafe quantities |
| CA06 | 64KiB, page25, malformed used fields | Enforce bounds; never forward unused fields |
| CA07 | Refresh, paging, close, thread switch, late reply | Native generation/thread binding and view epoch isolation |
| CA08 | Failure, cancellation and termination | Reuse deadline/proxy cleanup/settings mutex; no GUI lease or server auto-start |

Run Native memory-WebSocket/projection cases and existing conversation checks after defining them. Panel fixtures establish display/lifetime behavior only. Real Core 1.1 calls with the same candidate pair and final WebView/package verification are **NOT_RUN**. Reconcile candidate changes with generated schemas.

## Preparation evidence

- `context-composition-native-01`: 6 Native memory-WebSocket/projection cases passed, none failed/ignored, Cargo0. These cover the exact read, structured errors, target/snapshot mismatch, integers, nulls and limits. They do not call Core.
- `context-composition-ui-01`: 38 source fixture cases passed in 55.2s, exit0: panel14, parent3, existing context21. Parent cases cover thread binding, composition independent of size support, and a late reply after closing/reopening. These do not call Native IPC or Core.
- `tsc --noEmit`: exit0.
- The local existing-conversation Native regression stopped at the resource-floor preflight before Cargo started: **NOT_RUN**. Safety floors remain unchanged. Exact-commit CI also checks existing conversation and non-test adapter paths.

Evidence: `D:/codex/build/desktop210-integration-20261114` and the source fixture tool console. Core 1.1 availability, executable verification, same-pair integration and final packaging remain incomplete.
