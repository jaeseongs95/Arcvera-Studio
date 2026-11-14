# Local MCP tool-call provider fixture

[English](tool-provider-fixture.md) · [한국어](tool-provider-fixture.ko.md)

Status: **test-only fixture; Node tests 28 PASS / 0 FAIL; real Core/Native integration NOT_RUN**. This standalone loopback fixture is not imported by product code. Original user goal: “너의 목표는 2.1.0 릴리즈이다. cli 담당과 협업해서 2.1.0 릴리즈에 들어갈 개발 범위를 산정하고 작업하면 된다.” English summary: deliver Desktop2.1 with the CLI owner and agree its scope. This fixture does not establish real model, billing, or provider-cache behavior.

## Authority and minimal contract

In public source `D:/codex/build/core110-release-20261114/source`, `codex-rs/core/tests/common/responses.rs:1000`/`:1012` define function_call and namespaced function_call in `response.output_item.done`. The two-stage example at `:1498` follows a completed function call with a subsequent assistant completion. `codex-rs/core/src/tools/handlers/mcp.rs:546`/`:573` and `codex-rs/tools/src/responses_api.rs:59`/`:76` advertise `tools:[{type:"namespace",name:"mcp__<server>",tools:[{type:"function",name:"<tool>"}]}]`. `codex-rs/protocol/src/models.rs:2484`/`:2564` convert normal MCP text to `output:[{type:"input_text",text:"desktop-local-fixture"}]`. Only that exact output, based on `local-mcp-server.mjs`, is accepted. Flat advertisements, string outputs, and speculative fallbacks are unsupported. Producer source is a changing candidate; generated-schema/executable comparison remains outstanding.

The first JSON request must directly advertise the selected tool exactly once and contain no function_call_output. It receives function_call SSE with fixed call_id `desktop-fixture-call-1`, namespace/name, and arguments string `{}`. The second request receives a fixed assistant message and completed event only when exactly one output has that call_id and the exact successful array above. Both usage records are **synthetic zero**. A canned assistant message alone cannot establish tool success. The parent separately verifies actual MCP receipts and Core/Native integration.

## Before-implementation engineering-test-plan / CS conditions

The public boundary is the CLI child and real owned loopback HTTP, using Node's built-in `node:test`. Node is already allowed for native 2.1. No dependency, external service, Core, or GUI is introduced. Advertisement/output oracles come from the Core source and fixed local MCP text. Invariants: successful stage transitions only 0→1→2, at most one inflight, a POST budget of 2, bodies at most 8MiB, finite request/lifetime, and no raw retention/logging. JSON parsing needs additional memory proportional to body size; the 8MiB limit does not bound total process memory. Formal AGS plan/proof files and CLI are outside the four-file ownership and are not created/run. These cases were written before the tests and implementation.

| ID | Requirement/input | Failure oracle / expectation |
| --- | --- | --- |
| TP01 | Namespace advertisement→successful same-call_id output | First SSE has the exact function_call; only second has assistant; synthetic zero usage; automatic normal exit |
| TP02 | Missing/flat/duplicate/deferred advertisement | Fixed 422; no function_call or assistant |
| TP03 | First-request output; second missing/wrong call_id/error/text/shape | Fixed 422; no final assistant |
| TP04 | Authorization/Proxy-Authorization/Cookie, path/content-type/JSON errors | Fixed 400/404/415/422; no input reflection |
| TP05 | Over 8MiB, concurrent request, third POST | 413/429; at most 2 successful responses and one inflight |
| TP06 | Slow body, finite lifetime, owned child termination | 408/server exit; owned port refuses connections; no unrelated PID access |
| TP07 | Body/args/credential/CLI-sensitive sentinel | No raw value in stdout/stderr/HTTP errors; only bytes/hash/stage/verification booleans |
| TP08 | Invalid CLI, unknown/duplicate option | Fixed error and nonzero child exit; no option reflection |

## Preparation and execution

From the repository root, use argv `['node','tests/fixtures/core-tool-provider.mjs','--tool-name','mcp__desktop_probe.desktop_local_probe_v1']`. The parent verifies the actual Node executable and does not construct shell strings. Optional `--lifetime-ms` accepts 500..60000 (default 30000); `--request-timeout-ms` accepts 100..10000 (default 5000). Bind only a **dynamic port on 127.0.0.1**. Append `/v1` to the startup JSON origin for a separate owned HOME/project/profile provider base_url. The parent prepares existing local catalog/provider settings separately. Do not use external URLs, tokens/API keys, Authorization/Cookie, or actual HOME/config.

Prerequisites: Node built-in test/fetch, owned loopback permission, the exact directly advertised namespace/tool, an argument-free MCP tool, Responses HTTP/SSE (compressed requests unsupported), and both requests routed to the same fixture. WebSocket, tool search/defer_loading, and flat advertisements are unsupported. Bodies exist only in memory during validation and are not stored in files/logs. stdout has testOnly startup/shutdown and byte count/SHA256/stage/advertisement-result booleans only; startup also includes safe loopback origin and maxRequests. `/health` exposes only stage/attempts/maxRequests/inFlight and does not consume POST budget. Every `/v1/responses` POST counts toward the two-attempt budget, including credential/content-type errors and concurrent rejection. Limits include one inflight, 8MiB for both Content-Length and chunked bodies, request/lifetime deadlines, and owned-connection cleanup. No automatic retry. Successful two-stage completion, lifetime expiry, SIGINT or SIGTERM closes the owned server and its connections.

Command: `node --test --test-reporter=spec tests/core-tool-provider.test.mjs`. Final execution: **exit 0, 28 PASS / 0 FAIL / 0 cancelled / 0 skipped / 0 todo** (10 top-level tests plus 18 subtests, duration_ms 8037.8354). After the initial 26 passed, two boundary tests were added for credential/content-type attempt accounting and oversized chunked bodies; all 28 were rerun. Tests use real owned children/loopback for TP01~TP08, verify that raw request/credential sentinels do not appear in stdout/stderr/errors, and confirm that the owned port refuses connections after normal exit. This is a new fixture, so no RED→GREEN claim about an existing product defect is made. Real Core/Native/MCP integration and actual tool-call receipts remain **NOT_RUN**; physical cache/LLM behavior is not established. Installation, builds, existing regression suites and final audit were not run.
