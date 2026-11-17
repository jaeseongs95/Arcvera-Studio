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
