use super::*;
use std::{fs, process::Command, sync::mpsc};
use std::os::windows::{
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::CommandExt,
};
use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

// engineering-test-plan / F2-START-STOP:
// Engine.start -> saved launcher -> real command runner is the smallest boundary
// that distinguishes session cancellation from application-exit cancellation.
// The fixture replaces only the launcher: no real CLI/server/model is invoked.
// Oracle: an in-flight query's original process handle becomes signaled, and
// start/stop both return within 5 seconds of stop (the query waits 30 seconds).
// Target mutation: start's runtime_snapshot_cancelled(&session.stop_requested)
// becomes runtime_snapshot(). Cleanup's exiting flag is set AFTER observations,
// so it cannot make that mutation pass. Native CI/red-green status: NOT_RUN.

fn compile_waiting_launcher(root: &Path) -> Result<PathBuf, String> {
    let source = root.join("launcher.cs");
    fs::write(&source, r#"using System;
using System.IO;
using System.Threading;
class Fixture {
 static int Main(string[] args) {
  if (args.Length != 1 || args[0] != "--opendiy-resolve-json") return 23;
  string root = AppDomain.CurrentDomain.BaseDirectory;
  string pending = Path.Combine(root, "query.pid.pending");
  File.WriteAllText(pending, System.Diagnostics.Process.GetCurrentProcess().Id.ToString());
  File.Move(pending, Path.Combine(root, "query.pid"));
  Thread.Sleep(30000);
  return 24;
 }
}
"#).map_err(|error| format!("write launcher fixture: {error}"))?;
    // The exact stable-launcher basename selects the production resolver path.
    let executable = root.join("opendiy-codex-launcher.exe");
    let compiler = PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot is missing")?)
        .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
    let output = Command::new(compiler).arg("/nologo")
        .arg(format!("/out:{}", executable.display())).arg(&source)
        .creation_flags(0x08000000).output()
        .map_err(|error| format!("compile launcher fixture: {error}"))?;
    if !output.status.success() {
        return Err(format!("launcher fixture compiler {}: {} {}", output.status,
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)));
    }
    Ok(executable)
}

