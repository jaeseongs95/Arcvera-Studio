//! Explicit owned-fixture entry point. Reuses Engine; never initializes Tauri.
use crate::{codex::RuntimeTransitionPreview, process_job, session::Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::{self, File, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, sync::Arc};

const MARKER: &str = "opendiy-runtime-test-v1\n";
const CONFIRMATION_ERROR: &str = "서버 전환은 앱에서 작업 종료와 연결 변경을 확인한 뒤에만 실행할 수 있습니다.";
const STALE_ERROR: &str = "서버 또는 선택한 패키지가 바뀌었습니다. 버전을 다시 확인한 뒤 전환하세요.";

struct Options { root: PathBuf, cli: PathBuf, server: PathBuf, report: PathBuf, action: String, fingerprint: Option<String>, replace_existing: bool }

fn options(args: &[String]) -> Result<Options, String> {
    let mut values = BTreeMap::new();
    let mut replace_existing = false;
    let mut entry = false;
    let mut index = 1;
    while index < args.len() {
        let name = args[index].as_str();
        match name {
            "--runtime-transition-test" if !entry => entry = true,
            "--replace-existing" if !replace_existing => replace_existing = true,
            "--root" | "--cli" | "--server" | "--report" | "--action" | "--fingerprint" => {
                index += 1;
                let value = args.get(index).filter(|v| !v.starts_with("--")).ok_or_else(|| format!("missing value: {name}"))?;
                if values.insert(name, value.clone()).is_some() { return Err(format!("duplicate option: {name}")); }
            }
            _ => return Err(format!("unknown or duplicate runtime test option: {name}")),
        }
        index += 1;
    }
    if !entry { return Err("explicit --runtime-transition-test is required".into()); }
    let required = |name| values.get(name).cloned().ok_or_else(|| format!("required option: {name}"));
    let action = required("--action")?;
    if !matches!(action.as_str(), "preview" | "cancel" | "stale" | "apply") { return Err("action must be preview/cancel/stale/apply".into()); }
    let fingerprint = values.get("--fingerprint").cloned();
    if matches!(action.as_str(), "stale" | "apply") {
        if !fingerprint.as_ref().is_some_and(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))) {
            return Err("stale/apply requires the exact 64-hex --fingerprint".into());
        }
    } else if fingerprint.is_some() { return Err("fingerprint is only valid for stale/apply".into()); }
    Ok(Options { root: required("--root")?.into(), cli: required("--cli")?.into(), server: required("--server")?.into(), report: required("--report")?.into(), action, fingerprint, replace_existing })
}

fn inside(path: &Path, root: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() { return Err("fixture paths must be absolute".into()); }
    let path = path.canonicalize().map_err(|e| format!("canonical fixture path: {e}"))?;
    if path == root || !path.starts_with(root) { return Err("path escapes the marked fixture root".into()); }
    Ok(path)
}

fn guard(opts: &Options, mut environment: impl FnMut(&str) -> Option<PathBuf>) -> Result<(PathBuf, PathBuf), String> {
    if !opts.root.is_absolute() { return Err("fixture root must be absolute".into()); }
    let root = opts.root.canonicalize().map_err(|e| format!("fixture root: {e}"))?;
    let marker = inside(&root.join(".opendiy-runtime-test-owned"), &root)?;
    if marker.parent() != Some(root.as_path()) || fs::metadata(&marker).map_err(|e| e.to_string())?.len() != MARKER.len() as u64
        || fs::read_to_string(marker).map_err(|e| e.to_string())? != MARKER { return Err("invalid owned fixture marker".into()); }
    let home = inside(&environment("CODEX_HOME").ok_or("own CODEX_HOME is required")?, &root)?;
    if !home.is_dir() { return Err("CODEX_HOME must be an owned directory".into()); }
    for key in ["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP"] {
        let path = inside(&environment(key).ok_or_else(|| format!("own {key} is required"))?, &root)?;
        if !path.is_dir() { return Err(format!("{key} must be an owned directory")); }
    }
    for path in [&opts.cli, &opts.server] {
        if !inside(path, &root)?.is_file() { return Err("selected executable must be a fixture file".into()); }
    }
    if !opts.report.is_absolute() || opts.report.file_name().is_none() { return Err("report must be an absolute new file".into()); }
    inside(opts.report.parent().ok_or("report parent is required")?, &root)?;
    Ok((root, home))
}

