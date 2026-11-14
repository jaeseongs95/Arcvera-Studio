[English](runtime-extension-cases.md) | [한국어](runtime-extension-cases.ko.md)

# Current-conversation MCP runtime adapter pre-execution cases

## Scope and authoritative sources

Request MCP configuration reload and one page of server status through Desktop 2.1.0's existing conversation connection. This adds no new process, daemon operation, configuration-file change, OAuth login, or arbitrary RPC. The caller's existing ConversationConnection boundary owns session ownership, generation, executable selection, connection deadlines, watch, and shutdown cleanup.

The pre-implementation baseline is the following source in Core `d6026cbd6157fa18d966031792c537ac383377ac`:

- `codex-rs/app-server-protocol/src/protocol/v2/mcp.rs:51–109`: status requests/responses, nullable runtimeStatus, toolsError, and cursor.
- The same file, `288–309`: optional thread for reload and reconnectQueuedForThreadId. This response confirms only queue acceptance by the captured runtime; it does not guarantee readiness, current registry membership, or cleanup of previous clients.
- `codex-rs/app-server/src/request_processors/mcp_processor.rs:174–215`: capture the loaded thread before reload, recheck the registry, then enqueue reconnect.
- The same file, `485–509,604–703`: when both threadId and serverName are present, use that thread's server connection. A status page contains servers, not a page of tools.
- `codex-rs/app-server-protocol/src/protocol/v2/tests.rs:2836–2905,4132–4161`: serialization contracts for legacy responses without runtimeStatus and empty reload responses.

## Requirements and invariants

| ID | Requirement and decision boundary |
| --- | --- |
| RX1 | Both RPCs explicitly include the exact threadId validated by the caller. Status also explicitly includes the exact serverName. If the response contains threadId, require the same value. Do not return other or duplicate servers. |
| RX2 | Send reload once only. Only the exact reconnectQueuedForThreadId yields outcome=queued. Legacy `{}` and another thread's ACK are unconfirmed; never promote them to readiness, tool discovery, or completed cleanup. |
| RX3 | Missing/null runtimeStatus is unknown. A string toolsError yields toolsState=failed without its body. Missing toolsError or missing tools means unknown. Only explicit toolsError=null with an empty tools object is displayed as an empty catalog. Never infer runtime connection success from these conditions. |
| RX4 | Return only fixed operation, threadId, serverName, found, connection enum, auth enum, toolsState, toolNames, and nextCursor to the renderer. Do not return arbitrary RPC errors/server diagnostics, environment variables, headers, OAuth, URLs, tool schemas, or descriptions. |
| RX5 | Allow one status page and one server only. Do not traverse cursors automatically. Do not automatically resend after cancellation, disconnection, or a lost response. |
| RX6 | Reject exceeded byte/row/nesting limits on consumed input/response fields and the narrow output DTO; do not produce truncated success responses. Unused raw descriptions/schemas are accepted within the existing Rpc 8 MiB transport boundary and are not returned. |

The request API is `RuntimeExtensionRequest::{ReloadMcp {}, McpStatus { server_name, cursor }}`, pre-connection `validate(thread_id, &request)`, and `exchange(&mut Rpc<S>, thread_id, &request)`. exchange invokes the same validate function, so the same checks occur before transmission even if the caller skips prevalidation. The enum's serde operations are `reloadMcp`/`mcpStatus`; unknown request fields are rejected. ReloadMcp is an empty struct variant to avoid Serde's boundary where an internally tagged unit variant ignores extra fields.

The exact wire is below. Omit the cursor key when no cursor is supplied.

```json
{"method":"config/mcpServer/reload","params":{"threadId":"thread-a"}}
{"method":"mcpServerStatus/list","params":{"threadId":"thread-a","serverName":"server-a","detail":"toolsAndAuthOnly","limit":1,"cursor":"1"}}
```

The adapter output DTO ceiling is 1 MiB of JSON, 256 elements per ordinary array, 512 fields per object, 32 nesting levels, 8192 JSON values in total, 8192 bytes per string, and 256 bytes per object key. Consumed raw fields are limited to 1 data row and 256 tools; validate the exact thread/server, enums, and equality of tool map keys with Tool.name. Separate limits are 256 bytes for thread ID, 128 bytes for server name, 256 bytes for tool name, and 256 bytes for input/output cursors. Lengths are UTF-8 bytes; reject control characters, whitespace-only identifiers, and empty cursors. Existing Rpc checks the full text frame's 8 MiB limit before parsing JSON. The adapter validates targets and consumed fields, constructs a narrow DTO, then checks the 1 MiB and shape limits on that DTO. This does not reduce the transport layer's initial allocation limit.

## Pre-execution cases and oracles

Use `exchange` as the test entry point wherever possible, reusing existing Rust `#[test]` tests and in-memory WebSocket frame fixtures. Without connecting to a real server, parse outbound frames to independently check the exact method/params and send count. Expected fixed DTO fields and enums are specified manually from the authoritative sources and requirements above.

