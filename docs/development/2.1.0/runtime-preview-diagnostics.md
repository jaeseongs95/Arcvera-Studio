# Runtime preview phase diagnostics

[English](runtime-preview-diagnostics.md) · [한국어](runtime-preview-diagnostics.ko.md)

Status: hypotheses and decision criteria written before instrumentation. This is neither a confirmed cause nor proof of an improvement. Builds, tests, actual app execution and separate validation CLIs in this subtask are **NOT_RUN**; the parent owns serial verification. The goal remains Desktop 2.1.0 and real integration with the same Core 1.1 candidate. No final Core 1.1 schema/executable support is claimed.

## Observation and hypotheses

The explicitly shared technical results `D:/codex/build/avrev-a18/evidence/OPENDIY_DESKTOP-D-0085.json` and `native-run/native-input.json` show a preview timeout after valid selection using frozen Native 53/ca69 and the public Core 1.0.1 full package. This is distinct from the earlier codex-name rejection. The recorded interval is 04:16:44–04:17:10; the parent reported approximately 443 MB CLI/341 MB server files and a debug development executable. The exact checkpoint that exhausted the budget is unknown.

The existing preview shares one 20-second deadline across pair inspection, update help, package verification and streaming SHA256. Observe the **same file bytes, budget, functionality and guards**. The parent records profile, file sizes, warm/cold cache state, load and frozen executable/source identity in a separate technical receipt, not in the trace.

| Hypothesis | Discriminating observation | Limit |
| --- | --- | --- |
| H1: pair/help consumes most of the budget | Long `pair_probe`/`update_help`, with failure before/early in `package_probe` | Missing phases are not zero; distinguish not reached, process exit and missing collection |
| H2: unoptimized SHA cost | Large cumulative `file_sha256` within `file_hash`, reduced in a release-profile comparison under the same conditions | Wall time includes scheduling; profile differences alone do not prove a CPU cause |
| H3: file reads/other I/O | Large cumulative `file_read`, or a large gap between whole `file_hash` and read/SHA totals | Reads cover OS read calls only; the gap also includes open/metadata/checkpoint/other costs and does not identify a storage cause |
| Mixed/unknown | Multiple large phases or incomplete traces | Increasing timeouts, skipping hashing or weakening guards cannot replace diagnosis |

## Trace contract and preserved behavior

Only the parent's explicitly opted-in own fixture with `ARCVERA_STUDIO_RUNTIME_TRACE=1` writes to stderr. Missing/other values are off. Read the setting once at preview entry and pass it through private calls. Existing inline fixtures and postchecks keep disabled wrappers. Tests do not mutate process-global environment variables. No files, queues, threads, servers or retries are added.

Each completion record contains only the fixed prefix `runtime_preview_trace`, fixed `phase`, `elapsed_ms` and fixed `outcome`. File phases may also contain processed `bytes`. Never include paths, arguments, manifests, hashes, error text, process/thread/user identifiers or CLI output. Phases are limited to `pair_probe`, `update_help`, `package_probe`, `file_hash`, `file_read`, `file_sha256` and `preview_total`. `preview_total` outcome `blocked` means the existing `Ok` result has `can_apply=false`; it does not imply permission or success.

Accumulate only read/SHA call times, then emit each once **when the file operation finishes**. No per-chunk logging. `file_read` bytes are bytes returned by reads; `file_sha256` bytes are bytes actually supplied to hash update. A checkpoint failure after a read can make these differ. An unstarted suboperation has outcome `not_run`, not a normal zero-millisecond observation. Parent/child spans overlap and must not be added together. Normal full-package standalone preview emits at most 10 lines; runtime-only fallback at most 28. The parent collects repeated previews serially without automatically merging them.

Every normal `Result` return, including errors, completes its span. Panics, forced exits and blocked OS I/O have no completion guarantee. Ignore stderr write failures to preserve the product result; missing output then means missing evidence. Timing/stderr overhead remains inside the original deadline, and a slow sink can affect the result. Do not hide this using another thread or longer timeout.

Preserve the 20-second/1 GiB executable/16 KiB manifest limits, checkpoints, error strings and domain results, canonical paths, closed entrypoints, version/target/hash/helper/fingerprint checks, shared-server lifetime and public APIs. This does not add synchronous I/O cancellation guarantees.

## engineering-test-plan

Use existing Rust inline fixtures: unit tests do not run or read real Core/daemons/apps/HOME/auth. Write tests after this plan, then connect instrumentation. Do not delete, skip or weaken the existing eight D85 cases or full runtime_transition assertions.

| Case | Requirement/oracle | Level |
| --- | --- | --- |
| TR01 | Only exact env value `1` enables; missing/0/true/whitespace stay off | Pure input parsing |
| TR02 | Disabled trace writes zero bytes; enabled trace contains only static phase/outcome/time/count, never domain payload | Format/privacy boundary |
| TR03 | Preserve success/error closure results and distinguish completion outcomes; writer errors do not change domain results | Trace wrapper |
| TR04 | Preserve file read/hash bytes/digest and cancellation/expiry/missing-file errors; unstarted suboperations are not_run | Synthetic filesystem |
| TR05 | Classify allowed/blocked/error previews without changing results; do not fill missing phases with normal values | Existing preview fixture |

The parent preserves complete traces and original domain results for actual same-file debug/release-profile comparisons. Check trace off/on regressions, actual stderr capture/bounded line count/privacy, and completion records on pair/help/hash errors. Synthetic fixtures cannot establish H1/H2/H3 or claim the original Native failure is resolved.

## Written candidate handoff

The parent CI retains the builder's default debug fixture and adds a separate `-FixtureProfile release` executable. This comparator has independent manifests/resources/config under `release-profile/` and identifier `fixture<commit12>release`. It is an isolated optimization-profile comparator, not a final distribution or installation. Both retain the original 20-second budget and validation guards. When collecting results, bind the same CLI/server files and SHA values, load/cache conditions and actual traces/domain results. Build-profile names alone do not confirm a hypothesis.

Fixed-phase instrumentation and five inline unit tests are written. The eight D85 cases and all other existing assertions are retained. The Ready report records static diff/scope, UTF-8/LF and guard checks plus final file SHA256 values. Builds, tests and actual trace collection are **NOT_RUN**. H1/H2/H3 remain hypotheses pending the parent's comparable observations and existing regressions.