fn identity(path: &Path) -> Result<Value, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() == 0 || before.len() > 1024 * 1024 * 1024 { return Err("fixture executable must be a nonempty file <=1GiB".into()); }
    let mut hash = Sha256::new();
    let mut block = [0u8; 64 * 1024];
    let mut length = 0u64;
    loop {
        let n = file.read(&mut block).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        length += n as u64;
        if length > 1024 * 1024 * 1024 { return Err("fixture executable grew beyond limit".into()); }
        hash.update(&block[..n]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if length != before.len() || after.len() != length || before.modified().ok() != after.modified().ok() { return Err("fixture executable changed while hashing".into()); }
    Ok(json!({"path":path,"bytes":length,"sha256":format!("{:x}",hash.finalize())}))
}

fn preview_paths(preview: &RuntimeTransitionPreview, root: &Path, home: &Path) -> Result<(), String> {
    if let Some(path) = &preview.management_executable { observed_path(Path::new(path), root)?; }
    if let Some(path) = &preview.pair.managed_cli_executable { observed_path(Path::new(path), home)?; }
    if let Some(path) = &preview.pair.socket_path {
        observed_path(Path::new(path), home)?;
    }
    Ok(())
}

// A passive preview may name an installation/endpoint which does not exist yet.
// Resolve every existing ancestor and reject traversal; never create that path.
fn observed_path(path: &Path, root: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) { return Err("invalid observed fixture path".into()); }
    let mut ancestor = path;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                let canonical = ancestor.canonicalize().map_err(|e| e.to_string())?;
                if !canonical.starts_with(root) { return Err("observed runtime path escapes owned fixture".into()); }
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ancestor = ancestor.parent().ok_or("observed path has no owned ancestor")?,
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn observe(opts: &Options, root: &Path, home: &Path) -> Result<Value, String> {
    process_job::install_app_job()?;
    let cli = inside(&opts.cli, root)?;
    let server = inside(&opts.server, root)?;
    let engine = Engine::new(Path::new(":memory:"), Arc::new(|_| Ok(())))?;
    engine.set_codex_executable(Some(cli.to_str().ok_or("CLI path is not UTF-8")?))?;
    engine.set_app_server_executable(Some(server.to_str().ok_or("server path is not UTF-8")?))?;
    let preview = engine.runtime_transition_preview_mode(opts.replace_existing)?;
    preview_paths(&preview, root, home)?;
    let mut result = json!({"preview":preview,"daemonPidEvidence":"NOT_RUN: caller must bind owned daemon PID/image to this endpoint; observerPid is this Desktop process only"});
    let accepted = match opts.action.as_str() {
        "preview" => true,
        "cancel" => match engine.apply_runtime_transition_mode(false, &preview.fingerprint, opts.replace_existing) {
            Err(error) => { let valid = error == CONFIRMATION_ERROR; result["rejection"] = json!(error); valid }
            Ok(reply) => { result["unexpectedReply"] = json!(reply); false }
        },
        "stale" => {
            let fingerprint = opts.fingerprint.as_deref().expect("validated fingerprint");
            if !preview.can_apply || fingerprint == preview.fingerprint { return Err("stale test needs an applicable preview and a different fingerprint".into()); }
            match engine.apply_runtime_transition_mode(true, fingerprint, opts.replace_existing) {
                Err(error) => { let valid = error == STALE_ERROR; result["rejection"] = json!(error); valid }
                Ok(reply) => { result["unexpectedReply"] = json!(reply); false }
            }
        }
        "apply" => {
            // Exactly one apply. Unknown/lost responses are never retried.
            match engine.apply_runtime_transition_mode(true, opts.fingerprint.as_deref().expect("validated fingerprint"), opts.replace_existing) {
                Ok(reply) => { let valid = reply.success; result["reply"] = json!(reply); valid }
                Err(error) => { result["error"] = json!(error); false }
            }
        }
        _ => unreachable!("validated action"),
    };
    result["selectedAfter"] = json!({"cli":identity(&cli)?,"server":identity(&server)?});
    result["actionAccepted"] = json!(accepted);
    engine.shutdown()?;
    result["appJobProcesses"] = json!(process_job::app_owned_processes()?);
    result["cleanupScope"] = json!("No sessions created. App Job excludes explicitly detached shared daemon; caller owns daemon teardown in the isolated fixture.");
    Ok(result)
}

fn write_report(file: &mut File, report: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    file.set_len(0).map_err(|e| e.to_string())?;
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
}

pub fn run() -> i32 {
    let outcome = (|| -> Result<i32, String> {
        let opts = options(&std::env::args().collect::<Vec<_>>())?;
        let (root, home) = guard(&opts, |key| std::env::var_os(key).map(PathBuf::from))?;
        // Exclusive creation precedes every executable invocation; never overwrite evidence.
        let mut file = OpenOptions::new().write(true).create_new(true).open(&opts.report).map_err(|e| format!("exclusive report creation: {e}"))?;
        let mut report = json!({"schema":"opendiy-runtime-transition-test-v1","observerPid":std::process::id(),"action":opts.action,"replaceExisting":opts.replace_existing,"fixtureRoot":root,"codexHome":home,"requestedFingerprint":opts.fingerprint,"observationCompleted":false,"scope":"Actual in-memory Engine runtime transition only; no GUI/TUI/model or production runtime validation."});
        write_report(&mut file, &report)?;
        report["desktop"] = identity(&std::env::current_exe().map_err(|e| e.to_string())?)?;
        report["selectedBefore"] = json!({"cli":identity(&inside(&opts.cli,&root)?)?,"server":identity(&inside(&opts.server,&root)?)?});
        write_report(&mut file, &report)?;
        let exit = match observe(&opts, &root, &home) {
            Ok(result) => { let accepted = result["actionAccepted"] == true; report["observationCompleted"] = json!(true); report["result"] = result; if accepted { 0 } else { 1 } }
            Err(error) => { report["error"] = json!(error); 1 }
        };
        report["exitCode"] = json!(exit);
        write_report(&mut file, &report)?;
        Ok(exit)
    })();
    match outcome { Ok(code) => code, Err(error) => { eprintln!("runtime transition test rejected: {error}"); 2 } }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(action: &str) -> Vec<String> { ["desktop", "--runtime-transition-test", "--root", "root", "--cli", "cli", "--server", "server", "--report", "report", "--action", action].map(str::to_owned).to_vec() }
    #[test]
    fn options_refuse_ambiguous_or_incomplete_operations() {
        for action in ["preview", "cancel"] { assert!(options(&args(action)).is_ok()); }
        for action in ["apply", "stale", "unknown"] { assert!(options(&args(action)).is_err()); }
        let mut a = args("apply"); a.extend(["--fingerprint".into(), "a".repeat(64)]); assert!(options(&a).is_ok());
        a.push("--codex-smoke".into()); assert!(options(&a).is_err());
        let mut a = args("preview"); a.extend(["--action".into(), "apply".into()]); assert!(options(&a).is_err());
    }
    #[test]
    fn fixture_guard_rejects_external_environment_and_executables() {
        let root = std::env::temp_dir().join(format!("opendiy-runtime-guard-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("own")).unwrap(); fs::create_dir(root.join("reports")).unwrap();
        fs::write(root.join(".opendiy-runtime-test-owned"), MARKER).unwrap();
        fs::write(root.join("cli.exe"), b"not executed").unwrap(); fs::write(root.join("server.exe"), b"not executed").unwrap();
        let root = root.canonicalize().unwrap();
        let mut o = Options { cli:root.join("cli.exe"), server:root.join("server.exe"), report:root.join("reports/result.json"), root:root.clone(), action:"preview".into(), fingerprint:None, replace_existing:false };
        assert!(guard(&o, |_| Some(root.join("own"))).is_ok());
        assert!(observed_path(&root.join("own/not-created/daemon.sock"), &root.join("own")).is_ok());
        assert!(observed_path(&root.join("own/../outside.sock"), &root.join("own")).is_err());
        assert!(observed_path(&std::env::temp_dir().join("outside-daemon.sock"), &root.join("own")).is_err());
        assert!(guard(&o, |key| (key != "CODEX_HOME").then(|| root.join("own"))).is_err());
        assert!(guard(&o, |key| Some(if key == "USERPROFILE" { std::env::temp_dir() } else { root.join("own") })).is_err());
        o.cli = std::env::current_exe().unwrap(); assert!(guard(&o, |_| Some(root.join("own"))).is_err());
        o.cli = root.join("cli.exe"); o.report = std::env::temp_dir().join("outside-runtime-report.json"); assert!(guard(&o, |_| Some(root.join("own"))).is_err());
        o.report = root.join("reports/result.json"); fs::write(&o.report, b"original evidence").unwrap();
        assert!(OpenOptions::new().write(true).create_new(true).open(&o.report).is_err());
        assert_eq!(fs::read(&o.report).unwrap(), b"original evidence");
        fs::write(root.join(".opendiy-runtime-test-owned"), "wrong marker").unwrap(); assert!(guard(&o, |_| Some(root.join("own"))).is_err());
        assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(root.file_name().unwrap().to_string_lossy().starts_with("opendiy-runtime-guard-"));
        fs::remove_dir_all(root).unwrap();
    }
}