fn observe_start_stop(root: &Path, executable: &Path) -> Result<(), String> {
    let (allocated, allocation) = mpsc::channel();
    let engine = Engine::new(Path::new(":memory:"), Arc::new(move |event| {
        if let BackendEvent::Changed(info) = event {
            if info.state == "starting" { allocated.send(info).map_err(|error| error.to_string())?; }
        }
        Ok(())
    }))?;
    engine.inner.store.lock().map_err(|_| "fixture store lock poisoned")?
        .set_codex_executable(Some(executable.to_str().ok_or("fixture path is not UTF-8")?))?;
    let request = StartRequest {
        cwd: root.to_string_lossy().into_owned(), title: "런처 조회 중 세션 중단 시험".into(),
        project_id: None, project_assigned: false, thread_id: None, cols: 80, rows: 24,
    };
    let starting = engine.clone();
    let start_worker = thread::Builder::new().name("launcher-stop-start".into())
        .spawn(move || starting.start(request)).map_err(|error| format!("spawn start worker: {error}"))?;
    let mut stop_worker = None;
    let mut original = None;
    // No assertions or early returns from this function until all workers have
    // been joined. In particular, a beacon/setup failure still cancels start.
    let observation = (|| -> Result<SessionInfo, String> {
        let info = allocation.recv_timeout(Duration::from_secs(8))
            .map_err(|error| format!("Engine.start allocation was not observed: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(8);
        let beacon = root.join("query.pid");
        while !beacon.exists() {
            if start_worker.is_finished() { return Err("start returned before entering launcher query".into()); }
            if Instant::now() >= deadline { return Err("launcher query PID beacon timed out".into()); }
            thread::sleep(Duration::from_millis(2));
        }
        // File.Move publishes only a fully written and closed PID file.
        let pid = fs::read_to_string(beacon).map_err(|error| error.to_string())?
            .parse::<u32>().map_err(|error| error.to_string())?;
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) }; // SYNCHRONIZE only
        if handle.is_null() { return Err(format!("open original query process: {}", std::io::Error::last_os_error())); }
        original = Some(unsafe { OwnedHandle::from_raw_handle(handle.cast()) });
        if start_worker.is_finished() || unsafe { WaitForSingleObject(handle, 0) } != 258 {
            return Err("launcher query was not in flight before stop".into());
        }
        let stopping = engine.clone();
        let id = info.id.clone();
        let generation = info.generation;
        // Record all three observations before fallback cancellation can act.
        let deadline = Instant::now() + Duration::from_secs(5);
        stop_worker = Some(thread::Builder::new().name("launcher-stop-stop".into())
            .spawn(move || stopping.stop(&id, generation)).map_err(|error| format!("spawn stop worker: {error}"))?);
        loop {
            let start_done = start_worker.is_finished();
            let stop_done = stop_worker.as_ref().is_some_and(|worker| worker.is_finished());
            let query_terminated = unsafe { WaitForSingleObject(handle, 0) } == 0;
            if Instant::now() >= deadline {
                return Err(format!("stop did not cancel in-flight launcher within 5 seconds: start_returned={start_done}, stop_returned={stop_done}, original_query_terminated={query_terminated}"));
            }
            if start_done && stop_done && query_terminated { return Ok(info); }
            thread::sleep(Duration::from_millis(2));
        }
    })();

    // With the target mutation, session.stop_requested is not the query's
    // cancellation source. Exiting is the independent fallback for safe cleanup.
    engine.inner.exiting.store(true, Ordering::Release);
    if let Ok(sessions) = engine.inner.sessions.lock() {
        for session in sessions.values() { session.stop_requested.store(true, Ordering::Release); }
    }
    let start_result = start_worker.join();
    let stop_result = stop_worker.map(|worker| worker.join());
    let original_terminated = original.as_ref().map(|handle| unsafe {
        WaitForSingleObject(handle.as_raw_handle().cast(), 1000)
    });
    let info = observation?;
    if original_terminated != Some(0) { return Err(format!("original query survived worker cleanup: {original_terminated:?}")); }
    let started = start_result.map_err(|_| "Engine.start worker panicked")?;
    if !matches!(started, Err(ref error) if error.contains("cancelled")) {
        return Err(format!("Engine.start must return cancellation: {started:?}"));
    }
    stop_result.ok_or("Engine.stop was not called")?.map_err(|_| "Engine.stop worker panicked")??;
    let session = engine.lookup(&info.id, info.generation)?;
    let finished = session.info()?;
    if finished.active || finished.state != "stopped" || session.process.lock().map_err(|_| "fixture process lock poisoned")?.is_some() {
        return Err(format!("cancelled launcher left an active session/process: {finished:?}"));
    }
    Ok(())
}

fn remove_launcher_fixture(root: &Path) -> Result<(), String> {
    // No recursive deletion: only this invocation's fixed fixture files, under
    // its UUID-named direct child of the verified temporary directory.
    let temp = std::env::temp_dir().canonicalize().map_err(|error| error.to_string())?;
    let actual = root.canonicalize().map_err(|error| error.to_string())?;
    if actual.parent() != Some(temp.as_path()) || actual.file_name() != root.file_name() {
        return Err("refused fixture cleanup outside its original temporary directory".into());
    }
    for name in ["launcher.cs", "opendiy-codex-launcher.exe", "query.pid.pending", "query.pid"] {
        let path = actual.join(name);
        match fs::remove_file(path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(format!("remove fixture {name}: {error}")),
        }
    }
    fs::remove_dir(actual).map_err(|error| format!("remove fixture directory: {error}"))
}