| Case | Requirement | Input/situation | Oracle |
| --- | --- | --- | --- |
| RX01 | RX1,RX2 | Reload ACK targets thread-a | Exact reload params, 1 outbound RPC, outcome=queued. No success fields for ready, cleanup, etc. |
| RX02 | RX2 | Legacy `{}`, ack=null, or another queued thread | unconfirmed; distinguish ackMissing/targetMismatch. Do not return the other thread's string. |
| RX03 | RX1,RX6 | Invalid thread/server/cursor or arbitrary request fields | Reject before transmission; 0 outbound frames. |
| RX04 | RX1,RX5 | server-a status, with/without cursor | Fixed detail=toolsAndAuthOnly, limit=1, threadId/serverName; cursor round trip. No automatic next-page request. |
| RX05 | RX3 | connected, explicit toolsError=null, tools present | runtimeStatus=connected, toolsState=available; return names only. |
| RX06 | RX3 | connected, explicit toolsError=null, empty tools | toolsState=empty; preserve the separate connection enum. |
| RX07 | RX3 | Missing/null runtimeStatus, missing toolsError, or missing tools | Relevant value is unknown. Do not normalize to a valid empty result or ready. |
| RX08 | RX3,RX4 | Secret sentinel in toolsError; failed/authenticationRequired/starting/disabled, etc. | No raw text exposed; preserve enum; toolsState=failed. Do not turn a tool error into a successful empty list. |
| RX09 | RX1,RX6 | Malformed data/row/enum/tool value, duplicate/excess servers, another server, or another threadId in response | Fixed safe error. No partially valid result. |
| RX10 | RX5,RX6 | nextCursor, next manual request, repeated cursor | One page per request. A nextCursor equal to the current cursor is an error. Do not create unbounded traversal across calls. |
| RX11 | RX4,RX6 | Consumed tool count/name UTF-8 limits exceeded; or the output DTO limiter's own 1 MiB/string/array/field/nesting/value-count limits exceeded | Failure without truncation. Output-limiter cases directly test bounded_response; do not use them as rejection oracles for unused raw data. |
| RX12 | RX2,RX4,RX5 | RPC error or read disconnection after sending reload | Fixed safe error/unknown outcome; 1 outbound RPC. No automatic retry. |
| RX13 | RX4 | Sentinel in env/header/OAuth/URL query/server diagnostics/raw tool schema | No sentinel in success DTO or error strings. Project no arbitrary raw content beyond names and opaque cursors. |
| RX14 | RX1,RX3 | data=[] | found=false; connection/auth/tools all unknown. Do not report 0 tools as a valid catalog. |
| RX15 | RX4,RX6 | One valid server/tool with unused description >8192B, inputSchema enum >256 elements, schema >512 fields and >8192 values, and total raw >1 MiB but within 8 MiB | exchange succeeds; return only exact name/state DTO, without description/schema/sentinel. 1 outbound frame. |
| RX16 | RX4,RX6 | RX15's large unused data combined with invalid thread/server/cursor/tool name/enum/tool count | Preserve existing consumed-field rejections; safe error; 1 outbound frame. Raw size must not bypass target/name validation. |
| RX17 | RX4,RX6 | Raw RPC text frame >8 MiB, including unused description | Existing Rpc::read rejects before JSON parsing; adapter returns fixed safe error; 1 outbound frame. Transport ceiling unchanged. |

Names and cursors are explicitly permitted protocol data. If an arbitrary server places a secret in these fields, string content alone cannot identify it as a secret. The adapter guarantees non-disclosure of raw content outside the permitted fields.

## 2026-11-14 pre-change plan for the projection boundary

The tools field in Core `mcp.rs:13,93` is a map of original `McpTool` values. `ToolsAndAuthOnly` does not promise names only, so long unused descriptions/inputSchemas are valid input. Previously, exchange first ran `bounded_response(raw)`, incorrectly rejecting this data under the output DTO's 8192B string, 256-element array, 512-field, and 8192-value limits. Do not solve this by exposing raw data to the renderer or increasing the transport ceiling. Apply the same limiter only to the DTO after same_thread and status/reload projection.

CS invariants are (1) preserve Rpc's 8 MiB text-frame boundary, (2) preserve consumed-field and exact-target validation, (3) return only a bounded permitted DTO to the renderer, and (4) one request/one response with no retry. Unused schemas need no reserialization or traversal; raw JSON parsing cost remains within the existing Rpc boundary. Performance/global memory improvements are not claimed because they were not measured.

Keep the existing Rust inline MemoryStream fixture and runner. Exercise RX15–17 through exchange, and preserve RX11's tool-count/name rejection checks through exchange. Separate previous raw-size rejection cases for unused diagnostic/ignored data into direct output-limiter tests. State explicitly that the limiter's general shape cases are defensive boundaries unreachable through the current fixed DTO. This oracle change follows the separation of the Core tool source contract from the narrow DTO; it does not turn consumed-field rejections into passes.

Pre-change source/docs were preserved at `D:/codex/build/desktop210-integration-20261114/runtime-native-projection-boundary/before/`. The parent collects RED/GREEN by running the same post-change RX15 regression-test bytes against an isolated copy of the previous product code and the fixed candidate. Do not roll back the original worktree. This plan is saved before code changes; build/test/check-plan are NOT_RUN in this subtask. Earlier execution evidence in this document applies only to earlier candidates.

