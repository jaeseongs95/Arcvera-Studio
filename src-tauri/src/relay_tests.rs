use super::*;
use std::{fs, io::{BufRead, BufReader, Write}, path::PathBuf, process::{Child, Command, Stdio}, time::Instant};

#[test]
fn url_normalization_preserves_tls_and_never_accepts_remote_plaintext_or_credentials() {
    assert_eq!(normalize_url("wss://example.com/api/relay/stream?secret=not-used#x").unwrap().as_str(),"https://example.com/");
    assert_eq!(normalize_url("ws://127.0.0.1:3210/").unwrap().as_str(),"http://127.0.0.1:3210/");
    assert!(normalize_url("http://[::1]:3210").is_ok());
    for input in ["http://example.com","ws://192.168.0.1","file:///tmp/a","https://user:secret@example.com","https://example.com/private","http://127.0.0.1:0"] {assert!(normalize_url(input).is_err(),"{input}");}
}
#[test]
fn sse_handles_split_utf8_comments_crlf_and_multiline_data_and_rejects_invalid_or_large_input() {
    let bytes=": keepalive\r\ndata: {\"한글\":\r\ndata: \"값\"}\r\n\r\n".as_bytes();
    let mut decoder=SseDecoder::default();let mut output=Vec::new();
    for byte in bytes {output.extend(decoder.push(&[*byte]).unwrap());}
    assert_eq!(output,vec![json!({"한글":"값"})]);
    assert!(decoder.push(b"data: not-json\n\n").is_err());
    assert!(SseDecoder::default().push(&vec![b'x';MAX_FRAME+1]).is_err());
}
#[test]
fn duplicate_inflight_and_completed_requests_never_admit_a_second_execution() {
    let mut calls=Calls::default();
    assert!(matches!(calls.admit("one",b"fingerprint",4),Admission::New));
    assert!(matches!(calls.admit("one",b"fingerprint",4),Admission::Duplicate));
    assert!(matches!(calls.admit("one",b"different",4),Admission::Refused));
    calls.finish("one".into(),json!({"ok":true,"result":"once"}));
    match calls.admit("one",b"fingerprint",4) {Admission::Replay(reply)=>assert_eq!(reply["result"],"once"),_=>panic!("completed request must replay")}
    assert!(matches!(calls.admit("one",b"different",4),Admission::Refused));
    for index in 0..300 {let id=index.to_string();assert!(matches!(calls.admit(&id,b"f",4),Admission::New));calls.finish(id,json!({"ok":true}));}
    assert_eq!(calls.completed.len(),256);assert!(calls.bytes<=MAX_CACHE);
}
#[test]
fn pending_request_count_and_bytes_are_bounded() {
    let mut calls=Calls::default();
    assert!(matches!(calls.admit("big",b"f",MAX_FRAME),Admission::New));
    assert!(matches!(calls.admit("extra",b"f",1),Admission::Refused));
    calls.finish("big".into(),json!(null));
    for index in 0..4 {assert!(matches!(calls.admit(&index.to_string(),b"f",1),Admission::New));}
    assert!(matches!(calls.admit("fifth",b"f",1),Admission::Refused));
}
#[test]
fn remote_authority_excludes_desktop_exit_executable_selection_and_os_launch() {
    let commands=super::super::relay_dispatch::COMMANDS;
    assert_eq!(commands.len(),47);
    for blocked in ["complete_exit","unconfirmed_exit_available","set_codex_executable","set_app_server_executable","choose_codex_executable","choose_app_server_executable","apply_runtime_transition","open_external_url","relay_connect","relay_disconnect"] {assert!(!commands.contains(&blocked));}
    assert!(commands.contains(&"respond_pending_question"));assert!(commands.contains(&"session_write"));
}

