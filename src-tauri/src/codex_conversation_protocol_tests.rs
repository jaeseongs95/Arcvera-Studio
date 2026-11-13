    use super::*;
    fn supported_initialize() -> Value { json!({"opendiyCapabilities":{"passiveSubscription":true,"compactStartIfIdle":true}}) }

    fn async_item(call: &str) -> Value { json!({"type":"agentMessage","id":call,"text":"계속할까요?","delivery":"async","questions":[{"title":"계속할까요?","options":["진행","보류"]}]}) }
    fn async_history_entry(item: Value) -> Value {json!({"turnId":"turn-async","item":item,"startedAtMs":null,"completedAtMs":null})}
    fn async_reply_item(id: &str) -> Value { json!({"type":"userMessage","id":"user-answer","content":[{"type":"text","text":format!("<send_user_message_question_reply>\n{}\n</send_user_message_question_reply>",json!([{"questionItemId":id,"question":"계속할까요?","answer":"진행"}]))}]}) }
    fn async_notification(item: Value) -> Value { json!({"method":"item/completed","params":{"threadId":"original","turnId":"turn-async","item":item}}) }

    #[test]
    fn async_public_observer_subscribe_restores_only_active_turn_then_merges_live_answers() {
        let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        let answered=r#"["request_user_input_async","already-answered",0]"#;
        let server=thread::spawn(move|| {
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);
            reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"active","activeFlags":[]}}}));
            let turns=read_request(&mut socket);assert_eq!(turns["method"],"thread/turns/list");assert_eq!(turns["params"],json!({"threadId":"original","limit":1,"sortDirection":"desc","itemsView":"summary"}));
            reply(&mut socket,&turns,json!({"data":[{"id":"turn-async","status":"inProgress"}],"nextCursor":"older-turn-never-read"}));
            let items=read_request(&mut socket);assert_eq!(items["method"],"thread/items/list");assert_eq!(items["params"],json!({"threadId":"original","turnId":"turn-async","cursor":null,"limit":32,"sortDirection":"desc"}));
            reply(&mut socket,&items,json!({"data":[async_history_entry(async_reply_item(answered)),async_history_entry(async_item("pending")),async_history_entry(json!({"type":"contextCompaction","id":"other-item"}))],"nextCursor":"older-items","backwardsCursor":null}));
            let next=read_request(&mut socket);assert_eq!(next["params"]["cursor"],"older-items");assert_eq!(next["params"]["turnId"],"turn-async");
            reply(&mut socket,&next,json!({"data":[async_history_entry(async_item("already-answered"))],"nextCursor":null,"backwardsCursor":null}));
            assert_closed_without_input(&mut socket);
        });
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(control.question_metadata_complete().unwrap());
        let pending=control.pending_snapshot().unwrap().2;assert_eq!(pending.len(),1);assert_eq!(pending[0]["requestId"],r#"["request_user_input_async","pending",0]"#);assert_eq!(pending[0]["metadataAvailable"],true);
        control.observer_message("original",async_notification(async_item("pending"))).unwrap();assert_eq!(control.pending_snapshot().unwrap().2.len(),1);
        let mut other=async_notification(async_item("other-thread"));other["params"]["threadId"]=json!("different");control.observer_message("original",other).unwrap();assert_eq!(control.pending_snapshot().unwrap().2.len(),1);
        control.observer_message("original",async_notification(async_reply_item(r#"["request_user_input_async","pending",0]"#))).unwrap();assert!(control.pending_snapshot().unwrap().2.is_empty());
        control.observer_disconnected();assert!(!control.question_metadata_complete().unwrap());drop(rpc);server.join().unwrap();
    }
    #[test]
    fn async_public_history_accepts_r36_tuple_variants_beside_pending_questions() {
        let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        let server=thread::spawn(move|| {
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"active","activeFlags":[]}}}));
            let turns=read_request(&mut socket);assert_eq!(turns["method"],"thread/turns/list");reply(&mut socket,&turns,json!({"data":[{"id":"turn-async","status":"inProgress"}],"nextCursor":null}));
            let items=read_request(&mut socket);assert_eq!(items["method"],"thread/items/list");assert_eq!(items["params"],json!({"threadId":"original","turnId":"turn-async","cursor":null,"limit":32,"sortDirection":"desc"}));
            reply(&mut socket,&items,json!({"data":[
                async_history_entry(async_item("pending")),
                async_history_entry(json!({"type":"webSearch","id":"search-1","query":"fixture query","action":{"type":"search","query":"fixture query","queries":null},"results":[]})),
                async_history_entry(json!({"type":"sleep","id":"sleep-1","durationMs":1000})),
                async_history_entry(json!({"type":"imageGeneration","id":"image-1","status":"completed","revisedPrompt":null,"result":"fixture-image","transparentBackground":false,"failure":null,"generationId":null}))
            ],"nextCursor":null,"backwardsCursor":null}));assert_closed_without_input(&mut socket);
        });
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(control.question_metadata_complete().unwrap());
        let pending=control.pending_snapshot().unwrap().2;assert_eq!(pending.len(),1);assert_eq!(pending[0]["requestId"],r#"["request_user_input_async","pending",0]"#);assert_eq!(pending[0]["params"]["questions"][0]["question"],"계속할까요?");assert!(pending[0]["metadataDiagnostic"].is_null());
        drop(rpc);server.join().unwrap();
    }
    #[test]
    fn async_public_history_error_preserves_last_questions_without_claiming_zero() {
        for failure in ["unsupported","unmaterialized","cursorCycle"] {
            let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
            control.observer_message("original",async_notification(async_item("retained"))).unwrap();
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let subscribe=read_request(&mut socket);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"active","activeFlags":[]}}}));
                let turns=read_request(&mut socket);
                if failure=="unsupported" {socket.send(Message::Text(json!({"id":turns["id"],"error":{"code":-32601,"message":"unsupported"}}).to_string().into())).unwrap();}
                else if failure=="unmaterialized" {socket.send(Message::Text(json!({"id":turns["id"],"error":{"code":-32600,"data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}).to_string().into())).unwrap();}
                else {reply(&mut socket,&turns,json!({"data":[{"id":"turn-async","status":"inProgress"}]}));for _ in 0..2 {let items=read_request(&mut socket);reply(&mut socket,&items,json!({"data":[],"nextCursor":"cycle"}));}}
                assert_closed_without_input(&mut socket);
            });
            assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(!control.question_metadata_complete().unwrap());
            let pending=control.pending_snapshot().unwrap().2;assert_eq!(pending.len(),1);assert_eq!(pending[0]["metadataAvailable"],true);assert!(pending[0]["metadataDiagnostic"].is_string());assert!(!control.0.unknown.load(Ordering::Acquire));
            control.observer_message("original",json!({"method":"turn/completed","params":{"threadId":"original","turn":{"id":"older-turn"}}})).unwrap();assert!(!control.question_metadata_complete().unwrap());assert_eq!(control.pending_snapshot().unwrap().2.len(),1);
            control.observer_message("original",json!({"method":"turn/completed","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();assert!(control.question_metadata_complete().unwrap());assert!(control.pending_snapshot().unwrap().2.is_empty());
            control.observer_message("original",async_notification(async_item("retained"))).unwrap();assert!(control.pending_snapshot().unwrap().2.is_empty());
            drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn async_public_subscribe_history_failure_only_confirms_the_observed_current_turn_end() {
        for completed_turn in ["other-old-turn","turn-async"] {
            let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
            control.observer_message("original",async_notification(async_item("retained"))).unwrap();
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"active","activeFlags":[]}}}));
                let turns=read_request(&mut socket);assert_eq!(turns["method"],"thread/turns/list");assert_eq!(turns["params"],json!({"threadId":"original","limit":1,"sortDirection":"desc","itemsView":"summary"}));
                socket.send(Message::Text(json!({"method":"turn/completed","params":{"threadId":"original","turn":{"id":completed_turn}}}).to_string().into())).unwrap();
                socket.send(Message::Text(json!({"id":turns["id"],"error":{"code":-32601,"message":"fixture history method unsupported"}}).to_string().into())).unwrap();assert_closed_without_input(&mut socket);
            });
            assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());
            let (_,_,pending,complete)=control.question_snapshot().unwrap();
            if completed_turn=="turn-async" {assert!(complete);assert!(pending.is_empty());}
            else {assert!(!complete,"an unrelated old completion cannot confirm the current question inbox as empty");assert_eq!(pending.len(),1);assert_eq!(pending[0]["requestId"],r#"["request_user_input_async","retained",0]"#);assert_eq!(pending[0]["params"]["questions"][0]["question"],"계속할까요?");assert!(pending[0]["metadataDiagnostic"].is_string());}
            assert_eq!(rpc.next_id,2);assert!(!control.0.unknown.load(Ordering::Acquire));drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn async_public_history_rejects_every_malformed_entry_without_losing_last_questions() {
        let mut wrong_turn=async_history_entry(async_item("wrong-turn"));wrong_turn["turnId"]=json!("different-turn");
        for bad in [wrong_turn,json!({"item":async_item("missing-turn")}),json!({"turnId":"turn-async"}),async_history_entry(Value::Null),async_history_entry(json!("not-an-item")),async_history_entry(json!({"id":"no-type"})),async_history_entry(json!({"id":"bad-type","type":false})),async_history_entry(json!({"type":"reasoning"})),async_history_entry(json!({"type":"contextCompaction","id":""})),async_item("raw-item-is-not-entry")] {
            let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();control.observer_message("original",async_notification(async_item("retained"))).unwrap();
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"active","activeFlags":[]}}}));
                let turns=read_request(&mut socket);assert_eq!(turns["method"],"thread/turns/list");reply(&mut socket,&turns,json!({"data":[{"id":"turn-async","status":"inProgress"}],"nextCursor":null}));
                let items=read_request(&mut socket);assert_eq!(items["method"],"thread/items/list");assert_eq!(items["params"]["turnId"],"turn-async");
                reply(&mut socket,&items,json!({"data":[async_history_entry(async_item("must-not-partially-replace")),bad],"nextCursor":null,"backwardsCursor":null}));assert_closed_without_input(&mut socket);
            });
            assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(!control.question_metadata_complete().unwrap());
            let pending=control.pending_snapshot().unwrap().2;assert_eq!(pending.len(),1);assert_eq!(pending[0]["requestId"],r#"["request_user_input_async","retained",0]"#);assert!(pending[0]["metadataDiagnostic"].is_string());assert_eq!(rpc.next_id,3);
            drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn async_public_history_live_race_retries_once_without_replacing_newer_notifications() {
        let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        let server=thread::spawn(move|| {
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let subscribe=read_request(&mut socket);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"active","activeFlags":[]}}}));
            for attempt in 0..2 {
                let turns=read_request(&mut socket);assert_eq!(turns["method"],"thread/turns/list");
                if attempt==0 {socket.send(Message::Text(async_notification(async_item("live-newer")).to_string().into())).unwrap();}
                reply(&mut socket,&turns,json!({"data":[{"id":"turn-async","status":"inProgress"}]}));
                let items=read_request(&mut socket);assert_eq!(items["method"],"thread/items/list");
                reply(&mut socket,&items,if attempt==0 {json!({"data":[async_history_entry(async_item("history-older"))],"nextCursor":null,"backwardsCursor":null})}else{json!({"data":[async_history_entry(async_item("live-newer")),async_history_entry(async_item("history-older"))],"nextCursor":null,"backwardsCursor":null})});
            }
            assert_closed_without_input(&mut socket);
        });
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(control.question_metadata_complete().unwrap());
        assert_eq!(control.pending_snapshot().unwrap().2.len(),2);assert_eq!(rpc.next_id,5);drop(rpc);server.join().unwrap();
    }
    #[test]
    fn terminal_async_question_never_replies_to_an_observed_server_request() {
        // QI-05: the new observer writer is only for turn/steer envelopes.
        let(control,mut rpc,listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
        control.observer_message("original",json!({"id":44,"method":"item/tool/requestUserInput","params":{"threadId":"original","questions":[{"id":"q","question":"원래 터미널 질문"}]}})).unwrap();
        assert!(control.request("original",Path::new("."),ConversationRequest::Respond{request_id:"44".into(),result:json!({"answers":{"q":{"answers":["진행"]}}})}).is_err());
        control.observer_compact(&mut rpc,"original");assert_eq!(rpc.next_id,0);
        let pending=control.pending_snapshot().unwrap().2;assert_eq!(pending.len(),1);assert_ne!(pending[0]["responded"],true);
        assert!(control.0.worker.lock().unwrap().is_none());assert!(!control.0.observer_compaction.lock().unwrap().busy);
        let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);drop(rpc);assert_closed_without_input(&mut socket);
    }
    #[test]
    fn terminal_async_question_revalidates_queued_state_without_writing() {
        // QI-03/04: invalidate a queued answer before the observer consumes it.
        // All paths must leave the wire empty and release the bounded slot.
        for reason in ["expired","wrongThread","disconnect","newTurn","resolved","stopped","ownerChanged","unknown","incomplete","compacting"] {
            let(control,mut rpc,listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
            control.observer_message("original",async_notification(async_item("pending"))).unwrap();
            let id=r#"["request_user_input_async","pending",0]"#;
            let requested=control.clone();let caller=thread::spawn(move||requested.request("original",Path::new("."),ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})}));
            let deadline=Instant::now()+Duration::from_secs(3);
            while control.0.observer_compaction.lock().unwrap().pending.is_none() {
                assert!(!caller.is_finished(),"queued answer rejected: {reason}");assert!(Instant::now()<deadline);thread::sleep(Duration::from_millis(1));
            }
            match reason {
                "expired"=>control.0.observer_compaction.lock().unwrap().pending.as_mut().unwrap().deadline=Instant::now()-Duration::from_secs(1),
                "disconnect"=>control.observer_disconnected(),
                "newTurn"=>control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"next-turn"}}})).unwrap(),
                "resolved"=>control.observer_message("original",async_notification(async_reply_item(id))).unwrap(),
                "stopped"=>control.0.stopped.store(true,Ordering::Release),
                "ownerChanged"=>control.0.owned.store(true,Ordering::Release),
                "unknown"=>control.0.unknown.store(true,Ordering::Release),
                "incomplete"=>control.0.events.lock().unwrap().async_questions.unavailable("fixture history failed"),
                "compacting"=>control.0.events.lock().unwrap().context_compaction=Some(("turn-async".into(),"compact-item".into())),
                _=>{},
            }
            control.observer_compact(&mut rpc,if reason=="wrongThread"{"different"}else{"original"});
            assert!(caller.join().unwrap().is_err(),"{reason}");assert!(!control.0.observer_compaction.lock().unwrap().busy);
            assert_eq!(rpc.next_id,0,"{reason}");
            if !matches!(reason,"newTurn"|"resolved") {assert_eq!(control.pending_snapshot().unwrap().2[0]["params"]["questions"][0]["question"],"계속할까요?");}
            let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);drop(rpc);assert_closed_without_input(&mut socket);
        }
    }
    #[test]
    fn terminal_async_question_drains_resolution_and_bounds_incoming_traffic() {
        // QI-03: the resolution is on the real wire before the mutation. A ping
        // before it cannot be mistaken for an empty notification queue.
        for outcome in ["resolved","newTurn","saturated"] {
            let(control,mut rpc,listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
            control.observer_message("original",async_notification(async_item("pending"))).unwrap();
            let id=r#"["request_user_input_async","pending",0]"#;
            let requested=control.clone();let caller=thread::spawn(move||requested.request("original",Path::new("."),ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})}));
            let deadline=Instant::now()+Duration::from_secs(3);
            while control.0.observer_compaction.lock().unwrap().pending.is_none() {assert!(!caller.is_finished());assert!(Instant::now()<deadline);thread::sleep(Duration::from_millis(1));}
            let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            if outcome=="saturated" {
                for _ in 0..EVENT_COUNT {socket.send(Message::Text(json!({"method":"fixture/notification","params":{"threadId":"original"}}).to_string().into())).unwrap();}
            } else {
                socket.send(Message::Ping(Vec::new().into())).unwrap();
                let notification=if outcome=="resolved" {async_notification(async_reply_item(id))}else{json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"next-turn"}}})};
                socket.send(Message::Text(notification.to_string().into())).unwrap();
            }
            control.observer_compact(&mut rpc,"original");assert!(caller.join().unwrap().is_err(),"{outcome}");assert_eq!(rpc.next_id,0);
            assert!(!control.0.unknown.load(Ordering::Acquire));assert!(!control.0.observer_compaction.lock().unwrap().busy);
            // Reading a ping can emit a protocol pong, but never an app RPC.
            socket.get_mut().set_read_timeout(Some(Duration::from_millis(100))).unwrap();
            loop {match socket.read() {Ok(Message::Pong(_))=>{},Err(error) if timed_out(&error)=>break,other=>panic!("unexpected application write: {other:?}")}}
            drop(rpc);
        }
    }
    #[test]
    fn terminal_async_question_unknown_reply_is_not_resolved_or_retried() {
        for outcome in ["lost","wrongTurn","unsupported"] {
            let(control,mut rpc,listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
            control.observer_message("original",async_notification(async_item("pending"))).unwrap();
            let id=r#"["request_user_input_async","pending",0]"#;
            let respond=move||ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})};
            let requested=control.clone();let caller=thread::spawn(move||requested.request("original",Path::new("."),respond()));
            let deadline=Instant::now()+Duration::from_secs(3);
            while control.0.observer_compaction.lock().unwrap().pending.is_none() {assert!(!caller.is_finished());assert!(Instant::now()<deadline);thread::sleep(Duration::from_millis(1));}
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let request=read_request(&mut socket);assert_eq!(request["method"],"turn/steer");
                if outcome=="lost" {return;}
                if outcome=="unsupported" {socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32601,"message":"fixture unsupported"}}).to_string().into())).unwrap();}
                else {reply(&mut socket,&request,json!({"turnId":"different-turn"}));}
                assert_closed_without_input(&mut socket);
            });
            control.observer_compact(&mut rpc,"original");assert!(caller.join().unwrap().is_err());
            assert_eq!(control.0.unknown.load(Ordering::Acquire),outcome!="unsupported");
            assert_eq!(control.pending_snapshot().unwrap().2[0]["responded"],true);
            assert!(control.request("original",Path::new("."),respond()).is_err());assert_eq!(rpc.next_id,1);
            assert!(!control.0.owned.load(Ordering::Acquire));assert_eq!(control.terminal_write(||Ok(9)).unwrap(),9);
            drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn terminal_async_question_uses_observer_and_preserves_terminal_owner() {
        // QI-01/02/04: public request -> existing bounded observer slot -> real
        // loopback RPC. The fixture replaces the server, not the client writer.
        let(control,mut rpc,listener)=fixture();
        control.0.events.lock().unwrap().observer_connected=true;
        control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
        control.observer_message("original",async_notification(async_item("pending"))).unwrap();
        let id=r#"["request_user_input_async","pending",0]"#;
        let respond=move||ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})};
        let requested=control.clone();let caller=thread::spawn(move||requested.request("original",Path::new("."),respond()));
        let deadline=Instant::now()+Duration::from_secs(3);
        while control.0.observer_compaction.lock().unwrap().pending.is_none() {
            assert!(!caller.is_finished(),"터미널 질문 응답이 기존 관찰자 슬롯에 도달하기 전에 거절되었다");
            assert!(Instant::now()<deadline,"관찰자 슬롯 대기 기한 초과");thread::sleep(Duration::from_millis(1));
        }
        assert!(control.request("original",Path::new("."),respond()).is_err());
        assert!(control.request("original",Path::new("."),ConversationRequest::Attach).is_err());
        assert_eq!(control.terminal_write(||Ok(7)).unwrap(),7);
        let server=thread::spawn(move|| {
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let request=read_request(&mut socket);assert_eq!(request["method"],"turn/steer");
            assert_eq!(request["params"]["threadId"],"original");assert_eq!(request["params"]["expectedTurnId"],"turn-async");
            assert_eq!(request["params"]["input"][0]["text"],format!("<send_user_message_question_reply>\n{}\n</send_user_message_question_reply>",json!([{"answer":"진행","question":"계속할까요?","questionItemId":id}])));
            reply(&mut socket,&request,json!({"turnId":"turn-async"}));assert_closed_without_input(&mut socket);
        });
        control.observer_compact(&mut rpc,"original");
        let response=caller.join().unwrap().unwrap();assert_eq!(response["status"],"accepted");assert_eq!(response["resolved"],false);
        assert!(control.request("original",Path::new("."),respond()).is_err());
        assert_eq!(control.pending_snapshot().unwrap().2[0]["responded"],true);
        assert!(!control.0.owned.load(Ordering::Acquire));assert!(!*control.0.input.lock().unwrap());assert!(control.0.worker.lock().unwrap().is_none());
        assert_eq!(control.terminal_write(||Ok(8)).unwrap(),8);assert_eq!(rpc.next_id,1);
        control.observer_message("original",async_notification(async_reply_item(id))).unwrap();assert!(control.pending_snapshot().unwrap().2.is_empty());
        drop(rpc);server.join().unwrap();
    }
    #[test]
    fn async_public_respond_steers_exact_turn_as_user_input_and_waits_for_observed_resolution() {
        for outcome in ["accepted","unsupported","mismatch","lost","wrongTurn"] {
            let(control,mut rpc,listener)=fixture();control.0.owned.store(true,Ordering::Release);
            {let mut events=control.0.events.lock().unwrap();events.connected=true;events.observer_connected=true;}
            control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();
            control.observer_message("original",async_notification(async_item("pending"))).unwrap();
            let id=r#"["request_user_input_async","pending",0]"#;
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let request=read_request(&mut socket);assert_eq!(request["method"],"turn/steer");assert_eq!(request["params"]["threadId"],"original");assert_eq!(request["params"]["expectedTurnId"],"turn-async");assert!(Uuid::parse_str(request["params"]["clientUserMessageId"].as_str().unwrap()).is_ok());
                assert_eq!(request["params"]["input"].as_array().unwrap().len(),1);assert_eq!(request["params"]["input"][0]["type"],"text");
                assert_eq!(request["params"]["input"][0]["text"],format!("<send_user_message_question_reply>\n{}\n</send_user_message_question_reply>",json!([{"answer":"진행","question":"계속할까요?","questionItemId":id}])));
                match outcome {
                    "accepted"=>reply(&mut socket,&request,json!({"turnId":"turn-async"})),
                    "wrongTurn"=>reply(&mut socket,&request,json!({"turnId":"wrong-turn"})),
                    "unsupported"|"mismatch"=>{socket.send(Message::Text(json!({"id":request["id"],"error":{"code":if outcome=="unsupported"{-32601}else{-32600},"message":"fixture rejection"}}).to_string().into())).unwrap();},
                    _=>{return;},
                }
                assert_closed_without_input(&mut socket);
            });
            let response=execute(&control.0,&mut rpc,"original",ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})},&AtomicBool::new(false));
            assert_eq!(response.is_ok(),outcome=="accepted","{outcome}");if outcome=="accepted" {assert_eq!(response.unwrap()["resolved"],false);}
            assert_eq!(control.0.unknown.load(Ordering::Acquire),matches!(outcome,"lost"|"wrongTurn"));
            assert_eq!(control.pending_snapshot().unwrap().2[0]["responded"],true);
            assert!(execute(&control.0,&mut rpc,"original",ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})},&AtomicBool::new(false)).is_err());assert_eq!(rpc.next_id,1);
            if outcome=="accepted" {control.observer_message("original",async_notification(async_reply_item(id))).unwrap();assert!(control.pending_snapshot().unwrap().2.is_empty());}
            drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn async_public_respond_without_input_ownership_or_current_metadata_never_sends() {
        let(control,mut rpc,listener)=fixture();
        control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"turn-async"}}})).unwrap();control.observer_message("original",async_notification(async_item("pending"))).unwrap();
        let id=r#"["request_user_input_async","pending",0]"#;
        for (owned,connected,observer) in [(false,true,true),(true,false,true),(true,true,false)] {
            control.0.owned.store(owned,Ordering::Release);{let mut events=control.0.events.lock().unwrap();events.connected=connected;events.observer_connected=observer;}
            assert!(execute(&control.0,&mut rpc,"original",ConversationRequest::Respond{request_id:id.into(),result:json!({"answers":{id:{"answers":["진행"]}}})},&AtomicBool::new(false)).is_err());
        }
        assert_eq!(rpc.next_id,0);assert_eq!(control.pending_snapshot().unwrap().2[0]["responded"],false);
        let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);drop(rpc);assert_closed_without_input(&mut socket);
    }

    #[test]
    fn unsupported_declarations_never_acquire_input_or_send_passive_requests() {
        for initialized in [Value::Null,json!({}),json!({"opendiyCapabilities":null}),json!({"opendiyCapabilities":{"historySummary":true}}),
            json!({"opendiyCapabilities":{"passiveSubscription":false}}),json!({"opendiyCapabilities":{"passiveSubscription":"true"}}),
            json!({"opendiyCapabilities":{"passiveSubscription":null}}),json!({"passiveSubscription":true})] {
            let (_,mut rpc,listener)=fixture();
            let control=ConversationConnection::from_initialize(Endpoint::Unavailable,Arc::new(AtomicBool::new(false)),&initialized);
            assert!(!passive_capability(&initialized));
            assert_eq!(control.context_status("original").unwrap()["passiveSubscriptionSupported"],false);
            assert_eq!(control.request("original",Path::new("."),ConversationRequest::Attach).unwrap_err(),PASSIVE_UNSUPPORTED);
            assert_eq!(control.observer_subscribe(&mut rpc,"original",Path::new(".")).unwrap_err(),PASSIVE_UNSUPPORTED);
            assert!(!control.0.owned.load(Ordering::Acquire));assert!(control.0.worker.lock().unwrap().is_none());
            assert_eq!(control.terminal_write(||Ok(42)).unwrap(),42);assert_eq!(rpc.next_id,0);
            let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            drop(rpc);assert_closed_without_input(&mut socket);
        }
        assert!(passive_capability(&supported_initialize()));
    }
    #[test]
    fn context_support_flags_are_independent_and_require_literal_true() {
        for advertised in [Value::Null,json!(false),json!("true"),json!(true)] {
            let initialized=json!({"supportsSubagentManagement":advertised,"opendiyCapabilities":{"passiveSubscription":false,"compactStartIfIdle":advertised}});
            let control=ConversationConnection::from_initialize(Endpoint::Unavailable,Arc::new(AtomicBool::new(false)),&initialized);
            let snapshot=control.context_status("original").unwrap();
            assert_eq!(snapshot["passiveSubscriptionSupported"],false);
            assert_eq!(snapshot["subagentsSupported"],advertised==json!(true));
            assert_eq!(snapshot["compactSupported"],advertised==json!(true));
            assert_eq!(snapshot["canCompact"],false);
            assert_eq!(control.terminal_write(||Ok(42)).unwrap(),42);
        }
        for initialized in [json!({}),json!({"opendiyCapabilities":{"passiveSubscription":true}})] {
            let control=ConversationConnection::from_initialize(Endpoint::Unavailable,Arc::new(AtomicBool::new(false)),&initialized);
            let snapshot=control.context_status("original").unwrap();
            assert_eq!(snapshot["subagentsSupported"],false);assert_eq!(snapshot["compactSupported"],false);
            let mut events=control.0.events.lock().unwrap();events.connected=true;events.context_active=Some(false);events.gui_compact_supported=true;
            assert_eq!(context_block_reason(&control.0,&events,false),Some("unsupported"));
        }
    }
    #[test]
    fn worker_rechecks_its_own_initialize_before_any_passive_or_fallback_request() {
        for initialized in [json!({}),json!({"opendiyCapabilities":{"passiveSubscription":false}}),json!({"opendiyCapabilities":{"passiveSubscription":"true","historySummary":true}})] {
            let(control,mut rpc,listener)=fixture();
            assert_eq!(control.context_status("original").unwrap()["passiveSubscriptionSupported"],true);
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let init=read_request(&mut socket);assert_eq!(init["method"],"initialize");reply(&mut socket,&init,initialized);
                assert_closed_without_input(&mut socket);
            });
            let(_sender,receiver)=mpsc::sync_channel(1);
            assert_eq!(serve(&control.0,&mut rpc,"original",Path::new("."),receiver,&AtomicBool::new(false),&Mutex::new(None)).unwrap_err(),PASSIVE_UNSUPPORTED);
            assert_eq!(rpc.next_id,1);assert!(!control.0.events.lock().unwrap().connected);drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn immediate_attach_failure_returns_the_bootstrap_error_and_can_detach() {
        let(control,_rpc,_listener)=fixture();
        for _ in 0..16 {
            let error=control.request("original",Path::new("."),ConversationRequest::Attach).unwrap_err();
            assert_eq!(error,"Test fixture has no runtime endpoint");
            assert!(control.0.owned.load(Ordering::Acquire));assert!(control.terminal_write(||Ok(1)).is_err());
            control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();
            assert!(control.0.worker.lock().unwrap().is_none());assert_eq!(control.terminal_write(||Ok(1)).unwrap(),1);
        }
    }
    #[test]
    fn bootstrap_reply_does_not_block_for_a_full_or_disconnected_receiver() {
        for full in [false,true] {
            let(control,_rpc,_listener)=fixture();let(reply,response)=mpsc::sync_channel(1);
            if full {reply.try_send(Ok(json!({"preserved":true}))).unwrap();}else{drop(response);}
            let(done,finished)=mpsc::sync_channel(1);let state=control.0.clone();
            let join=thread::spawn(move||{finish_worker(&state,Err("actual bootstrap error".into()),&reply);done.send(()).unwrap();});
            finished.recv_timeout(Duration::from_secs(3)).expect("bootstrap completion must not wait for the caller");join.join().unwrap();
            assert_eq!(control.0.events.lock().unwrap().error.as_deref(),Some("actual bootstrap error"));
        }
    }
    fn fixture() -> (ConversationConnection,Rpc<TcpStream>,TcpListener) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        let connection = ConversationConnection::new(Endpoint::Unavailable,Arc::new(AtomicBool::new(false)));
        (connection,Rpc{core_version:Some("1.0.0+fixture".into()),ws:WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Client,None),next_id:0,cancelled:None,deadline:None},listener)
    }
    fn read_request(socket: &mut WebSocket<TcpStream>) -> Value {
        loop {if let Message::Text(text) = socket.read().unwrap() {return serde_json::from_str(&text).unwrap();}}
    }
    #[test]
    fn observed_initialize_carries_full_core_identity() {
        let (control, mut rpc, listener) = fixture();
        let worker = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut socket = WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);
            let request = read_request(&mut socket);
            assert_eq!(request["method"], "initialize");
            assert_eq!(request["params"]["clientInfo"]["version"], env!("CARGO_PKG_VERSION"));
            assert_eq!(request["params"]["capabilities"]["extensions"]["arcvera/core"]["version"], "1.0.0+fixture");
            reply(&mut socket, &request, json!({}));
        });
        let result = call(&control.0, &mut rpc, "original", "initialize", initialize(), &AtomicBool::new(false));
        worker.join().unwrap();
        assert!(result.is_ok());
    }
    #[test]
    fn unverified_or_unsupported_core_sends_no_observer_initialize() {
        for version in [None, Some(""), Some("0.99.0"), Some("1.0.0-rc.1"), Some("2.0.0"), Some("codex-cli 1.0.0")] {
            let (control, mut rpc, listener) = fixture();
            rpc.core_version = version.map(str::to_owned);
            let result = call(&control.0, &mut rpc, "original", "initialize", initialize(), &AtomicBool::new(false));
            assert!(result.is_err());
            assert_eq!(rpc.next_id, 0);
            let (stream, _) = listener.accept().unwrap();
            let mut socket = WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);
            drop(rpc);
            assert_closed_without_input(&mut socket);
        }
    }
    fn reply(socket: &mut WebSocket<TcpStream>,request: &Value,result: Value) {
        socket.send(Message::Text(json!({"id":request["id"],"result":result}).to_string().into())).unwrap();
    }
    fn assert_closed_without_input(socket: &mut WebSocket<TcpStream>) {
        let result=socket.get_mut().read(&mut[0u8;1]);
        let closed=match &result {Ok(0)=>true,Err(error)=>matches!(error.kind(),io::ErrorKind::ConnectionAborted|io::ErrorKind::ConnectionReset),_=>false};
        assert!(closed,"추가 송신 없이 실제 연결이 닫혀야 한다. 시간 초과는 종료 근거가 아니다: {result:?}");
    }
    #[test]
    fn context_snapshot_uses_existing_observer_without_draining_or_connecting() {
        let(control,mut rpc,listener)=fixture();
        control.0.events.lock().unwrap().observer_connected=true;
        let usage=json!({"total":{"totalTokens":100,"inputTokens":70,"cachedInputTokens":0,"outputTokens":30,"reasoningOutputTokens":0},"last":{"totalTokens":40,"inputTokens":30,"cachedInputTokens":0,"outputTokens":10,"reasoningOutputTokens":0},"modelContextWindow":1000});
        control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"foreign","turnId":"turn","tokenUsage":usage}})).unwrap();
        assert!(control.context_status("original").unwrap()["tokenUsage"].is_null());
        control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":usage}})).unwrap();
        observe_message(&control.0,"original",json!({"method":"test/event","params":{"threadId":"original"}})).unwrap();
        for _ in 0..3 {
            let status=control.request("original",Path::new("."),ConversationRequest::ContextStatus).unwrap();
            assert_eq!(status["connected"],true);assert_eq!(status["guiConnected"],false);
            assert_eq!(status["tokenUsage"]["last"]["totalTokens"],40);assert_eq!(status["tokenUsageFresh"],true);
            assert_eq!(status["reason"],"unsupported");assert_eq!(status["active"],Value::Null);
        }
        assert!(control.0.worker.lock().unwrap().is_none());assert_eq!(control.0.events.lock().unwrap().queue.len(),1);
        {let _worker=control.0.worker.lock().unwrap();assert_eq!(control.context_status("original").unwrap()["reason"],"closing");}
        assert_eq!(rpc.next_id,0);
        let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
        drop(rpc);assert_closed_without_input(&mut socket);
    }
    #[test]
    fn context_usage_is_unavailable_after_malformed_update_and_stale_after_compaction() {
        let(control,_rpc,_listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
        let usage=json!({"total":{"totalTokens":100,"inputTokens":70,"cachedInputTokens":0,"outputTokens":30,"reasoningOutputTokens":0},"last":{"totalTokens":40,"inputTokens":30,"cachedInputTokens":0,"outputTokens":10,"reasoningOutputTokens":0},"modelContextWindow":1000});
        let update=|usage:Value|json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":usage}});
        control.observer_message("original",update(usage.clone())).unwrap();
        control.observer_message("original",json!({"method":"item/started","params":{"threadId":"original","turnId":"compact","item":{"id":"compact-item","type":"contextCompaction"}}})).unwrap();
        control.observer_message("original",update(usage.clone())).unwrap();
        let status=control.context_status("original").unwrap();assert_eq!(status["compacting"],true);assert_eq!(status["tokenUsageFresh"],false);
        control.observer_message("original",json!({"method":"turn/completed","params":{"threadId":"original","turn":{"id":"other","status":"completed"}}})).unwrap();assert_eq!(control.context_status("original").unwrap()["compacting"],true);
        control.observer_message("original",json!({"method":"item/completed","params":{"threadId":"original","turnId":"compact","item":{"id":"compact-item","type":"contextCompaction"}}})).unwrap();
        assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],false);
        control.observer_message("original",update(usage.clone())).unwrap();assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],true);
        let mut invalid=usage;invalid["last"]["totalTokens"]=json!(-1);control.observer_message("original",update(invalid)).unwrap();
        let status=control.context_status("original").unwrap();assert!(status["tokenUsage"].is_null());assert_eq!(status["tokenUsageFresh"],false);
    }
    #[test]
    fn context_numeric_limits_reject_invalid_usage_without_inventing_zero() {
        let(control,_rpc,_listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
        let valid=json!({"total":{"totalTokens":400000,"inputTokens":300000,"cachedInputTokens":0,"outputTokens":100000,"reasoningOutputTokens":0},"last":{"totalTokens":56000,"inputTokens":40000,"cachedInputTokens":0,"outputTokens":16000,"reasoningOutputTokens":0},"modelContextWindow":100000});
        for (path,bad) in [
            ("/last/totalTokens",json!(-1)),("/last/totalTokens",json!(1.5)),("/last/totalTokens",json!(MAX_CONTEXT_INTEGER+1)),
            ("/total/totalTokens",json!(MAX_CONTEXT_INTEGER+1)),("/last/inputTokens",json!(-1)),
            ("/last/reasoningOutputTokens",json!(MAX_CONTEXT_INTEGER+1)),("/modelContextWindow",json!(0)),
            ("/modelContextWindow",json!(-1)),("/modelContextWindow",json!(MAX_CONTEXT_INTEGER+1)),
        ] {
            let mut usage=valid.clone();*usage.pointer_mut(path).unwrap()=bad.clone();
            control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":usage}})).unwrap();
            let snapshot=control.context_status("original").unwrap();assert!(snapshot["tokenUsage"].is_null(),"{path}={bad}");assert_eq!(snapshot["tokenUsageFresh"],false);
        }
        let mut missing=valid.clone();missing["last"].as_object_mut().unwrap().remove("totalTokens");
        control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":missing}})).unwrap();assert!(control.context_status("original").unwrap()["tokenUsage"].is_null());
        let mut unknown_window=valid.clone();unknown_window["modelContextWindow"]=Value::Null;
        control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":unknown_window}})).unwrap();
        let snapshot=control.context_status("original").unwrap();assert_eq!(snapshot["tokenUsage"]["last"]["totalTokens"],56000);assert_eq!(snapshot["tokenUsageFresh"],false);
        for used in [0,12000,100000,100001,MAX_CONTEXT_INTEGER] {
            let mut usage=valid.clone();usage["last"]["totalTokens"]=json!(used);
            control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":usage}})).unwrap();
            let snapshot=control.context_status("original").unwrap();assert_eq!(snapshot["tokenUsage"]["last"]["totalTokens"],used);assert_eq!(snapshot["tokenUsageFresh"],true);
        }
        println!("사용량 경계: 음수·소수·안전정수 초과·누락·분모 0 거부, 분모 null 미확인, 유효 0·큰수 보존");
    }
    #[test]
    fn context_observer_alone_owns_usage_across_gui_handoff_and_disconnect() {
        let(control,_rpc,_listener)=fixture();
        let usage=|used:i64|json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":{"total":{"totalTokens":400000,"inputTokens":300000,"cachedInputTokens":0,"outputTokens":100000,"reasoningOutputTokens":0},"last":{"totalTokens":used,"inputTokens":0,"cachedInputTokens":0,"outputTokens":used,"reasoningOutputTokens":0},"modelContextWindow":100000}}});
        {let mut events=control.0.events.lock().unwrap();events.connected=true;events.observer_connected=true;}
        control.observer_message("original",usage(56000)).unwrap();
        observe_message(&control.0,"original",usage(99000)).unwrap();assert_eq!(control.context_status("original").unwrap()["tokenUsage"]["last"]["totalTokens"],56000);
        for method in ["thread/settings/updated","model/rerouted"] {
            // GUI의 복사본은 관찰자의 사용량 순서를 바꾸지 않는다.
            observe_message(&control.0,"original",json!({"method":method,"params":{"threadId":"original"}})).unwrap();assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],true);
            control.observer_message("original",json!({"method":method,"params":{"threadId":"original"}})).unwrap();assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],false);
            control.observer_message("original",usage(12000)).unwrap();assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],true);
        }
        // GUI 해제 뒤에도 동일 관찰 스트림에서 감소한 새 사용량을 계속 받는다.
        control.0.events.lock().unwrap().connected=false;control.observer_message("original",usage(8000)).unwrap();
        let snapshot=control.context_status("original").unwrap();assert_eq!(snapshot["tokenUsage"]["last"]["totalTokens"],8000);assert_eq!(snapshot["tokenUsageFresh"],true);
        let revision=snapshot["revision"].as_u64().unwrap();control.observer_disconnected();
        let snapshot=control.context_status("original").unwrap();assert_eq!(snapshot["tokenUsageFresh"],false);assert!(snapshot["revision"].as_u64().unwrap()>revision);
        // 연결 여부만 다시 true로 해도 과거값이 fresh로 부활하지 않는다.
        control.0.events.lock().unwrap().observer_connected=true;assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],false);
        println!("수신 순서: observer 56000 → GUI 99000 무시 → observer 모델 변경 미확인 → 12000 감소 수락 → GUI 인계 후 8000 갱신 → 단절/재연결 과거값 미확인");
    }
    #[test]
    fn context_passive_resubscribe_keeps_old_usage_stale_until_a_new_observer_update() {
        let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        control.0.events.lock().unwrap().observer_connected=true;
        let usage=|used:i64|json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":{"total":{"totalTokens":used,"inputTokens":used,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0},"last":{"totalTokens":used,"inputTokens":used,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0},"modelContextWindow":100000}}});
        control.observer_message("original",usage(12000)).unwrap();control.observer_disconnected();
        let server=thread::spawn(move|| {
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let request=read_request(&mut socket);assert_eq!(request["method"],PASSIVE_SUBSCRIBE);
            reply(&mut socket,&request,json!({"thread":{"id":"original","cwd":expected_cwd,"status":{"type":"idle"},"turns":[]}}));assert_closed_without_input(&mut socket);
        });
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());
        let snapshot=control.context_status("original").unwrap();assert_eq!(snapshot["tokenUsage"]["last"]["totalTokens"],12000);assert_eq!(snapshot["tokenUsageFresh"],false);
        control.observer_message("original",usage(56000)).unwrap();assert_eq!(control.context_status("original").unwrap()["tokenUsageFresh"],true);
        assert_eq!(rpc.next_id,1);drop(rpc);server.join().unwrap();
    }
    #[test]
    fn context_ten_thousand_updates_keep_one_snapshot_and_existing_event_bounds() {
        let(control,_rpc,_listener)=fixture();control.0.events.lock().unwrap().observer_connected=true;
        for metadata_only in [true,false] {
            for index in 0..10000 {
                let message=json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"turn","tokenUsage":{"total":{"totalTokens":index,"inputTokens":index,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0},"last":{"totalTokens":index,"inputTokens":index,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0},"modelContextWindow":100000}}});
                record_message(&control.0,"original",message,metadata_only).unwrap();
            }
            let events=control.0.events.lock().unwrap();assert!(events.token_usage.is_some());assert_eq!(events.token_usage.as_ref().unwrap().last.total_tokens,9999);
            assert_eq!(events.queue.len(),if metadata_only {0}else{EVENT_COUNT});assert!(events.bytes<=EVENT_BYTES);assert_eq!(events.gap,!metadata_only);
            // gap은 표시 이력 큐의 퇴거다. 환원기는 매 프레임을 처리했으므로 최신 snapshot 자체는 유효하다.
            assert!(events.token_usage_fresh);
            println!("연속 사용량 10000회: 관찰자={metadata_only}, 스냅샷=1, 큐={}, 바이트={}, 표시이력누락={}, 최신값=9999",events.queue.len(),events.bytes,events.gap);
            drop(events);assert!(control.0.worker.lock().unwrap().is_none());
        }
    }
    fn observer_compact_ready(control: &ConversationConnection) {
        let mut events=control.0.events.lock().unwrap();
        events.observer_connected=true;events.observer_compact_supported=true;events.context_active=Some(false);
    }
    fn await_compact_slot(control: &ConversationConnection) {
        let deadline=Instant::now()+Duration::from_secs(3);
        while control.0.observer_compaction.lock().unwrap().pending.is_none() {
            assert!(Instant::now()<deadline,"압축 요청이 슬롯에 도착하지 않았다");thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn terminal_compact_uses_existing_observer_without_gui_lease_or_duplicate_queue() {
        let(control,mut rpc,listener)=fixture();observer_compact_ready(&control);
        assert_eq!(control.context_status("original").unwrap()["canCompact"],true);
        let server=thread::spawn(move|| {
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let request=read_request(&mut socket);assert_eq!(request["method"],COMPACT_IF_IDLE);assert_eq!(request["params"],json!({"threadId":"original"}));
            reply(&mut socket,&request,json!({"status":"started","turn":{"id":"compact-turn"}}));assert_closed_without_input(&mut socket);
        });
        let requested=control.clone();let caller=thread::spawn(move||requested.request("original",Path::new("."),ConversationRequest::Compact));
        await_compact_slot(&control);
        assert_eq!(control.request("original",Path::new("."),ConversationRequest::Compact).unwrap()["reason"],"compacting");
        assert!(control.request("original",Path::new("."),ConversationRequest::Attach).unwrap_err().contains("입력"));
        assert_eq!(control.terminal_write(||Ok(7)).unwrap(),7);
        control.observer_compact(&mut rpc,"original");assert_eq!(caller.join().unwrap().unwrap()["status"],"started");
        assert!(!control.0.owned.load(Ordering::Acquire));assert!(!*control.0.input.lock().unwrap());assert!(control.0.worker.lock().unwrap().is_none());
        assert_eq!(control.terminal_write(||Ok(8)).unwrap(),8);assert_eq!(rpc.next_id,1);
        drop(rpc);server.join().unwrap();
    }
    #[test]
    fn terminal_compact_preserves_canonical_order_and_unknown_wire_protection() {
        for outcome in ["skippedBusy","completeBeforeAck","lost","malformed"] {
            let(control,mut rpc,listener)=fixture();observer_compact_ready(&control);
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let request=read_request(&mut socket);assert_eq!(request["method"],COMPACT_IF_IDLE);
                if outcome=="lost" {return;}
                if outcome=="completeBeforeAck" {
                    for message in [json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"compact-turn"}}}),json!({"method":"turn/completed","params":{"threadId":"original","turn":{"id":"compact-turn"}}})] {socket.send(Message::Text(message.to_string().into())).unwrap();}
                }
                let value=match outcome {"skippedBusy"=>json!({"status":"skippedBusy"}),"malformed"=>json!({"status":"started","turn":{"id":""}}),_=>json!({"status":"started","turn":{"id":"compact-turn"}})};
                reply(&mut socket,&request,value);assert_closed_without_input(&mut socket);
            });
            let result=execute_compact(&control.0,&mut rpc,"original",&AtomicBool::new(false),true);
            let known=matches!(outcome,"skippedBusy"|"completeBeforeAck");assert_eq!(result.is_ok(),known,"{outcome}");assert_eq!(control.0.unknown.load(Ordering::Acquire),!known);
            if known {assert_eq!(control.context_status("original").unwrap()["active"],false);assert_eq!(control.context_status("original").unwrap()["compacting"],false);}
            else {
                assert_eq!(control.request("original",Path::new("."),ConversationRequest::Compact).unwrap()["reason"],"unknownResult");
                assert!(!control.0.owned.load(Ordering::Acquire));assert!(!*control.0.input.lock().unwrap());
                assert_eq!(control.terminal_write(||Ok(1)).unwrap(),1,"압축 결과 불명이어도 CLI 입력 소유권을 바꾸지 않는다");
            }
            assert_eq!(rpc.next_id,1);drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn terminal_compact_expiry_wrong_thread_disconnect_and_guards_send_nothing() {
        for reason in ["active","statusUnknown","pendingRequest","unsupported","disconnected","expired","wrongThread","observerLost"] {
            let(control,mut rpc,listener)=fixture();observer_compact_ready(&control);
            {
                let mut events=control.0.events.lock().unwrap();
                match reason {"active"=>events.context_active=Some(true),"statusUnknown"=>events.context_active=None,"pendingRequest"=>{events.observed_pending.insert("q".into(),json!({}));},"unsupported"=>events.observer_compact_supported=false,"disconnected"=>events.observer_connected=false,_=>{}}
            }
            if matches!(reason,"expired"|"wrongThread"|"observerLost") {
                let requested=control.clone();let caller=thread::spawn(move||requested.request("original",Path::new("."),ConversationRequest::Compact));await_compact_slot(&control);
                if reason=="expired" {control.0.observer_compaction.lock().unwrap().pending.as_mut().unwrap().deadline=Instant::now()-Duration::from_secs(1);}
                if reason=="observerLost" {control.observer_disconnected();}else{control.observer_compact(&mut rpc,if reason=="wrongThread"{"different"}else{"original"});}
                assert!(caller.join().unwrap().is_err());assert!(!control.0.observer_compaction.lock().unwrap().busy);
            } else {assert_eq!(control.request("original",Path::new("."),ConversationRequest::Compact).unwrap()["reason"],reason);}
            assert_eq!(rpc.next_id,0);assert!(!control.0.owned.load(Ordering::Acquire));assert!(!control.0.unknown.load(Ordering::Acquire));
            let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);drop(rpc);assert_closed_without_input(&mut socket);
        }
    }
    #[test]
    fn context_unsupported_compact_and_native_guards_never_send_legacy_compact() {
        for state in ["idle","active","statusUnknown","pendingRequest","unknownResult","compacting","deleted","closing"] {
            let(control,mut rpc,listener)=fixture();
            {let mut events=control.0.events.lock().unwrap();events.connected=true;events.context_active=Some(false);events.gui_compact_supported=state!="idle";
                match state {"active"=>events.context_active=Some(true),"statusUnknown"=>events.context_active=None,
                    "pendingRequest"=>{events.observed_pending.insert("1".into(),json!({"method":"item/tool/requestUserInput"}));},
                    "compacting"=>events.context_compaction=Some(("turn".into(),"item".into())),_=>{}}
            }
            control.0.unknown.store(state=="unknownResult",Ordering::Release);control.0.deleted.store(state=="deleted",Ordering::Release);control.0.stopped.store(state=="closing",Ordering::Release);
            let result=execute(&control.0,&mut rpc,"original",ConversationRequest::Compact,&AtomicBool::new(false));
            if matches!(state,"deleted"|"closing") {assert!(result.is_err());}
            else {let value=result.unwrap();assert_eq!(value["status"],if state=="idle" {"unsupported"}else{"notStarted"});assert_eq!(value["reason"],if state=="idle" {"unsupported"}else{state});}
            assert_eq!(rpc.next_id,0);let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);drop(rpc);assert_closed_without_input(&mut socket);
        }
    }
    fn compact_bootstrap(socket: &mut WebSocket<TcpStream>,cwd: &Path,capability: Option<Value>) {
        conversation_bootstrap(socket,cwd,capability,None,None);
    }
    fn conversation_bootstrap(socket: &mut WebSocket<TcpStream>,cwd: &Path,compact: Option<Value>,initialize_interrupt: Option<Value>,passive_interrupt: Option<Value>) {
        let init=read_request(socket);assert_eq!(init["method"],"initialize");let mut initialized=supported_initialize();
        assert_eq!(init["params"]["clientInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(init["params"]["capabilities"]["extensions"]["arcvera/core"]["version"], "1.0.0+fixture");
        let malformed=initialize_interrupt.as_ref().is_some_and(|value|!value.is_boolean());
        if let Some(capability)=initialize_interrupt {initialized["supportsExpectedTurnInterrupt"]=capability;}reply(socket,&init,initialized);if malformed {return;}
        assert_eq!(read_request(socket)["method"],"initialized");
        let subscribe=read_request(socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);assert_eq!(subscribe["params"],json!({"threadId":"original"}));
        let mut value=json!({"thread":{"id":"original","cwd":cwd,"turns":[],"status":{"type":"idle"}}});
        let malformed=compact.as_ref().into_iter().chain(passive_interrupt.as_ref()).any(|value|!value.is_boolean());
        if let Some(capability)=compact {value["supportsCompactIfIdle"]=capability;}
        if let Some(capability)=passive_interrupt {value["supportsExpectedTurnInterrupt"]=capability;}reply(socket,&subscribe,value);
        if malformed {return;}
        let history=read_request(socket);assert_eq!(history["method"],"thread/turns/list");reply(socket,&history,json!({"data":[]}));
    }
    fn compact_gui(control: &ConversationConnection,mut rpc: Rpc<TcpStream>,cwd: PathBuf) {
        control.0.events.lock().unwrap().observer_connected=true;
        control.observer_message("original",json!({"method":"thread/status/changed","params":{"threadId":"original","status":{"type":"idle"}}})).unwrap();
        control.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"previous","tokenUsage":{"total":{"totalTokens":100,"inputTokens":70,"cachedInputTokens":0,"outputTokens":30,"reasoningOutputTokens":0},"last":{"totalTokens":40,"inputTokens":30,"cachedInputTokens":0,"outputTokens":10,"reasoningOutputTokens":0},"modelContextWindow":1000}}})).unwrap();
        install_actor(control,move|state,receiver,cancelled|{let _=serve(&state,&mut rpc,"original",&cwd,receiver,&cancelled,&Mutex::new(None));});
        let(reply,attached)=mpsc::sync_channel(1);control.0.worker.lock().unwrap().as_ref().unwrap().sender.try_send(Work{request:ConversationRequest::Attach,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();attached.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
    }
    #[test]
    fn compact_requires_this_gui_initialize_even_when_passive_subscription_allows_it() {
        for capability in [None,Some(json!(false)),Some(json!("true")),Some(Value::Null)] {
            let(control,rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            let server=thread::spawn(move|| {
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let init=read_request(&mut socket);assert_eq!(init["method"],"initialize");let mut initialized=supported_initialize();initialized["opendiyCapabilities"].as_object_mut().unwrap().remove("compactStartIfIdle");
                if let Some(capability)=capability {initialized["opendiyCapabilities"]["compactStartIfIdle"]=capability;}reply(&mut socket,&init,initialized);
                assert_eq!(read_request(&mut socket)["method"],"initialized");let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);
                reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}},"supportsCompactIfIdle":true}));
                let history=read_request(&mut socket);assert_eq!(history["method"],"thread/turns/list");reply(&mut socket,&history,json!({"data":[]}));assert_closed_without_input(&mut socket);
            });
            compact_gui(&control,rpc,cwd);assert_eq!(control.context_status("original").unwrap()["canCompact"],false);
            assert_eq!(control.request("original",Path::new("."),ConversationRequest::Compact).unwrap()["status"],"unsupported");
            control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();server.join().unwrap();
        }
    }
    #[test]
    fn compact_requires_verified_gui_capability_and_resets_it_on_disconnect() {
        let control=fixture().0;
        for capability in [None,Some(json!(false)),Some(json!(true)),Some(json!(false))] {
            let(_,rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let supported=capability==Some(json!(true));
            control.observer_disconnected();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);compact_bootstrap(&mut socket,&expected_cwd,capability);
                if supported {let request=read_request(&mut socket);assert_eq!(request["method"],COMPACT_IF_IDLE);assert_eq!(request["params"],json!({"threadId":"original"}));reply(&mut socket,&request,json!({"status":"started","turn":{"id":"compact-turn"}}));}assert_closed_without_input(&mut socket);
            });
            compact_gui(&control,rpc,cwd);assert_eq!(control.context_status("original").unwrap()["canCompact"],supported);
            let value=control.request("original",Path::new("."),ConversationRequest::Compact).unwrap();assert_eq!(value["status"],if supported{"started"}else{"unsupported"});
            if supported {let status=control.context_status("original").unwrap();assert_eq!(status["compacting"],true);assert_eq!(status["active"],true);assert_eq!(status["canCompact"],false);assert_eq!(control.request("original",Path::new("."),ConversationRequest::Compact).unwrap()["status"],"notStarted");}
            control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();assert!(!control.0.events.lock().unwrap().gui_compact_supported);server.join().unwrap();
        }
        let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        let server=thread::spawn(move||{
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            for capability in [json!(true),json!(false)] {let request=read_request(&mut socket);assert_eq!(request["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&request,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}},"supportsCompactIfIdle":capability}));}
            let request=read_request(&mut socket);assert_eq!(request["method"],PASSIVE_SUBSCRIBE);socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32601,"message":"unsupported"}}).to_string().into())).unwrap();assert_closed_without_input(&mut socket);
        });
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(control.0.events.lock().unwrap().observer_compact_supported);
        // 관찰자의 지원 정보는 화면 연결의 지원 증거로 승격하지 않는다.
        control.0.events.lock().unwrap().connected=true;assert_eq!(execute(&control.0,&mut rpc,"original",ConversationRequest::Compact,&AtomicBool::new(false)).unwrap()["status"],"unsupported");
        control.observer_disconnected();assert!(!control.0.events.lock().unwrap().observer_compact_supported);
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());assert!(!control.0.events.lock().unwrap().observer_compact_supported);
        control.0.events.lock().unwrap().observer_compact_supported=true;assert!(control.observer_subscribe(&mut rpc,"original",&cwd).is_err());assert!(!control.0.events.lock().unwrap().observer_compact_supported);drop(rpc);server.join().unwrap();
    }
    #[test]
    fn compact_rejects_malformed_support_bits_before_history_or_compact() {
        for capability in [Value::Null,json!("true"),json!(1),json!({}),json!([])] {
            for observer in [false,true] {
                let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let advertised=capability.clone();
                {let mut events=control.0.events.lock().unwrap();events.gui_compact_supported=true;events.observer_compact_supported=true;}
                let server=thread::spawn(move||{
                    let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                    if observer {let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}},"supportsCompactIfIdle":advertised}));}
                    else {compact_bootstrap(&mut socket,&expected_cwd,Some(advertised));}
                    assert_closed_without_input(&mut socket);
                });
                if observer {assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap_err().contains("bool"));assert!(!control.0.events.lock().unwrap().observer_compact_supported);}
                else {let(_sender,receiver)=mpsc::sync_channel(1);assert!(serve(&control.0,&mut rpc,"original",&cwd,receiver,&AtomicBool::new(false),&Mutex::new(None)).unwrap_err().contains("bool"));assert!(!control.0.events.lock().unwrap().gui_compact_supported);}
                assert!(!control.0.events.lock().unwrap().connected);assert_eq!(rpc.next_id,if observer{1}else{2});drop(rpc);server.join().unwrap();
            }
        }
    }
    #[test]
    fn compact_wire_rejections_preserve_state_and_uncertain_results_block_resend() {
        for outcome in ["skippedBusy","notStarted","unsupported","invalidParams","internal","invalidRequest","lost","badStarted","badNotStarted","forgedUnsupported","hybridError"] {
            let(control,rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let state=control.clone();
            let known=matches!(outcome,"skippedBusy"|"notStarted"|"unsupported"|"invalidParams");
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);compact_bootstrap(&mut socket,&expected_cwd,Some(json!(true)));
                let request=read_request(&mut socket);assert_eq!(request["method"],COMPACT_IF_IDLE);assert_eq!(request["params"],json!({"threadId":"original"}));
                assert_eq!(state.context_status("original").unwrap()["reason"],"unknownResult","송신 관측 때 불명 보호가 이미 설정되고 worker/events 잠금은 해제돼야 한다");
                if outcome=="lost" {return;}
                let mut value=match outcome {
                    "unsupported"|"invalidParams"|"internal"|"invalidRequest"=>json!({"error":{"code":match outcome{"unsupported"=>-32601,"invalidParams"=>-32602,"invalidRequest"=>-32600,_=>-32603},"message":"서버 판정"}}),
                    "badStarted"=>json!({"result":{"status":"started","turn":{"id":""}}}),
                    "badNotStarted"=>json!({"result":{"status":"notStarted"}}),
                    "forgedUnsupported"=>json!({"result":{"status":"unsupported"}}),
                    "hybridError"=>json!({"error":{"code":-32603,"message":"내부 실패"},"result":{"status":"started","turn":{"id":"compact-turn"}}}),
                    _=>json!({"result":{"status":outcome,"reason":"대기 입력"}}),
                };value["id"]=request["id"].clone();socket.send(Message::Text(value.to_string().into())).unwrap();assert_closed_without_input(&mut socket);
            });
            compact_gui(&control,rpc,cwd);let before=control.context_status("original").unwrap();
            let result=control.request("original",Path::new("."),ConversationRequest::Compact);assert_eq!(result.is_ok(),known,"{outcome}");assert_eq!(control.0.unknown.load(Ordering::Acquire),!known,"{outcome}");
            let status=control.context_status("original").unwrap();assert_eq!(status["tokenUsage"],before["tokenUsage"]);assert_eq!(status["compacting"],false);
            if known {let value=result.unwrap();assert_eq!(value["status"],match outcome{"unsupported"=>"unsupported","invalidParams"=>"notStarted",other=>other});assert_eq!(status["active"],before["active"]);assert_eq!(status["tokenUsageFresh"],before["tokenUsageFresh"]);}
            else {assert!(control.terminal_write(||Ok(1)).is_err());let next=control.request("original",Path::new("."),ConversationRequest::Compact);if let Ok(value)=next {assert_eq!(value["reason"],"unknownResult");}}
            if outcome=="unsupported" {assert_eq!(control.request("original",Path::new("."),ConversationRequest::Compact).unwrap()["status"],"unsupported");}
            control.cancel();server.join().unwrap();
        }
    }
    #[test]
    fn compact_reply_never_resurrects_early_completed_or_newer_observed_turns() {
        for sequence in ["start","itemComplete","complete","nextTurn","observerComplete","observerNext","observerUsage","foreign"] {
            let(control,rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let observed=control.clone();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);compact_bootstrap(&mut socket,&expected_cwd,Some(json!(true)));
                let request=read_request(&mut socket);assert_eq!(request["method"],COMPACT_IF_IDLE);
                let thread_id=if sequence=="foreign"{"other"}else{"original"};
                let mut messages=vec![json!({"method":"turn/started","params":{"threadId":thread_id,"turn":{"id":"compact-turn"}}}),json!({"method":"item/started","params":{"threadId":thread_id,"turnId":"compact-turn","item":{"id":"compact-item","type":"contextCompaction"}}})];
                if sequence!="start" {messages.push(json!({"method":"item/completed","params":{"threadId":thread_id,"turnId":"compact-turn","item":{"id":"compact-item","type":"contextCompaction"}}}));}
                if sequence!="start"&&sequence!="itemComplete" {messages.push(json!({"method":"turn/completed","params":{"threadId":thread_id,"turn":{"id":"compact-turn","status":"completed"}}}));}
                if matches!(sequence,"nextTurn"|"observerNext") {messages.push(json!({"method":"turn/started","params":{"threadId":thread_id,"turn":{"id":"newer-turn"}}}));messages.push(json!({"method":"turn/completed","params":{"threadId":thread_id,"turn":{"id":"compact-turn"}}}));}
                for message in messages {if sequence.starts_with("observer") {observed.observer_message("original",message).unwrap();}else{socket.send(Message::Text(message.to_string().into())).unwrap();}}
                if sequence=="observerUsage" {observed.observer_message("original",json!({"method":"thread/tokenUsage/updated","params":{"threadId":"original","turnId":"compact-turn","tokenUsage":{"total":{"totalTokens":100,"inputTokens":100,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0},"last":{"totalTokens":10,"inputTokens":10,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0},"modelContextWindow":1000}}})).unwrap();}
                reply(&mut socket,&request,json!({"status":"started","turn":{"id":"compact-turn"}}));assert_closed_without_input(&mut socket);
            });
            compact_gui(&control,rpc,cwd);let value=control.request("original",Path::new("."),ConversationRequest::Compact).unwrap();assert_eq!(value["status"],"started");assert_eq!(value["turn"]["id"],"compact-turn");
            let status=control.context_status("original").unwrap();assert_eq!(status["compacting"],sequence=="foreign","{sequence}");
            if !sequence.starts_with("observer")&&sequence!="foreign" {
                assert!(status["active"].is_null(),"화면의 선행 활동을 관찰자 상태로 승격하면 안 된다: {sequence}");assert_eq!(status["reason"],"statusUnknown");
                control.observer_message("original",json!({"method":"turn/started","params":{"threadId":"original","turn":{"id":"compact-turn"}}})).unwrap();assert_eq!(control.context_status("original").unwrap()["active"],true);
                control.observer_message("original",json!({"method":"turn/completed","params":{"threadId":"original","turn":{"id":"compact-turn"}}})).unwrap();assert_eq!(control.context_status("original").unwrap()["active"],false);
            } else {assert_eq!(status["active"],!matches!(sequence,"observerComplete"|"observerUsage"),"{sequence}");}
            if sequence=="observerNext" {assert_eq!(control.0.events.lock().unwrap().context_turn.as_deref(),Some("newer-turn"));}
            if sequence=="observerUsage" {assert_eq!(status["tokenUsageFresh"],true);assert_eq!(status["tokenUsage"]["last"]["totalTokens"],10);}
            assert!(!control.0.unknown.load(Ordering::Acquire));control.cancel();server.join().unwrap();
        }
    }
    #[test]
    fn interrupt_requires_both_gui_capabilities_independently_of_compact_and_resets_on_reconnect() {
        let control=fixture().0;
        for compact in [false,true] {
            for (initialize_interrupt,passive_interrupt) in [(Some(true),Some(true)),(None,None),(Some(false),Some(false)),(Some(false),Some(true)),(Some(true),Some(false)),(Some(true),None),(None,Some(true))] {
                let(_,rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let supported=initialize_interrupt==Some(true)&&passive_interrupt==Some(true);
                let server=thread::spawn(move||{
                    let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                    conversation_bootstrap(&mut socket,&expected_cwd,Some(json!(compact)),initialize_interrupt.map(Value::Bool),passive_interrupt.map(Value::Bool));
                    let page=read_request(&mut socket);assert_eq!(page["method"],"thread/turns/list");reply(&mut socket,&page,json!({"data":[{"id":"preserved-turn","status":"inProgress"}],"nextCursor":"older"}));
                    if supported {let request=read_request(&mut socket);assert_eq!(request["method"],"turn/interrupt");assert_eq!(request["params"],json!({"threadId":"original","turnId":"requested-turn"}));reply(&mut socket,&request,json!({}));}
                    assert_closed_without_input(&mut socket);
                });
                compact_gui(&control,rpc,cwd);assert_eq!(control.context_status("original").unwrap()["canCompact"],compact);
                let before=control.context_status("original").unwrap();let page=control.request("original",Path::new("."),ConversationRequest::Page{cursor:None}).unwrap();assert_eq!(page["data"][0]["id"],"preserved-turn");assert_eq!(page["nextCursor"],"older");
                let result=control.request("original",Path::new("."),ConversationRequest::Interrupt{turn_id:"requested-turn".into()});assert_eq!(result.is_ok(),supported);
                if let Err(error)=result {assert!(error.contains("요청을 보내지 않았습니다"));assert!(!error.contains("supportsExpectedTurnInterrupt"));}
                assert!(!control.0.unknown.load(Ordering::Acquire));let after=control.context_status("original").unwrap();assert_eq!(after["active"],before["active"]);assert_eq!(after["tokenUsage"],before["tokenUsage"]);
                assert_eq!(control.0.events.lock().unwrap().gui_interrupt_supported,supported);
                assert_eq!(control.0.events.lock().unwrap().interrupt_diagnostic.is_some(),(initialize_interrupt==Some(true)) ^ (passive_interrupt==Some(true)));
                control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();assert!(!control.0.events.lock().unwrap().gui_interrupt_supported);assert!(control.0.events.lock().unwrap().interrupt_diagnostic.is_none());server.join().unwrap();
            }
        }
    }
    #[test]
    fn interrupt_malformed_capabilities_fail_before_further_queries_and_observer_never_grants_gui_support() {
        for capability in [Value::Null,json!("true"),json!(1),json!({}),json!([])] {
            for phase in ["initialize","passive","observer"] {
                let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let advertised=capability.clone();
                control.0.events.lock().unwrap().gui_interrupt_supported=phase!="observer";
                let server=thread::spawn(move||{
                    let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                    if phase=="observer" {let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}},"supportsExpectedTurnInterrupt":advertised}));}
                    else {conversation_bootstrap(&mut socket,&expected_cwd,Some(json!(true)),Some(if phase=="initialize"{advertised.clone()}else{json!(true)}),Some(if phase=="passive"{advertised}else{json!(true)}));}
                    assert_closed_without_input(&mut socket);
                });
                let error=if phase=="observer" {control.observer_subscribe(&mut rpc,"original",&cwd).unwrap_err()}
                    else {let(_sender,receiver)=mpsc::sync_channel(1);serve(&control.0,&mut rpc,"original",&cwd,receiver,&AtomicBool::new(false),&Mutex::new(None)).unwrap_err()};
                assert!(error.contains("지원 정보 형식"));assert!(!error.contains("supportsExpectedTurnInterrupt"));assert!(!control.0.events.lock().unwrap().gui_interrupt_supported);assert_eq!(rpc.next_id,if phase=="passive"{2}else{1});drop(rpc);server.join().unwrap();
            }
        }
        let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        let server=thread::spawn(move||{
            let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}},"supportsExpectedTurnInterrupt":true,"supportsCompactIfIdle":true}));assert_closed_without_input(&mut socket);
        });
        assert!(control.observer_subscribe(&mut rpc,"original",&cwd).unwrap());
        {let mut events=control.0.events.lock().unwrap();assert!(!events.gui_interrupt_supported);events.connected=true;}
        assert!(execute(&control.0,&mut rpc,"original",ConversationRequest::Interrupt{turn_id:"requested-turn".into()},&AtomicBool::new(false)).unwrap_err().contains("요청을 보내지 않았습니다"));assert!(!control.0.unknown.load(Ordering::Acquire));assert_eq!(rpc.next_id,1);drop(rpc);server.join().unwrap();
    }
    #[test]
    fn bounded_events_mark_gaps_and_release_drained_bytes() {
        let mut events = Events::default();
        for index in 0..EVENT_COUNT * 3 { events.push(json!({"method":"delta","params":{"delta":index.to_string()}})); }
        assert_eq!(events.queue.len(), EVENT_COUNT); assert!(events.gap); assert!(events.bytes <= EVENT_BYTES);
        let page = events.poll(); assert_eq!(page["events"].as_array().unwrap().len(), EVENT_COUNT); assert_eq!(page["gap"], true);
        assert_eq!(events.bytes, 0); assert!(events.queue.is_empty()); assert_eq!(events.poll()["gap"], false);
    }
    #[test]
    fn detail_chunks_round_trip_unicode_without_losing_content() {
        let text = "가🙂나".repeat(10000); let mut offset = 0; let mut collected = String::new();
        loop { let page = detail_chunk(&text,offset).unwrap(); collected.push_str(page["text"].as_str().unwrap());
            match page["nextOffset"].as_u64() {Some(next) => {assert!(next > offset as u64); offset = next as usize;},None => break}
        }
        assert_eq!(collected,text); assert!(detail_chunk("가",1).is_err());
    }
    #[test]
    fn large_fields_are_explicit_detail_references_and_unknown_attachments_fail() {
        let mut value = json!({"id":"same-item","text":"x".repeat(TEXT_PREVIEW+1)}); summarize(&mut value);
        assert_eq!(value["id"],"same-item"); assert_eq!(value["text"]["__detail"],true); assert_eq!(value["text"]["bytes"],TEXT_PREVIEW+1);
        assert!(user_input("",&[Attachment {r#type:"audio".into(),path:"/fake".into()}]).is_err());
        assert!(user_input("",&[]).is_err()); assert!(user_input(&"a".repeat(65537),&[]).is_err());
    }
    #[test]
    fn requests_belong_to_the_original_thread_and_resolve_only_once() {
        let control = ConversationConnection::new(Endpoint::Unavailable,Arc::new(AtomicBool::new(false)));
        observe_message(&control.0,"original",json!({"id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"other"}})).unwrap();
        assert!(control.0.events.lock().unwrap().pending.is_empty());
        observe_message(&control.0,"original",json!({"id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}})).unwrap();
        assert_eq!(control.0.events.lock().unwrap().pending.len(),1);
        observe_message(&control.0,"original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":7}})).unwrap();
        assert!(control.0.events.lock().unwrap().pending.is_empty());
    }
    #[test]
    fn terminal_input_is_exclusive_and_persistent_policy_responses_fail() {
        let control = ConversationConnection::new(Endpoint::Unavailable,Arc::new(AtomicBool::new(false)));
        assert_eq!(control.terminal_write(|| Ok(42)).unwrap(),42);
        *control.0.input.lock().unwrap() = true; assert!(control.terminal_write(|| Ok(42)).is_err());
        let request = json!({"method":"item/commandExecution/requestApproval"});
        assert!(validate_response(&request,&json!({"decision":"accept"})).is_ok());
        assert!(validate_response(&request,&json!({"decision":{"acceptWithExecpolicyAmendment":{}}})).is_err());
    }
    #[test]
    fn approval_choices_intersect_advertised_decisions_and_reject_unknown_or_empty_lists() {
        let params=json!({"availableDecisions":["decline","acceptForSession",{"acceptWithExecpolicyAmendment":{}},"futureDecision"]});
        assert_eq!(ConversationConnection::approval_decisions(&params),["acceptForSession","decline"]);
        let request=json!({"method":"item/commandExecution/requestApproval","params":params});
        assert!(validate_response(&request,&json!({"decision":"accept"})).is_err());
        assert!(validate_response(&request,&json!({"decision":"decline"})).is_ok());
        for advertised in [json!([]),Value::Null,json!(["futureDecision"]),json!({"accept":true})] {
            let params=json!({"availableDecisions":advertised});assert!(ConversationConnection::approval_decisions(&params).is_empty());
            assert!(validate_response(&json!({"method":"item/commandExecution/requestApproval","params":params}),&json!({"decision":"accept"})).is_err());
        }
        assert_eq!(ConversationConnection::approval_decisions(&json!({})),["accept","decline","cancel"]);
    }
    #[test]
    fn unsupported_passive_observer_and_gui_never_call_resume_or_create_a_thread() {
        for gui in [false,true] {
            for code in [-32600,-32601] {
            let (control,mut rpc,listener)=fixture();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);let mut methods=Vec::new();
                if gui {
                    let request=read_request(&mut socket);assert_eq!(request["method"],"initialize");methods.push("initialize");reply(&mut socket,&request,supported_initialize());
                    let request=read_request(&mut socket);assert_eq!(request["method"],"initialized");methods.push("initialized");
                }
                let request=read_request(&mut socket);assert_eq!(request["method"],PASSIVE_SUBSCRIBE);assert_eq!(request["params"],json!({"threadId":"original"}));methods.push(PASSIVE_SUBSCRIBE);
                socket.send(Message::Text(json!({"id":request["id"],"error":{"code":code,"message":"unknown method"}}).to_string().into())).unwrap();
                assert!(socket.read().is_err(),"미지원 응답 뒤 resume/start/read 대체 요청이 한 건이라도 나가면 안 된다");methods
            });
            let error=if gui {let(_sender,receiver)=mpsc::sync_channel(1);serve(&control.0,&mut rpc,"original",Path::new("."),receiver,&AtomicBool::new(false),&Mutex::new(None)).unwrap_err()}
                else {control.observer_subscribe(&mut rpc,"original",Path::new(".")).unwrap_err()};
            assert!(error.contains("호환되는 서버"));assert!(!control.0.unknown.load(Ordering::Acquire));drop(rpc);
            let expected=if gui {vec!["initialize","initialized",PASSIVE_SUBSCRIBE]} else {vec![PASSIVE_SUBSCRIBE]};assert_eq!(server.join().unwrap(),expected);
            }
        }
    }
    #[test]
    fn atomic_user_admission_has_no_pre_read_and_preserves_model_effort_and_client_id() {
        for status in ["started","skippedBusy","notStarted","unsupported","invalidParams","invalidRequest","internal","invalid"] {
            let (control,mut rpc,listener)=fixture();let client_id=Uuid::new_v4().to_string();let expected_id=client_id.clone();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let request=read_request(&mut socket);assert_eq!(request["method"],START_USER_IF_IDLE);assert_eq!(request["params"],json!({"threadId":"original","input":[{"type":"text","text":"원래 사용자 입력","text_elements":[]}],"clientUserMessageId":expected_id,"model":"server-model","effort":"high"}));
                if matches!(status,"unsupported" | "invalidParams" | "invalidRequest" | "internal") {
                    let code=match status {"unsupported"=>-32601,"invalidParams"=>-32602,"invalidRequest"=>-32600,_=>-32603};
                    socket.send(Message::Text(json!({"id":request["id"],"error":{"code":code,"message":"server rejection"}}).to_string().into())).unwrap();
                }
                else {reply(&mut socket,&request,json!({"status":status,"turn":{"id":"same-turn"},"reason":"busy mailbox"}));}
                assert!(socket.read().is_err(),"원자적 접수 뒤 일반 turn/start 등 대체·재전송이 나가면 안 된다");
            });
            let result=execute(&control.0,&mut rpc,"original",ConversationRequest::Send{request_id:client_id,text:"원래 사용자 입력".into(),attachments:vec![],model:Some("server-model".into()),effort:Some("high".into())},&AtomicBool::new(false));
            let unknown=matches!(status,"invalid" | "internal" | "invalidRequest");
            assert_eq!(result.is_ok(),status=="started");assert_eq!(control.0.unknown.load(Ordering::Acquire),unknown);
            if unknown {assert!(execute(&control.0,&mut rpc,"original",ConversationRequest::Send{request_id:Uuid::new_v4().to_string(),text:"다음 입력".into(),attachments:vec![],model:None,effort:None},&AtomicBool::new(false)).unwrap_err().contains("이전 전송 결과"));}
            if status=="unsupported" {assert!(result.unwrap_err().contains("호환되는 서버"));}
            drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn unavailable_history_is_not_an_empty_page_or_detail() {
        for request in [ConversationRequest::Page{cursor:Some("older".into())},ConversationRequest::Items{turn_id:"turn".into(),cursor:None},ConversationRequest::Detail{turn_id:"turn".into(),item_id:"item".into(),offset:Some(0),snapshot_id:None}] {
            let (control,mut rpc,listener)=fixture();let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);let request=read_request(&mut socket);
                socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32600,"message":"not materialized","data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}).to_string().into())).unwrap();
            });
            assert_eq!(execute(&control.0,&mut rpc,"original",request,&AtomicBool::new(false)).unwrap_err(),HISTORY_UNAVAILABLE);assert!(control.0.detail.lock().unwrap().is_none());drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn first_page_waits_only_for_exact_unmaterialized_error_without_inventing_history() {
        for(case,page_cursor,payload,waiting)in [
            ("exact",None,json!({"error":{"code":-32600,"data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}),true),
            ("older",Some("older"),json!({"error":{"code":-32600,"data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}),false),
            ("emptyCursor",Some(""),json!({"error":{"code":-32600,"data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}),false),
            ("internal",None,json!({"error":{"code":-32603,"data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}),false),
            ("stringCode",None,json!({"error":{"code":"-32600","data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}),false),
            ("otherKind",None,json!({"error":{"code":-32600,"data":{"kind":"other","reason":"unmaterialized"}}}),false),
            ("otherReason",None,json!({"error":{"code":-32600,"data":{"kind":"historyProjectionUnavailable","reason":"other"}}}),false),
            ("missingReason",None,json!({"error":{"code":-32600,"data":{"kind":"historyProjectionUnavailable"}}}),false),
            ("forgedSuccess",None,json!({"result":{"historyUnavailable":true,"reason":"unmaterialized"}}),false),
            ("forgedEmptySuccess",None,json!({"result":{"historyUnavailable":true,"reason":"unmaterialized","data":[]}}),false),
        ] {
            let(control,mut rpc,listener)=fixture();let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let request=read_request(&mut socket);assert_eq!(request["method"],"thread/turns/list");assert_eq!(request["params"]["threadId"],"original");assert_eq!(request["params"]["cursor"],json!(page_cursor));
                let mut response=payload;response["id"]=request["id"].clone();socket.send(Message::Text(response.to_string().into())).unwrap();assert_closed_without_input(&mut socket);
            });
            let result=execute(&control.0,&mut rpc,"original",ConversationRequest::Page{cursor:page_cursor.map(str::to_owned)},&AtomicBool::new(false));assert_eq!(result.is_ok(),waiting,"{case}");
            if waiting {let value=result.unwrap();assert_eq!(value,json!({"historyUnavailable":true,"reason":"unmaterialized"}));assert!(value.get("data").is_none());assert!(value.get("activeTurnId").is_none());}
            assert!(!control.0.unknown.load(Ordering::Acquire));assert!(control.0.detail.lock().unwrap().is_none());drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn passive_attach_preserves_metadata_when_history_is_unmaterialized() {
        let (control,mut rpc,listener)=fixture();let cwd=std::env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
        let server=thread::spawn(move||{
            let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let init=read_request(&mut socket);assert_eq!(init["method"],"initialize");reply(&mut socket,&init,supported_initialize());assert_eq!(read_request(&mut socket)["method"],"initialized");
            let request=read_request(&mut socket);assert_eq!(request["method"],PASSIVE_SUBSCRIBE);reply(&mut socket,&request,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[]},"model":"actual-model","modelProvider":"actual-provider","reasoningEffort":"high","serviceTier":"ultrafast"}));
            let history=read_request(&mut socket);assert_eq!(history["method"],"thread/turns/list");socket.send(Message::Text(json!({"id":history["id"],"error":{"code":-32600,"message":"not materialized","data":{"kind":"historyProjectionUnavailable","reason":"unmaterialized"}}}).to_string().into())).unwrap();
        });
        let(sender,receiver)=mpsc::sync_channel(2);let(reply,attached)=mpsc::sync_channel(1);sender.send(Work{request:ConversationRequest::Attach,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();let(reply,detached)=mpsc::sync_channel(1);sender.send(Work{request:ConversationRequest::Detach,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();
        serve(&control.0,&mut rpc,"original",&cwd,receiver,&AtomicBool::new(false),&Mutex::new(None)).unwrap();let value=attached.recv().unwrap().unwrap();assert_eq!(value["historyUnavailable"],true);assert_eq!(value["activeTurnKnown"],false);assert!(value["activeTurnId"].is_null());assert_eq!(value["model"],"actual-model");assert_eq!(value["serviceTier"],"ultrafast");assert!(detached.recv().unwrap().is_ok());drop(rpc);server.join().unwrap();
    }
    #[test]
    fn deleted_thread_invalidates_both_pending_maps_and_detail_but_releases_only_after_join() {
        for unknown in [false,true] {
            let(control,mut rpc,_listener)=fixture();install_finished_worker(&control);control.0.unknown.store(unknown,Ordering::Release);
            let request=json!({"id":7,"method":"item/tool/requestUserInput","params":{"threadId":"original","questions":[]}});control.observer_message("original",request.clone()).unwrap();observe_message(&control.0,"original",request).unwrap();
            *control.0.detail.lock().unwrap()=Some(DetailSnapshot{id:"snapshot".into(),turn_id:"turn".into(),item_id:"item".into(),text:Arc::from("old content")});
            let deleted=json!({"method":"thread/deleted","params":{"threadId":"original"}});assert!(control.observer_message("original",deleted.clone()).is_err());assert!(observe_message(&control.0,"original",deleted).is_err());
            assert!(control.is_deleted());assert!(control.pending_snapshot().unwrap().2.is_empty());assert!(control.0.detail.lock().unwrap().is_none());assert!(control.terminal_write(||Ok(1)).is_err(),"알림 수신만으로 입력 소유권을 해제하면 안 된다");
            let poll=control.request("original",Path::new("."),ConversationRequest::Poll).unwrap();assert_eq!(poll["events"].as_array().unwrap().len(),1);assert_eq!(poll["events"][0]["method"],"thread/deleted");assert_eq!(poll["connected"],false);
            assert!(execute(&control.0,&mut rpc,"original",ConversationRequest::Detail{turn_id:"turn".into(),item_id:"item".into(),offset:Some(1),snapshot_id:Some("snapshot".into())},&AtomicBool::new(false)).is_err());
            assert_eq!(control.request("original",Path::new("."),ConversationRequest::Detach).is_err(),unknown);assert_eq!(control.0.owned.load(Ordering::Acquire),unknown);
            assert!(control.request("original",Path::new("."),ConversationRequest::Attach).is_err());control.cancel();
        }
    }
    #[test]
    fn lost_mutation_reply_blocks_later_send_and_never_replays_the_message() {
        let (control,mut rpc,listener) = fixture();
        let server = thread::spawn(move || {
            let (stream,_) = listener.accept().unwrap();
            let mut socket = WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let send = read_request(&mut socket); assert_eq!(send["method"],START_USER_IF_IDLE);
            assert_eq!(send["params"]["threadId"],"original");
            assert_eq!(send["params"]["clientUserMessageId"],"00000000-0000-0000-0000-000000000001");
            assert!(send["params"].get("approvalPolicy").is_none());
            // 실제 서버가 요청을 적용한 직후 응답 없이 연결을 닫는 경계다.
            drop(socket); 1
        });
        let request = || ConversationRequest::Send {request_id:"00000000-0000-0000-0000-000000000001".into(),text:"hello".into(),attachments:vec![],model:None,effort:None};
        assert!(execute(&control.0,&mut rpc,"original",request(),&AtomicBool::new(false)).is_err());
        assert!(control.0.unknown.load(Ordering::Acquire));
        let error = execute(&control.0,&mut rpc,"original",request(),&AtomicBool::new(false)).unwrap_err();
        assert!(error.contains("이전 전송 결과")); assert_eq!(server.join().unwrap(),1);
    }
    #[test]
    fn approval_during_rpc_is_preserved_and_consumed_only_once() {
        let (control,mut rpc,listener) = fixture();
        let server = thread::spawn(move || {
            let (stream,_) = listener.accept().unwrap(); stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut socket = WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let read = read_request(&mut socket);
            socket.send(Message::Text(json!({"id":read["id"],"method":"item/commandExecution/requestApproval","params":{"threadId":"original","turnId":"turn","itemId":"item"}}).to_string().into())).unwrap();
            reply(&mut socket,&read,json!({"data":[],"nextCursor":null}));
            let approval = read_request(&mut socket); assert!(approval.get("method").is_none()); assert_eq!(approval["result"],json!({"decision":"accept"}));
            assert!(socket.read().is_err(),"동일 승인 응답을 재전송하면 안 된다");
        });
        call(&control.0,&mut rpc,"original","model/list",json!({}),&AtomicBool::new(false)).unwrap();
        assert_eq!(control.0.events.lock().unwrap().pending.len(),1);
        let respond = || ConversationRequest::Respond {request_id:"1".into(),result:json!({"decision":"accept"})};
        execute(&control.0,&mut rpc,"original",respond(),&AtomicBool::new(false)).unwrap();
        assert!(execute(&control.0,&mut rpc,"original",respond(),&AtomicBool::new(false)).is_err());
        assert_eq!(control.0.events.lock().unwrap().pending.len(),1,"처리 확인 전에는 승인 소유권을 유지해야 한다");
        drop(rpc); server.join().unwrap();
    }
    #[test]
    fn acknowledged_interrupt_does_not_erase_an_earlier_unknown_send() {
        let (control,mut rpc,listener) = fixture(); control.0.unknown.store(true,Ordering::Release);
        {let mut events=control.0.events.lock().unwrap();events.connected=true;events.gui_interrupt_supported=true;}
        let server = thread::spawn(move || {
            let (stream,_) = listener.accept().unwrap(); let mut socket = WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let request = read_request(&mut socket); assert_eq!(request["params"],json!({"threadId":"original","turnId":"original-turn"})); reply(&mut socket,&request,json!({}));
        });
        execute(&control.0,&mut rpc,"original",ConversationRequest::Interrupt{turn_id:"original-turn".into()},&AtomicBool::new(false)).unwrap();
        assert!(control.0.unknown.load(Ordering::Acquire)); server.join().unwrap();
    }
    #[test]
    fn request_queue_rejects_oversized_data_before_admission() {
        let request = ConversationRequest::Send {request_id:Uuid::new_v4().to_string(),text:"a".repeat(65537),attachments:vec![],model:None,effort:None};
        assert!(validate_request(&request).is_err());
        assert!(bounded_page(json!({"data":vec![json!({});21]}),20).is_err());
        assert!(validate_request(&ConversationRequest::Page{cursor:Some("a".repeat(4097))}).is_err());
    }
    #[test]
    fn deadline_closes_the_owned_socket_and_reaps_its_watcher() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (mut peer,_) = listener.accept().unwrap();peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let transport = Arc::new(Mutex::new(Transport{socket:Some(stream.try_clone().unwrap()),proxy:None}));
        let cancelled = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let mut watch = DeadlineWatch::start(Arc::new(Mutex::new(Some(start+Duration::from_millis(25)))),transport,cancelled.clone(),Arc::new(AtomicBool::new(false))).unwrap();
        // 감시자의 실제 종료를 먼저 회수한다. 스케줄러 지연을 연결 회수 실패로
        // 해석하지 않고, 이어지는 두 read의 실제 EOF/연결 종료만 판정한다.
        watch.join.take().unwrap().join().unwrap();
        let result = stream.read(&mut [0u8;1]);
        let closed=|result:&io::Result<usize>|match result {Ok(0)=>true,Err(error)=>matches!(error.kind(),io::ErrorKind::ConnectionAborted|io::ErrorKind::ConnectionReset),_=>false};
        assert!(closed(&result),"socket shutdown 대신 시간 초과·무관한 오류가 관측됨: {result:?}");
        assert!(cancelled.load(Ordering::Acquire));
        drop(watch);
        let peer_result=peer.read(&mut [0u8;1]);assert!(closed(&peer_result),"상대 연결도 종료되어야 한다: {peer_result:?}");
        eprintln!("기한 감시 수명 시험: local={result:?}, peer={peer_result:?}, watcherJoined=true, elapsed={:?}",start.elapsed());
    }
    #[test]
    fn pending_approval_survives_event_overflow_and_duplicate_delivery() {
        let (control,_rpc,_listener) = fixture();
        let request = json!({"id":"same","method":"item/tool/requestUserInput","params":{"threadId":"original","questions":[{"id":"q","question":"answer?"}]}});
        observe_message(&control.0,"original",request.clone()).unwrap();
        for _ in 0..EVENT_COUNT*2 {observe_message(&control.0,"original",json!({"method":"item/agentMessage/delta","params":{"threadId":"original","delta":"x"}})).unwrap();}
        observe_message(&control.0,"original",request).unwrap();
        let mut events = control.0.events.lock().unwrap();
        assert_eq!(events.pending.len(),1); assert!(events.gap); assert_eq!(events.poll()["pendingRequests"].as_array().unwrap().len(),1);
        assert_eq!(events.pending.len(),1,"화면 조회가 승인 요청을 소비하면 안 된다");
    }
    #[test]
    fn terminal_observer_preserves_requests_during_reads_without_claiming_or_responding() {
        let (control,mut rpc,listener) = fixture();
        let server = thread::spawn(move || {
            let (stream,_) = listener.accept().unwrap(); stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut socket = WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
            let request = read_request(&mut socket); assert_eq!(request["method"],"thread/read");
            socket.send(Message::Text(json!({"id":44,"method":"item/tool/requestUserInput","params":{"threadId":"original","questions":[{"id":"q","question":"원래 터미널 질문"}]}}).to_string().into())).unwrap();
            socket.send(Message::Text(json!({"method":"item/agentMessage/delta","params":{"threadId":"original","delta":"두 번 표시하면 안 됨"}}).to_string().into())).unwrap();
            reply(&mut socket,&request,json!({"thread":{"id":"original"}}));
            assert!(socket.read().is_err(),"관측 연결에서 질문 응답을 보내면 안 된다");
        });
        control.observer_call(&mut rpc,"original","thread/read",json!({"threadId":"original","includeTurns":false})).unwrap();
        let (owned,connected,pending) = control.pending_snapshot().unwrap();
        assert!(!owned&&!connected); assert_eq!(pending.len(),1); assert_eq!(pending[0]["params"]["questions"][0]["question"],"원래 터미널 질문"); assert_eq!(pending[0]["guiOwned"],false);
        assert!(control.0.events.lock().unwrap().queue.is_empty(),"관측 연결이 GUI delta를 중복 적재하면 안 된다");
        assert!(execute(&control.0,&mut rpc,"original",ConversationRequest::Respond{request_id:"44".into(),result:json!({"answers":{"q":{"answers":["답"]}}})},&AtomicBool::new(false)).is_err());
        drop(rpc); server.join().unwrap();
    }
    #[test]
    fn observer_and_gui_deduplicate_one_pending_request_and_resolution_clears_both() {
        let (control,_rpc,_listener)=fixture();
        let request=json!({"id":9,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}});
        control.observer_message("original",request.clone()).unwrap(); observe_message(&control.0,"original",request).unwrap();
        let (_,_,pending)=control.pending_snapshot().unwrap(); assert_eq!(pending.len(),1); assert_eq!(pending[0]["guiOwned"],true);
        control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":9}})).unwrap();
        assert!(control.pending_snapshot().unwrap().2.is_empty()); assert!(control.0.events.lock().unwrap().pending.is_empty());
    }
    #[test]
    fn resolved_ids_block_late_replay_and_stop_request_tracking_at_the_bounded_limit() {
        let(control,_rpc,_listener)=fixture();
        let pending=json!({"id":9,"method":"item/tool/requestUserInput","params":{"threadId":"original","questions":[]}});
        control.observer_message("original",pending.clone()).unwrap();observe_message(&control.0,"original",pending.clone()).unwrap();
        control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":9}})).unwrap();
        install_finished_worker(&control);control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();
        // 새 Attach의 준비와 같은 경계에서도 해결 기록을 지우지 않는다.
        assert!(!control.0.events.lock().unwrap().blocks_attach(false,false));assert!(control.0.events.lock().unwrap().answered.contains("9"));
        observe_message(&control.0,"original",pending.clone()).unwrap();control.observer_message("original",pending).unwrap();assert!(control.pending_snapshot().unwrap().2.is_empty());
        let(control,_rpc,_listener)=fixture();
        for id in 0..1024 {control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":id}})).unwrap();}
        control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":0}})).unwrap();
        let pending=json!({"id":9000,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}});observe_message(&control.0,"original",pending.clone()).unwrap();control.observer_message("original",pending).unwrap();
        {let mut events=control.0.events.lock().unwrap();events.connected=true;events.observer_connected=true;}
        assert!(control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":1024}})).unwrap_err().contains("상한"));
        assert!(observe_message(&control.0,"original",json!({"id":1024,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}})).is_err(),"상한 뒤 다른 연결의 늦은 사본도 삽입하면 안 된다");
        let(_,connected,pending)=control.pending_snapshot().unwrap();assert!(!connected);assert_eq!(pending.len(),1);assert_eq!(pending[0]["requestId"],"9000");assert_eq!(pending[0]["metadataAvailable"],false);
        let events=control.0.events.lock().unwrap();assert_eq!(events.answered.len(),1024);assert!(events.answered.contains("0"));assert!(events.blocks_attach(false,false));assert!(events.blocks_release(true));assert!(events.error.as_deref().is_some_and(|error|error.contains("상한")));
    }
    #[test]
    fn pending_metadata_is_current_only_when_the_connection_holding_that_id_is_live() {
        let (control,_rpc,_listener)=fixture();
        let request=json!({"id":9,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}});
        control.observer_message("original",request.clone()).unwrap();
        control.0.events.lock().unwrap().observer_connected=true;
        assert_eq!(control.pending_snapshot().unwrap().2[0]["metadataAvailable"],true);
        {let mut events=control.0.events.lock().unwrap();events.observer_connected=false;events.connected=true;}
        assert_eq!(control.pending_snapshot().unwrap().2[0]["metadataAvailable"],false,"관계없는 살아 있는 GUI 연결이 과거 관측을 확정하면 안 된다");
        observe_message(&control.0,"original",request).unwrap();
        let snapshot=control.pending_snapshot().unwrap().2;assert_eq!(snapshot.len(),1);assert_eq!(snapshot[0]["metadataAvailable"],true);assert_eq!(snapshot[0]["guiOwned"],true);
        {let mut events=control.0.events.lock().unwrap();events.connected=false;events.observer_connected=true;events.pending.get_mut("9").unwrap()["responded"]=json!(true);}
        let snapshot=control.pending_snapshot().unwrap().2;assert_eq!(snapshot[0]["metadataAvailable"],true);assert_eq!(snapshot[0]["responded"],true);
        control.0.events.lock().unwrap().observed_pending.clear();
        assert_eq!(control.pending_snapshot().unwrap().2[0]["metadataAvailable"],false);
        assert_eq!(control.pending_snapshot().unwrap().2.len(),1,"관측 단절은 서버 해결 확인이 아니므로 요청을 제거하면 안 된다");
    }
    fn install_finished_worker(control: &ConversationConnection) {
        let (sender,receiver)=mpsc::sync_channel(1);drop(receiver);
        let join=thread::spawn(||()); let deadline=Instant::now()+Duration::from_secs(2);
        while !join.is_finished() && Instant::now()<deadline {thread::yield_now();}
        assert!(join.is_finished());
        *control.0.worker.lock().unwrap()=Some(Worker{sender,cancelled:Arc::new(AtomicBool::new(false)),transport:Arc::new(Mutex::new(Transport::default())),join:Some(join),joined:false,closing:None});
        *control.0.input.lock().unwrap()=true;control.0.owned.store(true,Ordering::Release);
    }
    fn install_actor(control: &ConversationConnection, actor: impl FnOnce(Arc<Control>,mpsc::Receiver<Work>,Arc<AtomicBool>)+Send+'static) {
        let(sender,receiver)=mpsc::sync_channel(8);let cancelled=Arc::new(AtomicBool::new(false));
        let state=control.0.clone();let running=cancelled.clone();
        let join=thread::spawn(move||{actor(state.clone(),receiver,running);state.events.lock().unwrap().connected=false;});
        *control.0.worker.lock().unwrap()=Some(Worker{sender,cancelled,transport:Arc::new(Mutex::new(Transport::default())),join:Some(join),joined:false,closing:None});
        *control.0.input.lock().unwrap()=true;control.0.owned.store(true,Ordering::Release);
    }
    #[test]
    fn explicit_terminal_handoff_preserves_general_pending_and_joins_before_pty_input() {
        for method in ["item/commandExecution/requestApproval","item/fileChange/requestApproval","item/tool/requestUserInput","item/permissions/requestApproval"] {
            let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            let pending=json!({"id":17,"method":method,"params":{"threadId":"original","availableDecisions":[],"questions":[],"reason":"안전 선택지가 없거나 지원하지 않는 일반 요청 원문"}});let expected=pending.clone();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let init=read_request(&mut socket);assert_eq!(init["method"],"initialize");reply(&mut socket,&init,supported_initialize());assert_eq!(read_request(&mut socket)["method"],"initialized");
                let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);
                socket.send(Message::Text(expected.to_string().into())).unwrap();reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[]}}));
                let history=read_request(&mut socket);assert_eq!(history["method"],"thread/turns/list");reply(&mut socket,&history,json!({"data":[]}));
                assert_closed_without_input(&mut socket);
            });
            install_actor(&control,move|state,receiver,cancelled|{serve(&state,&mut rpc,"original",&cwd,receiver,&cancelled,&Mutex::new(None)).unwrap();});
            // 동일 원문의 두 연결 사본을 보존한다. Attach 응답은 초기 구독 완료의 장벽이다.
            control.observer_message("original",pending.clone()).unwrap();control.0.events.lock().unwrap().observer_connected=true;
            let(reply,attached)=mpsc::sync_channel(1);control.0.worker.lock().unwrap().as_ref().unwrap().sender.try_send(Work{request:ConversationRequest::Attach,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();attached.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
            assert!(control.request("original",Path::new("."),ConversationRequest::Detach).is_err(),"자동 정리 규칙은 완화하면 안 된다");
            assert!(control.0.worker.lock().unwrap().as_ref().unwrap().closing.is_none(),"거절된 자동 정리는 살아 있는 actor의 admission을 복원해야 한다");
            assert!(control.terminal_write(||Ok(1)).is_err());
            control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal).unwrap();
            assert!(control.0.worker.lock().unwrap().is_none());assert!(!control.0.owned.load(Ordering::Acquire));assert_eq!(control.terminal_write(||Ok(1)).unwrap(),1);
            let(owned,connected,snapshot)=control.pending_snapshot().unwrap();assert!(!owned&&!connected);assert_eq!(snapshot.len(),1);assert_eq!(snapshot[0]["method"],method);assert_eq!(snapshot[0]["params"],pending["params"]);assert_eq!(snapshot[0]["requestId"],"17");assert_eq!(snapshot[0]["guiOwned"],false);assert_eq!(snapshot[0]["metadataAvailable"],true);assert_ne!(snapshot[0]["responded"],true);
            {let events=control.0.events.lock().unwrap();assert_eq!(events.pending.len(),1);assert_eq!(events.observed_pending.len(),1);assert!(events.answered.is_empty());}
            server.join().unwrap();
            control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":17}})).unwrap();assert!(control.pending_snapshot().unwrap().2.is_empty());
        }
    }
    #[test]
    fn handoff_rejects_every_unsafe_copy_and_unknown_without_releasing_input() {
        for reason in ["guiResponded","observerResponded","answered","verification","mcpForm","futureMethod","unknown"] {
            let(control,_rpc,_listener)=fixture();install_finished_worker(&control);
            let method=match reason {"verification"|"mcpForm"=>"mcpServer/elicitation/request","futureMethod"=>"item/future/requestApproval",_=>"item/commandExecution/requestApproval"};
            let request=json!({"id":9,"method":method,"params":{"threadId":"original","mode":if reason=="verification" {"openai/userVerification"} else {"form"}}});
            control.observer_message("original",request.clone()).unwrap();observe_message(&control.0,"original",request).unwrap();
            {let mut events=control.0.events.lock().unwrap();match reason {"guiResponded"=>events.pending.get_mut("9").unwrap()["responded"]=json!(true),"observerResponded"=>events.observed_pending.get_mut("9").unwrap()["responded"]=json!(true),"answered"=>{events.answered.insert("9".into());},"unknown"=>control.0.unknown.store(true,Ordering::Release),_=>{}}}
            assert!(control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal).is_err(),"{reason} 상태에서 입력을 인계하면 안 된다");assert!(control.0.owned.load(Ordering::Acquire));assert!(control.terminal_write(||Ok(1)).is_err());assert_eq!(control.pending_snapshot().unwrap().2.len(),1);control.cancel();
        }
    }
    #[test]
    fn handoff_closes_admission_and_rechecks_late_requests_after_join() {
        for late in ["general","owner","responded","unknown","panic"] {
            let(control,_rpc,_listener)=fixture();let(entered,ready)=mpsc::sync_channel(1);let(release,released)=mpsc::sync_channel(1);
            control.0.events.lock().unwrap().connected=true;
            install_actor(&control,move|state,receiver,_cancelled|{
                let work=receiver.recv_timeout(Duration::from_secs(3)).unwrap();assert!(matches!(work.request,ConversationRequest::HandoffToTerminal));work.reply.send(Ok(json!({}))).unwrap();entered.send(()).unwrap();released.recv_timeout(Duration::from_secs(3)).unwrap();
                if late=="panic" {panic!("인계 중 actor 종료 실패 합성");}
                if late=="unknown" {state.unknown.store(true,Ordering::Release);} else {
                    let method=if late=="owner" {"mcpServer/elicitation/request"} else {"item/commandExecution/requestApproval"};
                    observe_message(&state,"original",json!({"id":19,"method":method,"params":{"threadId":"original","mode":"openai/userVerification"}})).unwrap();
                    if late=="responded" {state.events.lock().unwrap().pending.get_mut("19").unwrap()["responded"]=json!(true);}
                }
            });
            let request_control=control.clone();let caller=thread::spawn(move||request_control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal));
            ready.recv_timeout(Duration::from_secs(3)).unwrap();assert!(control.terminal_write(||Ok(1)).is_err(),"join 전에는 PTY 입력을 허용하면 안 된다");
            // release가 worker 잠금으로 join 중일 수 있으므로 admission 검사는 별도 요청자가
            // 종료를 기다리는 것이 허용된다. 종료 뒤에는 살아 있는 actor가 없어 wire 입력은 0이다.
            release.send(()).unwrap();let result=caller.join().unwrap();assert_eq!(result.is_ok(),late=="general");
            assert_eq!(control.terminal_write(||Ok(1)).is_ok(),late=="general");assert_eq!(control.0.owned.load(Ordering::Acquire),late!="general");
            if late=="panic" {let poll=control.request("original",Path::new("."),ConversationRequest::Poll).unwrap();assert_eq!(poll["connected"],false);assert!(poll["error"].as_str().is_some_and(|error|error.contains("비정상 종료")));}
            if late!="panic"&&late!="unknown" {assert_eq!(control.pending_snapshot().unwrap().2.len(),1);}
            if late!="general" {assert!(control.request("original",Path::new("."),ConversationRequest::Page{cursor:None}).is_err());}control.cancel();
        }
    }
    #[test]
    fn queued_response_is_settled_before_handoff_and_rejection_restores_live_admission() {
        let(control,_rpc,_listener)=fixture();let(entered,ready)=mpsc::sync_channel(1);let(release,released)=mpsc::sync_channel(1);
        observe_message(&control.0,"original",json!({"id":9,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}})).unwrap();
        install_actor(&control,move|state,receiver,cancelled|{
            let work=receiver.recv_timeout(Duration::from_secs(3)).unwrap();assert!(matches!(work.request,ConversationRequest::Respond{..}));entered.send(()).unwrap();released.recv_timeout(Duration::from_secs(3)).unwrap();
            {let mut events=state.events.lock().unwrap();events.pending.get_mut("9").unwrap()["responded"]=json!(true);events.answered.insert("9".into());}work.reply.send(Ok(json!({}))).unwrap();
            let work=receiver.recv_timeout(Duration::from_secs(3)).unwrap();assert!(matches!(work.request,ConversationRequest::HandoffToTerminal));assert!(state.events.lock().unwrap().blocks_release(true));work.reply.send(Err("송신된 응답의 해결을 기다립니다.".into())).unwrap();
            let work=receiver.recv_timeout(Duration::from_secs(3)).unwrap();assert!(matches!(work.request,ConversationRequest::Models{..}));work.reply.send(Ok(json!({"data":[]}))).unwrap();
            while !cancelled.load(Ordering::Acquire) {thread::sleep(Duration::from_millis(1));}
        });
        let response_control=control.clone();let response=thread::spawn(move||response_control.request("original",Path::new("."),ConversationRequest::Respond{request_id:"9".into(),result:json!({"decision":"accept"})}));ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let request_control=control.clone();let handoff=thread::spawn(move||request_control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal));
        let deadline=Instant::now()+Duration::from_secs(3);loop {if control.0.worker.lock().unwrap().as_ref().unwrap().closing.is_some(){break;}assert!(Instant::now()<deadline);thread::yield_now();}
        for request in [ConversationRequest::Respond{request_id:"9".into(),result:json!({"decision":"decline"})},ConversationRequest::Send{request_id:Uuid::new_v4().to_string(),text:"경합 입력".into(),attachments:vec![],model:None,effort:None}] {assert!(control.request("original",Path::new("."),request).unwrap_err().contains("인계 중"));}
        assert!(control.0.sent.lock().unwrap().is_empty());assert!(control.terminal_write(||Ok(1)).is_err());release.send(()).unwrap();response.join().unwrap().unwrap();assert!(handoff.join().unwrap().is_err());
        assert!(control.0.worker.lock().unwrap().as_ref().unwrap().closing.is_none());assert!(control.terminal_write(||Ok(1)).is_err());control.request("original",Path::new("."),ConversationRequest::Models{cursor:None}).unwrap();control.cancel();
    }
    #[test]
    fn observer_defers_until_verified_loaded_and_marks_only_successful_subscription() {
        for outcome in ["success","unsupported","internal"] {
            let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(4))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);let mut methods=Vec::new();
                // 최초 thread/read가 notLoaded이면 수동 구독을 보내지 않고,
                // 동일 대화가 loaded임을 검증한 두 번째 조회 뒤에만 구독한다.
                for state in ["notLoaded","active"] {
                    let request=read_request(&mut socket);assert_eq!(request["method"],"thread/read");assert_eq!(request["params"],json!({"threadId":"original","includeTurns":false}));methods.push("thread/read");reply(&mut socket,&request,json!({"thread":{"id":"original","cwd":expected_cwd,"status":{"type":state}}}));
                }
                let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);assert_eq!(subscribe["params"],json!({"threadId":"original"}));methods.push(PASSIVE_SUBSCRIBE);
                if outcome=="success" {
                    socket.send(Message::Text(json!({"id":31,"method":"item/tool/requestUserInput","params":{"threadId":"original","questions":[{"id":"q","question":"재개 후의 질문 원문"}]}}).to_string().into())).unwrap();
                    // The subscribe snapshot is already idle; no active async turn exists.
                    reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}}}));
                } else {socket.send(Message::Text(json!({"id":subscribe["id"],"error":{"code":if outcome=="unsupported" {-32601} else {-32603},"message":"준비 여부를 분류할 최종 오류 계약 없음"}}).to_string().into())).unwrap();}
                let request=read_request(&mut socket);assert_eq!(request["method"],"thread/read","실패한 구독을 성공으로 간주하거나 일반 resume/문자열 기반 재시도를 하면 안 된다");methods.push("thread/read");reply(&mut socket,&request,json!({"thread":{"id":"original","cwd":expected_cwd,"status":{"type":"idle"}}}));
                assert_closed_without_input(&mut socket);methods
            });
            let states=std::cell::RefCell::new(Vec::new());let stopped=AtomicBool::new(false);
            let sink=|event|if let CodexEvent::Status(state)=event {
                states.borrow_mut().push(state.clone());
                if state=="notLoaded" {assert!(!control.0.events.lock().unwrap().observer_connected);assert!(control.pending_snapshot().unwrap().2.is_empty());}
                if state=="running" {
                    let(owned,connected,pending)=control.pending_snapshot().unwrap();assert!(!owned&&!connected);assert_eq!(control.0.events.lock().unwrap().observer_connected,outcome=="success");assert_eq!(pending.len(),usize::from(outcome=="success"));
                    if outcome=="success" {assert_eq!(pending[0]["params"]["questions"][0]["question"],"재개 후의 질문 원문");assert_eq!(pending[0]["guiOwned"],false);assert_eq!(pending[0]["metadataAvailable"],true);}
                }
                if state=="idle" {stopped.store(true,Ordering::Release);}
            };
            observe(&mut rpc,Some("original".into()),&cwd,&stopped,&sink,Some(&control));
            assert_eq!(*states.borrow(),["notLoaded","running","idle"]);assert!(!control.0.events.lock().unwrap().observer_connected);assert!(!control.0.owned.load(Ordering::Acquire));assert_eq!(control.pending_snapshot().unwrap().2.len(),usize::from(outcome=="success"));drop(rpc);assert_eq!(server.join().unwrap(),["thread/read","thread/read",PASSIVE_SUBSCRIBE,"thread/read"]);
        }
    }
    #[test]
    fn observer_preparation_classifier_uses_only_exact_rpc_error_data() {
        for(case,error,retryable)in [
            ("notLoaded",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":"notLoaded"}}),true),
            ("listenerNotReady",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":"listenerNotReady"}}),true),
            ("closing",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":"closing"}}),false),
            ("runtimeChanged",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":"runtimeChanged"}}),false),
            ("unsupportedRuntime",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":"unsupportedRuntime"}}),false),
            ("unknownReason",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":"later"}}),false),
            ("wrongCode",json!({"code":-32603,"data":{"kind":"threadSubscriptionUnavailable","reason":"notLoaded"}}),false),
            ("stringCode",json!({"code":"-32600","data":{"kind":"threadSubscriptionUnavailable","reason":"notLoaded"}}),false),
            ("wrongKind",json!({"code":-32600,"data":{"kind":"other","reason":"notLoaded"}}),false),
            ("wrongType",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":["notLoaded"]}}),false),
            ("oldDispatcher",json!({"code":-32600,"message":"listenerNotReady notLoaded"}),false),
            ("missingReason",json!({"code":-32600,"data":{"kind":"threadSubscriptionUnavailable"}}),false),
        ] {
            let(control,mut rpc,listener)=fixture();let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);let request=read_request(&mut socket);assert_eq!(request["method"],PASSIVE_SUBSCRIBE);socket.send(Message::Text(json!({"id":request["id"],"error":error}).to_string().into())).unwrap();assert_closed_without_input(&mut socket);
            });
            let result=control.observer_subscribe(&mut rpc,"original",Path::new("."));assert_eq!(result.as_ref().ok(),if retryable{Some(&false)}else{None},"{case}");assert!(!control.0.events.lock().unwrap().observer_connected);assert!(!control.0.unknown.load(Ordering::Acquire));if case=="oldDispatcher"{assert!(result.unwrap_err().contains("호환되는 서버"));}drop(rpc);server.join().unwrap();
        }
    }
    #[test]
    fn observer_preparation_retries_are_spaced_and_bounded_and_terminal_errors_stop() {
        for outcome in ["success","exhausted","terminal"] {
            let(control,mut rpc,listener)=fixture();let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(4))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);let mut subscriptions=Vec::new();
                let attempts=if outcome=="terminal"{2}else{3};
                for attempt in 0..attempts {
                    let read=read_request(&mut socket);assert_eq!(read["method"],"thread/read");reply(&mut socket,&read,json!({"thread":{"id":"original","cwd":expected_cwd,"status":{"type":"active"}}}));
                    let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);assert_eq!(subscribe["params"],json!({"threadId":"original"}));subscriptions.push(Instant::now());
                    if attempt==2&&outcome=="success" {reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[],"status":{"type":"idle"}}}));}
                    else {let reason=if outcome=="terminal"&&attempt==1 {"closing"}else if attempt==0{"notLoaded"}else{"listenerNotReady"};socket.send(Message::Text(json!({"id":subscribe["id"],"error":{"code":-32600,"data":{"kind":"threadSubscriptionUnavailable","reason":reason}}}).to_string().into())).unwrap();}
                }
                let read=read_request(&mut socket);assert_eq!(read["method"],"thread/read");reply(&mut socket,&read,json!({"thread":{"id":"original","cwd":expected_cwd,"status":{"type":"idle"}}}));assert_closed_without_input(&mut socket);
                for pair in subscriptions.windows(2){assert!(pair[1].duration_since(pair[0])>=Duration::from_millis(900),"준비 재시도는 즉시 반복하지 않고 다음 1초 조회 뒤에 해야 한다");}subscriptions.len()
            });
            let stopped=AtomicBool::new(false);let sink=|event|if let CodexEvent::Status(state)=event {if state=="idle"{assert_eq!(control.0.events.lock().unwrap().observer_connected,outcome=="success");stopped.store(true,Ordering::Release);}};
            observe(&mut rpc,Some("original".into()),&cwd,&stopped,&sink,Some(&control));assert!(!control.0.events.lock().unwrap().observer_connected);drop(rpc);assert_eq!(server.join().unwrap(),if outcome=="terminal"{2}else{3});
        }
    }
    #[test]
    fn handed_off_pending_is_rebound_only_by_identical_replay_and_failed_reentry_preserves_it() {
        for outcome in ["same","resolved","missing","changed","startFailure","deleted","responded","stopped"] {
            let(control,mut rpc,listener)=fixture();install_finished_worker(&control);
            let pending=json!({"id":19,"method":"item/commandExecution/requestApproval","params":{"threadId":"original","availableDecisions":[],"reason":"인계 전 원문"}});
            observe_message(&control.0,"original",pending.clone()).unwrap();control.observer_message("original",pending.clone()).unwrap();control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal).unwrap();
            assert!(!control.0.events.lock().unwrap().blocks_attach(false,false));let cwd=env::current_dir().unwrap().canonicalize().unwrap();let expected_cwd=cwd.clone();let expected=pending.clone();
            let(response_sent,response_ready)=mpsc::sync_channel(1);let(replay_release,replay_ready)=mpsc::sync_channel(1);
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);let init=read_request(&mut socket);assert_eq!(init["method"],"initialize");
                if outcome=="startFailure"{socket.send(Message::Text(json!({"id":init["id"],"error":{"code":-32603,"message":"합성 초기화 실패"}}).to_string().into())).unwrap();assert_closed_without_input(&mut socket);return;}
                reply(&mut socket,&init,supported_initialize());assert_eq!(read_request(&mut socket)["method"],"initialized");let subscribe=read_request(&mut socket);assert_eq!(subscribe["method"],PASSIVE_SUBSCRIBE);
                reply(&mut socket,&subscribe,json!({"thread":{"id":"original","cwd":expected_cwd,"turns":[]}}));response_sent.send(()).unwrap();replay_ready.recv_timeout(Duration::from_secs(3)).unwrap();
                match outcome {
                    "same"|"changed"=>{let mut replay=expected;if outcome=="changed"{replay["params"]["reason"]=json!("다른 본문");}socket.send(Message::Text(replay.to_string().into())).unwrap();},
                    "resolved"=>{
                        socket.send(Message::Text(json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":19}}).to_string().into())).unwrap();
                        // 실제 CLI의 복사 후 소비 경합: 해결 알림보다 오래된 replay가 늦게 온다.
                        socket.send(Message::Text(expected.to_string().into())).unwrap();
                    },
                    "deleted"=>{socket.send(Message::Text(json!({"method":"thread/deleted","params":{"threadId":"original"}}).to_string().into())).unwrap();},
                    _=>{},
                }
                if matches!(outcome,"same"|"resolved"){let history=read_request(&mut socket);assert_eq!(history["method"],"thread/turns/list");reply(&mut socket,&history,json!({"data":[]}));}assert_closed_without_input(&mut socket);
            });
            let(finished,result)=mpsc::sync_channel(1);let(start,started)=mpsc::sync_channel(1);install_actor(&control,move|state,receiver,cancelled|{started.recv_timeout(Duration::from_secs(3)).unwrap();let value=serve(&state,&mut rpc,"original",&cwd,receiver,&cancelled,&Mutex::new(Some(Instant::now()+Duration::from_secs(1))));finished.send(value).unwrap();});
            let(reply,attached)=mpsc::sync_channel(1);control.0.worker.lock().unwrap().as_ref().unwrap().sender.try_send(Work{request:ConversationRequest::Attach,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();
            start.send(()).unwrap();
            if outcome!="startFailure" {
                response_ready.recv_timeout(Duration::from_secs(3)).unwrap();assert!(matches!(attached.try_recv(),Err(mpsc::TryRecvError::Empty)));assert!(control.terminal_write(||Ok(1)).is_err());assert_eq!(control.pending_snapshot().unwrap().2[0]["guiOwned"],false);
                if outcome=="responded" {let mut events=control.0.events.lock().unwrap();events.pending.get_mut("19").unwrap()["responded"]=json!(true);events.answered.insert("19".into());}
                if outcome=="stopped" {control.0.stopped.store(true,Ordering::Release);}
                replay_release.send(()).unwrap();
            }
            if matches!(outcome,"same"|"resolved") {
                attached.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
                if outcome=="same" {assert_eq!(control.pending_snapshot().unwrap().2[0]["guiOwned"],true);assert!(control.request("original",Path::new("."),ConversationRequest::Detach).is_err());control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal).unwrap();}
                else {assert!(control.pending_snapshot().unwrap().2.is_empty());control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();}
                assert!(result.recv_timeout(Duration::from_secs(3)).unwrap().is_ok());
            }
            else {
                let error=result.recv_timeout(Duration::from_secs(3)).unwrap().unwrap_err();if outcome=="missing" {assert!(error.contains("기한"),"정상 기한 만료 대신 다른 실패가 발생했다: {error}");}assert!(attached.recv_timeout(Duration::from_secs(3)).is_err());assert!(control.terminal_write(||Ok(1)).is_err());
                let deadline=Instant::now()+Duration::from_secs(3);while !control.0.worker.lock().unwrap().as_ref().unwrap().join.as_ref().unwrap().is_finished(){assert!(Instant::now()<deadline);thread::yield_now();}
                let handoff=control.request("original",Path::new("."),ConversationRequest::HandoffToTerminal);assert_eq!(handoff.is_err(),outcome=="responded");
            }
            let snapshot=control.pending_snapshot().unwrap().2;assert_eq!(snapshot.len(),if matches!(outcome,"resolved"|"deleted"){0}else{1});
            if let Some(request)=snapshot.first() {assert_eq!(request["params"],pending["params"]);assert_eq!(request["guiOwned"],false);assert_eq!(request["responded"]==true,outcome=="responded");}
            assert_eq!(control.is_deleted(),outcome=="deleted");server.join().unwrap();control.cancel();
        }
        for unsafe_copy in ["owned","unknown","responded","answered","owner","unclassified","notHandedOff"] {
            let(control,_rpc,_listener)=fixture();let request=json!({"id":9,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}});observe_message(&control.0,"original",request.clone()).unwrap();control.observer_message("original",request).unwrap();
            {let mut events=control.0.events.lock().unwrap();events.pending.get_mut("9").unwrap()["guiOwned"]=json!(false);match unsafe_copy {"responded"=>events.observed_pending.get_mut("9").unwrap()["responded"]=json!(true),"answered"=>{events.answered.insert("9".into());},"owner"=>events.observed_pending.get_mut("9").unwrap()["method"]=json!("mcpServer/elicitation/request"),"unclassified"=>events.pending.get_mut("9").unwrap()["method"]=json!("future/request"),"notHandedOff"=>events.pending.get_mut("9").unwrap()["guiOwned"]=json!(true),_=>{}}}
            control.0.owned.store(unsafe_copy=="owned",Ordering::Release);control.0.unknown.store(unsafe_copy=="unknown",Ordering::Release);assert!(control.request("original",Path::new("."),ConversationRequest::Attach).is_err(),"{unsafe_copy}에서 재진입을 허용하면 안 된다");assert!(control.0.worker.lock().unwrap().is_none());assert_eq!(control.pending_snapshot().unwrap().2.len(),1);
        }
    }
    #[test]
    fn failed_attach_releases_input_only_after_a_finished_worker_joins_with_no_pending() {
        let (control,_rpc,_listener)=fixture();install_finished_worker(&control);
        control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();
        assert!(!control.0.owned.load(Ordering::Acquire));assert!(control.0.worker.lock().unwrap().is_none());assert_eq!(control.terminal_write(||Ok(1)).unwrap(),1);
        for source in ["gui","observer","unknown"] {
            let (control,_rpc,_listener)=fixture();install_finished_worker(&control);
            let request=json!({"id":9,"method":"item/commandExecution/requestApproval","params":{"threadId":"original"}});
            match source {"gui"=>observe_message(&control.0,"original",request).unwrap(),"observer"=>control.observer_message("original",request).unwrap(),_=>control.0.unknown.store(true,Ordering::Release)}
            assert!(control.request("original",Path::new("."),ConversationRequest::Detach).is_err());assert!(control.0.owned.load(Ordering::Acquire));assert!(control.terminal_write(||Ok(1)).is_err());
            control.observer_message("original",json!({"method":"serverRequest/resolved","params":{"threadId":"original","requestId":9}})).unwrap();
            if source!="unknown" {control.request("original",Path::new("."),ConversationRequest::Detach).unwrap();assert!(!control.0.owned.load(Ordering::Acquire));}
            control.cancel();
        }
    }
    #[test]
    fn disconnected_detach_joins_before_rechecking_late_pending_or_unknown_state() {
        for state in ["clean","guiPending","observerPending","unknown"] {
            let(control,_rpc,_listener)=fixture();let(ready,started)=mpsc::sync_channel(1);
            install_actor(&control,move|control,receiver,cancelled|{
                drop(receiver);ready.send(()).unwrap();
                while !cancelled.load(Ordering::Acquire) {thread::sleep(Duration::from_millis(1));}
                let request=json!({"id":91,"method":"item/tool/requestUserInput","params":{"threadId":"original"}});
                match state {
                    "guiPending"=>observe_message(&control,"original",request).unwrap(),
                    "observerPending"=>record_message(&control,"original",request,true).unwrap(),
                    "unknown"=>control.unknown.store(true,Ordering::Release),
                    _=>{},
                }
            });
            started.recv_timeout(Duration::from_secs(3)).unwrap();
            let result=control.request("original",Path::new("."),ConversationRequest::Detach);
            assert_eq!(result.is_ok(),state=="clean","{state}");
            assert_eq!(control.terminal_write(||Ok(1)).is_ok(),state=="clean","{state}");
            assert_eq!(control.0.owned.load(Ordering::Acquire),state!="clean");
            if state!="clean" {assert!(control.0.worker.lock().unwrap().as_ref().unwrap().joined);}
            control.cancel();
        }
    }
    #[test]
    fn a_full_detach_queue_does_not_cancel_or_release_a_running_worker() {
        let(control,_rpc,_listener)=fixture();
        install_actor(&control,move|_control,receiver,cancelled|{
            while !cancelled.load(Ordering::Acquire) {thread::sleep(Duration::from_millis(1));}
            drop(receiver);
        });
        {
            let worker=control.0.worker.lock().unwrap();let worker=worker.as_ref().unwrap();
            for _ in 0..8 {
                let(reply,_response)=mpsc::sync_channel(1);
                worker.sender.try_send(Work{request:ConversationRequest::Models{cursor:None},reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();
            }
        }
        assert!(control.request("original",Path::new("."),ConversationRequest::Detach).is_err());
        assert!(control.0.owned.load(Ordering::Acquire));assert!(control.terminal_write(||Ok(1)).is_err());
        {let worker=control.0.worker.lock().unwrap();let worker=worker.as_ref().unwrap();assert!(!worker.cancelled.load(Ordering::Acquire));assert!(!worker.joined);assert!(worker.closing.is_none());}
        control.cancel();
    }
    #[test]
    fn a_delayed_worker_is_not_treated_as_a_failed_attach() {
        let (control,_rpc,_listener)=fixture();let (sender,receiver)=mpsc::sync_channel(1);drop(receiver);
        let cancelled=Arc::new(AtomicBool::new(false));let running=cancelled.clone();
        let join=thread::spawn(move||while !running.load(Ordering::Acquire){thread::sleep(Duration::from_millis(1));});
        *control.0.worker.lock().unwrap()=Some(Worker{sender,cancelled,transport:Arc::new(Mutex::new(Transport::default())),join:Some(join),joined:false,closing:None});
        *control.0.input.lock().unwrap()=true;control.0.owned.store(true,Ordering::Release);
        assert!(!control.release(true,false).unwrap());assert!(control.0.owned.load(Ordering::Acquire));assert!(control.terminal_write(||Ok(1)).is_err());
        control.cancel();assert!(control.0.worker.lock().unwrap().is_none());
    }
    #[cfg(windows)]
    fn staged_send_fixture() -> (SendContext,Attachment,PathBuf) {
        let root=std::env::temp_dir().join(format!("opendiy-send-retention-test-{}",Uuid::new_v4()));
        let store=crate::conversation_attachments::ConversationAttachments::test_store(root.clone());
        let session_id=Uuid::new_v4().to_string();
        let staged=store.stage_image(&session_id,"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=").unwrap();
        (SendContext{session_id,store},Attachment{r#type:"localImage".into(),path:staged.path},root)
    }
    #[cfg(windows)]
    fn remove_staged_send_fixture(context: SendContext,attachment: Attachment,root: PathBuf) {
        context.store.cleanup_all().unwrap();let path=PathBuf::from(&attachment.path);let directory=path.parent().unwrap().to_owned();
        assert!(path.starts_with(&root)&&directory.starts_with(&root));drop(context);
        if path.exists(){std::fs::remove_file(path).unwrap();}if directory.exists(){std::fs::remove_dir(directory).unwrap();}
        std::fs::remove_file(root.join(".staging.lock")).unwrap();std::fs::remove_dir(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn known_pre_send_rejections_leave_staged_images_removable() {
        for reason in ["noWorker","fullQueue","unknown","invalidInput","expired","cancelled"] {
            let (context,attachment,root)=staged_send_fixture();let (control,mut rpc,listener)=fixture();
            let request=ConversationRequest::Send{request_id:Uuid::new_v4().to_string(),text:"hello".into(),attachments:vec![Attachment{r#type:if reason=="invalidInput" {"unsupported".into()} else {"localImage".into()},path:attachment.path.clone()}],model:None,effort:None};
            let result=match reason {
                "noWorker"=>control.request_for_session("original",Path::new("."),request,&context.session_id,context.store.clone()),
                "fullQueue"=>{
                    let (sender,receiver)=mpsc::sync_channel(1);let (reply,_reply_rx)=mpsc::sync_channel(1);
                    sender.try_send(Work{request:ConversationRequest::Poll,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}).ok().unwrap();
                    let cancelled=Arc::new(AtomicBool::new(false));let running=cancelled.clone();
                    let join=thread::spawn(move||{while !running.load(Ordering::Acquire){thread::sleep(Duration::from_millis(1));}drop(receiver);});
                    *control.0.worker.lock().unwrap()=Some(Worker{sender,cancelled,transport:Arc::new(Mutex::new(Transport::default())),join:Some(join),joined:false,closing:None});
                    control.request_for_session("original",Path::new("."),request,&context.session_id,context.store.clone())
                },
                _=>{
                    if reason=="unknown" {control.0.unknown.store(true,Ordering::Release);}
                    if reason=="expired" {rpc.deadline=Some(Instant::now()-Duration::from_millis(1));}
                    if reason=="cancelled" {rpc.cancelled=Some(Arc::new(AtomicBool::new(true)));}
                    execute_context(&control.0,&mut rpc,"original",request,&AtomicBool::new(false),Some(&context))
                },
            };
            assert!(result.is_err(),"{reason}이 송신 경계에 도달하면 안 된다");
            assert_eq!(control.0.unknown.load(Ordering::Acquire),reason=="unknown","{reason}에서 기존 불명 상태를 훼손하거나 미전송을 불명으로 바꾸면 안 된다");
            context.store.cleanup_staged(&context.session_id,&attachment.path).unwrap();assert!(!Path::new(&attachment.path).exists(),"{reason}에서 미전송 이미지를 보존 처리하면 안 된다");
            control.cancel();drop(rpc);remove_staged_send_fixture(context,attachment,root);
        }
    }
    #[cfg(windows)]
    #[test]
    fn atomic_rejections_rollback_only_this_send_and_unknown_errors_block_next_send() {
        for outcome in ["skippedBusy","notStarted","unsupported","invalidParams","internal","invalidRequest","lost","started"] {
            for previously_retained in [false,true] {
                let(context,attachment,root)=staged_send_fixture();let(control,mut rpc,listener)=fixture();
                if previously_retained {drop(context.store.retain_for_send(&context.session_id,&[attachment.path.clone()]).unwrap());}
                let known_rejected=matches!(outcome,"skippedBusy" | "notStarted" | "unsupported" | "invalidParams");
                let unknown=matches!(outcome,"internal" | "invalidRequest" | "lost");
                let server=thread::spawn(move||{
                    let(stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                    let first=read_request(&mut socket);assert_eq!(first["method"],START_USER_IF_IDLE);
                    if outcome=="lost" {return 1;}
                    match outcome {
                        "unsupported" | "invalidParams" | "internal" | "invalidRequest" => {
                            let code=match outcome {"unsupported"=>-32601,"invalidParams"=>-32602,"invalidRequest"=>-32600,_=>-32603};
                            socket.send(Message::Text(json!({"id":first["id"],"error":{"code":code,"message":"authoritative or uncertain error"}}).to_string().into())).unwrap();
                        },
                        _=>reply(&mut socket,&first,json!({"status":outcome,"turn":{"id":"turn"},"reason":"admission rejected"})),
                    }
                    if unknown {assert!(socket.read().is_err(),"결과 불명 뒤 후속 입력을 보내면 안 된다");1}
                    else {let next=read_request(&mut socket);assert_eq!(next["method"],START_USER_IF_IDLE);assert_ne!(next["params"]["clientUserMessageId"],first["params"]["clientUserMessageId"]);reply(&mut socket,&next,json!({"status":"skippedBusy"}));assert!(socket.read().is_err());2}
                });
                let request=ConversationRequest::Send{request_id:Uuid::new_v4().to_string(),text:String::new(),attachments:vec![Attachment{r#type:"localImage".into(),path:attachment.path.clone()}],model:None,effort:None};
                let first=execute_context(&control.0,&mut rpc,"original",request,&AtomicBool::new(false),Some(&context));
                assert_eq!(first.is_ok(),outcome=="started");assert_eq!(control.0.unknown.load(Ordering::Acquire),unknown);
                assert_eq!(context.store.cleanup_staged(&context.session_id,&attachment.path).is_ok(),known_rejected&&!previously_retained,"{outcome}, 이전 보존={previously_retained}");
                let next=execute_context(&control.0,&mut rpc,"original",ConversationRequest::Send{request_id:Uuid::new_v4().to_string(),text:"새로운 사용자 입력".into(),attachments:vec![],model:None,effort:None},&AtomicBool::new(false),Some(&context)).unwrap_err();
                assert_eq!(next.contains("이전 전송 결과"),unknown);assert_eq!(control.0.unknown.load(Ordering::Acquire),unknown);
                drop(rpc);assert_eq!(server.join().unwrap(),if unknown {1} else {2});remove_staged_send_fixture(context,attachment,root);
            }
        }
    }
    #[cfg(windows)]
    #[test]
    fn actual_send_attempts_retain_images_on_acknowledgement_and_lost_reply() {
        for acknowledge in [true,false] {
            let (context,attachment,root)=staged_send_fixture();let (control,mut rpc,listener)=fixture();
            let expected_path=attachment.path.clone();let store=context.store.clone();let session_id=context.session_id.clone();
            let server=thread::spawn(move||{
                let(stream,_)=listener.accept().unwrap();let mut socket=WebSocket::from_raw_socket(stream,tungstenite::protocol::Role::Server,None);
                let send=read_request(&mut socket);assert_eq!(send["method"],START_USER_IF_IDLE);assert_eq!(send["params"]["input"][0]["path"],expected_path);
                assert!(store.cleanup_staged(&session_id,&expected_path).is_err(),"송신 바이트 관측 시에는 원문이 보존되어야 한다");
                if acknowledge {reply(&mut socket,&send,json!({"status":"started","turn":{"id":"original-turn"}}));}1
            });
            let request=ConversationRequest::Send{request_id:Uuid::new_v4().to_string(),text:String::new(),attachments:vec![Attachment{r#type:"localImage".into(),path:attachment.path.clone()}],model:None,effort:None};
            let result=execute_context(&control.0,&mut rpc,"original",request,&AtomicBool::new(false),Some(&context));
            assert_eq!(result.is_ok(),acknowledge);assert_eq!(control.0.unknown.load(Ordering::Acquire),!acknowledge);assert_eq!(server.join().unwrap(),1);
            assert!(context.store.cleanup_staged(&context.session_id,&attachment.path).is_err());context.store.cleanup_session(&context.session_id).unwrap();assert!(Path::new(&attachment.path).is_file());
            drop(rpc);remove_staged_send_fixture(context,attachment,root);
        }
    }
#[test]
fn runtime_extensions_ipc_binds_the_reviewed_thread_and_rejects_extra_fields() {
    let valid = json!({"type":"reloadMcp","expectedThreadId":"original"});
    assert!(serde_json::from_value::<ConversationRequest>(valid.clone()).is_ok());
    for extra in ["method", "threadId", "cwd"] {
        let mut request = valid.clone(); request[extra] = json!("arbitrary");
        assert!(serde_json::from_value::<ConversationRequest>(request).is_err());
    }
    assert!(serde_json::from_value::<ConversationRequest>(json!({"type":"reloadMcp"})).is_err());
    let status = json!({"type":"mcpStatus","expectedThreadId":"original","serverName":"fixture","cursor":null});
    assert!(serde_json::from_value::<ConversationRequest>(status).is_ok());
    let hooks = json!({"type":"hooksStatus","expectedThreadId":"original"});
    assert!(serde_json::from_value::<ConversationRequest>(hooks.clone()).is_ok());
    for extra in ["method", "threadId", "cwd"] {
        let mut invalid = hooks.clone(); invalid[extra] = json!("arbitrary");
        assert!(serde_json::from_value::<ConversationRequest>(invalid).is_err());
    }
    assert!(serde_json::from_value::<ConversationRequest>(json!({"type":"hooksStatus"})).is_err());
    let control = ConversationConnection::new(Endpoint::Unavailable, Arc::new(AtomicBool::new(false)));
    for request in [ConversationRequest::ReloadMcp { expected_thread_id: "different".into() },
        ConversationRequest::McpStatus { expected_thread_id: "different".into(), server_name: "fixture".into(), cursor: None },
        ConversationRequest::HooksStatus { expected_thread_id: "different".into() }] {
        let error = control.request("original", Path::new("."), request).unwrap_err();
        assert!(error.contains("검토한 대화가 변경"), "reject stale review before creating any runtime connection");
    }
}
