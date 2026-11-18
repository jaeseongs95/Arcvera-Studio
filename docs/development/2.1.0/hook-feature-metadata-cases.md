# Current-chat Hook feature metadata candidate consumption

Pre-implementation check plan. **NOT_RUN**: this unit runs no build, tests, or servers. The parent serializes Rust inline fixtures, Playwright source fixtures, and integration checks. Fixtures are not evidence of a real Core, CLI, network, or Hook execution.

## Source and contract

Directly compared the candidate `HooksListResponse`, `HooksListScope`, and `HooksRuntimeInfo` in `core110-release-20261114/source/codex-rs/app-server-protocol/src/protocol/v2/plugin.rs`. Generated schemas, compilation results, and a development executable have not arrived. Candidate consumption is not a claim of supported live APIs. This extends, rather than replaces, the inventory cases.

- Preserve the single `hooks/list {threadId}` send, thread/cwd binding, safe Hook parser, 4096-row ceiling, and shared 8 MiB frame limit. Do not change MCP, configuration/trust mutations, or read-only SHA256 IDs.
- Add DTO `runtime: {featureEnabled:boolean,baseHandlerCount:number} | null`. Native always emits runtime; the UI also accepts older DTOs without it. Count is an integer in 0..9007199254740991. False does not prevent built-in Hooks from operating. Count includes installed base handlers only, excluding per-turn environment/executor overlays. Do not equate it to inventory length.
- Only wire `scope=currentThread` authorizes runtime projection. Absent/null scope is legacy and yields runtime=null; absent/null runtime also yields null. Explicit configuration, unknown, or malformed scope fails with a fixed safe error. A present non-null runtime must have valid bool/count fields even without scope; malformed data must not become unknown.
- Native drops unknown runtime fields through an allowlist. The pure panel accepts exactly the two runtime fields and rejects extras. Raw content, secrets, commands, diagnostics, and unknown fields are never displayed.
- executionGate and applicationState remain unknown. Ordinary feature state, per-entry configuration, trust, and execution evidence remain distinct. Missing runtime never becomes zero/false. Pending, unconfirmed, and invalid results clear the prior runtime and inventory.
- The public panel API remains unchanged. Preserve parent disabled state, language state, and disposal lifetime. Add no network, timers, retries, mutations, or subscriptions.

## Cases and independent oracles

Native entry point: `exchange_hooks`. UI entry point: `createHookRuntimePanel().render`. Reuse the existing Rust in-memory RPC fixture and isolated `tests/hook-runtime.spec.ts` source harness. Parent-run commands use the existing Rust `runtime_extensions` filter and `npx playwright test tests/hook-runtime.spec.ts`. The worker's execution prohibition also covers additional check-plan/build commands.

| ID | Boundary | Expected result |
| --- | --- | --- |
| HF01 Native | Legacy absent/null scope/runtime; valid runtime without scope | Preserve inventory; runtime=null; gate/application unknown; exactly one send |
| HF02 Native | currentThread false+3, true+0, maximum safe count | Project exactly the two values; independent of inventory length; gate/application unknown |
| HF03 Native | configuration, unknown, malformed scope | Fixed safe error; no sentinel reflection or resend |
| HF04 Native | Missing/null runtime; missing/malformed bool/count, negative/fractional/unsafe count | Only absence/null becomes unknown; malformed fails without sentinel reflection |
| HF05 Native | Large unused raw/secret runtime fields | Accept within the shared frame limit; return only the two allowlisted fields; single query |
| HF06 UI | Legacy absent runtime, null, false+3, true+0 | Missing means unknown rather than false/zero; show explicit feature/base count, built-in/overlay limits, and both unknown states |
| HF07 UI | Malformed bool/count, extra fields, HTML/secrets, other thread | Return false; clear prior runtime/inventory; fixed notice; no raw reflection |
| HF08 UI | Disabled, language change, rerender, pending/unconfirmed, dispose | Preserve parent disabled state; bilingual explanation; clear prior runtime; no DOM/callback revival after disposal |

Do not remove or skip existing HR/HU assertions. HR05's previously uninterpreted legacy runtime now expects explicit runtime=null while preserving its gate-unknown and no-sentinel oracles. Add five Native tests (HF01–05) and three UI tests (HF06–08). Real Core1.1 matched-candidate integration, generated schemas, Native GUI/busy/epoch behavior, final packaging, and independent audit remain pending parent work.

Subsequent parent execution: `context-hook-ui-03` source/IPC fixtures passed57 in49.3s, exit0: composition14, parent context25, Hook panel13, manager5. tsc exited0. The new five Native cases will run in exact-commit CI while local resource floors remain enforced. This does not establish real Core1.1 behavior.
