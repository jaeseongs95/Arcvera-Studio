# Terminal link click ownership

## Preparation query and shutdown-race regression

The shutdown-race test in full CI 43600272698 failed before its observer proxy connected: the synthetic CLI treated the new `resume --help` preparation query as a long-lived TUI launch. Add a valid resume help response that advertises no link-owner option. Preserve production deadlines and every shutdown/retransmission assertion. Recheck both stop and shutdown paths in the same `shared_reaper_handoff_during_observer_join_reports_unknown_for_stop_and_shutdown` test. This fixes the test CLI contract; it is not evidence that real Core link duplication is resolved.

After the correction, that existing test passed both paths on local Windows: 1 passed, 0 failed, 0 ignored, Cargo exit 0. Evidence: `desktop210-integration-20261114/d93-resume-help-green/{stdout.log,result.json}`. Preserve the original full-CI result of 574 passed, 1 failed, 7 ignored separately from subsequent complete regression runs.

[한국어](link-owner-cases.ko.md)

Current status: the mode-based rules below are preserved only as a diagnostic prototype. An xterm-detected URL that Core does not recognize could stop opening, so the product changes were restored and are not considered a completed fix. A session-scoped `--terminal-link-owner=tui|host` capability and single ownership of mouse HTTP(S) links are under discussion with Core. Preserve standalone Core, non-HTTP links and non-mouse authentication requests. The prototype and its 41 source-fixture results remain in `desktop210-integration-20261114/d93-mode-prototype` and `d93-green-01`.

The current consumer draft uses the actual resume help declaration, not mouse-mode inference, to pass the host option. Gesture movement is tracked so a drag returning to its start under mouse reporting does not become a link click. That same drag case failed before the change; the existing link/selection cases and it then passed 13 tests with TypeScript exit 0. The new Native negotiation test and integration with a supporting Core are NOT_RUN; D93 remains unresolved.

Agreed consumer boundary: query `resume --help` on the same selected CLI through the existing bounded, cancellable command path. Add `--terminal-link-owner host` only when the actual option column declares that exact option. Do not infer support from versions or prose. Preserve the old argv if absent; reject invalid help or a failed query before creating a session. Keep Core's default tui owner, non-HTTP links and non-mouse authentication behavior. The source11 development package does not yet have this option.

Native cases B01–B05, defined before implementation: an exact declaration adds one host option; an absent declaration preserves argv; prose and suffixed names do not advertise support; malformed help/UTF-8 is rejected; the same CLI/thread/socket/cwd and token-free boundary remains. Verify one combined opener with a supporting actual Core separately; argv unit checks do not replace it.

Host ownership must not treat a drag as a click. The additional case enables mouse reporting, drags a plain/OSC8 link back to its starting point, requires no opener, then requires one opener for a fresh click. Track movement for that gesture instead of relying only on DOM selection. Release tracking listeners on mouseup, blur and disposal without blocking event propagation or PTY reports.

D93 compares the user's duplicate-window report with a5ea3845. Desktop activates its own opener while xterm sends the same gesture to the PTY. Core can open a link on an unmodified stationary Up or a Ctrl/Super Down. Neither preventDefault alone nor a modifier exception establishes a single owner.

Use actual xterm reports and the Core click contract to choose the opener. Desktop handles none and X10: X10 sends no Up and strips Ctrl and other modifiers. In vt200/drag/any, Core can open on Ctrl Down or on an unmodified Down/Up pair; Desktop must not open again. Retain Desktop handling for Alt-only clicks, which Core does not open. Preserve Shift selection, document mouseup cleanup and standalone Core behavior.

Cases defined before execution:
- LO01: real xterm none/X10/1000/1002/1003 × plain/OSC8 clicks. None requires one Desktop opener and no report. X10 requires one Desktop opener and only a Down report. The other modes require zero Desktop openers and actual Down/Up reports. This does not prove Core's opener or OS window count.
- LO02: Ctrl clicks in release-reporting modes produce zero additional Desktop openers and retain modified reports. X10 strips modifiers and uses Desktop; Alt-only also retains Desktop handling because Core does not open it. Shift selection produces no additional opener or mouse report.
- LO03: preserve existing LD selection, Shift drag and mouseup cleanup checks. LD03 explicitly disables reporting before its subsequent Desktop-owned click; its one-open assertion remains. LO01/02 cover application-owned clicks separately.
- LO04: disabling reporting after Ctrl Down must not open again. An unmodified Down followed by disabled reporting has no reported Up, so Desktop handles it. Enabling reporting after a none-mode Down preserves Desktop ownership.
- LO05: verify one combined opener at an actual recognized Core TUI link using the same candidate. Do not claim the user-visible issue resolved before actual Core/OS and selection verification.

Use the existing Playwright source fixtures and real xterm public API. Do not add dependencies, PTY data filtering, stopPropagation or a product-wide mouse-mode override. Compare the reporting-active LO01 counterexample before and after with the same assertion. Actual Core/OS verification is NOT_RUN.

This shares the cause tracked by Core D53. Source comparison confirmed that `CoreMouseService.X10.restrict` removes modifiers and Up, while Core `pointer_down` opens immediately for Ctrl/Super and a normal Up requires an unmodified gesture. The first RED had 6 failures and 2 passes. A second RED first confirmed one actual Down and Up each, then failed because Desktop also opened the URL. Both original results remain in `d93-red-01` and `d93-red-02`.
