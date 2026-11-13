# Current-conversation Hook inspection verification

[한국어](hook-verification.ko.md)

These are parent-run results after the handoff's NOT_RUN records. Only the existing `hooks/list {threadId}` contract is consumed. Unfrozen Core1.1 runtime metadata is not implemented; complete handler counts, execution gates and application remain unconfirmed.

| Scope | Result and evidence |
|---|---|
| Native runtime adapter | `hook-runtime-native-01`: 27 PASS/0 FAIL/0 ignored/Cargo0; 16 existing MCP + 11 new Hook memory-WebSocket fixtures |
| Conversation authority/lifetime | `hook-conversation-native-02`: 124 PASS/0 FAIL/0 ignored/Cargo0, including Hook expectedThreadId/extra-field rejection |
| New panels/parent/MCP source fixtures | `artifacts/hook-runtime-ui-01`: 34 PASS/2 FAIL; HP03/M210 exposed D82, a late close clearing a reopened view |
| Same failure conditions after fix | `artifacts/hook-runtime-ui-02`: 36 PASS/0 FAIL. Before source preserved as `extensions-before.ts`, SHA449344fd. Retire the old view before reopening and do not apply a late close to the new open dialog. |
| Existing extensions | Direct project Playwright command, exec session17903/chunk56f54c: 22 PASS/exit0. The compound shell wrapper was blocked before execution by AGS path classification; the standard direct test command ran. |
| Layout/minimum viewport | HP01/HP05 through direct Node Playwright: 2 PASS/exit0, `artifacts/hook-layout/results`. No tests ran during the earlier npm.cmd regex-pipe argument failure. |
| TypeScript | `npm exec -- tsc --noEmit` exit0 |
| Non-test development app | `actual-app-build-04` exit0; exe SHAd5aa9d5d402df020049351bb4ad6e6726158fe7c6b6938085569e53241d8b464. Separate identifier fixture, not a final package. |

The common actual-execution evidence directory is `D:/codex/build/desktop210-integration-20261114`. An owned Core1.0.1 home used a local custom provider/catalog and two real TUI threads. The 14 Native checks in `runtime-core101-04` cover inspection/session preservation/language/resume errors/geometry/order/synthetic Korean input/exit. `runtime-core101-05` adds actual extension-popup buttons and rendering and reported 14 PASS/0 errors/exit0.

One MCP tool and zero Hook inventory entries were observed for the selected thread; stale expectedThreadId was rejected. Zero entries were not interpreted as a disabled feature. The runtime projection exports readonly hash IDs and allowlisted metadata rather than raw keys/commands/matchers/paths/diagnostics. Session IDs/generations/threads and production-app identity were preserved. The owned app/Core stopped; provider final health recorded 0 requests/inFlight=false and its process exited.

This does not establish model turns, real authentication, MCP tool calls, Hook execution, current-thread reload after configuration changes, provider cache hits, Core1.1, a final installer, long-load behavior or completed independent audit. Synthetic and real-path case counts must not be combined as if all executed Core.
