use super::*;

// The non-Send management permit remains on the blocking installer worker.
// Drop restores admission on download, verification or installer-launch failure.
struct DesktopUpdateGate {
    exiting: Arc<AtomicBool>,
    _extensions: codex::ExtensionPermit,
}
impl Drop for DesktopUpdateGate {
    fn drop(&mut self) { self.exiting.store(false, Ordering::Release); }
}

impl Engine {
    pub(crate) fn update_preferences(&self) -> Result<crate::desktop_updates::UpdatePreferences, String> {
        Ok(crate::desktop_updates::UpdatePreferences { background_checks: self.inner.store.lock().map_err(|_| "metadata store lock poisoned")?.background_update_checks()? })
    }

    pub(crate) fn set_update_preferences(&self, enabled: bool) -> Result<crate::desktop_updates::UpdatePreferences, String> {
        self.inner.store.lock().map_err(|_| "metadata store lock poisoned")?.set_background_update_checks(enabled)?;
        Ok(crate::desktop_updates::UpdatePreferences { background_checks: enabled })
    }

    pub(crate) fn codex_update_version(&self, deadline: Instant) -> Result<String, String> {
        let executable = self.codex_executable()?;
        codex::update_version_with_executable(&executable, &self.inner.exiting, deadline)
    }

    /// Updates never stop a session or contact/stop the shared daemon. Only an
    /// already drained app can install. Registry and allocate share the check.
    pub(crate) fn with_desktop_update_gate<T>(&self, install: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let extensions = codex::extension_admission().map_err(|_| "실행 중인 관리 작업이 있어 업데이트를 보류했습니다.")?;
        self.inner.exiting.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "앱이 이미 종료 또는 업데이트 중입니다.")?;
        let _gate = DesktopUpdateGate { exiting: self.inner.exiting.clone(), _extensions: extensions };
        {
            let runtime = self.inner.runtime.lock().map_err(|_| "실행파일 선택 잠금이 손상되었습니다.")?;
            if runtime.preparing || runtime.changing {
                return Err("실행파일 준비 또는 선택 변경이 진행 중이어서 업데이트를 보류했습니다.".into());
            }
        }
        {
            let sessions = self.inner.sessions.lock().map_err(|_| "session registry lock poisoned")?;
            for session in sessions.values() {
                if session.phase.load(Ordering::Acquire) != 3
                    || session.process.lock().map_err(|_| "session process lock poisoned")?.is_some()
                    || !*session.done.0.lock().map_err(|_| "session completion lock poisoned")?
                    || session.cleanup_failed.load(Ordering::Acquire) {
                    return Err("진행 중이거나 정리가 확인되지 않은 세션이 있어 업데이트를 보류했습니다. 세션을 직접 정리한 후 다시 시도하세요.".into());
                }
            }
        }
        install()
    }

    /// This is only an offer. The explicit exit action rechecks under admission.
    pub fn unconfirmed_exit_available(&self) -> Result<bool, String> {
        let sessions = self.inner.sessions.lock().map_err(|_| "session registry lock poisoned")?;
        let mut unknown = false;
        for session in sessions.values() {
            if session.phase.load(Ordering::Acquire) != 3
                || session.process.lock().map_err(|_| "session process lock poisoned")?.is_some()
                || !*session.done.0.lock().map_err(|_| "session completion lock poisoned")? {
                return Ok(false);
            }
            unknown |= session.cleanup_failed.load(Ordering::Acquire);
        }
        Ok(unknown)
    }

    /// Close only after the user separately accepts that remote work may continue.
    /// No remote request or persisted session status is changed by this path.
    pub fn shutdown_with_unconfirmed_remote(&self) -> Result<(), String> {
        let _extensions = codex::extension_admission().map_err(|error| format!("Cannot exit while extension management is busy: {error}"))?;
        let was_exiting = self.inner.exiting.swap(true, Ordering::AcqRel);
        // Allocate rechecks exiting while holding the same registry lock used by
        // the availability check, so a stale renderer offer cannot bypass a PTY.
        let result = self.unconfirmed_exit_available().and_then(|ready| {
            if ready { Ok(()) } else { Err("로컬 자원 정리가 완료되고 원격 중단이 미확인인 경우에만 이 종료 방식을 사용할 수 있습니다.".into()) }
        });
        if result.is_err() { self.inner.exiting.store(was_exiting, Ordering::Release); }
        result
    }
}

#[cfg(test)]
#[path = "session_unconfirmed_exit_tests.rs"]
mod tests;

#[cfg(test)]
mod update_gate_tests {
    use super::*;

