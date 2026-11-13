[English](verification-cases.md) | [한국어](verification-cases.ko.md)

# Desktop 2.1.0 pre-execution verification cases

2026-11-14 KST. Defined before execution. All cases are currently NOT_RUN; concrete inputs and expected wire will be fixed after the CLI owner delivers the contract and executables. Reuse existing runners and tests, and record mock boundaries separately from actual Core and GUI execution.

| ID | Requirement and actual entry point | Inputs and failure boundaries | Pass criteria and evidence |
|---|---|---|---|
| P01 | S04, shared-server preparation and connection | Actual product prefixes, executable paths, and first/existing preparation with the Core 1.0.1 baseline and 1.1 development candidate | Connect to the exact selected file, version, and generation. No D70/71 regression. Original settings and production daemons unchanged. |
| P02 | S01, context settings in session tools | Different sizes for two threads, restore defaults, late response, incorrect ACK, rejection, and lost response | UI matches authoritative requested/effective size. Other threads unchanged. Preserve unknown outcomes; no duplicate sends. |
| P03 | S01, actual compaction requests, events, and history | Successful/failed/cancelled compaction, lost connection, and old-generation completion events | Display failure once; do not misjudge completion or context preservation. Compare actual model requests/lineage with Core evidence. |
| P04 | S02, MCP management | STDIO/HTTP add/change/delete, existing name, CAS conflict, policy override, and readback mismatch | Change only the exact user layer through narrow requests. Distinguish persistence from connection; do not expose arbitrary secrets/config to the renderer. |
| P05 | S02, supported thread reload | Actual tool lists/calls before and after add/change/delete; reload failure, unsupported reload, active work, and lost response | Show success only within the agreed application scope. Verify differences between new/existing threads and actual tool discovery. |
| P06 | S03, hook list/review/change | Feature off/on, trusted/untrusted/modified/managed, preserving disabled state, hash changes after display, and policy rejection | Approve trust only for the current hash. Do not conflate trust, enabled state, and feature state. No automatic permission expansion. |
| P07 | S03, actual hook application | Harmless fixture execution/failure/duplication/lost connection in new/current threads after config changes | Actual execution matches display within the agreed scope. Readback alone is not execution-success evidence. |
| P08 | S04/S05, cancellation/reconnection/close | Cancellation during a request, proxy EOF, timeout, observer connection closure, and late events | Distinguish cancellation acceptance from the end of its effects. Do not apply old-generation state. Reclaim owned connections/processes; preserve shared/unrelated processes. |
| P09 | S01/T78, context_index tool output | Common tool entries and required state supplied in actual live and history paths | Verify preservation/display under the current authoritative contract. Do not add a dedicated UI speculatively. |
| P10 | S06, final candidate | Final Windows package, documentation, CI, and audit after joint development-candidate tests | Verify the actual package, version, and hash; CI for the exact commit; English/Korean README/release notes and license preservation. Publication requires separate approval. |

P06-H1 (D77, added before execution): Query a hook with `enabled=true` and `trustStatus=untrusted/modified` in the existing extensions Playwright fixture. The list/detail must display “사용 설정됨” (configured as enabled) and trust blocking separately, without saying “사용 중” (in use). An empty hook list must state only that no entries were found in the queried scope; it must not infer that feature enablement or executability has been confirmed. Distinguish the fixture's substituted IPC from actual Core gate verification. Use the same new test before the fix (RED) and after it (GREEN).

Environment and execution plan: Use task-specific paths under `D:/codex/build` and a dedicated HOME/TEMP/project. Do not read or copy actual authentication, other sessions' raw conversations, or private logs. The root agent uses the existing Playwright runner on port 1420 serially, first checking whether an existing server belongs to another task. Check the existing Cargo toolchain/cache and current resources before applying limits only to this task's commands. Perform the final Rust release build after checking integration results from both development executables.

Results record the candidate commit, executable hash, command, exit code, tests/failures/errors/skips, raw-evidence location, and unverified boundaries. Distinguish invalid fixtures or runner errors from product RED results; partial passes do not establish overall completion.

## Evidence from unit verification in progress

The following results concern the existing Playwright UI boundary with synthetic IPC. They do not establish completion of all P01–P10 cases or actual Core integration. Paths are relative to the worktree's `artifacts/` directory.

| Target | Actual result | Remaining boundary |
|---|---|---|
| Existing context/extension baseline | `baseline-ui/run.log`, exit 0, 56 PASS, 0 failures/skips | Core/Tauri execution boundaries replaced by fixtures |
| D77 / P06-H1 before the fix | `d77-red/run.log`, exit 1, 1 assertion FAIL. The actual DOM displayed “사용 중” (in use) for an untrusted hook. | Expected-behavior mismatch, not a runner failure |
| Same test and input after the fix | `d77-green/run.log`, exit 0, 1 PASS | Checks configuration/trust display and empty-list wording only |
| Extension/MCP affected regression | `d77-regression/run.log`, exit 0, 27 PASS, 0 failures/skips. `typecheck.exit.txt` is 0. | Actual feature gate, current-thread application, MCP discovery, cache composition inspection, and final package unverified |

`frontend.yml` checks frontend changes on the current 2.1 work branch; it does not replace Rust/Core or final Windows package verification. Only manual execution is permitted in the private repository. The final candidate also requires the existing full Windows checks separately.

After the MCP additions, `mcp-runtime-ui-03` reported 44 PASS across new management/result-panel/parent-lifetime cases and existing extension regressions, 0 failures/skips, exit0; TypeScript checking also returned 0. The synthetic IPC screens in Windows headless Chromium were inspected directly, and the placement of configuration actions and runtime inspection controls was refined. These screens are not evidence of an actual Tauri/Core connection. `conversation-native-05` reported 124 PASS, 0 failures/ignored, Cargo0, covering existing conversation/context behavior and the new expectedThreadId boundary. The valid large unused-schema counterexample in `projection-red-06` produced 1 FAIL/Cargo101 on the previous code; the fixed `projection-green-07`, using the same inline test, reported 16 PASS/0FAIL/0ignored/Cargo0. Refer to the relevant MCP case documents for each result's evidence scope and detailed raw paths.
