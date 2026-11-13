# Native PTY resource observation

This T62 evidence unit executes the existing public v1.8.0 headless self-test in one isolated Windows CI job. It does not install the application. Source: `3460042da2024c7a3ecbdcead1655c98bbf06241`. Executable: 16,093,184 bytes, SHA256 `60ef6aeda944b6ba65baa14c5124c4d04abe2e25df7a1e2a087ce232534aeb49`.

Cases written before implementation and execution:

| Case | Actual boundary and oracle |
| --- | --- |
| NR01 | Download public release executable into the job's isolated output directory; exact size and SHA256 must match before launch. |
| NR02 | Run `--self-test --duration-seconds 1800 --report <own-directory>/self-test.json`; require exit 0, positive iterations, all reported cases passing, empty errors and duration at least 1800 seconds. The suite first runs isolation/cleanup and 100 create-stop cycles, then retains eight PTYs for its sustained heartbeat. |
| NR03 | Append own-process samples every five seconds with run ID, PID, creation time, monotonic elapsed time, memory, handles and CPU time. Require at least 361 samples, no missing measurements and no interval over 15 seconds. Preserve partial JSONL and summary even on failure. Report descriptive trends without inventing a leak slope threshold. |
| NR04 | Require final owned-job process count of one and every fixture identity in the existing process-tree report signaled exited. Normal completion uses the application's existing shutdown. On deadline/error, terminate only the exact process handle launched by this wrapper; the application's kill-on-close job owns its descendants. No global process enumeration or kill. |

The job checks initial memory/disk headroom and has one worker, a 260-minute job deadline, and a wrapper deadline of requested duration plus 15 minutes. This suite's process creation phase runs on isolated CI, not the shared two-core test server. Evidence is retained for 14 days. The wrapper records the workflow commit separately from the pinned executable source commit.

The in-process report samples owned-job counts approximately every 30 seconds. The external five-second samples cover only the executable, not aggregate descendant memory. The initial in-process sample is taken before job installation and may lack an owned-process count; it is kept as missing. Subsequent samples must contain every native resource metric. The wrapper does not force GC or infer that missing values are zero.

The self-test initializes no Tauri/WebView and uses synthetic PTYs. GUI typing/chat switching, actual CLI/provider connections, physical IME, multi-hour native application behavior and installation/upgrade remain unverified. No product defect or fix is inferred from a passing observation. See the [Korean version](native-resource-observation.ko.md).
