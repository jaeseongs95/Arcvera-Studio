# Conversation resource lifetime test plan

This is the first verification unit for T62 after Arcvera-Studio1.8.0. It measures the existing conversation frontend using generated history. It does not complete T62 or establish a native application leak fix. Rust remains the implementation language; no Java migration is planned.

The frozen product baseline is3460042da2024c7a3ecbdcead1655c98bbf06241. Existing Playwright/Chromium source fixtures run with one worker, one browser and1440×1000 viewport. The RPC fixture generates100 unique messages per history page and retains counters rather than an accumulating call log. No credentials, user conversations or operating services are used.

| Case | Boundary and oracle |
| --- | --- |
| CR01 | Replace history and foreground24 times. Exactly100 rows remain visible, background has zero rows, the exact Korean draft survives, and history reloads49 times. |
| CR02 | Create/dispose16 controllers, remove hosts and explicitly collect garbage through CDP. All32 weak references must clear; polling must remain stopped across a production polling interval. An isolated route mutation omitting the actual language unsubscribe must fail this same collection assertion. |
| CR03 | Opt-in sustained four-session switching. Record a unique page generation, fixed workload, UTC/monotonic time, required heap/task metrics and DOM counters every5 seconds. Require every expected checkpoint and requested duration; a missing metric or scheduling gap of5 seconds fails. Preserve partial JSONL and a completion/error summary. |

Run `npx playwright test tests/conversation-resource.spec.ts`. CR03 is skipped without `RESOURCE_LIFETIME_SOAK_SECONDS`; skipped is not pass. Set10..14400 seconds for the sustained case. The dedicated Windows workflow runs30 seconds on branch pushes; dispatch defaults to1800 seconds. Raw artifacts are uploaded even on failure. Heap/DOM trends are observations, without an invented leak pass threshold. Explicit collection belongs to CR02; CR03 observes normal GC.

The original T62 acceptance remains: compare long-duration resource behavior under equivalent generation/load conditions and distinguish observation gaps. Native WebView, real CLI, process/connection lifetime, physical IME and multi-hour actual application behavior need separate isolated Windows evidence. Short Chromium success, build success or audit success does not establish those outcomes. No installation or release is performed by this work.