## Verification status and limitations

The cases document was saved before the product adapter and inline fixtures were authored. Build/test/check-plan execution is prohibited in this subtask, so those checks are NOT_RUN. The parent must run `cargo test --manifest-path src-tauri/Cargo.toml runtime_extensions` serially and associate actual output with the final candidate digest. No separate test runner or dependencies are added.

Parent execution report: RuntimeNative03 compiled adapter SHA256 `26100D4A5D1B0E19DD197991F42F2FAAF89DAACC9DBF0FA5DA967724CD33724A`, then reported 10 PASS/1 FAIL out of 11, Cargo exit 101. Originals are preserved at `D:/codex/build/desktop210-integration-20261114/runtime-native-03/`. The failing assertion required rejection of `{operation:"reloadMcp",method:"arbitrary"}` during deserialization, catching the defect where an internally tagged unit variant accepted extra fields. The expected behavior was retained and ReloadMcp was changed to an empty struct variant. Build/retest of this fix candidate is still NOT_RUN.

Fixtures provide unit evidence for RPC framing, request binding, response meaning, secret projection, and prohibition of retransmission. They do not prove actual Core build, networking, MCP handshake/tool discovery, UI, live-thread generation/deadline/watch/cleanup, release of previous clients, or preservation of active tool calls. These integration boundaries require separate parent verification; fixture passes do not replace it.

# Parent integration and execution evidence

Define UI integration cases before execution. Management of saved configuration and runtime requests bound to native session ID/generation/expectedThreadId are separate paths. (1) Reconnection distinguishes review/acceptance from readiness and sends no config mutation. (2) Inspect only the specific server. (3) After closing the window and opening another conversation, do not display late responses; keep tracking existing requests. (4) App exit drains accepted runtime requests once. Synthetic IPC tests do not prove actual Core readiness/tool calls or network-resource reclamation; perform native/live tests separately.

Additional UI boundary: Refreshing only the configuration list or changing language for the same conversation/server preserves the last runtime inspection data and list position. Dispose the existing panel when server/conversation/generation/window changes. Distinguish discovered tool names from successful tool execution. During browser-fixture visual inspection, separate configuration edit/delete controls from runtime inspection controls, and place inspection controls before long tool lists.

The parent uses native session ID/generation lookup in `conversation_request` and the actual `SessionInfo.threadId`. The renderer's expectedThreadId is not authority; it detects a changed review target. Reject a mismatch with the native value before opening a proxy. Use the same ConversationConnection endpoint and existing deadline/watch/cleanup, and serialize metadata requests within a bound using the existing session lock. Do not acquire new GUI/CLI input ownership or restart the terminal. Manual reload requires observed idle state, no pending questions/approvals, and a confirmed connection; explicitly block unknown/active states. Confirmation of this consumer policy is requested from the CLI owner.

The 10 PASS/1 FAIL (Cargo101) in `runtime-native-03` concern the unit enum variant allowing extra fields. The failing input/rejection assertion was retained and the variant changed to an empty struct variant. Rust variant-construction expressions also changed, so do not describe the complete files as byte-identical RED/GREEN. `runtime-native-04`, including required expectedThreadId and rejection of extra method/threadId/cwd fields in the new IPC, reported 12 PASS, 0 failures/ignored, Cargo0. Raw results and source-sha256 are at `D:/codex/build/desktop210-integration-20261114/runtime-native-04/`. A public-request rejection assertion for stale expectedThreadId was strengthened afterward; that additional boundary remains NOT_RUN until the next execution. This does not establish actual Core/network/GUI success.

The subsequent `conversation-native-05` included that public-request rejection assertion for stale expectedThreadId and reported 124 PASS, 0 failures/ignored, Cargo0. The valid large unused-description/schema counterexample was actually reproduced as 1 FAIL/Cargo101 in `projection-red-06`, using the previous handler and the same new inline test. The original working code was not rolled back; execution used only a local `cfg(test)` historical module, and its temporary module registration was removed after execution. The new code's `projection-green-07` reported 16 PASS, 0 failures/ignored, Cargo0. The inline test bodies in before-with-regression and after have the same SHA256, `4aef86558e3b91ffd2debd5bffff47bd1ac0a0bd9d1b5352630e8e25d45da5ec`. Do not describe this as byte identity of the complete product files or verification of an actual Core service.

Raw paths are `D:/codex/build/desktop210-integration-20261114/{conversation-native-05,projection-red-06,projection-green-07}/`. The RED source is preserved at `runtime-native-projection-boundary/before-with-regression.rs`. The bulk source-copy/after-snapshot shell command was rejected by AGS because it could not verify the paths and was not executed. Instead, the counterexample ran through an explicit `cfg(test)` historical module in the existing test runner, and source/test digests were directly compared. The 8 MiB transport ceiling and consumed-field scope checks were preserved. Do not hide existing Rust warnings or report zero warnings.
