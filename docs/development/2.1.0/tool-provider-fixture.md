# Local MCP tool-call provider fixture

[English](tool-provider-fixture.md) · [한국어](tool-provider-fixture.ko.md)

## Opt-in GUI title contract — plan before tests and implementation

Actual GUI cases 01–03 remain FAIL. In case03, the second request had four messages, no function output or advertised tool, an exact frozen title JSON schema, strict/known-name JSON-schema format, and a different prompt-cache identity. This matches Core's separate ephemeral title-request path. The direct-stdio two-POST oracle did not account for the TUI auxiliary request. The following is a new explicit GUI fixture contract, not the same old two-POST RED/GREEN condition. Original provider SHA256 `3e4665e3d8cd5ae39dc9153a0b2a747ed58d86005da6ef07cfc938721efc919b` and original 77-test source SHA256 `ad22b53e6d81afd088a0cb0a3a5052e47c321f7a8e988faad1e191c51b702320` are preserved with relative test paths under `D:/codex/build/desktop210-integration-20261114/gui-title-fixture-before/`.

Optional `--title-model <exact-ID>` enables one synthetic title request. Without it, the original two-POST behavior and stage-2 shutdown remain unchanged. The ID must be nonempty, trimmed, at most 256 UTF-8 bytes, and contain no control/format characters. A title request requires that exact model, `tools:[]`, nonempty message-only input and exactly four format keys: type=`json_schema`, strict=true, name=`codex_output_schema`, schema=`{type:'object',properties:{title:{type:'string',minLength:1,maxLength:36}},required:['title'],additionalProperties:false}`. This is frozen Core's public schema, not a stored real request. Unknown schema/format/model or enabled tools are rejected; prompt wording is not inspected or echoed.

In opt-in mode, every Responses POST consumes the total budget of 3, including preclassification auth/byte/concurrency/JSON failures. Parsed text objects with their own format key consume the one-title attempt and never fall back to main acceptance. Other parsed bodies consume at most two main attempts, including rejected main requests. Title handling never advances the main stage or creates `toolResultConfirmed`. Its separate `synthetic-title-response` receipt uses `mainStage` and `syntheticTitleConfirmed:true`, with no `stage` or `toolResultConfirmed`. The fixed synthetic title JSON is at most 36 characters and has synthetic zero usage. Completion shutdown requires both successful main stages and one successful title; otherwise the original lifetime/signal cleanup applies. Title may arrive before, between or after main requests.

| Case | Planned public CLI/real loopback oracle |
|---|---|
| GT01 | Three valid request orders preserve both exact main SSE responses, one distinct fixed title response, normal exit and closed owned port. |
| GT02 | Wrong/extra schema or format key, strict/name/type/model, missing/enabled tools, empty/non-message input return fixed rejection with no title/main advancement. |
| GT03 | A duplicate or previously rejected title consumes the single title budget; retry cannot create a title response. |
| GT04 | Two parsed main attempts, including rejection, prevent a third main retry despite opt-in. |
| GT05 | Preclassification failures consume total budget; health is read-only; incomplete main/title handling never reports completion. |
| GT06 | No option means original two-POST title rejection; preserve all original 77 assertions and auth/size/inflight/deadline/cleanup guards. |
| GT07 | Missing/duplicate/empty/oversized/control-bearing title-model values produce the fixed CLI error without reflection. |

The author may run only this Node unit-test file with test concurrency 1 and an owned command heap limit. The new cases are NOT_RUN until the targeted run is recorded below. Actual Core/Native/GUI, Git mutation, ledger, real model/cache and release acceptance remain outside this unit. The following sections retain the original direct-stdio fixture evidence.

### Opt-in implementation and targeted validation

`node --max-old-space-size=256 --test --test-concurrency=1 --test-reporter=tap tests/core-tool-provider.test.mjs` completed with **exit 0, 113 PASS, 0 FAIL/cancelled/skipped/todo**, duration `34216.7933 ms`. This includes all original 77 cases and 36 added cases including containers. It covered three successful orders, 20 malformed title shapes, duplicate/rejected-title budgets, main/total ceilings, auth/type/size/JSON/concurrency, incomplete lifetime, default rejection and CLI safety. Only owned Node children and dynamic loopback HTTP ran. TAP was captured in tool stdout; no separate TAP file or receipt artifact was saved, and the run was not repeated to create one.

