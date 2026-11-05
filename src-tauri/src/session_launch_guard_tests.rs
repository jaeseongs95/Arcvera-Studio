use super::*;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS},
    System::{JobObjects::{QueryInformationJobObject, JobObjectBasicAccountingInformation, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION},
        Threading::{GetCurrentProcess, OpenProcess, WaitForSingleObject}},
};

#[derive(Clone, Debug)]
struct LaunchCompletionObservation { state: String, active: bool, process_slot_empty: bool, original_child_signaled: bool }

#[test]
fn launch_post_install_failure_publishes_unknown_once_after_owned_cleanup() {
    const INJECTED: &str = "fixture post-install launch failure";
    let observations = Arc::new(Mutex::new(Vec::<LaunchCompletionObservation>::new()));
    let observed_session = Arc::new(Mutex::new(None::<std::sync::Weak<Session>>));
    let original = Arc::new(Mutex::new(None::<(u32, OwnedHandle)>));
    let (sink_observations, sink_session, sink_original) = (observations.clone(), observed_session.clone(), original.clone());
    let sink = Arc::new(move |event| {
        if let BackendEvent::Changed(info) = event {
            if matches!(info.state.as_str(), "stopped" | "error" | "unknown") {
                // The delivery callback only records. Assertions/panics here
                // would alter the failure path whose ordering we are testing.
                let session = sink_session.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(std::sync::Weak::upgrade);
                let process_slot_empty = session.as_ref().is_some_and(|session| session.process.lock().unwrap_or_else(|e| e.into_inner()).is_none());
                let original_child_signaled = sink_original.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
                    .is_some_and(|(_, handle)| unsafe { WaitForSingleObject(handle.as_raw_handle().cast(), 0) } == 0);
                sink_observations.lock().unwrap_or_else(|e| e.into_inner()).push(LaunchCompletionObservation {
                    state: info.state, active: info.active, process_slot_empty, original_child_signaled,
                });
            }
        }
        Ok(())
    });
    let engine = Engine::new(Path::new(":memory:"), sink).unwrap();
    let request = StartRequest { cwd: std::env::current_dir().unwrap().to_string_lossy().into_owned(), title: "설치 후 시작 실패 순서 시험".into(), project_id: None, project_assigned: false, thread_id: None, cols: 80, rows: 24 };
    let session = engine.allocate(&request).unwrap();
    *observed_session.lock().unwrap() = Some(Arc::downgrade(&session));
    let probe_original = original.clone();
    *engine.inner.post_install_launch_probe.lock().unwrap() = Some(Arc::new(move |session, child| {
        let pid = child.process_id().ok_or("fixture child has no PID")?;
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
        if handle.is_null() { return Err(format!("open fixture child handle: {}", std::io::Error::last_os_error())); }
        let handle = unsafe { OwnedHandle::from_raw_handle(handle.cast()) };
        *probe_original.lock().map_err(|_| "fixture child handle lock poisoned")? = Some((pid, handle));
        let process = session.process.lock().map_err(|_| "fixture process slot lock poisoned")?.clone().ok_or("fixture process slot was not installed")?;
        process.shared_cleanup_failed.store(true, Ordering::Release);
        process.shared_stop_attempted.store(true, Ordering::Release);
        session.request_stop()?;
        Err(INJECTED.into())
    }));
    let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = CommandBuilder::new(shell);
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30"]);
    let launching_engine = engine.clone();
    let launching_session = session.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let result = launching_engine.launch(launching_session, command, None, 80, 24);
        let _ = send.send(result);
    });
    // Keep the real launch closure's not-yet-started reader state. In
    // particular, do not add a test-only drainer to hide ConPTY close failures.
    let result = receive.recv_timeout(Duration::from_secs(15)).expect("실제 launch 실패 정리가 반환되지 않았습니다. ConPTY/reader 정체를 확인하세요.");
    worker.join().unwrap();
    assert_eq!(result.unwrap_err(), INJECTED);
    let observations = observations.lock().unwrap().clone();
    assert_eq!(observations.len(), 1, "완료가 정리 전 또는 두 번 발표됐습니다: {observations:?}");
    assert_eq!(observations[0].state, "unknown");
    assert!(!observations[0].active);
    assert!(observations[0].process_slot_empty, "완료 발표 시 소유 프로세스 slot이 남았습니다.");
    assert!(observations[0].original_child_signaled, "완료 발표 시 원래 소유 자식이 종료되지 않았습니다.");
    assert!(session.process.lock().unwrap().is_none());
    assert!(*session.done.0.lock().unwrap());
    assert!(session.cleanup_failed.load(Ordering::Acquire));
    let info = session.info().unwrap();
    assert_eq!(info.state, "unknown");
    assert!(!info.active);
    assert!(engine.stop(&info.id, info.generation).unwrap_err().contains("재전송하지 않았습니다"));
    println!("실제 launch 실패 완료 순서: ownedPid={}, completionCount=1, state=unknown, processSlotEmpty=true, originalChildSignaled=true", original.lock().unwrap().as_ref().unwrap().0);
}

