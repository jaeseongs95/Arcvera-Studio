# Korean composition input cases for 2.1.0

Scope: Desktop keyboard handling only. Keep xterm's composition and UTF-8 transport; do not modify the CLI. A CLI-owned defect belongs to the next version. Browser composition injection is not physical Windows IME evidence.

| Case | Entry point and input | Expected result |
| --- | --- | --- |
| IME01 | Maximized terminal; Escape with `isComposing`, legacy keyCode229, or an active composition event sequence | Composition keeps the layout and focus. Escape after composition ends restores the grid. |
| IME02 | Ctrl+Shift+F or Ctrl+Tab during composition | No search opening or session switch; the ordinary shortcuts still work after composition ends. |
| IME03 | Terminal search Enter/Escape during composition | Neither navigate nor close search. After composition ends Enter navigates and Escape closes. |
| IME04 | Terminal Shift+Enter during an active composition with missing keyboard composition flags | No CLI newline sequence; a normal Shift+Enter after composition ends sends exactly one ESC CR. |
| IME05 | Chromium composition updates `ㅎ` → `하` → `한`, then commit `한` | The real xterm sends exactly `한` once, without a control sequence or duplicate jamo. |
| IME06 | Existing plain Enter, Shift+Enter, Unicode paste and conversation composition cases | Preserve Enter/line break, complete UTF-8 text and no conversation send during composition. |
| IME07 | Search output with two Korean matches; composition cancel or compositionend followed by input | Keep the current match on cancel; a changed committed value searches once; ordinary Enter advances once. |
| IME08 | Compositionstart, focusout, then an ordinary shortcut | Clear the stale composition state and restore ordinary shortcuts. |

Before changing production code, run IME01–04 against the frozen a02913d main module through the Vite pre-load hook. Preserve the failures. Run identical test bytes against the candidate. Native IPC remains the explicit test fixture, so these checks do not establish real CLI handling, WebView2 or physical IME behavior.

## Observed verification on 2026-11-17

- Frozen a02913d product module: 8 failed, 8 passed, exit 1. Candidate before search review: 16 passed, exit 0.
- The separate read-only review found cancelled composition and compositionend/input duplicate searches could advance an existing match. The added cases failed at actual xterm selection coordinates: 2 failed, 18 passed. The corrected candidate passed the same 20 cases, exit 0 (19.7 seconds).
- Final product SHA256: main.ts 8610cf3447942cbf58f0fde9994316a2b9b4b430f29ff4438267419aed7a290c; terminal-flow.spec.ts 88323a8bd83c1c20fa530dad3c0f110f45938762170d58956280fe763272c035.
- TypeScript --noEmit and Vite production frontend build both exited 0. Vite retained its existing large-chunk warning. The final separate read-only code review reported no blocking finding; this was a focused diff review, not release acceptance.
- Local evidence: artifacts/korean-input-20261117/before/{execution.json,report.json,stdout.log}, search-red/{execution.json,stdout.log,results/}, and after/{execution.json,report.json,stdout.log}. Final report SHA256 edd5f12737fe62dbe29251be0a6803eea4c5bcb3a39bd28540bebd2a40ac423c; execution a8b8fc6fefca2d116e7bd523d39fecb8bb2cabcbb11e3cde374f8c472272af2e. The initial relative JSON report path resolved below the config directory; the preserved baseline/final JSON was copied from that path, and the runner now uses an absolute path. Intermediate overwritten JSON is unavailable; its actual stdout, execution record and failure traces remain.
- The checked plan now names the actual workflow task digest and wrapper command. The earlier plan used a different direct Playwright argv; no signed proof or check-proof acceptance is claimed. Initial server-cwd setup failure and an interrupted run are preserved separately and are not counted as product evidence.

Physical Windows IME, native WebView2, actual CLI and final Core pairing remain unverified. The native IPC replacement is limited to the browser fixture. No CLI source or binary was changed. A development executable must be bound to the new commit before native testing; these results do not authorize installation or establish full 2.1.0 release readiness.

CI follow-up: run 44039888650 failed IME07 commit-input-end with a found result but no retained xterm selection. The xterm SelectionService clears selection when row count changes. This is a plausible layout race, not a directly recorded resize in that CI trace. Before applying the selection oracle, the case must open search, await fonts.ready and assert actual terminal rows/columns equal FitAddon.proposeDimensions(). Keep the same first/next selection coordinates and all cancellation assertions. Do not retry a failing case or change its expected result.

With the fit-readiness precondition, identical 20-case tests produced 11 failures/9 passes on frozen a02913d (exit 1), then 20 passes on the unchanged product candidate (exit 0, 18.6 seconds). Test SHA256 a96fd14e76b63e1d5d1d101a9218784d468ff922f467f364e3a69bfeee158c13; TypeScript exited 0. Separate read-only review found no blocking issue or weakened oracle. This checks composition search after geometry is ready, not arbitrary overlap with later resizing. Current before/after directories contain this pair; original 16-case baseline and original 20-pass run are preserved as before-original16 and after-original20. CI44039888650 remains a recorded failure; its bundled/source-1/native jobs passed but packaging was blocked.


## Product rework after review

The earlier layout-ready pass did not establish correctness for immediate searches. IME09 opens search and dispatches input in one DOM task: a match initially exists, then disappears after two real animation frames on 8bf0908. Product code now fits the actual hosts before searching and when changing search visibility. The IME07 font/dimension wait was removed.