Static checks confirmed that the original test bytes are an exact prefix of the extended test, all five original main predicates are byte-identical, and the original main `eventsFor` SSE generator is byte-identical. Evidence copies remain unchanged. Tested provider SHA256: `e2546ec77e5c5276f133b8907bf9bb0d54084ccbd5b43ff09e04f64d6c525adc`; tested test-file SHA256: `81007e3617cd69beed8b7a80591d7ca3650248efeff8581290f9322e0914c2db`.

The initial parent oracle required one title receipt as well as both main stages. Case04 preserved that oracle and reported **FAIL**, although its actual GUI roundtrip and normal cleanup passed: no auxiliary title was requested. The fixed Core source has a server-owned-settings path that does not start title generation; the exact scheduling cause of the difference between runs is not established. The original case04 result was not rewritten.

### Actual Native GUI scope

Before case05, the parent corrected only the unnecessary title-count requirement to **0..1**. Mandatory main stage-2 `toolResultConfirmed`, the one GUI submission and one precisely bound GUI approval, actual MCP/same-turn/assistant completion, Native history equality and normal owned cleanup were retained. The provider, its three-attempt total ceiling, two-main ceiling, one-title ceiling and all original result predicates were unchanged. No-title mode uses the existing lifetime cleanup; it does not fabricate a title-completion receipt.

`D:/codex/build/s210g2/case05/` passed with a source-bound Desktop `deafe7f` release fixture and public Core1.0.1. GUI create/submit/approval were each 1; one MCP item, one completed turn and the assistant matched Native history. Both main responses were observed, auxiliary title count was 0, and the app/collector/provider exited 0. Own Core stopped, the two observed direct children disappeared, config bytes and the operating app identity were preserved. `verify-case05.mjs` checked the original reports and produced `verification.json` SHA256 `b6263009ef559ef90d31bd3d0fb5dd307013ae067001bd21009751beeab05e30`.

Parent result SHA256 is `a9f8ea85f938e1d943548e23f744b988122c2c418477b6f8b0c5d93f27f27ab8`; GUI report SHA256 is `c71f5a26cb84f34c9eb8d73407cca5e406c21e5f5b9a230d9768e0180363e0ae`. Original cases01–04 remain as recorded. This is a new functional verification, not an identical-oracle RED/GREEN pair. It covers a controlled conversation-view approval and synthetic local model. Core1.1, real inference/cache, Native question-inbox interaction, generic-form origin defect D92 and final release acceptance remain unverified or unresolved.

## Original direct-stdio evidence (preserved history)

Status: **test-only fixture; Node tests 77 PASS / 0 FAIL; public Core1.0.1 stdio/MCP baseline PASS; Core1.1/Desktop GUI integration NOT_RUN**. This standalone loopback fixture is not imported by product code. Original user goal: “너의 목표는 2.1.0 릴리즈이다. cli 담당과 협업해서 2.1.0 릴리즈에 들어갈 개발 범위를 산정하고 작업하면 된다.” English summary: deliver Desktop2.1 with the CLI owner and agree its scope. This fixture does not establish real model, billing, or provider-cache behavior.

## Authority and minimal contract

In public source `D:/codex/build/core110-release-20261114/source`, `codex-rs/core/tests/common/responses.rs:1000`/`:1012` define function_call and namespaced function_call in `response.output_item.done`. The two-stage example at `:1498` follows a completed function call with a subsequent assistant completion. `codex-rs/core/src/tools/handlers/mcp.rs:546`/`:573` and `codex-rs/tools/src/responses_api.rs:59`/`:76` advertise `tools:[{type:"namespace",name:"mcp__<server>",tools:[{type:"function",name:"<tool>"}]}]`. `codex-rs/protocol/src/models.rs:2484`/`:2564` convert normal MCP text to `output:[{type:"input_text",text:"desktop-local-fixture"}]`. Only that exact output, based on `local-mcp-server.mjs`, is accepted. Flat advertisements, string outputs, and speculative fallbacks are unsupported. Producer source is a changing candidate; generated-schema/executable comparison remains outstanding.

