#[test]
fn executable_uses_real_ptys_with_generation_fencing_and_bounded_cleanup() {
    let report = std::env::temp_dir().join(format!("agent-grid-native-{}.json", uuid::Uuid::new_v4()));
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_arcvera-studio"))
        .args(["--self-test", "--report"]).arg(&report).status().expect("run headless executable PTY checks");
    let text = std::fs::read_to_string(&report).expect("headless JSON report must exist");
    let result: serde_json::Value = serde_json::from_str(&text).expect("parse headless JSON report");
    assert!(status.success(), "{text}");
    assert_eq!(result["fail"], 0, "{text}");
    // Three isolation sizes, lifecycle, output cancellation, input stop, and
    // Windows descendant cleanup. The removed owned-server path has no case.
    let expected_checks = if cfg!(windows) { 7 } else { 6 };
    assert_eq!(result["tests"], expected_checks, "{text}");
    assert_eq!(result["pass"], expected_checks, "{text}");
    assert_eq!(result["iterations"], 0, "short suite must not run sustained cycles: {text}");
    assert_eq!(result["errors"], serde_json::json!([]), "{text}");
    assert_eq!(result["sessionsStarted"], if cfg!(windows) { 32 } else { 31 }, "{text}");
    #[cfg(windows)] {
        let tree = &result["processTree"];
        for field in ["baselineOwnedProcesses", "liveOwnedProcesses", "finalOwnedProcesses"] {
            assert!(tree[field].as_u64().is_some(), "프로세스 수 계측 누락: {field}; {text}");
        }
        assert_eq!(tree["finalOwnedProcesses"], tree["baselineOwnedProcesses"], "{text}");
        let processes = tree["processes"].as_array().expect("fixture 프로세스 관측 필요");
        assert_eq!(processes.len(), 2, "{text}");
        for (process, role) in processes.iter().zip(["shell", "grandchild"]) {
            assert_eq!(process["role"], role, "{text}");
            assert!(process["pid"].as_u64().is_some_and(|pid| pid > 0), "{text}");
            assert!(process["createdFiletime"].as_u64().is_some_and(|time| time > 0), "{text}");
            assert_eq!(process["aliveBeforeStop"], true, "{text}");
            assert_eq!(process["exitSignaled"], true, "{text}");
        }
    }
    std::fs::remove_file(report).expect("remove only the owned test report");
}
