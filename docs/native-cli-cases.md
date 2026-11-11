# Native CLI startup cases

Scope: public v1.8.0 application, source `3460042da2024c7a3ecbdcead1655c98bbf06241`, and official Codex CLI 0.166.0 on a disposable Windows CI runner. These cases are defined before execution. [한국어](native-cli-cases.ko.md).

- NC01: Verify the application's published SHA-256 and size, and the native CLI SHA-256. A mismatch fails before execution. The selected CLI must have no sibling `codex-app-server.exe`.
- NC02: Reject registry environment names that could import credentials, a Codex home, proxy or CLI override. Create a new report root, HOME, USERPROFILE and CODEX_HOME. Clear the child environment and explicitly supply only OS paths and the isolated application/CLI settings. No existing user files are read.
- NC03: Query `app-server --help` and `app-server daemon start --help` using the isolated environment. Both installation guard options must be absent. Never start or install a daemon. Then execute only `--codex-smoke --sessions 1 --report <path>`, without diagnostic terminal capture, user text, engine controls or model requests.
- NC04: Require exit 0, the expected report schema and fresh-start mode, exactly one UUID-bound session, nonempty PTY bytes, empty errors, complete memory measurements and a final owned-process count of one. A trust/authentication prompt is an explicit failed startup; do not answer it. Retain failure reports without changing expectations.
- NC05: Bound every help process to 20 seconds and smoke to 150 seconds. Retain exact-process identity, exit status and whether deadline cleanup was needed. Terminate only the process tree created by this runner. Preserve partial artifacts even on failure.

The existing smoke harness provides startup/binding and normal cleanup assertions. This observation adds no product behavior or mutation claim. Deadline cleanup is a fallback, not successful normal cleanup. It does not establish physical GUI/IME behavior, input latency, long-run actual CLI resource stability, leak absence or full T62 acceptance. Registry isolation on the shared Windows TEST host was not sufficient, so this case uses disposable CI.
