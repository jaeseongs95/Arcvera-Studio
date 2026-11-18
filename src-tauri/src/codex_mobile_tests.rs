use super::*;

fn status_value() -> Value { json!({"status":"connected","serverName":"fixture","environmentId":"env-fixture","installationId":"not-exported"}) }
fn fixture_connection(_endpoint: String) -> MobileConnection {
    MobileConnection::shared(shared::SharedEndpoint::test_fixture(), Arc::new(AtomicBool::new(false)))
}
// In-memory protocol boundary over a synthetic socket; production only opens a Core proxy.
impl MobileConnection {
    fn run_stream(&self, action: MobileAction, stream: TcpStream, timeout: Duration) -> Result<MobileReply, String> {
        self.run_stream_version(action, stream, timeout, Some("1.0.0+fixture"))
    }
    fn run_stream_version(&self, action: MobileAction, stream: TcpStream, timeout: Duration, version: Option<&str>) -> Result<MobileReply, String> {
        // Handshake setup is synchronous; the short RPC polling interval must
        // not expire while the fixture server is still scheduling its reply.
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        stream.set_write_timeout(Some(timeout)).unwrap();
        let (ws, _) = tungstenite::client("ws://localhost/", stream).map_err(|e| e.to_string())?;
        ws.get_ref().set_read_timeout(Some(Duration::from_millis(10))).unwrap();
        let mut rpc = Rpc { core_version:version.map(str::to_owned), ws, next_id:0, cancelled:Some(self.0.stopped.clone()), deadline:Some(Instant::now()+timeout) };
        self.run_rpc_until(action, &mut rpc, Instant::now()+timeout)
    }
}
fn fixture(operation: impl FnOnce(WebSocket<TcpStream>) + Send + 'static) -> (MobileConnection, TcpStream, JoinHandle<()>) {
    fixture_with_handshake_delay(Duration::ZERO, operation)
}
fn fixture_with_handshake_delay(delay: Duration, operation: impl FnOnce(WebSocket<TcpStream>) + Send + 'static) -> (MobileConnection, TcpStream, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        thread::sleep(delay);
        let ws = tungstenite::accept(stream).unwrap();
        operation(ws);
    });
    (fixture_connection(format!("ws://{address}")), TcpStream::connect(address).unwrap(), worker)
}

