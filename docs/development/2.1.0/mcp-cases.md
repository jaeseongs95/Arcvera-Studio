[English](mcp-cases.md) | [한국어](mcp-cases.ko.md)

# MCP user-base edit/delete verification plan

The targets are the `list`/`edit`/`remove` helpers in `codex_mcp.rs`. Preserve the existing add contract. RPC fixtures are not evidence of actual Core persistence, connection, current-thread reload, or tool discovery. Build/test execution is coordinated serially by the parent and is currently NOT_RUN.

## Contracts and cases

| ID | Requirement / entry point | Input, expected result, and failure criterion | Level / result |
|---|---|---|---|
| E01 | Stale review / edit, remove | 0 writes when the inventory's user-base configVersion differs from the current version | Inline native RPC fixture / NOT_RUN |
| E02 | Exact single-server CAS / edit | `mcp_servers."server.name"`, backend-read filePath, expectedVersion, one replace. Preserve env secrets, unknown fields, and siblings. | Fixture / NOT_RUN |
| E03 | Keep/set/delete / edit | Omitted fields unchanged; args[]/env{} explicitly empty; clearEnv/clearBearerTokenEnvVar delete keys. Reject simultaneous set+clear. | Fixture / NOT_RUN |
| E04 | Transport change / edit | Require STDIO/HTTP command/url; remove only opposite-transport keys rejected by Core; preserve shared/unknown fields. | Fixture / NOT_RUN |
| E05 | Scope / list, edit, remove | Editable only when solely in the user base and matching effective config. 0 writes for project/profile/managed/disabled/duplicate/missing config or effective mismatch. | Fixture / NOT_RUN |
| E06 | Delete / remove | null/replace CAS at the quoted key. Readback confirms absence from user/effective/other layers; preserve siblings. | Fixture / NOT_RUN |
| E07 | Unknown outcome / edit, remove | Fail on CAS conflict/readonly/lost ACK/override/incomplete reply/readback failure or mismatch/reappearance in another layer; no automatic retry; no secrets in errors. | Fixture / NOT_RUN |
| E08 | Existing add / add | Recheck existing inline fixtures and GUI baseline. GUI 56 PASS is a parent report, not execution evidence for this document's new helper tests. | Awaiting parent's serial execution |

Existing runner: `cargo test --manifest-path src-tauri/Cargo.toml codex::extensions::mcp::tests -- --test-threads=1`. Do not add runners, dependencies, or credential stores. Compare fixture write payloads with independent expected values from the specification; verify that siblings are excluded from the write payload. Red/green, mutation, and actual Core integration are NOT_RUN. New helpers do not retroactively establish a defect in the previous implementation. The plan-check CLI is also NOT_RUN because execution is outside the permitted scope.

## Authoritative transport fields and deletion notice

Compared the raw types and actual transport validation at `codex-rs/config/src/mcp_types.rs:391` and `:513` in Core `d6026cbd6157fa18d966031792c537ac383377ac`.

- HTTP→STDIO: Remove `url`, `bearer_token_env_var`, `bearer_token`, `http_headers_helper`, `http_headers`, `env_http_headers`, `oauth`, `oauth_resource`, and `auth`. The review screen must explain removal of existing HTTP authentication, header, and OAuth settings.
- STDIO→HTTP: Remove `command`, `args`, `env`, `env_vars`, and `cwd`. Explain removal of existing execution, argument, environment, and working-directory settings.
- Preserve `scopes`, `environment_id`, timeouts, tool policies, and unknown fields. Within the same transport, preserve fields outside the explicit patch. Do not return raw secrets in metadata, review, or errors.

## IPC integration contract

`McpServerPatch { transport?: McpTransportPatch, enabled?: boolean }`; transport is a `type`-tagged union. STDIO has optional command/args/env and clearEnv; HTTP has optional url/bearerTokenEnvVar and clearBearerTokenEnvVar. Omitted/null optionals preserve values, explicit args[]/env{} empty them, and clear flags delete keys; simultaneous set+clear is rejected. A transport change must explicitly supply the new command/url.

`edit(directory,name,expected_version,&patch,rpc)` and `remove(directory,name,expected_version,rpc)` verify persistence and exact readback only. `list` returns configVersion and editable. The renderer does not specify raw keyPath/filePath. Current-thread reload/status is a separate parent-owned implementation.

## UI cases (defined before implementation/execution)

- U01: The edit screen for a user-only server neither returns nor fills existing protected values. Blank fields preserve values; only explicit JSON arrays/objects enter the patch. Send the reviewed name/configVersion.
- U02: Transport-change review explains the categories of existing authentication/header/OAuth or execution/argument/environment/working-directory settings that will be deleted. The summary does not expose the new URL/arguments/environment-variable values.
- U03: Deletion reviews and confirms one name/version; cancellation produces 0 mutations. Do not display configuration deletion as completed tool cleanup in the current thread.
- U04: Do not offer edit/delete for readonly or higher-layer servers. If a refreshed version changes during editing, preserve input and block submission.
- U05: Block duplicate requests while a mutation response is pending and include the request in exit drain. Closing the window clears input secrets without resending an already submitted request or misreporting cancellation as complete.

These UI tests also use synthetic IPC; they do not replace evidence of actual Core CAS persistence or current-thread application.

## Execution results in progress

Direct parent execution on 2026-11-14 KST: `mcp-native-01` stopped in the pre-test build stage with Cargo101 because required THIRD_PARTY_NOTICES generation was missing. Raw records were preserved. After preparation using the official notices generator and task-specific offline Cargo configuration, `mcp-native-02` reported 15 targeted PASS, 0 failures/ignored, Cargo0. Raw logs, execution arguments, and source-sha256 are at `D:/codex/build/desktop210-integration-20261114/mcp-native-02/`. Existing Rust warnings and one new unused import were recorded; the new import was removed. Do not report a warning-free run.

The first UI case for the new functionality failed by timeout while waiting for an edit button that did not yet exist; it is preserved at `artifacts/mcp-ui-red/`. After implementation, 10 cases covering the same UI case, existing add/failure/exit behavior, and new edit/delete/transport-change/stale/readonly/secret cleanup behavior passed at `artifacts/mcp-ui-green/`, exit0. This checks absence before the feature was added and behavior after implementation; it does not retroactively declare a pre-existing product defect. TypeScript checking also exited 0. Later changes connecting the current-thread UI module will be checked again in the next affected regression run. Actual Core persistence and tool discovery after a change remain NOT_RUN.

Subsequent affected regressions for management/result-panel/existing extension behavior at `artifacts/mcp-runtime-ui-03/`, including parent integration, English/Korean language changes, preservation of inspection data, and layout refinements, reported 44 PASS, 0 failures/skips, exit0; TypeScript checking also returned 0. This remains synthetic IPC verification and does not establish completed actual Core CAS, reload, or tool calls.
