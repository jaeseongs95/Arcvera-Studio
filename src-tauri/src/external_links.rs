use std::io::Write;
use std::sync::{Mutex, OnceLock};

// Explicit isolated test output only. No URL or error text enters these records.
struct LinkTraceState<W> {
    writer: W,
    attempts: u64,
    completed: u64,
    suppressed: u64,
    write_errors: u64,
    finished: bool,
}
struct LinkDiagnostics<W> { state: Mutex<LinkTraceState<W>> }
impl<W: Write> LinkDiagnostics<W> {
    fn new(writer: W) -> Self {
        let trace = Self { state: Mutex::new(LinkTraceState {
            writer, attempts: 0, completed: 0, suppressed: 0, write_errors: 0, finished: false,
        }) };
        trace.record("phase=init".to_owned());
        trace
    }
    fn write(state: &mut LinkTraceState<W>, record: &str) {
        if writeln!(state.writer, "link_open_trace owner=desktop pid={} {record}", std::process::id())
            .and_then(|_| state.writer.flush()).is_err() { state.write_errors += 1; }
    }
    fn record(&self, record: String) {
        if let Ok(mut state) = self.state.lock() { Self::write(&mut state, &record); }
    }
    fn begin(&self) -> Option<u64> {
        let mut state = self.state.lock().ok()?;
        // ceiling: bounded test trace; any suppression invalidates the evidence.
        if state.finished || state.attempts == 4096 {
            state.suppressed += 1;
            return None;
        }
        state.attempts += 1;
        let id = state.attempts;
        Self::write(&mut state, &format!("phase=attempt id={id}"));
        Some(id)
    }
    fn complete(&self, id: u64, ok: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.completed += 1;
            Self::write(&mut state, &format!("phase=completion id={id} ok={ok}"));
        }
    }
    fn finish(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.finished { return; }
            state.finished = true;
            let record = format!("phase=final attempts={} completed={} suppressed={} write_errors={}",
                state.attempts, state.completed, state.suppressed, state.write_errors);
            Self::write(&mut state, &record);
        }
    }
    fn observe<T>(&self, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let id = self.begin();
        // Release the diagnostic lock before calling the real OS opener.
        let result = work();
        if let Some(id) = id { self.complete(id, result.is_ok()); }
        result
    }
}

fn diagnostic_setting_enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}
fn create_diagnostics(setting: Option<&std::ffi::OsStr>, path: Option<&std::ffi::OsStr>)
    -> Option<LinkDiagnostics<std::fs::File>> {
    if !diagnostic_setting_enabled(setting) { return None; }
    let path = std::path::PathBuf::from(path?);
    if !path.is_absolute() { return None; }
    // Never append to, truncate or reuse another run's output.
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).ok()?;
    Some(LinkDiagnostics::new(file))
}
static LINK_DIAGNOSTICS: OnceLock<Option<LinkDiagnostics<std::fs::File>>> = OnceLock::new();
pub(crate) fn initialize_diagnostics() {
    LINK_DIAGNOSTICS.get_or_init(|| create_diagnostics(
        std::env::var_os("ARCVERA_LINK_DIAGNOSTICS").as_deref(),
        std::env::var_os("ARCVERA_LINK_DIAGNOSTICS_PATH").as_deref()));
}
pub(crate) fn finish_diagnostics() {
    if let Some(Some(trace)) = LINK_DIAGNOSTICS.get() { trace.finish(); }
}
fn launch_url(url: &str) -> Result<(), String> {
    match LINK_DIAGNOSTICS.get().and_then(Option::as_ref) {
        Some(trace) => trace.observe(|| launch_url_native(url)),
        None => launch_url_native(url),
    }
}