    fn request() -> StartRequest {
        StartRequest { cwd: std::env::current_dir().unwrap().to_string_lossy().into_owned(), title: "update fixture".into(), project_id: None, project_assigned: false, thread_id: None, cols: 80, rows: 24 }
    }
    fn engine() -> Engine { Engine::new(Path::new(":memory:"), Arc::new(|_| Ok(()))).unwrap() }

    #[test]
    fn update_gate_excludes_new_sessions_and_releases_after_failed_install() {
        let _serial = lifecycle_tests::EXTENSION_ADMISSION_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let engine = engine();
        let result: Result<(), String> = engine.with_desktop_update_gate(|| {
            assert!(engine.allocate(&request()).is_err());
            assert!(engine.with_desktop_update_gate(|| Ok(())).is_err());
            Err("fixture installation failure".into())
        });
        assert!(result.is_err());
        assert!(!engine.inner.exiting.load(Ordering::Acquire));
        engine.allocate(&request()).unwrap().finish("exited").unwrap();
        engine.with_desktop_update_gate(|| Ok(())).unwrap();
    }

    #[test]
    fn update_gate_preserves_active_unknown_and_unreaped_sessions() {
        let _serial = lifecycle_tests::EXTENSION_ADMISSION_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let engine = engine();
        let session = engine.allocate(&request()).unwrap();
        assert!(engine.with_desktop_update_gate::<()>(|| panic!("must not install")).is_err());
        assert!(!session.stop_requested.load(Ordering::Acquire));
        assert_eq!(session.phase.load(Ordering::Acquire), 0);
        session.finish("exited").unwrap();
        session.cleanup_failed.store(true, Ordering::Release);
        assert!(engine.with_desktop_update_gate::<()>(|| panic!("must not install")).is_err());
        session.cleanup_failed.store(false, Ordering::Release);
        *session.done.0.lock().unwrap() = false;
        assert!(engine.with_desktop_update_gate::<()>(|| panic!("must not install")).is_err());
        *session.done.0.lock().unwrap() = true;
        engine.with_desktop_update_gate(|| Ok(())).unwrap();
    }

    #[test]
    fn update_gate_yields_to_management_and_existing_exit() {
        let _serial = lifecycle_tests::EXTENSION_ADMISSION_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let engine = engine();
        let permit = codex::extension_admission().unwrap();
        assert!(engine.with_desktop_update_gate(|| Ok(())).is_err());
        assert!(!engine.inner.exiting.load(Ordering::Acquire));
        drop(permit);
        engine.inner.exiting.store(true, Ordering::Release);
        assert!(engine.with_desktop_update_gate(|| Ok(())).is_err());
        assert!(engine.inner.exiting.load(Ordering::Acquire));
    }

    #[test]
    fn update_gate_session_race_has_only_one_winner_without_stop() {
        let _serial = lifecycle_tests::EXTENSION_ADMISSION_TEST.lock().unwrap_or_else(|e| e.into_inner());
        // Gate wins: keep it held until the concurrent allocation has returned.
        let engine = engine();
        let other = engine.clone();
        engine.with_desktop_update_gate(|| {
            assert!(std::thread::spawn(move || other.allocate(&request()).map(|_| ())).join().unwrap().is_err());
            Ok(())
        }).unwrap();
        // Allocation wins: the gate cannot stop or replace its in-flight work.
        let other = engine.clone();
        let session = std::thread::spawn(move || other.allocate(&request())).join().unwrap().unwrap();
        assert!(engine.with_desktop_update_gate::<()>(|| panic!("must not install")).is_err());
        assert!(!session.stop_requested.load(Ordering::Acquire));
        session.finish("exited").unwrap();
    }

    #[test]
    fn update_gate_preserves_runtime_preparation_and_selection_changes() {
        let _serial = lifecycle_tests::EXTENSION_ADMISSION_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let engine = engine();
        engine.inner.runtime.lock().unwrap().preparing = true;
        assert!(engine.with_desktop_update_gate::<()>(|| panic!("runtime preparation must finish first")).is_err());
        assert!(engine.inner.runtime.lock().unwrap().preparing);
        assert!(!engine.inner.exiting.load(Ordering::Acquire));
        {
            let mut runtime = engine.inner.runtime.lock().unwrap();
            runtime.preparing = false; runtime.changing = true;
        }
        assert!(engine.with_desktop_update_gate::<()>(|| panic!("runtime selection must finish first")).is_err());
        assert!(engine.inner.runtime.lock().unwrap().changing);
        assert!(!engine.inner.exiting.load(Ordering::Acquire));
        engine.inner.runtime.lock().unwrap().changing = false;
        engine.with_desktop_update_gate(|| Ok(())).unwrap();
    }
}
