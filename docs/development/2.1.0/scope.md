[English](scope.md) | [한국어](scope.ko.md)

# Desktop 2.1.0 development scope and draft joint verification

2026-11-14 KST. User goal, quoted in the original Korean: “너의 목표는 2.1.0 릴리즈이다. cli 담당과 협업해서 2.1.0 릴리즈에 들어갈 개발 범위를 산정하고 작업하면 된다.”

English summary of the goal: release Desktop 2.1.0, collaborating with the CLI owner to determine and implement its development scope.

Development of 3.x is stopped; existing worktrees and unverified changes are preserved. This work covers integration between the current Rust/Tauri Desktop and Rust Core. Java 3.x code and the proposed Core HTTP/SSE contract are excluded from this release.

The Desktop baseline is the published 2.0.1 commit `beb6fe54c160d04d01d57c04c305c2fc519f9490`. The work branch is `feature/desktop-2.1.0-core110`. Remote main, checked on 2026-11-14 KST, is the 2.0.0 commit `0fa0f9c2da3f832db9d01cb7b0f00ddec90a538a`, so the version-prefix and first-preparation compatibility fixes from 2.0.1 must be preserved. Check remote main again at final merge.

The new Core owner's source is `D:/codex/build/core110-release-20261114/source`, preserving the inherited `d6026cbd`-based changes. The `ThreadContext*`/`HooksList*` candidate types and error/status values have been inspected directly. Generated schemas, runtime evidence and a development executable have not been delivered. Candidate consumption preparation is distinct from verified runtime support.

| ID | Development and verification scope | Ownership boundary |
|---|---|---|
| S01 | Match Desktop displays to authoritative context size, isolation between sessions, and compaction state; discuss the inspection contract for fragments, selection, prefixes, and cache metadata. Do not display unknown usage or expiry as zero or confirmed expiry. | Core owns configuration, model input, fragment storage, and cache state; Desktop owns requests, observations, state display, and rejection of stale responses. |
| S02 | MCP add/change/delete persistence, readback, and actual tool discovery; distinguish support and application scope for existing-thread reload. | Core owns config/runtime reload and tool state; Desktop owns narrowly scoped management requests, review, and result display. |
| S03 | Distinguish hook feature state, individual enabled state, trust approval for the current content hash, and application to the current thread. | Core owns execution and trust state; Desktop displays advertised state and requests only permitted changes. |
| S04 | Shared app-server preparation and connection; conversation, cancellation, reconnection, and error regressions. | Verify the actual executables, wire, generations, and owned resources of both candidates. |
| S05 | Lost responses, conflicts, policy overrides, permission denials, and cleanup failures at changed boundaries. | Distinguish an unknown outcome from an explicit rejection; do not automatically resend mutation requests. |
| S06 | Version 2.1.0, English/Korean public documentation, package, CI for the exact commit, and an independent audit of complete work units. | The Desktop owner integrates the work; existing separate approvals for publication, main, tags, and installation/deployment remain in place. |

Existing T78 does not require a dedicated context_index UI. Verify actual handler/wire/live/history delivery and whether Desktop changes are needed. Keep T62's full long-duration load testing recorded as incomplete carryover, and perform actual verification of the connection, queue, UI responsiveness, and resource-reclamation boundaries affected by this change. Unit tests on one side or synthetic responses alone do not establish that both products are usable.

Acceptance criteria:

- AC-001: Agree with the CLI owner on features, wire, application scope, the meaning of unsupported/unknown outcomes, and shared test fixtures.
- AC-002: Exercise the normal and failure boundaries of S01–S05 with the same Core/Desktop candidate pair, and compare observed results with state and display.
- AC-003: Preserve existing authentication, settings, user data, work in progress, and other sessions. Tests use only their own HOME/project/temporary files and processes.
- AC-004: After targeted regression and actual integration verification, build the final package and check CI for the exact candidate, licenses, and English/Korean documentation. An auditor separate from the implementer checks the final changes and evidence.
- AC-005: Report the release complete only after separately approved publication steps and post-publication verification. Disclose remaining unverified items.

Editing the work branch, installing development dependencies, testing, commit/push, and CI proceed within the assigned development scope. Separate approval boundaries remain for default-branch merge, tag/release, installation/deployment, permanent deletion, permission expansion, and production-data changes. The CLI owner's build reservations or goal-control instructions do not constitute separate user approval for this session.

This document is the working source for scope discussion and acceptance criteria. It is not a machine TaskEnvelope/validator pass or a product-completion judgment.

The Native adapter and composition/cache/TTL view are prepared against the CLI owner's actual candidate types. They distinguish fragment identity/index/source/reason/required status/size/order/residency and cache policy/observation/estimate/unavailable data. Aggregate cache counters are not assigned to individual fragments; raw content and secrets are excluded. No different RPC is inferred. Generated schemas and the same Core1.1 executable must be checked before runtime support is established. The bilingual `context-adapter-cases` and `context-composition-ui-cases` documents distinguish preparation, executed scopes and pending verification.