/// Keep OS launch policy at the native boundary, including callers bypassing TS.
fn validated_url(value: &str) -> Result<String, String> {
    let invalid = || "HTTP(S) 웹 주소만 열 수 있습니다.".to_string();
    if value.is_empty() || value.len() > 8192 || value.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return Err(invalid());
    }
    let lower = value.to_ascii_lowercase();
    let authority = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")).ok_or_else(invalid)?;
    if authority.is_empty() || authority.starts_with('/') || authority.starts_with('?') || authority.starts_with('#') { return Err(invalid()); }
    if lower.as_bytes().windows(3).any(|part| part[0] == b'%' && u8::from_str_radix(std::str::from_utf8(&part[1..]).unwrap_or(""), 16).is_ok_and(|byte| byte <= 0x1f || byte == 0x7f)) {
        return Err(invalid());
    }
    if lower.as_bytes().windows(6).any(|part| &part[..4] == b"%c2%" && matches!(part[4], b'8' | b'9') && part[5].is_ascii_hexdigit()) { return Err(invalid()); }
    let url = tauri::Url::parse(value).map_err(|_| "올바른 웹 주소가 아닙니다.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() { return Err(invalid()); }
    Ok(url.to_string())
}

#[cfg(windows)]
fn launch_url_native(url: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    // ShellExecute receives the URL as one native argument; no command shell.
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), operation.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), 1) } as isize;
    if result <= 32 { return Err(format!("기본 브라우저 실행에 실패했습니다 (Windows 오류 {result}).")); }
    Ok(())
}

#[cfg(not(windows))]
fn launch_url_native(url: &str) -> Result<(), String> {
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let result = std::process::Command::new(program).arg(url).status().map_err(|error| format!("기본 브라우저 실행에 실패했습니다: {error}"))?;
    if !result.success() { return Err(format!("기본 브라우저 실행에 실패했습니다 ({result}).")); }
    Ok(())
}

#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_url_with(&url, launch_url)).await.map_err(|error| format!("브라우저 실행 작업자 오류: {error}"))?
}

fn open_url_with(value: &str, launch: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
    launch(&validated_url(value)?)
}

#[cfg(test)]
mod tests {
    use super::{open_url_with, validated_url};

    #[test]
    fn link_trace_requires_exact_opt_in() {
        use std::ffi::OsStr;
        for value in [None, Some(""), Some("0"), Some("true"), Some(" 1"), Some("1 ")] {
            assert!(!super::diagnostic_setting_enabled(value.map(OsStr::new)));
        }
        assert!(super::diagnostic_setting_enabled(Some(OsStr::new("1"))));
    }

