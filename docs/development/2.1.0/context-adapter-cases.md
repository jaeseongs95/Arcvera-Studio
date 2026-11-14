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
| CA09 | Full dialog at1440px/360px with25 fragments | Actual desktop width exceeds700px; no narrow horizontal overflow; close remains reachable before/after scrolling |
| CA10 | Candidate optional requestSizes absent/null/zero/observation; false sum/unsafe quantity | Unknown is not zero; preserve exact revision strings and separate observation time; validate only the four JSON components and exclude bodies |

Run Native memory-WebSocket/projection cases and existing conversation checks after defining them. Panel fixtures establish display/lifetime behavior only. Real Core 1.1 calls with the same candidate pair and final WebView/package verification are **NOT_RUN**. Reconcile candidate changes with generated schemas.

## Preparation evidence

- `context-composition-native-01`: 6 Native memory-WebSocket/projection cases passed, none failed/ignored, Cargo0. These cover the exact read, structured errors, target/snapshot mismatch, integers, nulls and limits. They do not call Core.
- `context-composition-ui-01`: 38 source fixture cases passed in 55.2s, exit0: panel14, parent3, existing context21. Parent cases cover thread binding, composition independent of size support, and a late reply after closing/reopening. These do not call Native IPC or Core.
- `tsc --noEmit`: exit0.
- The local existing-conversation Native regression stopped at the resource-floor preflight before Cargo started: **NOT_RUN**. Safety floors remain unchanged. Exact-commit CI also checks existing conversation and non-test adapter paths.

Evidence: `D:/codex/build/desktop210-integration-20261114` and the source fixture tool console. Core 1.1 availability, executable verification, same-pair integration and final packaging remain incomplete.

The subsequent Core candidate adds `requestSizes` for the last prepared prompt's four JSON components. Its scope/provenance are `prepared_sampling_prompt`/`utf8_json_components_before_provider_normalization`, distinct from transport envelopes, provider normalization, delta bytes and billed tokens. Different prepared revisions are not promoted to the current snapshot; bodies are excluded. The three new CA10 Native tests are NOT_RUN at authoring.

Subsequent parent execution: `request-sizes-native-01` passed9 Native memory-WebSocket/projection cases including the three additions, with0 failures/ignored and Cargo0. `artifacts/request-sizes-ui-01` passed57 in1.1m, exit0: panel20, parent context25, conversation integration12; tsc exited0. These are source/IPC and memory fixtures. Actual Core1.1 values, sums and observation revisions remain unverified.
