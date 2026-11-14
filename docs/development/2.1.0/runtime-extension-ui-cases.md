[English](runtime-extension-ui-cases.md) | [한국어](runtime-extension-ui-cases.ko.md)

# Current-conversation MCP result UI pre-execution cases

This contract was written before the UI tests and implementation. It covers the standalone panel in `src/mcp-runtime.ts`; its parent manager owns configuration editing, execution review, permission decisions, network calls, and generation checks. No new confirmation dialog is introduced.

The public API is `createMcpRuntimePanel({target?, serverName, onReload, onInspect})`. `target` is `{id, generation, threadId, title}` and is copied at creation. The returned members are `element`, `render(unknown): boolean`, `showPending()`, `showUnconfirmed()`, `setDisabled(boolean)`, and `dispose()`. `onReload` opens the existing reconnect review flow; `onInspect(cursor?)` conveys only an inspection intent. There is no automatic inspection, automatic page merging, retry, timer, worker, or state database.

Status results follow the native adapter's narrow DTO. Required fields are `operation=mcpStatus`, exact `threadId`/`serverName`, boolean `found`, `runtimeStatus`, `authStatus`, `toolsState`, `toolNames`, and `nextCursor`. Permitted runtime values are unknown/notStarted/starting/connected/authenticationRequired/failed/cancelled/disabled; auth values are unknown/unsupported/notLoggedIn/bearerToken/oAuth; tools values are available/empty/failed/unknown. found=false permits only unknown in all three states and an empty tool list. available requires a nonempty name list; failed/empty require an empty list. Because native may return unknown together with names when toolsError is absent, names accompanying unknown are also checked for limits and duplicates, but are not displayed. Duplicate names, unknown enums, target mismatches, and missing/malformed fields clear the previous result and return false with fixed error wording.

Limits are 256 UTF-8 bytes for thread/id, 128 bytes for server, 256 bytes for cursor, 256 bytes for each tool name, and 256 toolNames. Names are rendered only through `textContent`; show more reveals 50 at a time. Raw toolsError/schema/env/headers/url and unknown fields are neither stored nor displayed. nextCursor is used only by the explicit next-page button callback and is not displayed. Reload results project only `operation=reloadMcp`, the exact thread, and outcome=queued or unconfirmed(reason=ackMissing/targetMismatch). queued is displayed as acceptance; unconfirmed as unconfirmed.

If the target is absent or invalid, display only an explicit notice and no control buttons. showPending/showUnconfirmed/invalid results clear previous connection/tool displays. The parent's setDisabled value is maintained as a separate bool and survives language changes, render, and showUnconfirmed. dispose releases the DOM, all button listeners, the language subscription, and results; later render returns false and other methods/callbacks have no effect.

| Case | Requirement and decision oracle |
| --- | --- |
| Missing/invalid target | Notice displayed, 0 control buttons, render false |
| Target and actions | Displayed title/server match; keyboard buttons invoke review/inspection callbacks once each |
| queued/unconfirmed | Distinguish acceptance from connection completion; remove the previous connection state after unconfirmed |
| runtime/auth/tools | Distinguish connected, authentication required, available, empty, failed, and unknown |
| Names/private fields | HTML-looking names remain literal text; no script/img; no raw errors, URLs, or unknown fields exposed |
| Invalid DTO | Different thread/server, unknown operation/enum, duplicates, missing fields, and exceeded limits return false with fixed wording |
| List and pages | First 50 names, show more in increments of 50 up to 256; 0 cursor callbacks before a click; pages replace the previous page |
| Parent lock and pending | 0 callbacks while disabled or pending; preserve parent lock after language/state changes |
| State removal after failure | After connected, showUnconfirmed/invalid/pending leave no connection display or list |
| Language change | Update ko/en notices, states, aria, and button names; preserve tool names/list position; no additional callbacks |
| dispose and late results | Clicking retained buttons invokes no callbacks; language change/render/setDisabled do not recreate DOM |

Tests use an independent source fixture in `tests/mcp-runtime.spec.ts`. This fixture does not call actual Core/CLI/native code and does not prove the manager's generation/busy/exit-drain behavior, actual MCP connection, app visual layout/performance, or accessibility on a real device. Parent integration and actual-environment verification must cover these boundaries.

Execution status: **NOT_RUN**. The owner of this unit does not run builds/tests or install dependencies. The parent coordinates port 1420 and test execution serially. Authoring order is this document → test code → implementation. Pass/fail and actual execution evidence are recorded in the parent's results.

The NOT_RUN above records the authoring/handoff stage. The parent confirmed a combined 44 PASS at `artifacts/mcp-runtime-ui-03/`: 8 standalone-panel cases, 14 management/parent-integration cases, and 22 existing extension cases, with 0 failures/skips, exit0; TypeScript checking also returned 0. Configuration refresh and language changes within the same conversation/server preserve the existing panel and inspection data; switching to another target/window disposes it. In the test-browser screens, configuration edit/delete and runtime inspection actions were placed separately, with controls before long lists. Wording also distinguishes discovering tool names from successful actual calls. These results do not cover actual Tauri/Core connection, real tool calls, real-device accessibility, or performance.