    #[test]
    fn link_trace_file_requires_opt_in_and_never_reuses_existing_output() {
        use std::ffi::OsStr;
        let path = std::env::temp_dir().join(format!("arcvera-link-trace-{}.log", uuid::Uuid::new_v4()));
        assert!(super::create_diagnostics(None, Some(path.as_os_str())).is_none());
        assert!(!path.exists());
        assert!(super::create_diagnostics(Some(OsStr::new("1")), Some(OsStr::new("relative.log"))).is_none());
        let trace = super::create_diagnostics(Some(OsStr::new("1")), Some(path.as_os_str())).unwrap();
        trace.finish();
        drop(trace);
        let bytes = std::fs::read(&path).unwrap();
        assert!(super::create_diagnostics(Some(OsStr::new("1")), Some(path.as_os_str())).is_none());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn link_trace_pairs_success_failure_and_zero_summary_without_private_text() {
        let trace = super::LinkDiagnostics::new(Vec::new());
        assert_eq!(trace.observe(|| Ok::<_, String>(7)), Ok(7));
        assert_eq!(trace.observe(|| Err::<(), _>("private-url-or-error".into())), Err("private-url-or-error".into()));
        trace.finish();
        trace.finish();
        let state = trace.state.lock().unwrap();
        let text = String::from_utf8(state.writer.clone()).unwrap();
        assert!(text.contains("phase=attempt id=1\n"));
        assert!(text.contains("phase=completion id=1 ok=true\n"));
        assert!(text.contains("phase=completion id=2 ok=false\n"));
        assert!(text.contains("phase=final attempts=2 completed=2 suppressed=0 write_errors=0\n"));
        assert_eq!(text.lines().count(), 6);
        assert!(!text.contains("private"));
        drop(state);
        let zero = super::LinkDiagnostics::new(Vec::new());
        zero.finish();
        assert!(String::from_utf8(zero.state.lock().unwrap().writer.clone()).unwrap()
            .contains("phase=final attempts=0 completed=0 suppressed=0 write_errors=0\n"));
    }

    #[test]
    fn link_trace_failed_sink_preserves_opener_results() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> { Err(std::io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> std::io::Result<()> { Err(std::io::ErrorKind::BrokenPipe.into()) }
        }
        let trace = super::LinkDiagnostics::new(Broken);
        assert_eq!(trace.observe(|| Ok::<_, String>(31)), Ok(31));
        assert_eq!(trace.observe(|| Err::<(), _>("native failure".into())), Err("native failure".into()));
        trace.finish();
        let state = trace.state.lock().unwrap();
        assert_eq!((state.attempts, state.completed), (2, 2));
        assert!(state.write_errors > 0);
    }

    #[test]
    fn link_trace_concurrent_ids_are_unique_and_no_lock_is_held_across_opener() {
        let trace = std::sync::Arc::new(super::LinkDiagnostics::new(Vec::new()));
        // Single-threaded reentrancy checks our lock scope, without a race with
        // another worker that can legitimately hold the diagnostic lock.
        trace.observe(|| {
            assert!(trace.state.try_lock().is_ok());
            Ok::<_, String>(())
        }).unwrap();
        let threads: Vec<_> = (0..16).map(|_| {
            let trace = trace.clone();
            std::thread::spawn(move || trace.observe(|| Ok::<_, String>(())).unwrap())
        }).collect();
        for thread in threads { thread.join().unwrap(); }
        trace.finish();
        let state = trace.state.lock().unwrap();
        let text = String::from_utf8(state.writer.clone()).unwrap();
        let ids = |phase: &str| text.lines().filter_map(|line| line.split_once(phase))
            .map(|(_, tail)| tail.split_whitespace().next().unwrap().parse::<u64>().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids("phase=attempt id="), (1..=17).collect());
        assert_eq!(ids("phase=completion id="), (1..=17).collect());
        assert_eq!((state.attempts, state.completed), (17, 17));
    }

    #[test]
    fn link_trace_rejection_emits_no_attempt_and_cap_marks_incomplete_evidence() {
        let trace = super::LinkDiagnostics::new(Vec::new());
        assert!(open_url_with("javascript:private", |_| trace.observe(|| Ok(()))).is_err());
        assert_eq!(trace.state.lock().unwrap().attempts, 0);
        for _ in 0..4097 { trace.observe(|| Ok::<_, String>(())).unwrap(); }
        trace.finish();
        let state = trace.state.lock().unwrap();
        assert_eq!((state.attempts, state.completed, state.suppressed), (4096, 4096, 1));
        assert!(state.writer.len() < 1024 * 1024);
    }
    /// Opt-in only: opens one real default-browser tab; never closes browser tabs.
    #[cfg(windows)]
    #[test]
    #[ignore = "explicit OS browser launch; set OPENDIY_REAL_BROWSER_TEST=1 and select this exact test"]
    fn real_windows_default_browser_reaches_own_loopback() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::time::{Duration, Instant};

        assert_eq!(std::env::var("OPENDIY_REAL_BROWSER_TEST").ok().as_deref(), Some("1"),
            "실제 기본 브라우저 시험에는 OPENDIY_REAL_BROWSER_TEST=1이 필요합니다.");
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("자체 loopback listener 생성 실패");
        listener.set_nonblocking(true).expect("listener nonblocking 설정 실패");
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let target = format!("/opendiy-link-smoke/{nonce}?source=native-command");
        let url = format!("http://127.0.0.1:{}{target}", listener.local_addr().unwrap().port());
        println!("EL-10 native command target: {url}");
        tauri::async_runtime::block_on(super::open_external_url(url)).expect("실제 native 외부 링크 명령 실패");
        println!("EL-10 native command: PASS; waiting for default-browser HTTP GET");

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut received = false;
        while Instant::now() < deadline {
            let mut stream: TcpStream = match listener.accept() {
                Ok((stream, peer)) => {
                    assert!(peer.ip().is_loopback(), "자체 loopback 외부의 연결을 거부했습니다.");
                    stream
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(error) => panic!("자체 loopback accept 실패: {error}"),
            };
            stream.set_nonblocking(true).expect("수신 socket nonblocking 설정 실패");
            // Bounded headers are consumed for a complete HTTP exchange, never
            // retained or printed. Only the synthetic request line is evidence.
            let mut request = Vec::new();
            let mut headers_complete = false;
            while Instant::now() < deadline && request.len() < 8192 {
                let mut chunk = [0_u8; 1024];
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(bytes) => {
                        request.extend_from_slice(&chunk[..bytes]);
                        if request.windows(4).any(|part| part == b"\r\n\r\n") { headers_complete = true; break; }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(25)),
                    Err(error) => panic!("자체 loopback HTTP 수신 실패: {error}"),
                }
            }
            if !headers_complete { continue; }
            let request_line = request.split(|byte| *byte == b'\n').next().unwrap_or_default();
            let expected = format!("GET {target} HTTP/1.1\r");
            let matches = request_line == expected.as_bytes();
            let body = if matches { "Arcvera-Studio 외부 링크 시험을 확인했습니다. 이 탭은 직접 닫아도 됩니다." } else { "페이지를 찾을 수 없습니다." };
            let response = format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", if matches { "200 OK" } else { "404 Not Found" }, body.len());
            stream.write_all(response.as_bytes()).expect("자체 loopback HTTP 응답 실패");
            stream.flush().expect("자체 loopback HTTP 응답 flush 실패");
            if matches {
                println!("EL-10 default-browser HTTP receive: PASS ({})", expected.trim_end());
                received = true;
                break;
            }
        }
        assert!(received, "기본 브라우저의 정확한 nonce HTTP GET을 20초 안에 확인하지 못했습니다.");
        // Drop only the owned listener/socket. Browser tabs/processes stay open.
    }

