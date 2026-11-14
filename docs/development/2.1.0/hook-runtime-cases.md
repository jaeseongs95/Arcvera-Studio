# Current-thread Hook inventory: existing wire cases

This plan was prepared before fixture and implementation changes. Status: **NOT_RUN**. This unit does not run builds, tests, or a real Core. The parent owns native fixture execution, caller binding, real Core checks, and GUI verification against the same candidate.

## Sources and scope

- Desktop baseline: `50a237e83ce430c7faa7c2994b91f21460283fab`, `src-tauri/src/codex_runtime_extensions.rs`, `codex_hooks.rs`, and `codex_extensions.rs`.
- Core baseline: `d6026cbd`, `HooksListParams`, `HooksListResponse`, `HooksListEntry`, and `HookMetadata` in `app-server-protocol/src/protocol/v2/plugin.rs`; `hooks_list_response` in `app-server/src/request_processors/catalog_processor.rs`.
- The existing public `hooks/list {"threadId":...}` request omits `cwds`, resolves a loaded thread, and returns its configuration snapshot cwd and `thread.list_hooks()`. Its response contains one `data` entry with `cwd/hooks/warnings/errors`. It does not acknowledge a thread ID or execution gate.
- `hooks_to_info` filters builtins, so the inventory is not the complete execution-handler count. New 1.1 `scope/runtime/featureEnabled/baseHandlerCount` metadata is not interpreted; this does not assert that the server omitted it. Configuration inventory, trust, enablement, mutations, models, and process setters are unchanged.

## API and meaning

`RuntimeExtensionRequest::HooksStatus {}` rejects additional fields. `validate` reuses the existing thread ID checks: at most 256 UTF-8 bytes, nonblank, no control characters. `exchange_hooks(rpc, thread_id, cwd)` sends exactly one query on the caller's admitted connection. Passing a Hook request to the existing cwd-free `exchange` fails before sending.

The DTO is `{operation:"hooksStatus",threadId,provenance:"loadedThreadHooksList",executionGate:"unknown",applicationState:"unknown",hooks:{items:[{id,eventName,handlerType,source,enabled,isManaged,trustStatus}],warningCount,errorCount}}`. Items explicitly allow only validated enum/bool fields and a read-only ID: `hook-` followed by the lowercase SHA256 hex digest of the raw key. This ID distinguishes/matches inventory entries; it is neither the raw key nor authority/approval and cannot replace the key required by existing trust mutations. Provenance identifies the request path used, not server acknowledgement of new metadata. The parent binds the expected thread/cwd and rejects stale responses. The existing wire has no thread echo, so one is not fabricated; an explicitly present mismatching top-level `threadId` is rejected.

The existing safe parser validates required fields, enums, duplicates, bounded integers, absolute sourcePath, and the exact cwd. Its complete result is never cloned or serialized for runtime output. Core derives keys from the source and event/declaration position; sourcePath is the declaration file path, and currentHash is a declaration/plugin trust fingerprint. Runtime output needs none of these action identities, so they are excluded. Raw names/action IDs, pluginId, server/tool, command/matcher/statusMessage, diagnostic content/paths, unknown fields, unused prompt/agent bodies, inputs, environment, and authentication content are also excluded. Only warning/error counts are projected. The existing configuration review parser retains its behavior.

## Invariants and limits

- One send, no retries, additional RPCs, new connections, or transport ownership. Neither errors nor empty inventory imply execution success or a gate state.
- Preserve the shared Rpc 8 MiB frame boundary and Hook parser limits: 4096 rows, 128 diagnostics, and existing field limits such as 64 KiB command/matcher/statusMessage. String limits count UTF-8 bytes.
- Do not apply MCP's 256-array/8 KiB-string/1 MiB-DTO limits to Hooks. Large unused content within the shared frame boundary is accepted but not projected. A separate 8 MiB serialization limit bounds the Hook DTO.
- No cache, queue, retry, or concurrency mechanism is added. ACK meaning and finite input boundaries inform this design; actual lifecycle, reordered epochs, and busy/exit checks remain parent integration work.

## Planned cases and oracles

Unless stated otherwise, the entry point is `exchange_hooks`, using existing Rust `#[test]` and an in-memory WebSocket fixture. These fixtures are not real Core, CLI, or network calls and do not prove loaded-thread behavior in a real process.

| ID | Scenario | Independent expected result / failure oracle |
| --- | --- | --- |
| HR01 | Valid command/mcpTool/prompt/agent | Exactly one `hooks/list {threadId}` request, no cwds; inventory/provenance and two unknown states; raw command content removed |
| HR02 | Invalid thread/cwd inputs; cwd-free `exchange` | Zero sends; safe fixed errors; HooksStatus deserialization rejects extra fields |
| HR03 | Other cwd, duplicate/multiple project entries, mismatching top-level thread echo | Failure with no other-project projection; one send |
| HR04 | Missing/malformed data/hooks/warnings/errors, used fields/enums/numbers/absolute paths, duplicate keys | Failure rather than a successful empty inventory |
| HR05 | Legacy empty response | Empty inventory succeeds; executionGate/applicationState remain unknown, without enabled/disabled inference |
| HR06 | 257 and 4096 Hooks | Return complete lists; reject 4097; independent of MCP's array ceiling |
| HR07 | Unused >8 KiB strings/large arrays/wide objects; used command at 64 KiB and excessive UTF-8 bytes | Accept but never expose unused content; validate then remove a boundary-sized command; reject oversized used fields |
| HR08 | Secret sentinel in key/hash/sourcePath/pluginId/server/tool, diagnostics, command/matcher/statusMessage, unknown fields | No sentinel in output or errors; retain diagnostic counts and only a read-only ID plus allowlisted enum/bool fields |
| HR09 | Unsupported, transport, timeout, disconnect, cancellation | Safe error class without raw diagnostics; one send and no automatic retry |
| HR10 | Raw frame >8 MiB; synthetic Hook DTO >8 MiB | Shared Rpc and Hook projection byte boundaries reject respectively; existing MCP contracts/fixtures remain intact |
| HR11 | Requery the same key; different keys with identical event/type/source | Stable ID for the same key, distinct IDs for different keys, agreement with a known SHA256 vector, no raw key |

## Execution handoff and limitations

Changes follow plan → fixtures → implementation. The parent runs `runtime_extensions_hooks_` through the existing Rust test runner and checks MCP regressions, recording commands, results, and candidate hashes. Prepared documentation/fixtures are not evidence of real Core behavior, native GUI operation, gate application, or release completion.
