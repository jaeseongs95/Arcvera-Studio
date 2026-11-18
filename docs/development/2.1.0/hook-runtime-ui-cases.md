# Current-chat Hook panel: planned UI cases

Status: **NOT_RUN**. This plan precedes fixtures and implementation. The parent serializes execution; this unit does not run builds, tests, or real Core/CLI/native requests.

## Audience and contract

- Users inspect Hook inventory for the selected current chat. Reuse the existing pure MCP panel pattern, `quiet-button`, native buttons/lists/dl, and i18n `t`/`onLanguageChange`. Add no CSS, dialog, mutation, or trust approval action.
- `createHookRuntimePanel({target?:{id,generation,threadId,title},onInspect})` → `{element,render(unknown):boolean,showPending(),showUnconfirmed(),setDisabled(boolean),dispose()}`. Copy and freeze the target binding. The parent owns current target/generation/exit and response epochs.
- `render` accepts only the native `hooksStatus` projection: `threadId`, `provenance:"loadedThreadHooksList"`, `executionGate:"unknown"`, `applicationState:"unknown"`, and `hooks:{items:[{id,eventName,handlerType,source,enabled,isManaged,trustStatus}],warningCount,errorCount}`. Reject raw/unknown fields with a safe fixed notice and false.
- Item IDs contain `hook-` plus 64 lowercase SHA256 hex characters. They distinguish read-only inventory entries, not raw keys or authority/trust mutation keys. The only callback is argument-free `onInspect`.
- Validate the existing native enums without guessing new 1.1 metadata fields or meanings. Display configured enablement separately from trust and unknown execution gate/application. Enabled/trusted entries and an empty list do not confirm execution.
- Always briefly explain that loaded-thread inventory is not the complete execution-handler list, including builtins and per-turn overlays. Display diagnostics only as counts from 0 to 128 and fixed notices.
- This DTO has no unsupported enum. The panel distinguishes unconfirmed state from a successful empty inventory; the parent's safe error boundary classifies and reports unsupported Core requests. Do not infer support from raw diagnostics.

## Lifetime and input bounds

- Missing/invalid targets show guidance without controls. Target id/threadId are nonblank, control-free strings up to 256 UTF-8 bytes, title up to 1024 bytes, and generation a nonnegative safe integer.
- Match the current thread. Validate all IDs, enums, booleans, diagnostic counts, and at most 4096 unique item IDs before retaining a small projected snapshot. Target mismatches, missing/malformed fields, and exceeded bounds clear prior results and return false with a fixed notice.
- Render 50 items initially and 50 more per explicit button press. No additional requests, automatic paging, networking, timers, retries, or state DB.
- Pending clears the previous list and blocks actions. Preserve the parent's disabled flag across renders and language changes. Unconfirmed clears previous inventory. Language changes retain the result, pending state, and visible item count.
- Dispose releases the language subscription, button listeners, and snapshot, clears/removes DOM, and prevents retained buttons, late renders, language changes, or other methods from reviving callbacks/DOM.

## Fixture cases and oracles

Use the existing Playwright isolated source harness. These fixtures do not prove real Core/CLI/native wire behavior or generation gates. Entry points are public panel methods and DOM/keyboard interactions.

| ID | Scenario | Expected result |
| --- | --- | --- |
| HU01 | Missing/invalid target | Explicit guidance, zero buttons, render false |
| HU02 | Displayed target, inspect, external target mutation | Plain-text title, one keyboard callback, original target thread binding |
| HU03 | Valid inventory; identical event/type/source with different IDs | Preserve distinguishable rows; separate enablement, trust, management/source; two unknown execution states |
| HU04 | Successful empty, unconfirmed, pending, reinspect | Empty is a successful query with unknown execution; clear stale rows on unconfirmed/failure; pending blocks callbacks |
| HU05 | Wrong thread/operation/provenance/gate/enums, missing fields, duplicate/bad IDs, booleans, count limits | Fixed safe notice/false, no stale results or reflected payload |
| HU06 | HTML/secrets in raw keys/commands/paths/diagnostics/unknown fields | Reject without reflecting raw content, HTML nodes, or secrets; target-title HTML remains plain text |
| HU07 | 51/4096/4097 items, batches of 50 | No request while expanding; final-page focus retained; all 4096 preserved, 4097 rejected |
| HU08 | Disabled state, language changes, rerender | Preserve busy/pending/result/visible count; disabled remains until parent releases it; no extra callback |
| HU09 | Dispose, late responses, retained buttons, language changes, repeated dispose | Render false, DOM stays empty/detached, no extra callback |
| HU10 | Warning/error counts; unknown versus unsupported | Counts and fixed notices only; distinguish unconfirmed from successful empty; do not fabricate unsupported without parent error input |

## Parent integration cases defined before execution

`hook-runtime-manager.spec.ts` uses a synthetic host to check HP01 Native session/generation/expectedThreadId binding even with empty configuration; HP02 same-target observation retention across language/metadata refresh; HP03 rejection of a late reply after close/reopen on another thread; and HP04 draining an accepted read on exit without duplicate submission. These are not actual Native/Core execution evidence and are NOT_RUN before execution.

## Execution handoff

Order: planned cases → source fixtures → implementation. The parent adds `hook-runtime` to Playwright sourceFixtures and uses the existing runner. Type checks, actual application placement/keyboard/narrow-screen/language checks, real Core inventory, and independent audit remain parent verification. Prepared artifacts are not product PASS or release completion.