    #[test]
    fn native_launch_boundary_rejects_before_launch_and_preserves_failure() {
        assert!(open_url_with("javascript:alert(1)", |_| panic!("invalid URL launched")).is_err());
        let result = open_url_with("https://example.com/경로?q=한글", |url| {
            assert_eq!(url, "https://example.com/%EA%B2%BD%EB%A1%9C?q=%ED%95%9C%EA%B8%80");
            Err("native launch failed (test fixture)".to_string())
        });
        assert_eq!(result.unwrap_err(), "native launch failed (test fixture)");
    }
    #[test]
    fn external_url_boundary_accepts_http_and_unicode_query() {
        assert_eq!(validated_url("https://community.openai.com/c/codex/37").unwrap(), "https://community.openai.com/c/codex/37");
        assert_eq!(validated_url("http://example.com/a?b=1&c=2#part").unwrap(), "http://example.com/a?b=1&c=2#part");
        let unicode = validated_url("https://예시.한국/경로?q=한글").unwrap();
        assert_eq!(validated_url(&unicode).unwrap(), unicode);
        assert!(unicode.starts_with("https://xn--"));
        assert!(unicode.contains("/%EA%B2%BD%EB%A1%9C?q="));
    }
    #[test]
    fn external_url_boundary_rejects_protocols_controls_and_missing_authority() {
        for value in ["javascript:alert(1)", "file:///C:/Windows", "data:text/html,x", "mailto:a@example.com", " https://example.com", "https://example.com\n", "https://example.com/\0x", "https://example.com/%00", "https://example.com/%0a", "https://example.com/%7F", "https://example.com/%c2%9f", "https://example.com\\path", "https:///path", "https://?q=a", "https://", "https://[invalid", "https://example.com:99999/"] {
            assert!(validated_url(value).is_err(), "unexpected accepted fixture {value:?}");
        }
        assert!(validated_url(&format!("https://example.com/{}", "a".repeat(8192))).is_err());
    }
}
