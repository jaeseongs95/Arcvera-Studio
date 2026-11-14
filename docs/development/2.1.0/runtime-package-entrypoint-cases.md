# Runtime package entrypoint regression cases — D85

[English](runtime-package-entrypoint-cases.md) · [한국어](runtime-package-entrypoint-cases.ko.md)

Status: cases and test design written before implementation. Compilation, tests and real Native execution in this subtask are **NOT_RUN**; the parent owns serial verification. Original user goal: “너의 목표는 2.1.0 릴리즈이다. cli 담당과 협업해서 2.1.0 릴리즈에 들어갈 개발 범위를 산정하고 작업하면 된다.” English summary: deliver Desktop2.1 in collaboration with the CLI owner and agree its scope.

The observed failure is the shared technical result `D:/codex/build/avrev-a13/evidence/A13-F01.json`: actual Native 105d625/a88 plus a valid public Core1.0.1 full package failed replacement preview because the old name `codex.exe` was required. The directly read public `D:/codex/build/desktop210-integration-20261114/core101-baseline/runtime/codex-package.json` declares layoutVersion 1, version 1.0.1, target x86_64-pc-windows-msvc and entrypoint `bin/arcvera.exe`. This is existing evidence, not a new test execution.

## Minimum change and invariants

Preserve the existing `package_evidence`/`replacement_package_evidence` APIs. Accept only the exact manifest strings `bin/arcvera.exe` (current) and `bin/codex.exe` (legacy). Bind the manifest to the canonical selected CLI, `codex-app-server.exe` in the same canonical bin or the CLI itself, and the exact version/target/layout. Reject traversal, arbitrary executables, a different bin, and manifest/selection name mismatches. Runtime-only ZIP reuse requires the CLI name bound by the installed complete package's manifest and byte-identical CLI/server/`codex-code-mode-host.exe`. Copying/renaming files or weakening fallback checks is not a solution.

Post-replacement checks retain the preview's CLI name and executable hashes, including the helper hash collected for runtime-only reuse. Preserve fingerprint binding to paths/manifest/hashes/generation observations, cancellation/deadline checks, read bounds/change detection and no automatic retry. The CLI owns full-tree integrity and actual daemon replacement; preview hashes do not establish those properties. No future helper names are inferred, and this change does not expand existing full-package preview helper checks.

## engineering-test-plan / failure oracles

First add cases to the existing inline Rust native fixtures. Fixtures use synthetic bytes in UUID-owned temporary directories, without reading or running actual executables, daemons, HOME or credentials. New tests use existing APIs so that they also compile against the old implementation. Cancellation flags and expired deadlines are checked directly without sleep. CS duties bind manifest→canonical CLI→same-bin files→preview/postcheck identity and preserve cancellation/deadlines. Execution and separate AGS CLI checks are forbidden in this assignment and remain NOT_RUN.

| ID | Boundary | Oracle |
| --- | --- | --- |
| EP01 | Current full/CLI modes | Matching arcvera manifest/selection produces evidence/hashes; CLI mode has equal server/CLI hashes |
| EP02 | Legacy full/CLI | Preserve valid codex manifest/selection behavior |
| EP03 | Manifest/selection mismatch | Reject even when both executables exist with identical bytes |
| EP04 | Traversal, unsupported names, other bin | Reject `../`, backslash/absolute/unsupported entrypoints and other-bin server/CLI |
| EP05 | Current runtime-only | Reuse installed arcvera full package only when all three executable bytes match |
| EP06 | Runtime-only tampering, omission, different manifest name | Reject changed CLI/server/helper bytes, missing helper, or installed manifest CLI-name mismatch |
| EP07 | Same identity after replacement | Accept the same name/bytes; reject CLI/server/helper changes and switching CLI name plus manifest despite identical bytes |
| EP08 | Cancellation/expiry | Package/replacement reads do not succeed; retain existing fingerprint/generation tests |

First preserve the old module with the new regressions at `D:/codex/build/desktop210-integration-20261114/d85-entrypoint/before-with-regression.rs`. The parent runs RED/GREEN using identical inline test bodies, fixtures and commands, without counting compilation failures as product RED. No existing assertions are deleted, skipped or weakened. Compare the `#[cfg(test)] mod tests` bytes/hashes between before and fixed modules.

The subtask Ready report records preparation/static comparisons and digests. Fix effectiveness, actual public package preview, replacement/cancellation/generation regressions and independent audit remain unverified until the parent's actual execution.

## Written candidate and handoff status

Production changes and eight `d85_` regression tests are written. Public APIs and serialized DTOs are unchanged; private `manifest_matches` returns its validated entrypoint, and private replacement identity records the CLI name. Existing assertions are retained. Static comparison confirms identical complete inline test bodies between before and fixed modules, with UTF-8 SHA256 `ba5b6dca58a9f28292020c35305c45ceb9ec7434c74208a541540157a9996bfe`. The full before snapshot SHA256 is `d77058683f9ce0064d06e99ee6e601c84d94d0bbdc5f14cd6a7186af84d522f4`.

This means the parent can run the same tests for RED/GREEN. Compilation, tests, actual Native execution, installation/deployment and separate AGS CLI checks in this subtask are all **NOT_RUN**. The Ready report records the final candidate digest.

## Parent execution environment

Local D: free space is 23.17 GiB, below the existing native runner's 24 GiB admission floor. Local compilation and RED/GREEN remain **NOT_RUN**; the floor is retained and existing evidence is preserved. In the existing Windows CI's isolated checkout, combine production code from the observed baseline `916417d6b4c93fe253be3f94412fd05328c42936` with the identical current inline test module. Require exactly the three assertion failures corresponding to EP01, EP05 and EP07 among the eight new cases; a compilation/environment failure does not count as RED. Restore and hash-check the original fixed file bytes in `finally`, then require the same eight cases to pass. Run this pair only when the observed baseline is the immediate parent. Ordinary full `runtime_transition` regressions continue on later commits. Preserve before/fixed logs and source/test digest receipts in the native-adapter CI artifact.

These checks cover Windows filesystem boundaries with synthetic executable bytes. Actual Native preview/replacement using the public Core package and preservation of existing user sessions require the same real cases against a frozen fixed executable.