struct WorkerFailureIdentity { pid: u32, child: OwnedHandle, job: OwnedHandle }
fn worker_failure_job_active(job: &OwnedHandle) -> Option<u32> {
    let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
    let queried = unsafe { QueryInformationJobObject(job.as_raw_handle(), JobObjectBasicAccountingInformation,
        &mut info as *mut _ as _, std::mem::size_of_val(&info) as u32, std::ptr::null_mut()) };
    (queried != 0).then_some(info.ActiveProcesses)
}

#[test]
fn launch_output_worker_install_failure_reaps_owned_child_and_drops_reader_capture() {
    worker_install_failure_case("pty-output", 0);
}

#[test]
fn launch_reaper_worker_install_failure_reaps_owned_child_and_joins_live_reader() {
    worker_install_failure_case("pty-reaper", 1);
}

fn worker_install_failure_case(name: &'static str, expected_readers: usize) {
    let observations = Arc::new(Mutex::new(Vec::<(LaunchCompletionObservation, Option<u32>, usize)>::new()));
    let observed_session = Arc::new(Mutex::new(None::<std::sync::Weak<Session>>));
    let original = Arc::new(Mutex::new(None::<WorkerFailureIdentity>));
    let (sink_observations, sink_session, sink_original) = (observations.clone(), observed_session.clone(), original.clone());
    let sink = Arc::new(move |event| {
        if let BackendEvent::Changed(info) = event {
            if matches!(info.state.as_str(), "stopped" | "error" | "unknown") {
                let session = sink_session.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(std::sync::Weak::upgrade);
                let process_slot_empty = session.as_ref().is_some_and(|session| session.process.lock().unwrap_or_else(|e| e.into_inner()).is_none());
                let identity = sink_original.lock().unwrap_or_else(|e| e.into_inner());
                let original_child_signaled = identity.as_ref().is_some_and(|identity| unsafe { WaitForSingleObject(identity.child.as_raw_handle(), 0) } == 0);
                let job_active = identity.as_ref().and_then(|identity| worker_failure_job_active(&identity.job));
                let finished_readers = crate::owned_worker::snapshot().finished;
                sink_observations.lock().unwrap_or_else(|e| e.into_inner()).push((LaunchCompletionObservation {
                    state: info.state, active: info.active, process_slot_empty, original_child_signaled,
                }, job_active, finished_readers));
            }
        }
        Ok(())
    });
    let engine = Engine::new(Path::new(":memory:"), sink).unwrap();
    let request = StartRequest { cwd: std::env::current_dir().unwrap().to_string_lossy().into_owned(), title: format!("{name} 설치 실패 시험"), project_id: None, project_assigned: false, thread_id: None, cols: 80, rows: 24 };
    let session = engine.allocate(&request).unwrap();
    *observed_session.lock().unwrap() = Some(Arc::downgrade(&session));
    let probe_original = original.clone();
    *engine.inner.post_install_launch_probe.lock().unwrap() = Some(Arc::new(move |session, child| {
        let pid = child.process_id().ok_or("fixture child has no PID")?;
        let raw = unsafe { OpenProcess(0x00100000, 0, pid) };
        if raw.is_null() { return Err(format!("open worker fixture child: {}", std::io::Error::last_os_error())); }
        let child = unsafe { OwnedHandle::from_raw_handle(raw) };
        let process = session.process.lock().map_err(|_| "fixture process slot lock poisoned")?.clone().ok_or("fixture process was not installed")?;
        let job = process.job.lock().map_err(|_| "fixture Job lock poisoned")?;
        let job = job.as_ref().ok_or("fixture Job was not installed")?;
        let mut copied = std::ptr::null_mut();
        if unsafe { DuplicateHandle(GetCurrentProcess(), job.as_handle().as_raw_handle(), GetCurrentProcess(), &mut copied, 0, 0, DUPLICATE_SAME_ACCESS) } == 0 {
            return Err(format!("duplicate worker fixture Job: {}", std::io::Error::last_os_error()));
        }
        let job = unsafe { OwnedHandle::from_raw_handle(copied) };
        *probe_original.lock().map_err(|_| "fixture identity lock poisoned")? = Some(WorkerFailureIdentity { pid, child, job });
        // Observe ownership only. The failure must occur at the real worker
        // spawn branch after this probe has returned successfully.
        Ok(())
    }));
    let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = CommandBuilder::new(shell);
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30"]);
    let session_owners_before_launch = Arc::strong_count(&session);
    let launching_engine = engine.clone();
    let launching_session = session.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let observed = crate::owned_worker::fail_once(name, || {
            let result = launching_engine.launch(launching_session, command, None, 80, 24);
            (result, crate::owned_worker::snapshot(), OWNED_CHILD_DROP_METADATA.with(|observations| observations.borrow().clone()))
        });
        let _ = send.send(observed);
    });
    let (result, trace, child_drops) = receive.recv_timeout(Duration::from_secs(15)).expect("실제 worker 설치 실패 정리가 반환되지 않았습니다. reader/ConPTY 정체를 확인하세요.");
    worker.join().unwrap();
    assert!(result.unwrap_err().contains(if name == "pty-output" { "start PTY reader:" } else { "start PTY reaper:" }));
    assert_eq!(trace.failed, 1);
    assert_eq!(trace.spawned, expected_readers);
    assert_eq!(trace.started, expected_readers);
    assert_eq!(trace.started_at_failure, expected_readers);
    assert_eq!(trace.finished_at_failure, 0, "reaper 실패 전에 첫 reader가 살아 있어야 합니다.");
    assert_eq!(trace.finished, expected_readers, "실패 task capture 또는 이미 실행한 reader가 회수되지 않았습니다.");
    let observations = observations.lock().unwrap().clone();
    assert_eq!(observations.len(), 1, "worker 설치 실패 완료가 중복됐습니다: {observations:?}");
    let (completion, job_active, finished_at_completion) = &observations[0];
    assert_eq!(completion.state, "error");
    assert!(!completion.active);
    assert!(completion.process_slot_empty);
    assert!(completion.original_child_signaled);
    assert_eq!(*job_active, Some(0));
    assert_eq!(*finished_at_completion, expected_readers, "reader 회수 전에 완료를 발표했습니다.");
    assert_eq!(Arc::strong_count(&session), session_owners_before_launch, "실패 worker task capture가 세션 소유권을 남겼습니다.");
    assert!(session.process.lock().unwrap().is_none());
    assert!(*session.done.0.lock().unwrap());
    let identity = original.lock().unwrap();
    let identity = identity.as_ref().unwrap();
    assert_eq!(child_drops, [(Some(identity.pid), true)], "child drop/wait held metadata or did not observe the original child: {child_drops:?}");
    assert_eq!(unsafe { WaitForSingleObject(identity.child.as_raw_handle(), 0) }, 0);
    assert_eq!(worker_failure_job_active(&identity.job), Some(0));
    println!("실제 worker 설치 실패: name={name}, ownedPid={}, failed=1, startedReaders={expected_readers}, finishedReaders={expected_readers}, completionCount=1, processSlotEmpty=true, originalChildSignaled=true, jobActive=0, childDropMetadataAvailable=true", identity.pid);
}

