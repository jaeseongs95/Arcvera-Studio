//! Regression for abandoned long-lived proxies whose descendants retain stdout.
use super::*;
use std::{fs, os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle}, path::{Path, PathBuf}};
use windows_sys::Win32::System::{JobObjects::IsProcessInJob, Threading::{OpenProcess, WaitForSingleObject}};

struct Fixture {
    root: PathBuf,
    job: Arc<ProcessJob>,
    processes: Vec<OwnedHandle>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Also contains the expected RED failure, without PID-based termination.
        let _ = self.job.terminate();
        for process in &self.processes { unsafe { WaitForSingleObject(process.as_raw_handle(), 3000); } }
    }
}

fn retain_live_process(path: &Path) -> OwnedHandle {
    let pid = fs::read_to_string(path).unwrap().parse::<u32>().unwrap();
    let handle = unsafe { OpenProcess(0x00100400, 0, pid) }; // SYNCHRONIZE | PROCESS_QUERY_INFORMATION.
    assert!(!handle.is_null(), "fixture process must still exist");
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    assert_eq!(unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) }, 258, "retain a live original process, not a reused PID");
    handle
}

fn exercise_drop(after_rpc_error: bool) {
    let root = env::temp_dir().join(format!("proxy-drop-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("mode"), if after_rpc_error { "rpc-error" } else { "abandon" }).unwrap();
    fs::write(root.join("fixture.cs"), r#"using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;
class ProxyTreeFixture {
 static int Main(string[] args) {
  string root = AppDomain.CurrentDomain.BaseDirectory;
  if (args.Length == 1 && args[0] == "hold-stdout") {
   File.WriteAllText(Path.Combine(root, "descendant.pid"), Process.GetCurrentProcess().Id.ToString());
   // This process inherits the actual proxy stdout handle and never closes it.
   File.WriteAllText(Path.Combine(root, "descendant.ready"), "stdout inherited");
   Thread.Sleep(30000); return 0;
  }
  if (args.Length != 2 || args[0] != "app-server" || args[1] != "proxy") return 2;
  File.WriteAllText(Path.Combine(root, "direct.pid"), Process.GetCurrentProcess().Id.ToString());
  var input = Console.OpenStandardInput();
  var output = Console.OpenStandardOutput();
  var header = new StringBuilder();
  while (!header.ToString().EndsWith("\r\n\r\n")) { int value = input.ReadByte(); if (value < 0) return 3; header.Append((char)value); }
  string key = Array.Find(header.ToString().Split(new string[]{"\r\n"}, StringSplitOptions.None), line => line.StartsWith("Sec-WebSocket-Key:", StringComparison.OrdinalIgnoreCase)).Substring(18).Trim();
  string accept;
  using (var sha = System.Security.Cryptography.SHA1.Create()) accept = Convert.ToBase64String(sha.ComputeHash(Encoding.ASCII.GetBytes(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11")));
  byte[] response = Encoding.ASCII.GetBytes("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + "\r\n\r\n");
  output.Write(response, 0, response.Length); output.Flush();
  var child = Process.Start(new ProcessStartInfo(Process.GetCurrentProcess().MainModule.FileName, "hold-stdout") { UseShellExecute = false, CreateNoWindow = true });
  if (child == null) return 4;
  if (File.ReadAllText(Path.Combine(root, "mode")) == "rpc-error") {
   if (input.ReadByte() < 0) return 5;
   output.WriteByte(0x88); output.WriteByte(0); output.Flush();
  }
  Thread.Sleep(30000); return 0;
 }
}
"#).unwrap();
    fs::write(root.join("compile.ps1"), "param([string]$SourcePath,[string]$DestinationPath)\n$ErrorActionPreference='Stop'\nAdd-Type -Path $SourcePath -OutputAssembly $DestinationPath -OutputType ConsoleApplication\n").unwrap();
    let powershell = PathBuf::from(env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe").into_os_string();
    let args = vec!["-NoProfile".into(), "-NonInteractive".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-File".into(), root.join("compile.ps1").to_string_lossy().into_owned(), "-SourcePath".into(), root.join("fixture.cs").to_string_lossy().into_owned(), "-DestinationPath".into(), root.join("fixture.exe").to_string_lossy().into_owned()];
    extensions::command_stdout(&powershell, &root, &args, Duration::from_secs(30)).unwrap();
    let mut proxy = Proxy::open(&root.join("fixture.exe").into_os_string()).unwrap();
    assert!(proxy.expiry.is_none(), "long-lived Drop must not rely on an expiry timer");
    let mut fixture = Fixture {root, job: proxy.job.as_ref().unwrap().clone(), processes: Vec::new()};
    let ready_deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.root.join("descendant.ready").is_file() {
        assert!(Instant::now() < ready_deadline, "descendant must signal readiness");
        thread::sleep(Duration::from_millis(10));
    }
    fixture.processes.push(retain_live_process(&fixture.root.join("direct.pid")));
    fixture.processes.push(retain_live_process(&fixture.root.join("descendant.pid")));
    for process in &fixture.processes {
        let mut owned = 0;
        assert_ne!(unsafe { IsProcessInJob(process.as_raw_handle(), fixture.job.as_handle().as_raw_handle(), &mut owned) }, 0);
        assert_ne!(owned, 0, "each original fixture process must belong to this proxy Job");
    }
    assert!(fixture.job.active_processes().unwrap() >= 2, "both live fixture processes are present; platform helpers may also be owned");
    if after_rpc_error {
        let error = proxy.call("fixture/close", json!({})).unwrap_err();
        assert!(error.contains("request failed"), "{error}");
    }
    let reader = proxy.reader.take().unwrap();
    let reader_done = Arc::new(AtomicBool::new(false));
    let reader_flag = reader_done.clone();
    proxy.reader = Some(thread::spawn(move || {
        reader.join().unwrap();
        reader_flag.store(true, Ordering::Release);
    }));
    let (done_tx, done_rx) = mpsc::channel();
    let drop_deadline = Instant::now() + Duration::from_secs(4);
    let dropping = thread::spawn(move || { drop(proxy); let _ = done_tx.send(()); });
    let completed = done_rx.recv_timeout(Duration::from_secs(4)).is_ok();
    if !completed {
        eprintln!("DROP_TIMEOUT: direct_wait={} descendant_wait={} reader_done={} job_active={}",
            unsafe { WaitForSingleObject(fixture.processes[0].as_raw_handle(), 0) },
            unsafe { WaitForSingleObject(fixture.processes[1].as_raw_handle(), 0) },
            reader_done.load(Ordering::Acquire), fixture.job.active_processes().unwrap());
        fixture.job.terminate().unwrap();
        done_rx.recv_timeout(Duration::from_secs(3)).expect("emergency own-Job termination must contain RED failure");
    }
    dropping.join().unwrap();
    assert!(completed, "Proxy Drop must return within four seconds without emergency Job termination");
    assert!(reader_done.load(Ordering::Acquire), "actual pipe reader must join before Drop returns");
    assert_eq!(fixture.job.active_processes().unwrap(), 0);
    for process in &fixture.processes {
        let remaining = drop_deadline.saturating_duration_since(Instant::now()).as_millis().min(u32::MAX as u128) as u32;
        assert_eq!(unsafe { WaitForSingleObject(process.as_raw_handle(), remaining) }, 0, "original process must signal within the same Drop deadline");
    }
    fs::remove_dir_all(&fixture.root).unwrap();
}

#[test]
fn abandoned_proxy_reaps_stdout_inheriting_descendant() { exercise_drop(false); }

#[test]
fn errored_proxy_reaps_stdout_inheriting_descendant() { exercise_drop(true); }