struct Fixture { directory:std::path::PathBuf, child:Option<Child> }
impl Fixture {
    fn new()->Self {let directory=std::env::temp_dir().join(format!("desktop-relay-{}",uuid::Uuid::new_v4()));fs::create_dir(&directory).unwrap();Self{directory,child:None}}
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(mut child)=self.child.take() {drop(child.stdin.take());if child.try_wait().ok().flatten().is_none(){let _=child.kill();}let _=child.wait();}
        // This UUID directory was created solely by this fixture.
        fs::remove_dir_all(&self.directory).unwrap();
    }
}
#[test]
fn explicit_remote_paths_are_checked_without_native_dialogs() {
    let fixture=Fixture::new();let root=fixture.directory.to_str().unwrap();
    let file=fixture.directory.join("file.txt");fs::write(&file,"fixture").unwrap();
    let paths=super::super::relay_dispatch::paths;
    assert!(paths("choose_project_folder",&json!({"path":"relative"})).is_err());
    assert!(paths("choose_project_folder",&json!({"path":root})).unwrap().is_string());
    let listing=paths("list_directory",&json!({"path":root,"includeFiles":true})).unwrap();
    assert!(listing["currentPath"].is_string());assert_eq!(listing["entries"].as_array().unwrap().len(),1);
    assert_eq!(paths("choose_conversation_attachments",&json!({"paths":[file.to_str().unwrap()]})).unwrap()[0]["type"],"file");
    assert!(paths("choose_conversation_attachments",&json!({"paths":[root]})).is_err());
}

async fn wait_state(relay:&RelayState,expected:&str) {
    let deadline=Instant::now()+Duration::from_secs(10);
    loop {if relay.snapshot().unwrap().state==expected{return;}assert!(Instant::now()<deadline,"relay state did not reach {expected}");tokio::time::sleep(Duration::from_millis(10)).await;}
}

#[test]
fn pinned_actual_remote_relay_roundtrip_shares_owned_engine_and_disconnects() {
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(),Ok("true"),"Native relay integration is CI-only; never run local Rust");
    let mut fixture=Fixture::new();
    let script=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../verification/remote-integration/relay-fixture.mjs");
    let mut child=Command::new("node").arg(script).arg(&fixture.directory).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap();
    let mut line=String::new();BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let address:Value=serde_json::from_str(line.trim()).expect("actual relay must report listening address");
    let address=address["address"].as_str().unwrap().to_owned();fixture.child=Some(child);
    let engine=super::super::session::Engine::new(&fixture.directory.join("desktop.sqlite3"),Arc::new(|_|Ok(()))).unwrap();
    let relay=RelayState::default();let dispatch_engine=engine.clone();
    // Real Engine/storage, restricted to a read and an owned project creation;
    // this transport integration does not claim the Tauri adapter or CLI/TUI path.
    let dispatch:Dispatch=Arc::new(move |command,args|{let engine=dispatch_engine.clone();Box::pin(async move {
        match command.as_str() {
            "list_projects"=>serde_json::to_value(engine.list_projects()?).map_err(|e|e.to_string()),
            "create_project"=>serde_json::to_value(engine.create_project(args["name"].as_str().ok_or("name")?,None)?).map_err(|e|e.to_string()),
            _=>Err("Not allowed by the owned fixture".into()),
        }
    })});
    tauri::async_runtime::block_on(async {
        relay.start(&address,"relay-host-test-only".into(),dispatch.clone(),Arc::new(|_|{})).unwrap();wait_state(&relay,"connected").await;
        let client=Client::new();
        let invoke=|command:&str,args:Value|client.post(format!("{address}/api/invoke")).header("Authorization","Bearer relay-browser-test-only").header("Content-Type","application/json").body(json!({"command":command,"args":args}).to_string());
        let response=invoke("create_project",json!({"name":"remote-owned-fixture"})).send().await.unwrap();
        let result:Value=serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();assert_eq!(result["ok"],true,"{result}");
        assert_eq!(engine.list_projects().unwrap().len(),1,"relay command must update the same owned Engine");
        let response=invoke("list_projects",json!({})).send().await.unwrap();let result:Value=serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
        assert_eq!(result["result"][0]["name"],"remote-owned-fixture");
        let response=invoke("complete_exit",json!({"confirmed":true})).send().await.unwrap();assert!(!response.status().is_success());
        let before_revision=relay.snapshot().unwrap().revision;
        let original_host=relay.active.lock().unwrap().as_ref().unwrap().connection.host_id.clone();
        fixture.child.as_mut().unwrap().stdin.as_mut().unwrap().write_all(b"drop-owned-connections\n").unwrap();
        let deadline=Instant::now()+Duration::from_secs(10);
        loop {
            let current=relay.snapshot().unwrap();
            if current.state=="connected" && current.revision>=before_revision+2 {break;}
            assert!(Instant::now()<deadline,"same-host reconnection did not complete");tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(relay.active.lock().unwrap().as_ref().unwrap().connection.host_id,original_host);
        assert_eq!(engine.list_projects().unwrap().len(),1,"reconnection must not repeat project creation");
        let second=RelayState::default();second.start(&address,"relay-host-test-only".into(),dispatch.clone(),Arc::new(|_|{})).unwrap();wait_state(&second,"error").await;
        assert!(second.snapshot().unwrap().error.unwrap().contains("409"));assert!(second.stop().await.is_ok());
        let old_connection=relay.active.lock().unwrap().as_ref().unwrap().connection.clone();
        let result=relay.stop().await.unwrap();assert_eq!(result.state,"disconnected");assert_eq!(result.release_confirmed,Some(true));
        old_connection.fatal("late POST failure".into());assert_eq!(relay.snapshot().unwrap().state,"disconnected","late failure must not overwrite explicit disconnection");
        relay.start(&address,"relay-host-test-only".into(),dispatch.clone(),Arc::new(|_|{})).unwrap();wait_state(&relay,"connected").await;
        old_connection.status("error",Some("old generation".into()));assert_eq!(relay.snapshot().unwrap().state,"connected");
        assert!(relay.stop().await.unwrap().release_confirmed.unwrap());
        let bad=RelayState::default();bad.start(&address,"wrong-fixture-secret".into(),dispatch,Arc::new(|_|{})).unwrap();wait_state(&bad,"error").await;
        assert!(bad.snapshot().unwrap().error.unwrap().contains("401"));assert!(bad.stop().await.is_ok());
    });
    drop(engine);
}