#[test]
fn failed_launch_guard_preserves_unknown_shared_stop_and_reaps_owned_pty() {
    for resume_before_stop in [false, true] { failed_launch_guard_case(resume_before_stop); }
}

fn failed_launch_guard_case(resume_before_stop: bool) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink_events = events.clone();
    let sink = Arc::new(move |event| {
        if let BackendEvent::Changed(info) = event { sink_events.lock().unwrap().push(info.state); }
        Ok(())
    });
    let engine = Engine::new(Path::new(":memory:"), sink).unwrap();
    let request = StartRequest {cwd:std::env::current_dir().unwrap().to_string_lossy().into_owned(),title:"시작 정리 시험".into(),project_id:None,project_assigned:false,thread_id:None,cols:80,rows:24};
    let session = engine.allocate(&request).unwrap();
    // process 설치 뒤 launch가 실패한 경계만 직접 구성한다. 정규 reaper가 아직
    // 없는 단계이므로 LaunchGuard가 정리 결과와 최종 상태를 모두 인계해야 한다.
    let pair = native_pty_system().openpty(size(80, 24)).unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let drainer = thread::spawn(move || { let _ = std::io::copy(&mut reader, &mut std::io::sink()); });
    let writer = pair.master.take_writer().unwrap();
    let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = CommandBuilder::new(shell);
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30"]);
    let job = ProcessJob::new(false).unwrap();
    let suspended = pair.slave.spawn_command_in_job_suspended(command, job.as_handle()).unwrap();
    let (pty_child, resume) = suspended.into_parts();
    let mut resume = Some(resume);
    let mut child = OwnedChild {child:pty_child,reaped:false,metadata:Arc::downgrade(&session)};
    drop(pair.slave);
    let owned_pid = child.child.process_id().unwrap();
    let handle = unsafe { OpenProcess(0x00100000, 0, owned_pid) };
    assert!(!handle.is_null());
    let original = unsafe { OwnedHandle::from_raw_handle(handle.cast()) };
    let process = Arc::new(ProcessControl {master:Mutex::new(Some(pair.master)),writer:Mutex::new(Some(writer)),reader:Mutex::new(None),killer:Mutex::new(child.child.clone_killer()),job:Mutex::new(Some(job)),adapter:Mutex::new(None),
        #[cfg(windows)] cron:Mutex::new(None),
        termination:Mutex::new(()),shared_cleanup_failed:AtomicBool::new(true),shared_stop_attempted:AtomicBool::new(true)});
    // 공유 RPC가 불명 결과를 기록한 인계 플래그만 주입한다. 이후 adapter가 이미
    // 회수되었더라도 추가 owned 정리가 그 불명을 성공으로 바꾸면 안 된다.
    *session.process.lock().unwrap() = Some(process.clone());
    if resume_before_stop { resume.take().unwrap().resume().unwrap(); }
    session.request_stop().unwrap();
    drop(LaunchGuard {session:session.clone(),armed:true});
    // Cancellation before commit and failure after commit must both reap the
    // same direct PTY. The retained token cannot revive a stopped generation.
    drop(resume);
    assert_eq!(unsafe { WaitForSingleObject(original.as_raw_handle().cast(), 1000) }, 0, "원래 소유 PTY는 실패 정리 반환 전에 종료되어야 한다");
    child.wait().unwrap();
    drainer.join().unwrap();
    let info = session.info().unwrap();
    assert_eq!(info.state, "unknown", "공유 중단 불명을 inactive/stopped로 바꾸면 안 된다");
    assert!(!info.active);
    assert!(session.cleanup_failed.load(Ordering::Acquire));
    assert!(session.process.lock().unwrap().is_none());
    assert!(*session.done.0.lock().unwrap());
    assert!(!events.lock().unwrap().iter().any(|state| state == "stopped"));
    assert!(engine.stop(&info.id, info.generation).unwrap_err().contains("재전송하지 않았습니다"));
    println!("시작 실패 guard: resumeBeforeStop={resume_before_stop}, ownedPid={owned_pid}, state=unknown, active=false, done=true, stoppedNotPublished=true");
}