#[test]
fn delayed_handshake_does_not_use_the_rpc_poll_interval() {
    let (connection, stream, worker) = fixture_with_handshake_delay(Duration::from_millis(100), |mut ws| {
        initialize_fixture(&mut ws);
        let request = read(&mut ws);
        assert_eq!(request["method"], "remoteControl/status/read");
        reply(&mut ws, &request, status_value());
        assert!(ws.read().is_err());
    });
    let result = connection.run_stream(MobileAction::Status, stream, Duration::from_secs(2));
    worker.join().unwrap();
    assert_eq!(result.unwrap().status.unwrap().status, "connected");
}
fn read(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(text) = ws.read().unwrap() { return serde_json::from_str(&text).unwrap(); }
    }
}
fn reply(ws: &mut WebSocket<TcpStream>, request: &Value, result: Value) {
    ws.send(Message::Text(json!({"id":request["id"],"result":result}).to_string().into())).unwrap();
}
fn initialize_fixture(ws: &mut WebSocket<TcpStream>) {
    let init = read(ws);
    assert_eq!(init["method"], "initialize");
    assert_eq!(init["params"]["clientInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(init["params"]["capabilities"]["extensions"]["arcvera/core"]["version"], "1.0.0+fixture");
    assert_eq!(init["params"]["capabilities"]["experimentalApi"], true);
    reply(ws, &init, json!({}));
    assert_eq!(read(ws)["method"], "initialized");
}

#[test]
fn unverified_or_unsupported_core_sends_no_mobile_initialize() {
    for version in [None, Some(""), Some("0.99.0"), Some("1.0.0-rc.1"), Some("2.0.0"), Some("codex-cli 1.0.0")] {
        let (connection, stream, worker) = fixture(|mut ws| {
            assert!(ws.read().is_err(), "unverified Core must receive no initialize or management request");
        });
        assert!(connection.run_stream_version(MobileAction::Enable, stream, Duration::from_secs(2), version).is_err());
        worker.join().unwrap();
    }
}

#[test]
fn initialize_rejection_is_distinct_from_authentication_and_sends_nothing_else() {
    for code in [-32600, -32601, -32000] {
        let (connection, stream, worker) = fixture(move |mut ws| {
            let request = read(&mut ws);
            assert_eq!(request["method"], "initialize");
            assert_eq!(request["params"]["capabilities"]["extensions"]["arcvera/core"]["version"], "1.0.0+fixture");
            ws.send(Message::Text(json!({"id":request["id"],"error":{"code":code,"message":"Incompatible Arcvera Core client: Bearer private-capability","data":{"secret":"body-secret"}}}).to_string().into())).unwrap();
            assert!(ws.read().is_err(), "initialize rejection must not be followed by initialized or management requests");
        });
        let started = Instant::now();
        let error = connection.run_stream(MobileAction::Enable, stream, Duration::from_secs(2)).err().unwrap();
        assert!(error.contains("Arcvera Core") && error.contains("초기화"), "initialize rejection was misclassified: {error}");
        if code == -32600 { assert!(error.contains("호환")); }
        assert!(!error.contains("로그인"));
        for secret in ["Bearer", "private-capability", "body-secret"] { assert!(!error.contains(secret)); }
        assert!(started.elapsed() < Duration::from_secs(1), "explicit rejection must return before the RPC timeout");
        worker.join().unwrap();
    }
}

#[test]
fn initialize_without_reply_is_bounded_and_sends_nothing_else() {
    let (connection, stream, worker) = fixture(|mut ws| {
        assert_eq!(read(&mut ws)["method"], "initialize");
        assert!(ws.read().is_err(), "unanswered initialize must not continue to management requests");
    });
    let started = Instant::now();
    let error = connection.run_stream(MobileAction::Enable, stream, Duration::from_millis(80)).err().unwrap();
    assert!(error.contains("초과"), "expected initialization timeout: {error}");
    assert!(started.elapsed() < Duration::from_secs(1));
    worker.join().unwrap();
}

#[test]
fn four_actions_are_closed_and_mutations_are_ephemeral() {
    for action in [MobileAction::Status,MobileAction::Enable,MobileAction::Disable,MobileAction::Pair] {
        let (connection, stream, worker) = fixture(move |mut ws| {
            initialize_fixture(&mut ws);
            let status = read(&mut ws);
            assert_eq!(status["method"], "remoteControl/status/read");
            reply(&mut ws, &status, status_value());
            if !matches!(action, MobileAction::Status) {
                let request = read(&mut ws);
                let result = match action {
                    MobileAction::Enable => {assert_eq!(request["method"],"remoteControl/enable");assert_eq!(request["params"],json!({"ephemeral":true}));status_value()},
                    MobileAction::Disable => {assert_eq!(request["method"],"remoteControl/disable");assert_eq!(request["params"],json!({"ephemeral":true}));status_value()},
                    MobileAction::Pair => {assert_eq!(request["method"],"remoteControl/pairing/start");assert_eq!(request["params"],json!({"manualCode":true}));json!({"pairingCode":"machine-code-must-not-escape","manualPairingCode":"ABCD-1234","environmentId":"env-fixture","expiresAt":2000000000})},
                    MobileAction::Status => unreachable!(),
                };
                reply(&mut ws,&request,result);
            }
            assert!(ws.read().is_err()); // Every success closes its one management connection.
        });
        let result = connection.run_stream(action,stream,Duration::from_secs(2)).unwrap();
        assert!(result.shared && result.mutations_allowed);
        assert_eq!(result.status.is_none(),matches!(action,MobileAction::Pair));
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(!encoded.contains("machine-code"));
        assert!(!encoded.contains("installationId"));
        assert!(!encoded.contains("fixture-private-capability"));

        worker.join().unwrap();
    }
    for invalid in ["stop","start","remoteControl/enable","restart",""] { assert!(serde_json::from_value::<MobileAction>(json!(invalid)).is_err()); }
}

#[test]
fn errors_never_echo_server_secrets_and_never_continue_to_a_mutation() {
    for code in [-32601,-32000] {
        let (connection,stream,worker) = fixture(move |mut ws| {
            initialize_fixture(&mut ws);
            let request=read(&mut ws);
            ws.send(Message::Text(json!({"id":request["id"],"error":{"code":code,"message":"Bearer fixture-private-capability machine-code","data":{"secret":"body-secret"}}}).to_string().into())).unwrap();
            assert!(ws.read().is_err());
        });
        let error=connection.run_stream(MobileAction::Enable,stream,Duration::from_secs(2)).err().unwrap();
        assert!(!error.contains("Bearer"));assert!(!error.contains("private-capability"));assert!(!error.contains("body-secret"));
        assert!(error.contains(if code == -32601 {"지원하지"} else {"ChatGPT"}));
        worker.join().unwrap();
    }
}

#[test]
fn cancellation_closes_pending_read_and_clones_share_admission() {
    let (ready_tx,ready_rx)=mpsc::channel();
    let (connection,stream,worker)=fixture(move |mut ws| {
        initialize_fixture(&mut ws);let _request=read(&mut ws);ready_tx.send(()).unwrap();
        assert!(ws.read().is_err());
    });
    let clone=connection.clone();
    let request=thread::spawn(move || clone.run_stream(MobileAction::Status,stream,Duration::from_secs(2)));
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();connection.cancel();
    assert!(request.join().unwrap().is_err());
    worker.join().unwrap();
    assert!(connection.request(MobileAction::Enable).is_err());
    let fresh=fixture_connection("ws://127.0.0.1:1".into());
    let _guard=fresh.0.admission.lock().unwrap();
    assert!(fresh.clone().request(MobileAction::Status).err().unwrap().contains("진행 중"));
}

#[test]
fn timeout_and_oversized_response_close_the_socket() {
    for oversized in [false,true] {
        let (connection,stream,worker)=fixture(move |mut ws| {
            initialize_fixture(&mut ws);let request=read(&mut ws);
            if oversized { let _=ws.send(Message::Text(json!({"id":request["id"],"result":{"secret":"x".repeat(MAX_REPLY+1)}}).to_string().into())); }
            assert!(ws.read().is_err());
        });
        let error=connection.run_stream(MobileAction::Status,stream,Duration::from_millis(30)).err().unwrap();
        assert!(!error.contains("secret"));assert!(!error.contains("private-capability"));
        if !oversized { assert!(error.contains("초과")); }
        worker.join().unwrap();
    }
}

#[test]
fn response_shape_bounds_are_fail_closed() {
    assert!(parse_status(&status_value(),"private").is_ok());
    for value in [json!({}),json!({"status":"unknown","serverName":"ok","environmentId":null}),json!({"status":"connected","serverName":"x".repeat(257),"environmentId":null}),json!({"status":"connected","serverName":"Bearer private","environmentId":null})] { assert!(parse_status(&value,"private").is_err()); }
    for value in [json!({"manualPairingCode":null}),json!({"manualPairingCode":"bad\ncode","expiresAt":1,"environmentId":"id"}),json!({"manualPairingCode":"ok","expiresAt":0,"environmentId":"id"})] { assert!(parse_pairing(&value,"private").is_err()); }
}

#[test]
fn server_requests_with_matching_ids_do_not_complete_management_calls() {
    let (connection, stream, worker) = fixture(|mut ws| {
        let init = read(&mut ws);
        assert_eq!(init["method"], "initialize");
        ws.send(Message::Text(json!({"id":init["id"],"method":"item/commandExecution/requestApproval","params":{}}).to_string().into())).unwrap();
        reply(&mut ws, &init, json!({}));
        assert_eq!(read(&mut ws)["method"], "initialized", "관리 연결이 승인을 대신 응답하면 안 된다");
        let status = read(&mut ws);
        assert_eq!(status["method"], "remoteControl/status/read");
        ws.send(Message::Text(json!({"id":status["id"],"method":"item/commandExecution/requestApproval","params":{}}).to_string().into())).unwrap();
        reply(&mut ws, &status, status_value());
        assert!(ws.read().is_err(), "관리 연결은 승인 응답을 보내지 않고 닫아야 한다");
    });
    let result = connection.run_stream(MobileAction::Status, stream, Duration::from_secs(2));
    worker.join().unwrap();
    assert_eq!(result.unwrap().status.unwrap().status, "connected");
}
