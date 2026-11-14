# Context-inspection/compaction integration fixture

[한국어](context-compaction-fixture-cases.ko.md)

PR Windows source-1 CI43544691413 timed out in both existing terminal/conversation compaction cases. Local `context-compression-red-01` reproduced two failures; the raw trace shows the open context dialog intercepting compact/toggle clicks. Modal blocking is correct for the new inspection flow; the old fixture assumed clicking the icon only refreshed usage.

- CF01: Keep both existing startingView cases. After icon/40% assertions, verify the inspection dialog and close it through its actual close button.
- CF02: Retain compaction-once, same id/generation, view/input ownership, draft preservation and no-new-session/send assertions. No retry, timeout increase, force click or skip.
- CF03: Run the full conversation-integration file, preserving its other transition/failure/draft protections. Include it in targeted CI.

Do not change production code to fit the fixture. The test flow changes, so this is not a same-bytes product RED/GREEN pair. Post-adjustment execution is NOT_RUN at authoring. Synthetic IPC does not establish real Core1.1/Native compaction behavior.

Parent execution: all12 conversation-integration cases passed in18.8s, exit0, in `context-compression-flow-02`. Both originally failing cases and the other ten ran with their original assertions preserved. This does not establish real Core/Native or full PR CI success.