The first JSON request must directly advertise the selected tool exactly once and contain no function_call_output. It receives function_call SSE with fixed call_id `desktop-fixture-call-1`, namespace/name, and arguments string `{}`. The second request receives a fixed assistant message and completed event only when exactly one output has that call_id and either the exact successful array above or the exact timing-prefix array below. Both usage records are **synthetic zero**. A canned assistant message alone cannot establish tool success. The parent separately verifies actual MCP receipts and Core/Native integration.

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

## Planned timing-metadata oracle correction

Related test-oracle defect: `OPENDIY_DESKTOP-D-0086` (TST/P2, parent registered OPEN v1). This fixture correction and its unit tests provide fix evidence, not independent verification or a successful final turn on real Core.

At fixed Core commit `d6026cbd6157fa18d966031792c537ac383377ac`, `codex-rs/core/src/tools/context.rs:235`–`:247` formats `Duration::as_secs_f64()` as `Wall time: {wall_time_seconds:.4} seconds\nOutput:` and prepends one `input_text` to content items. `context_tests.rs:220`–`:234` at the same commit expects this separate header item. The exact single successful payload remains accepted. The only additional shape has exactly two items: a timing-only `input_text` with only `type,text` keys, followed by the existing fixed successful payload. Seconds must be finite and nonnegative, with 1–10 integer digits (zero allowed), exactly four fractional digits, and no sign, exponent or redundant leading zero. This is a test-only accepted subset, not support for every Core Duration value. The header is not merged with result text, and arbitrary error text cannot become success.

| ID | Requirement/input | Failure oracle / expectation |
| --- | --- | --- |
| TP09 | Same call_id, exact timing header + existing successful result | Fixed assistant/completed, synthetic zero usage, exactly two receipts, no raw logging, normal owned-server exit |
| TP10 | Wrong fraction/sign/exponent/nonfinite value/11-digit integer/leading zero/unit/control character/extra text | Fixed 422, no toolResultConfirmed |
| TP11 | Extra metadata key/wrong type/raw value, missing/duplicate/reversed/excess items, wrong call_id/result/extra field | Fixed 422, no toolResultConfirmed |

Cases and regression tests precede the fixture change. All existing 28 tests and assertions remain, using the same Node command for comparison. The parent's separate `s210t1/a2` observation was MCP-item completion after one precisely bound approval, followed by final-turn failure. The original `never` policy case and a2 failure remain preserved. This corrects the test oracle for observed Core metadata framing; it is not product RED→GREEN evidence. A real Core/Native rerun with the corrected fixture belongs to the parent and is currently **NOT_RUN**. Approval guards, hints and remember/persistent permissions are unchanged.

Correction execution: the same test SHA256 `b66acc7036db9cfaf57d8dab0cfe4b13a136b7e88d5ed3a08e135d0175b3a322` and command below yielded **exit 1 / 56 PASS / 4 FAIL** before the change (duration_ms 15217.1735), then **exit 0 / 60 PASS / 0 FAIL / 0 cancelled / 0 skipped / 0 todo** (duration_ms 13289.432). Before-change failures were `422 !== 200` for the three valid TP09 frames plus their parent test. All existing 28 tests passed in both runs. Additions comprise two top-level tests and 30 subtests (3 valid and 27 rejection cases). Children and dynamic loopback ports were isolated by ownership per execution; a separate source copy and formal AGS CLI verification were outside ownership and **NOT_RUN**. Tests, inputs and command stayed unchanged while the fixture changed in the same worktree. Core/Native, services, product builds and full regression suites were not run by this unit.

## Preparation and execution

### Optional wire ID and actual boundary confirmation

