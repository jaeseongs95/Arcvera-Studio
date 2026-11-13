[English](local-provider-fixture.md) | [한국어](local-provider-fixture.ko.md)

# Local provider preparation for current-thread verification

2026-11-14 KST. **Preparation only; execution is NOT_RUN.** These test-only materials support verification of actual Desktop 2.1/Core thread creation and MCP/hook inspection boundaries. They add no unfrozen Core1.1 metadata RPCs. Original Core sources, actual HOME/config/auth, and production apps/daemons are unchanged. Execution and configuration below are procedures for the parent in a separate owned environment; this unit has not performed them.

## Scope that does not require HTTP

A catalog alone does not change authentication policy. Configure a fresh owned custom provider with `requires_openai_auth=false`, no env_key/token/auth settings, and a local `model_catalog_json`. This source path uses the catalog through `StaticModelsManager` and skips the TUI login screen, so thread creation/inspection without inference has a path that does not require a model HTTP server. The catalog does not replace a model turn. Require an actual threadId created by the TUI; a blank screen or process launch alone does not prove thread creation. This also does not prove the absence of external communication from other startup behavior.

Authoritative sources are below `D:/codex/build/core110-context-20261113/source`. The review uses the assigned d602-family source; compatibility with the distributed Core1.0.1 executable remains unverified.

| Evidence | Confirmed source contract |
| --- | --- |
| `codex-rs/core/src/config/mod.rs:2350–2378` | Load catalog as ModelsResponse; reject empty models |
| `codex-rs/model-provider/src/provider.rs:549–559` | An explicit catalog selects StaticModelsManager rather than provider-catalog HTTP discovery |
| `codex-rs/model-provider-info/src/lib.rs:219–229,521–540` | requires_openai_auth=false without env_key does not require that provider's API key |
| `codex-rs/model-provider/src/provider.rs:879–894` | Existing non-OpenAI scoped-auth test returns empty auth headers |
| `codex-rs/tui/src/lib.rs:2525–2531` | Providers not requiring authentication skip the login screen |
| `codex-rs/tui/src/app_server_session.rs:1123–1184` | Explicit TUI thread-start request path, distinct from a model turn |
| `codex-rs/app-server/tests/common/config.rs:136–150` | Existing mock-provider base_url, Responses wire, and zero-retry configuration |
| `codex-rs/app-server/tests/common/mock_model_server.rs:68–83` | One created → assistant message → completed SSE sequence |
| `codex-rs/core/tests/common/responses.rs:782–819,850–860,1114–1118` | event/data framing, message/completion payloads, text/event-stream |
| `codex-rs/codex-api/src/endpoint/responses.rs:123–152`, `sse/responses.rs:386–391,451–507` | POST /responses, SSE receipt, message/completion interpretation |
| `codex-rs/protocol/src/openai_models.rs:414–528,963–1063` | ModelInfo, models wrapper, promotion of existing base_instructions to an instruction template; reject a missing template |
| `codex-rs/config/src/loader/mod.rs:133–134`, `codex-rs/cli/src/main.rs:504–505` | User base and separate `<name>.config.toml` profile, --profile |

## Files and fixed contract

- `tests/fixtures/core-local-catalog.json`: Synthetic metadata for one `desktop-local-fixture`, not a real model. It advertises text input, disabled shell, synthetic context window 32768, and a high setting. It uses the existing ModelsResponse `base_instructions` contract; these values do not represent real model capability or context capacity.
- `tests/fixtures/core-local-provider.mjs`: Uses only Node's built-in http/crypto. Binds only to `127.0.0.1`, with default port=0 and maxRequests=1. Provides GET /health and POST /v1/responses only; no /models, other endpoints, or WebSocket. No upstream connections, child processes, or file storage.
- Request bodies are neither accumulated nor parsed as JSON; only wire-byte count/sha256 are computed. Original prompts/context, request headers, and tokens are not logged. Model requests carrying Authorization/Proxy-Authorization/Cookie are rejected without printing their values. This server does not validate request semantics or model selection.
- The response uses the same three events as the existing Core fixture, with fixed message `desktop-local-provider-fixture`. Completed usage zeros and nulls are **test-only synthetic values**. They do not prove actual token usage, cache hits, provider caching, inference, or MCP tool calls. No cached_tokens or TTL is fabricated.
- ceiling: 8 MiB request-wire limit, one concurrent model request, four connections, 5-second headers timeout, 10-second request timeout, and 1-second keep-alive timeout. Explicit `--max-requests` accepts 1..16 only. Excess size/quota yields 413/429, other paths 404, and auth headers/invalid content type 400/415. No unlimited response replay or alternate format after a failure.