#[test]
fn cache_byte_ceiling_evicts_large_replies_before_the_entry_ceiling() {
    let mut calls=Calls::default();
    for index in 0..6 {
        let id=index.to_string();
        assert!(matches!(calls.admit(&id,b"large",1),Admission::New));
        calls.finish(id,json!({"result":"x".repeat(3*1024*1024)}));
        assert!(calls.bytes<=16*1024*1024);
    }
    assert_eq!(calls.completed.len(),5,"six independently sized 3MiB replies exceed16MiB");
    assert!(matches!(calls.admit("0",b"large",1),Admission::New),"oldest large reply must be evicted");
    assert!(matches!(calls.admit("5",b"large",1),Admission::Replay(_)),"newest reply must remain replayable");
}

fn owned_relay_server(fixture:&mut Fixture)->String {
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(),Ok("true"),"CI-only native integration");
    let script=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../verification/remote-integration/relay-fixture.mjs");
    let mut child=Command::new("node").arg(script).arg(&fixture.directory).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap();
    let mut line=String::new();BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let address:Value=serde_json::from_str(line.trim()).expect("actual relay address");
    fixture.child=Some(child);address["address"].as_str().unwrap().to_owned()
}

async fn wait_owned_file(path:&std::path::Path) {
    let deadline=Instant::now()+Duration::from_secs(5);
    while !path.is_file() {
        assert!(Instant::now()<deadline,"owned fixture did not reach {}",path.display());
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[test]
fn actual_event_transport_backpressure_enforces_byte_and_channel_caps() {
    // Each scenario owns its relay, processes and files. Only event POST is
    // deliberately stalled; registration/disconnection use the actual server.
    for bytes_cap in [true,false] {
        let mut fixture=Fixture::new();let address=owned_relay_server(&mut fixture);
        fixture.child.as_mut().unwrap().stdin.as_mut().unwrap().write_all(b"pause-owned-events\n").unwrap();
        let relay=RelayState::default();
        tauri::async_runtime::block_on(async {
            wait_owned_file(&fixture.directory.join("event-pause-ready")).await;
            relay.start(&address,"relay-host-test-only".into(),Arc::new(|_,_|Box::pin(async{Err("no native command expected".into())})),Arc::new(|_|{})).unwrap();
            wait_state(&relay,"connected").await;
            relay.forward("session-changed",json!({"probe":"first"}));
            wait_owned_file(&fixture.directory.join("event-post-blocked")).await;
            let data=if bytes_cap {"x".repeat(900*1024)} else {"small".to_owned()};
            for _ in 0..40 {
                relay.forward("session-changed",json!({"probe":data}));
                let active=relay.active.lock().unwrap();let connection=&active.as_ref().unwrap().connection;
                assert!(connection.queued.load(Ordering::Acquire)<=4*1024*1024);
                if connection.stopped.load(Ordering::Acquire) {break;}
            }
            let status=relay.snapshot().unwrap();assert_eq!(status.state,"error");
            assert!(status.error.unwrap().contains(if bytes_cap {"출력 대기 상한"} else {"대기열이 가득"}));
            assert_eq!(relay.stop().await.unwrap().release_confirmed,Some(true));
            assert!(relay.active.lock().unwrap().is_none());
            relay.start(&address,"relay-host-test-only".into(),Arc::new(|_,_|Box::pin(async{Ok(Value::Null)})),Arc::new(|_|{})).unwrap();
            wait_state(&relay,"connected").await;
            assert_eq!(relay.stop().await.unwrap().release_confirmed,Some(true));
        });
    }
}

#[test]
fn accepted_commands_survive_stop_without_bypassing_global_limit_on_reconnect() {
    let mut fixture=Fixture::new();let address=owned_relay_server(&mut fixture);
    let engine=super::super::session::Engine::new(&fixture.directory.join("desktop.sqlite3"),Arc::new(|_|Ok(()))).unwrap();
    let gate=Arc::new(Semaphore::new(0));let started=Arc::new(AtomicUsize::new(0));
    let dispatch_engine=engine.clone();let dispatch_gate=gate.clone();let dispatch_started=started.clone();
    let dispatch:Dispatch=Arc::new(move |_,args| {
        let engine=dispatch_engine.clone();let gate=dispatch_gate.clone();let started=dispatch_started.clone();
        Box::pin(async move {
            started.fetch_add(1,Ordering::AcqRel);
            gate.acquire().await.unwrap().forget();
            serde_json::to_value(engine.create_project(args["name"].as_str().unwrap(),None)?).map_err(|e|e.to_string())
        })
    });
    let relay=RelayState::default();
    tauri::async_runtime::block_on(async {
        relay.start(&address,"relay-host-test-only".into(),dispatch.clone(),Arc::new(|_|{})).unwrap();wait_state(&relay,"connected").await;
        let client=Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
        let mut browsers=Vec::new();
        for index in 0..4 {
            let request=client.post(format!("{address}/api/invoke")).header("Authorization","Bearer relay-browser-test-only").header("Content-Type","application/json").body(json!({"command":"create_project","args":{"name":format!("accepted-{index}")}}).to_string());
            browsers.push(tauri::async_runtime::spawn(async move {request.send().await.unwrap()}));
        }
        let deadline=Instant::now()+Duration::from_secs(5);
        while started.load(Ordering::Acquire)!=4 {assert!(Instant::now()<deadline,"four commands not accepted");tokio::time::sleep(Duration::from_millis(10)).await;}
        assert_eq!(engine.list_projects().unwrap().len(),0);
        let old=relay.active.lock().unwrap().as_ref().unwrap().connection.clone();
        let stopped=relay.stop().await.unwrap();assert_eq!(stopped.release_confirmed,Some(true));assert_eq!(stopped.pending_commands,4);
        for browser in browsers {assert!(!browser.await.unwrap().status().is_success(),"disconnect must report unknown/failure to waiting browser");}
        relay.start(&address,"relay-host-test-only".into(),dispatch,Arc::new(|_|{})).unwrap();wait_state(&relay,"connected").await;
        let response=client.post(format!("{address}/api/invoke")).header("Authorization","Bearer relay-browser-test-only").header("Content-Type","application/json").body(json!({"command":"create_project","args":{"name":"must-not-run"}}).to_string()).send().await.unwrap();
        let denied:Value=serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
        assert_eq!(denied["ok"],false);assert_eq!(started.load(Ordering::Acquire),4);assert_eq!(engine.list_projects().unwrap().len(),0);
        gate.add_permits(4);
        let deadline=Instant::now()+Duration::from_secs(5);
        while relay.snapshot().unwrap().pending_commands!=0 {assert!(Instant::now()<deadline,"accepted work did not finish");tokio::time::sleep(Duration::from_millis(10)).await;}
        assert_eq!(engine.list_projects().unwrap().len(),4,"all accepted Engine effects must complete exactly once");
        assert_eq!(old.calls.lock().unwrap().completed.len(),4);
        old.fatal("late accepted completion".into());assert_eq!(relay.snapshot().unwrap().state,"connected");
        assert_eq!(relay.stop().await.unwrap().release_confirmed,Some(true));
    });
    drop(engine);
}
