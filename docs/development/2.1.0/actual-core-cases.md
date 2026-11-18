# Actual Core baseline cases

[한국어](actual-core-cases.ko.md)

First verify the real public Core 1.0.1/development Desktop path. This does not replace integration of the final paired Core 1.1.0 candidate. Use the separate identifier `io.agentgrid.desktop.integration210` and fresh workspace, WebView profile and HOME. Preserve production apps, daemons, authentication and other sessions' data.

`actual-core-fixture` is an explicit Cargo test feature. Its test code is excluded from default product builds. The dedicated runner supplies an owned HOME/executable and existing shared server; no mock transport replaces it. Missing prerequisites fail instead of producing a pass.

| Case | Oracle |
|---|---|
| Actual config/read projection | Distinguish raw user STDIO configuration from typed effective defaults when judging editability. Log names/field names, never raw values. |
| Native/WebView2 ownership | Verify the root-launched executable hash, CDP endpoint and separate identifier before application-data IPC. |
| Actual add/edit/delete | One user server, exact readback, preserved protected env, unchanged bytes on stale version, scoped transport-field deletion, final absence. |
| Cleanup | Exit the fixture app only with no unexpected sessions; preserve the production app PID/start time. Reconcile the daemon separately through its owned HOME. |

`actual-mcp-red-01` reported 0 PASS/1 FAIL/Cargo101 through the real Core read path. The differing keys were `enabled`, `environment_id` and `tool_timeout_sec`; the full-equality guard rejected valid user configuration. After the fix, `actual-mcp-green-02` reported 19 PASS/0 FAIL/0 ignored/Cargo0: one actual shared-Core read and 18 synthetic RPC regressions.

`tests/native-mcp-manager.mjs` requires a root launch receipt in the explicit `AGENT_GRID_NATIVE_OUTPUT`. It uses real WebView2/Tauri IPC but does not verify a model turn, real authentication, current-thread reload, final installer or provider cache hit. In the first `native-core101-02` run, ownership and actual inventory passed, then the add-result `editable=false` assertion failed; the fixture app exited. After the fix, all seven cases on the same actual path in `native-core101-03` passed with 0 errors/exit0. The executable SHA256 is `1c128725789919ca67eccf9922b020f68520d60bd7b18bcba0114f40e16aa142`, a development app with a separate identifier. `app-exit.json` records production-app identity preservation as true; `D:/codex/build/s210c1/stop.json` confirms the owned daemon stopped.