## Parent reproduction preparation

Node 18.2 or later with `server.closeAllConnections()` is required. The parent verifies the distributed Core executable path/SHA and separate Desktop identifier. Create fresh owned HOME/project/profile locations and confirm any parent AGENTS/project configuration belongs to the test. Pin process-specific ARCVERA_HOME and HOME/USERPROFILE paths to owned directories and do not inherit actual auth stores or API-key environments. Do not change global environment settings or read real credentials to clean them up. Set `cli_auth_credentials_store="file"` to use only the owned HOME and create no token/key files. Respect organizational policy; stop if it disallows the setup.

The initial no-turn stage does not require starting the HTTP fixture. Use a parent-verified unused loopback port as base_url so an unexpected model request fails, and first verify the selected model/provider/catalog and own threadId. Do not fall back to an external provider.

After explicit authorization for an actual turn, pass the HTTP fixture argv below through a process API rather than shell composition. nodeExe is a verified absolute Node path; the script uses this worktree's absolute path.

```json
[
  "<nodeExe>",
  "D:/codex/build/desktop210-core110-20261114/tests/fixtures/core-local-provider.mjs",
  "--port", "0",
  "--max-requests", "1"
]
```

Replace `<port>` in the TOML below with the actual listening JSON port. The following belongs in the parent's **fresh owned** `desktop-local-fixture.config.toml`. Use the catalog JSON's absolute path with forward slashes for `<catalogAbsolutePath>`. Do not write this sample into an actual HOME.

```toml
model = "desktop-local-fixture"
model_provider = "desktop_local_fixture"
model_catalog_json = "<catalogAbsolutePath>"
model_reasoning_effort = "high"
approval_policy = "never"
sandbox_mode = "read-only"
cli_auth_credentials_store = "file"

[model_providers.desktop_local_fixture]
name = "Desktop loopback test fixture"
base_url = "http://127.0.0.1:<port>/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
request_max_retries = 0
stream_max_retries = 0
```

Draft TUI argv: `["<verifiedCoreExe>","--profile","desktop-local-fixture","--cd","<ownProjectAbsolutePath>"]`. Supply no positional prompt. The parent first compares this with actual Core1.0.1 help and profile-selection support. Desktop's owned app-server must use the same effective provider/catalog/profile. Stop on a profile/generation mismatch; verify the actual endpoint and SessionInfo.threadId before inspecting only that thread. Reuse the parent's already verified version-specific daemon start/stop argv; this document invents no new daemon command. Start Windows background processes with hidden windows and retain only owned process handles for cleanup.

## Verification oracles and remaining limits

1. No-turn stage: observe the correct synthetic model/provider/catalog and own threadId without auth/tokens. HTTP fixture request count is zero. Current-thread MCP/hook inspection targets only that threadId/generation and observes actual state. This does not verify hook execution or tool calls.
2. Explicitly authorized single synthetic turn: health changes from 0 to 1; compare byte count/sha256, the fixed three-event message, and actual Core turn completion. Health's completedRequests counts consumed request bodies for which a response was attempted, not Core completion ACKs. Do not hide failure by resending.
3. Requests directed externally, required auth headers, or another wire/endpoint are failures. Do not add speculative response formats to this fixture. Establish differences between Core1.0.1 and the d602 family from actual failure evidence.
4. On fixture shutdown, reclaim only its own listener/connections/child resources and verify preservation of production apps/daemons/auth. Normal use, performance, memory, real provider cache, real model quality, and final Core1.1 integration are outside this preparation's scope.

This unit performed file authoring and direct source comparison only. Syntax checking, JSON parser/schema validation, server startup, HOME/config creation, catalog loading, actual Core/TUI/Desktop thread/turn execution, and MCP/hook inspection are all **NOT_RUN**.