#[test]
fn stop_cancels_engine_start_during_launcher_query_and_reaps_the_original_child() {
    let _serial = lifecycle_tests::EXTENSION_ADMISSION_TEST.lock().unwrap_or_else(|error| error.into_inner());
    let root = std::env::temp_dir().join(format!("opendiy-runtime-source-stop-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let result = compile_waiting_launcher(&root).and_then(|executable| observe_start_stop(&root, &executable));
    let cleanup = remove_launcher_fixture(&root);
    assert!(result.is_ok() && cleanup.is_ok(), "F2-START-STOP: {result:?}; fixture cleanup: {cleanup:?}");
}

// RFR01-RFR08: only the existing Engine resolution/preparation boundary is used.
// Each arrival is sent after the real function captured its generation. Release
// channels form a deterministic barrier; no CLI, sleep or product I/O is used.
mod first_resolution_tests {
    use super::*;

    type Pair = (std::ffi::OsString, std::ffi::OsString, String, Option<String>);

    fn engine() -> Engine { Engine::new(Path::new(":memory:"), Arc::new(|_| Ok(()))).unwrap() }
    fn pair() -> Pair { ("cli.exe".into(), "server.exe".into(), "standalone".into(), Some("manifest-a".into())) }
    fn select(engine: &Engine, pair: &Pair) -> RuntimeSnapshot {
        engine.runtime_pair_snapshot_with(|| Ok(pair.clone())).unwrap()
    }
    fn assert_pair(snapshot: &RuntimeSnapshot, pair: &Pair) {
        assert_eq!((&snapshot.executable, &snapshot.server_executable, &snapshot.server_kind, &snapshot.source_manifest_sha256), (&pair.0, &pair.1, &pair.2, &pair.3));
    }

    struct PendingResolution {
        entered: mpsc::Receiver<()>,
        release: mpsc::Sender<()>,
        cancelled: Arc<AtomicBool>,
        worker: thread::JoinHandle<Result<RuntimeSnapshot, String>>,
    }
    impl PendingResolution {
        fn new(engine: &Engine, pair: Pair) -> Self {
            let engine = engine.clone();
            let (entered_tx, entered) = mpsc::channel();
            let (release, release_rx) = mpsc::channel();
            let cancelled = Arc::new(AtomicBool::new(false));
            let stopped = cancelled.clone();
            let worker = thread::spawn(move || engine.runtime_pair_snapshot_with(|| {
                assert!(engine.inner.runtime.try_lock().is_ok(), "resolution held the runtime mutex across external work");
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "resolution barrier was not released".to_string())?;
                if stopped.load(Ordering::Acquire) { return Err("fixture resolver cancelled".into()); }
                Ok(pair)
            }));
            Self { entered, release, cancelled, worker }
        }
        fn entered(&self) { self.entered.recv_timeout(Duration::from_secs(5)).expect("resolver did not reach the barrier"); }
        fn finish(self) -> Result<RuntimeSnapshot, String> {
            self.release.send(()).unwrap();
            self.worker.join().expect("resolution worker panicked")
        }
    }

    #[test]
    fn concurrent_first_resolution_joins_only_the_identical_pair() {
        let engine = engine();
        let pair = pair();
        let first = PendingResolution::new(&engine, pair.clone());
        first.entered();
        let second = PendingResolution::new(&engine, pair.clone());
        second.entered();
        let selected = first.finish().unwrap();
        assert_eq!(selected.generation, 1);
        let prepared = engine.prepare_selected_runtime_with(selected.clone(), false, &AtomicBool::new(false), |_| Ok(())).unwrap();
        let preparation = prepared.preparation;
        let joined = second.finish().expect("the same concurrent first resolution must join the committed pair");
        assert_eq!(joined.generation, selected.generation);
        assert_pair(&joined, &pair);
        engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
        let runtime = engine.inner.runtime.lock().unwrap();
        assert_eq!(runtime.generation, selected.generation);
        assert_eq!(runtime.preparation, preparation);
        assert!(runtime.ready && !runtime.preparing);
        drop(runtime);

        // RFR08: joining cannot bypass the existing cancellation admission fence.
        let cancelled = AtomicBool::new(true);
        assert!(engine.prepare_start_with::<()>(joined, &cancelled,
            |_| panic!("cancelled join prepared a runtime"),
            |_| panic!("cancelled join opened a connection")).is_err());
        engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
    }

    #[test]
    fn first_resolution_rejects_a_user_selection_change_without_populating_the_memo() {
        let engine = engine();
        let pending = PendingResolution::new(&engine, pair());
        pending.entered();
        engine.change_runtime_selection_with(|| Ok(())).unwrap();
        assert!(pending.finish().is_err(), "user selection changes cannot be joined as initial memoization");
        let runtime = engine.inner.runtime.lock().unwrap();
        assert_eq!(runtime.generation, 1);
        assert!(runtime.executable.is_none() && runtime.server_executable.is_none() && runtime.server_kind.is_none() && runtime.source_manifest_sha256.is_none());
        assert!(!runtime.ready && !runtime.preparing);
    }

    #[test]
    fn first_resolution_rejects_same_path_user_selection_aba_and_preserves_latest_ready() {
        let engine = engine();
        let pair = pair();
        let pending = PendingResolution::new(&engine, pair.clone());
        pending.entered();
        let original = select(&engine, &pair);
        engine.change_runtime_selection_with(|| Ok(())).unwrap();
        engine.change_runtime_selection_with(|| Ok(())).unwrap();
        let latest = select(&engine, &pair);
        assert!(latest.generation > original.generation + 1);
        let prepared = engine.prepare_selected_runtime_with(latest.clone(), false, &AtomicBool::new(false), |_| Ok(())).unwrap();
        assert!(pending.finish().is_err(), "an equal path after user selection ABA is a different generation");
        engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
        assert_eq!(engine.inner.runtime.lock().unwrap().generation, latest.generation);
        assert_pair(&latest, &pair);
    }

    #[test]
    fn first_resolution_rejects_each_changed_pair_field_without_overwriting_ready() {
        let original = pair();
        let mut cli = original.clone(); cli.0 = "other-cli.exe".into();
        let mut server = original.clone(); server.1 = "other-server.exe".into();
        let mut kind = original.clone(); kind.2 = "cli".into();
        let mut manifest = original.clone(); manifest.3 = Some("manifest-b".into());
        for changed in [cli, server, kind, manifest] {
            let engine = engine();
            let pending = PendingResolution::new(&engine, original.clone());
            pending.entered();
            let latest = select(&engine, &changed);
            let prepared = engine.prepare_selected_runtime_with(latest.clone(), false, &AtomicBool::new(false), |_| Ok(())).unwrap();
            assert!(pending.finish().is_err(), "a competing pair or manifest cannot overwrite the winner");
            engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
            assert_eq!(engine.inner.runtime.lock().unwrap().generation, latest.generation);
            assert_pair(&latest, &changed);
        }
    }

    #[test]
    fn an_already_resolved_snapshot_cannot_join_a_later_matching_pair() {
        let engine = engine();
        let original = pair();
        select(&engine, &original);
        let mut changed = original; changed.3 = Some("manifest-b".into());
        let pending = PendingResolution::new(&engine, changed.clone());
        pending.entered();
        let latest = select(&engine, &changed);
        let prepared = engine.prepare_selected_runtime_with(latest.clone(), false, &AtomicBool::new(false), |_| Ok(())).unwrap();
        assert!(pending.finish().is_err(), "only an initially empty memo permits first-resolution joining");
        engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
        assert_eq!(engine.inner.runtime.lock().unwrap().generation, latest.generation);
    }

    #[test]
    fn a_cancelled_resolver_preserves_the_committed_ready_pair() {
        let engine = engine();
        let pair = pair();
        let pending = PendingResolution::new(&engine, pair.clone());
        pending.entered();
        let latest = select(&engine, &pair);
        let prepared = engine.prepare_selected_runtime_with(latest.clone(), false, &AtomicBool::new(false), |_| Ok(())).unwrap();
        pending.cancelled.store(true, Ordering::Release);
        assert_eq!(pending.finish().unwrap_err(), "fixture resolver cancelled");
        engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
        assert_eq!(engine.inner.runtime.lock().unwrap().generation, latest.generation);
    }

    #[test]
    fn first_resolution_join_preserves_in_progress_preparation_and_its_duplicate_rejection() {
        let engine = engine();
        let pair = pair();
        let first = PendingResolution::new(&engine, pair.clone());
        first.entered();
        let second = PendingResolution::new(&engine, pair);
        second.entered();
        let selected = first.finish().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let preparing = engine.clone();
        let prepare = thread::spawn(move || preparing.prepare_selected_runtime_with(selected, false, &AtomicBool::new(false), |_| {
            assert!(preparing.inner.runtime.try_lock().is_ok());
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "prepare barrier was not released".to_string())?;
            Ok(())
        }));
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let preparation = engine.inner.runtime.lock().unwrap().preparation;
        let joined = second.finish().expect("first-resolution join must preserve the current preparation");
        {
            let runtime = engine.inner.runtime.lock().unwrap();
            assert!(!runtime.ready && runtime.preparing);
            assert_eq!(runtime.preparation, preparation);
        }
        assert!(engine.prepare_selected_runtime_with(joined, false, &AtomicBool::new(false), |_| panic!("duplicate preparation ran")).is_err());
        release_tx.send(()).unwrap();
        let prepared = prepare.join().unwrap().unwrap();
        assert_eq!(prepared.preparation, preparation);
        engine.verify_prepared_runtime(&prepared, &AtomicBool::new(false)).unwrap();
    }
}
