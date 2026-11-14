# Context composition panel: cases defined before implementation

Status: source-fixture preparation. Build, Playwright, real Core/Native, and full-dialog checks are **NOT_RUN**. This consumes a type candidate without generated schema, compilation evidence, or a development executable; it does not establish API support.

The candidate authority is `core110-release-20261114/source/codex-rs/app-server-protocol/src/protocol/v2/thread.rs`, `ThreadContextReadResponse` and related types (candidate lines 1915–2032). Estimate, selection-reason, and observation semantics were checked against `core/src/context_manager/history_composition.rs`, `core/src/session/context_composition.rs`, `core/src/session/context_cache.rs`, and `history/src/context_index.rs`. No conversation bodies, personal logs, or authentication material are inputs.

## Public boundary and ownership

`createContextCompositionPanel({ threadId, onRefresh, onNext(snapshotId, cursor) })` returns `element`, `render(unknown): boolean`, `showPending()`, `showUnavailable('unsupported' | 'stale' | 'unavailable')`, `setDisabled(boolean)`, and `dispose()`. It owns pure DOM presentation and explicit callbacks. The parent owns Native requests, session/generation/thread binding, response ordering, stale error classification, permissions, shutdown, dialog integration, and sourceFixtures configuration. This panel does not parse raw errors.

Only the current validated page of 0–100 rows is retained and replaced. Repeated source IDs, distinct resident occurrences, and null identities remain separate rows. Next-page activation passes the current snapshotId/nextCursor explicitly. There are no automatic reads, accumulated pages, timers, network calls, mutation, or trust controls. Language changes preserve state and parent disabled state. Disposal releases language subscription, listeners, DOM, and result; a late render returns false.

## Input and presentation contract

- Candidate camelCase fields are used. Native supplies u64 identities (contextIndexRevision/historyVersion/cache.requestContextIndexRevision/fragments.sourceOrdinal/residentOccurrence) as exact decimal strings or their original null. Strings are canonical `0` or positive decimals without leading zeros, up to u64 max, never converted to numbers.
- Counts/sizes/cursors are integers from 0 through MAX_SAFE_INTEGER; requested/effective context windows are null or positive safe integers (zero is invalid). Nullable values remain explicit null. Fragment selected is boolean and required is boolean|null. Missing fields, wrong types, another thread, or more than 100 rows return false, clear previous details/list, and show a fixed message. The parent classifies Native `-32600`, `data.reason=staleContextSnapshot` and calls `showUnavailable('stale')`.
- Thread/snapshot/source/task/observation IDs are nonblank strings of at most 256 UTF-8 bytes, without controls, bidi controls, or malformed surrogates. Timestamps are valid RFC3339 strings of at most 64 bytes. Fields outside the candidate allowlist, including body/auth/schema/diagnostics, are rejected without reflection. Allowed IDs use textContent only.
- String statuses/provenance/selection reasons are bounded to 256 bytes. Only source-confirmed constants receive labels; future values become fixed Unknown text and their original value is neither retained nor displayed. Unknown fixed-enum kind/residency values are rejected. This is not a secret detector: Native must produce a body-free DTO.
- Resident/selected/excluded UTF-8 serialized bytes and archive use reported values. Archive null means unloaded/unknown. Selected body/navigation/base/tool/total heuristic tokens, provenance, and requested/effective limits are separated from provider-reported usage. No totals are recalculated and null never becomes zero.
- Cache shows aggregate response read/write/uncached/total counters, provenance, observation time/revision, and reported current/stale/unknown. Revision status is not recalculated from IDs. Requested/effective TTL, support/application, minimumValidUntil lower bound/scope/provenance, estimatedExpiry, and serverConfirmedExpiry have distinct labels. The lower bound is not server-confirmed expiry. Reads do not restart TTL, elapsed bounds do not establish actual expiry, and aggregate counts are not attributed to fragments.

## engineering-test-plan

Level: public DOM/API boundaries using the existing Playwright/Vite source-fixture pattern. `tests/context-composition.spec.ts` supplies synthetic data and never calls Core/CLI/Tauri. Oracles come from the candidate types and assigned presentation semantics, not copied total/TTL calculations. Parent-only candidate command: `npx playwright test tests/context-composition.spec.ts`. Execution and check-plan calls are outside this delegation and remain NOT_RUN. No runner or dependency is added.

| Case | Requirement/scenario | Expected behavior and failure oracle |
| --- | --- | --- |
| CC01 | Initial/invalid thread | Initial guidance and refresh; invalid target cannot invoke callbacks |
| CC02 | Valid page/conflicting totals/null | Show each reported byte/heuristic value; no recomputation or null-to-zero |
| CC03 | u64 identities/repeated source occurrences/null rows | Preserve identities above MAX_SAFE_INTEGER exactly; no merged or dropped rows |
| CC04 | Cache observation/TTL/past lower bound | Distinct provenance and lower/estimated/confirmed expiry labels; no inferred expiry or restarted TTL |
| CC05 | Future String statuses/provenance/reasons | Accept with Unknown labels; do not reflect future values or secret sentinels |
| CC06 | Empty/unsupported/stale/unavailable | Distinct fixed guidance; failures including stale clear previous list/details |
| CC07 | Pending/parent busy/language | Disabled buttons and aria-busy; parent busy survives render/language; callbacks blocked |
| CC08 | Explicit next page/replacement | Exact snapshot/cursor callback once; no automatic request; old rows removed |
| CC09 | Wrong thread/missing/shape/numeric/u64/time bounds | False, fixed message, cleared details/list, no payload reflection |
| CC10 | 100/101 rows/repeated source ID | 100 accepted, 101 rejected; repeated source ID alone is not rejected or merged |
| CC11 | HTML IDs/raw secret extras/accessors | Text only, no script/img; non-contract fields/accessors rejected and hidden |
| CC12 | Dispose/late result/retained button/language | False render; no revived DOM, callback, or language reaction |
| CC13 | Korean/English and actual values | Numeric/identity/page state retained, labels translated, no callbacks |
| CC14 | Narrow viewport/keyboard | Named native controls and keyboard activation/focus style; no panel overflow at 360px |

Remaining integration evidence: generated schema/Native field and bound alignment, real Core output, actual thread/generation/shutdown boundaries, full-dialog sizing and visual observation, and Windows accessibility/load checks. The parent connected sourceFixtures. Passing fixtures does not prove real integration.

## Parent execution evidence

After authoring, the parent ran CC01–CC14 plus three parent-wiring and 21 existing context cases in `context-composition-ui-01`: 38 passed, exit0. `tsc --noEmit` also exited0. These are source/IPC fixtures, not real Core, Native or final-application execution. The six Native memory-WebSocket cases are distinguished in `context-adapter-cases.md`. Generated schemas, same-candidate Core1.1 integration and actual WebView/package verification remain incomplete.