IME09–14 cover immediate open/input vs resize, the same Korean query after selection loss/reopening/session switch, conversation input/search recovery after missing compositionend and focusout, and extension/relay/catalog flags, keyCode229 and sequence tracking. Identical test bytes on 8bf0908 yielded 12 failed / 28 passed in 69.1s (exit1); the first product correction yielded 40 passed in 31.5s (exit0). The before-run IME10 reopening and other-session cases failed at their initial selection due to resize, not at their subsequent re-composition assertion; only the selection-cleared case directly reached and failed that re-composition boundary.

The shared ime.ts uses document-lifetime capture listeners and weak targets for flag/229/sequence detection, cleared on focusout. Conversation-local composition booleans were removed. Search event deduplication is weakly keyed by the actual Terminal and includes query and selection coordinates, so selection loss or another terminal permits the same Korean query. No new dependency or CLI change.

Raw execution, JSON reports, stdout and failed traces are under artifacts/korean-input-20261117/review-before and review-after. A separate review-final records verification after removing the readiness wait and weakening cache ownership. These Chromium/xterm product-handler tests replace native IPC only; physical Windows IME, WebView2, actual CLI, final Core pairing and full release acceptance remain unverified.


The final product/test combination passed all 40 tests without the readiness wait (32.9s, exit0, zero skipped/flaky). Final report SHA256=d630396dddc1914ddbb6f511d00712ad0149c2c84482ac47bb4d0bd5301b6ab7; execution=b91b832165c1db7b8a2d58f560eb6f9fe0e85a182915a2db163f1b0487561d5d. TypeScript --noEmit and the production Vite build exited 0; the existing large-chunk warning remains.

## Empty-code Windows Process key follow-up (2026-11-18)

The isolated 08310ed WebView2 executable composed `한글` with the installed Windows Korean IME. In run n17, the confirming key was `Process`, keyCode 229, with an empty `code`; `compositionend` was followed by a non-composing Enter/13 before release. The valid form submitted once. The earlier `code: Enter` replay did not cover this actual event sequence. The remaining two native cases were not run after this failure. The application exited normally and all seven owned process generations were absent afterward.

The shared guard now tracks an opaque composing press until release. If composition ends while that press remains down, the trailing Enter stays guarded. A character released before compositionend leaves no Enter latch. Ordinary keydown, focusout, reset and window blur clear the relevant state; weak targets and the existing xterm handler remain in use. No timer, dependency or CLI change was added.

IME19 checks zero form starts on confirmation and one start on a newly pressed Enter. IME20 checks one Korean payload, no confirmation CR and one new-press CR through real xterm. IME21–23 preserve character release, focusout and blur recovery. Identical test bytes and Playwright subprocess arguments on the isolated checkout produced two expected assertion failures and three passes before the guard change, then five passes afterward (exit 0). The initial web-server working-directory error ran zero tests and is preserved separately, not counted as a product failure. Evidence is under `artifacts/ime-empty-code-20261118/{red-01,green-01}`.

Separate source review identified overlapping character release and modifier press clearing an already confirmed Enter latch. IME24–26 reproduced three failures and one preservation pass, then passed all four after separating opaque-key release from confirmed Enter release and retaining the latch through modifiers. Final affected-consumer regression: 48 passed, zero skipped/flaky, exit 0; TypeScript and Vite also exited 0. The existing Vite chunk-size warning remains. This overlap is a directly reproduced browser failure; its frequency with actual Windows IME input is unverified.

Browser native IPC remains an explicit fixture. These checks do not establish the new executable's Windows IME behavior or Core integration. A new source-bound development executable and native cases are still required. Final packaging and installation remain outside this task.

## Native Windows commit Enter follow-up

Physical Korean IME input in the isolated 980f143 Windows WebView2 executable composed `gksrmf` as `한글`. On a valid new-session form, the commit press emitted `Process/Enter/229`, `compositionend`, then `Enter/13` before keyup. That trailing event incorrectly submitted the form once. An invalid form concealed the defect. The explicit missing-CLI fixture stopped before starting Core; this is a Desktop failure.

IME15 replays this ordering against a valid form and checks zero starts, then exactly one start after keyup and a new Enter. IME16 checks recovery after focusout without keyup. IME17 checks duplicate/repeat Enter suppression and explicit button use. The unchanged three tests failed 2/passed 1 on 980f143, then passed 3 after the shared guard correction. The guard preserves the composing Enter until keyup, another key, focusout, reset or window blur, with weak targets and no timer. It prevents only the trailing browser action and leaves committed input available. Existing terminal fixtures now release their synthetic Enter before the later independent press; all original assertions remain.

The targeted combined run passed 38 cases (28.7s, exit0). These are Chromium handler regressions with native IPC replaced, not proof of the new executable's physical IME behavior. Rebuild and repeat the native valid-form case before declaring that boundary verified. CLI code remains unchanged; no final build or installation is requested.

Separate review found xterm translates a trailing Enter even after document `preventDefault`. IME18 reproduced `['한글', '\r']` where only `['한글']` was allowed on 88f01a3 (exit1). The existing xterm custom handler now also rejects composition Enter. The identical case passed with exactly one Korean payload, zero commit CR, and one CR after release/new Enter. IME15–18 together passed 4 (exit0). This is actual xterm with browser IME APIs and replaced native IPC; physical terminal/Core integration still needs its authorized native dependency.

For the earlier 980f143 candidate, separate read-only review covered all six product modules and five test files with zero blocking findings. The reviewer read raw reports, source and hashes but did not execute tests. After that candidate's final execution, only one trailing EOF LF was removed from each test file; exact executed bytes are digest-verified under review-final/frozen-tests. Those six product files were identical to that candidate's final snapshot.