The metadata-only HTTP observation `s210t1/a4` found an optional `id` alongside the exact timing frame and fixed result. At fixed d602, `ResponseItem::FunctionCallOutput.id` is `Option<ResponseItemId>`, a transparent string. It is separate from `call_id` and is never call authority. Absence remains supported; a present value must be a nonblank UTF-8 string of at most256 bytes without control/format characters. Reject null, other types and oversized values without reflecting the original ID. Wrong call IDs, payloads and unknown keys remain rejected.

Identical final test bytes, SHA256 `ad22b53e6d81afd088a0cb0a3a5052e47c321f7a8e988faad1e191c51b702320`, produced **69 PASS / 8 FAIL, Node exit1** against the isolated original53 provider and **77 PASS / 0 FAIL / 0 cancelled / 0 skipped, exit0** against the fixed fixture. The original fixture SHA256 is `378261ca0dd2f3de262f0cfe66942cfd2d8791f8014515d37e6b691baa10f034`; raw TAP and receipt are preserved in `D:/codex/build/s210t1/proof-before/`. This compares fixture oracles, not product/Core RED/GREEN.

The parent's `s210t1/a5` passed an actual fixed public Core1.0.1 app-server with the explicit local model/thread, exactly one bound normal MCP approval, a completed MCP item and matching turn, and the provider's exact two-stage output confirmation. Own config bytes remained unchanged; Core EOF0, provider exit0, closed listener and disappearance of the three observed direct children were checked. No remember/persistent approval metadata was sent; unknown requests are denied. Preserve original never/a2/a3/a4 results. This covers real Core/stdIO/MCP with synthetic model responses, not Core1.1, DesktopGUI, real inference/billing/cache or general cleanup of every possible descendant.

From the repository root, use argv `['node','tests/fixtures/core-tool-provider.mjs','--tool-name','mcp__desktop_probe.desktop_local_probe_v1']`. The parent verifies the actual Node executable and does not construct shell strings. Optional `--lifetime-ms` accepts 500..60000 (default 30000); `--request-timeout-ms` accepts 100..10000 (default 5000). Bind only a **dynamic port on 127.0.0.1**. Append `/v1` to the startup JSON origin for a separate owned HOME/project/profile provider base_url. The parent prepares existing local catalog/provider settings separately. Do not use external URLs, tokens/API keys, Authorization/Cookie, or actual HOME/config.

Prerequisites: Node built-in test/fetch, owned loopback permission, the exact directly advertised namespace/tool, an argument-free MCP tool, Responses HTTP/SSE (compressed requests unsupported), and both requests routed to the same fixture. WebSocket, tool search/defer_loading, and flat advertisements are unsupported. Bodies exist only in memory during validation and are not stored in files/logs. stdout has testOnly startup/shutdown and byte count/SHA256/stage/advertisement-result booleans only; startup also includes safe loopback origin and maxRequests. `/health` exposes only stage/attempts/maxRequests/inFlight and does not consume POST budget. Every `/v1/responses` POST counts toward the two-attempt budget, including credential/content-type errors and concurrent rejection. Limits include one inflight, 8MiB for both Content-Length and chunked bodies, request/lifetime deadlines, and owned-connection cleanup. No automatic retry. Successful two-stage completion, lifetime expiry, SIGINT or SIGTERM closes the owned server and its connections.

Command: `node --test --test-reporter=spec tests/core-tool-provider.test.mjs`. Initial fixture execution: **exit 0, 28 PASS / 0 FAIL / 0 cancelled / 0 skipped / 0 todo** (10 top-level tests plus 18 subtests, duration_ms 8037.8354). After the initial 26 passed, two boundary tests were added for credential/content-type attempt accounting and oversized chunked bodies; all 28 were rerun. Tests use real owned children/loopback for TP01~TP08, verify that raw request/credential sentinels do not appear in stdout/stderr/errors, and confirm that the owned port refuses connections after normal exit. The latest correction result is the 60-test run above; the original record remains preserved. This is a new fixture, so no RED→GREEN claim about an existing product defect is made. These test executions contain no real Core/Native/MCP integration or actual tool-call receipt, and do not establish physical cache/LLM behavior. Installation, builds, existing regression suites and final audit were not run.
