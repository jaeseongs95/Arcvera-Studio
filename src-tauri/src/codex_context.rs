//! Loaded-thread settings only: no resume, GUI lease, config write or retry.
use super::*;
const SETTINGS: &str = "thread/settings/update";

#[derive(Default)]
pub(super) struct State { thread: Option<String>, pending: Option<Option<i64>> }
impl State {
    pub(super) fn bind_thread(&mut self, thread_id: &str) -> Result<(), String> {
        if self.thread.as_deref().is_some_and(|thread| thread != thread_id) {
            return Err("다른 대화의 컨텍스트 설정을 재사용하지 않았습니다.".into());
        }
        self.thread.get_or_insert_with(|| thread_id.to_owned());
        Ok(())
    }
}
pub(super) fn nullable_window<'de,D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<i64>,D::Error> { Option::<i64>::deserialize(deserializer) }
pub(super) fn validate_window(value: Option<i64>) -> Result<(),String> {
    if value.is_some_and(|value| !(1..=MAX_CONTEXT_INTEGER).contains(&value)) {Err("컨텍스트 크기는 정확하게 표현할 수 있는 양의 정수여야 합니다.".into())} else {Ok(())}
}
#[derive(Clone,PartialEq,Debug)]
struct Settings { requested: Option<i64>, effective: Option<i64> }
fn optional_window(value: &Value) -> Option<Option<i64>> {
    if value.is_null() {Some(None)} else {value.as_i64().filter(|value|(1..=MAX_CONTEXT_INTEGER).contains(value)).map(Some)}
}
fn settings(value: &Value) -> Option<Settings> {
    if value["settingsApplied"].as_bool()!=Some(true) {return None;}
    let fields=value.get("threadSettings")?.as_object()?;
    Some(Settings {requested:optional_window(fields.get("modelContextWindow")?)?,effective:optional_window(fields.get("effectiveContextWindow")?)?})
}
fn view(state: &State, value: &Settings) -> Value {
    json!({"supported":true,"canChange":state.pending.is_none(),"modelContextWindow":value.requested,"effectiveContextWindow":value.effective,
        "reason":if state.pending.is_some(){Some("unknownResult")}else{None},"appliesTo":"nextTurn"})
}
impl ConversationConnection {
    pub(super) fn context_settings_request(&self,thread_id: &str,request: ConversationRequest) -> Result<Value,String> {
        valid_id(thread_id)?; validate_request(&request)?;
        if let ConversationRequest::ContextComposition { expected_thread_id, .. } = &request {
            if expected_thread_id != thread_id { return Err("대화가 변경되어 컨텍스트 구성을 조회하지 않았습니다.".into()); }
        }
        // Bounded concurrency: a second request never waits behind network I/O.
        // This lock belongs to this session connection, not another session.
        let mut state=self.0.context_settings.try_lock().map_err(|_|"이 세션의 컨텍스트 설정 요청이 이미 진행 중입니다.")?;
        state.bind_thread(thread_id)?;
        live(&self.0,&AtomicBool::new(false))?;
        let deadline=Instant::now()+RPC_TIMEOUT;
        let cancelled=Arc::new(AtomicBool::new(false)); let transport=Arc::new(Mutex::new(Transport::default()));
        let _watch=DeadlineWatch::start(Arc::new(Mutex::new(Some(deadline))),transport.clone(),cancelled.clone(),self.0.stopped.clone())?;
        let permissions=matches!(request,ConversationRequest::PermissionStatus);
        let result=match &self.0.endpoint {
            Endpoint::Shared(endpoint)=>{
                let mut proxy=endpoint.open_until(Some(cancelled.clone()),Some(deadline))?;
                transport.lock().map_err(|_|"컨텍스트 연결 정리 상태 오류")?.proxy=proxy.job.as_ref().map(Arc::downgrade);
                let result=exchange(&self.0,&mut state,proxy.rpc.as_mut().ok_or("컨텍스트 연결을 찾을 수 없습니다.")?,thread_id,request,&cancelled,deadline);
                match (result,proxy.finish()) {(Ok(value),Ok(()))=>Ok(value),(Err(error),Ok(()))=>Err(error),(result,Err(error))=>Err(format!("{}; 컨텍스트 연결 정리 실패: {error}",result.err().unwrap_or_default()))}
            },
            #[cfg(test)]
            Endpoint::Unavailable => Err("Test fixture has no runtime endpoint".into()),
        };
        if !permissions {return result;}
        let mut value=result?;
        let (_,_,requests,complete)=self.question_snapshot()?;
        value["pendingApprovals"]=approval_metadata(requests,complete);
        Ok(value)
    }
}
fn exchange<S: Read+Write>(control: &Control,state: &mut State,rpc: &mut Rpc<S>,thread_id: &str,request: ConversationRequest,cancelled: &Arc<AtomicBool>,deadline: Instant) -> Result<Value,String> {
    rpc.cancelled=Some(cancelled.clone());rpc.deadline=Some(deadline);live(control,cancelled)?;
    let mut init=initialize();init["capabilities"]=json!({"experimentalApi":true});
    rpc.call("initialize",init)?;rpc.send(json!({"method":"initialized","params":{}}))?;
    // Omission is the actual R36 read contract. Official CLI returns {}, which
    // is deliberately insufficient proof that modelContextWindow is supported.
    if matches!(request,ConversationRequest::PermissionStatus) {
        let value=read_permissions(control,rpc,thread_id,cancelled,deadline)?;
        live(control,cancelled)?;
        return Ok(value);
    }
    if let ConversationRequest::ContextComposition { snapshot_id, cursor, .. } = request {
        let value = composition::exchange(rpc, thread_id, snapshot_id.as_deref(), cursor.unwrap_or(0))?;
        live(control, cancelled)?;
        return Ok(value);
    }
    let value=rpc.call(SETTINGS,json!({"threadId":thread_id}))?;
    live(control,cancelled)?;
    let Some(current)=settings(&value) else {return Ok(json!({"supported":false}));};
    if state.pending.as_ref()==Some(&current.requested) {state.pending=None;}
    let ConversationRequest::SetContextWindow {model_context_window,expected_model_context_window}=request else {return Ok(view(state,&current));};
    if state.pending.is_some() {return Ok(json!({"supported":false,"status":"unknown","reason":"unknownResult"}));}
    // Local stale-UI guard, not a server CAS. Other clients may still change
    // settings after this read; the authoritative acknowledgement is displayed.
    if current.requested!=expected_model_context_window {let mut value=view(state,&current);value["status"]=json!("conflict");return Ok(value);}
    live(control,cancelled)?;check_cancelled(rpc.cancelled.as_ref())?;
    if Instant::now()>=deadline {return Err("컨텍스트 변경 기한이 지나 전송하지 않았습니다.".into());}
    let Some(applied)=write(control,state,rpc,thread_id,model_context_window,cancelled,deadline)? else {
        let mut value=view(state,&current);value["status"]=json!("rejected");value["reason"]=json!("serverRejected");return Ok(value);
    };
    live(control,cancelled)?;
    let Some(applied)=settings(&applied) else {return Ok(json!({"supported":false,"status":"unknown","reason":"unconfirmedAck"}));};
    if applied.requested!=model_context_window {return Ok(json!({"supported":false,"status":"unknown","reason":"mismatchedAck"}));}
    state.pending=None;
    let mut value=view(state,&applied);value["status"]=json!("applied");Ok(value)
}

fn approval_metadata(requests: Vec<Value>, complete: bool) -> Value {
    let complete=complete && requests.iter().all(|request|match request["method"].as_str() {
        Some("item/commandExecution/requestApproval"|"item/fileChange/requestApproval"|"item/permissions/requestApproval")=>request["metadataAvailable"]==true,
        Some("item/tool/requestUserInput"|"opendiy/asyncQuestion")=>true,
        _=>false,
    });
    let requests=requests.into_iter().filter_map(|request| {
        let method=request["method"].as_str()?;
        if !matches!(method,"item/commandExecution/requestApproval"|"item/fileChange/requestApproval"|"item/permissions/requestApproval") {return None;}
        Some(json!({"requestId":request["requestId"],"method":method,"metadataAvailable":request["metadataAvailable"]==true}))
    }).collect::<Vec<_>>();
    json!({"status":if complete {"known"}else{"unknown"},"requests":requests})
}

fn read_permissions<S: Read+Write>(control: &Control,rpc: &mut Rpc<S>,thread_id: &str,cancelled: &AtomicBool,deadline: Instant) -> Result<Value,String> {
    live(control,cancelled)?;
    if Instant::now()>=deadline {return Err("권한 조회 기한이 지나 전송하지 않았습니다.".into());}
    rpc.next_id+=1;let id=rpc.next_id;
    rpc.send(json!({"id":id,"method":SETTINGS,"params":{"threadId":thread_id}}))?;
    while Instant::now()<deadline {
        live(control,cancelled)?;
        if let Some(message)=rpc.read()? {if let Some(result)=response(&message,id) {
            if message["error"]["code"].as_i64()==Some(-32601) {return Ok(json!({"supported":false,"status":"unsupported","reason":"methodUnsupported"}));}
            return result.map(|value|permission_view(&value));
        }}
    }
    Err("권한 조회 응답 기한이 지났습니다. 자동으로 다시 요청하지 않았습니다.".into())
}

// Return only permission fields; neither the complete settings nor request bodies
// cross this boundary. Unknown structures never turn into permissive defaults.
fn permission_view(value: &Value) -> Value {
    if value["settingsApplied"]!=true {return json!({"supported":false,"status":"unsupported","reason":"noAuthoritativeSettings"});}
    match permission_fields(&value["threadSettings"]) {
        Some(fields)=>json!({"supported":true,"status":"known","permissions":fields}),
        None=>json!({"supported":true,"status":"unknown","reason":"invalidPermissionFields"}),
    }
}
fn permission_fields(value: &Value) -> Option<Value> {
    let policy=&value["approvalPolicy"];
    let approval=if policy.as_str().is_some_and(|v|matches!(v,"untrusted"|"on-request"|"never")) {policy.clone()} else {
        let fields=policy.get("granular")?.as_object()?;
        let mut output=serde_json::Map::new();
        for key in ["sandbox_approval","rules","mcp_elicitations"] {output.insert(key.into(),json!(fields.get(key)?.as_bool()?));}
        for key in ["skill_approval","request_permissions"] {if let Some(value)=fields.get(key) {output.insert(key.into(),json!(value.as_bool()?));}}
        json!({"granular":output})
    };
    let reviewer=value["approvalsReviewer"].as_str()?;
    if !matches!(reviewer,"user"|"auto_review"|"guardian_subagent") {return None;}
    let source=&value["sandboxPolicy"];
    let kind=source["type"].as_str()?;
    let mut sandbox=json!({"type":kind});
    match kind {
        "dangerFullAccess"=>{},
        "readOnly"=>sandbox["networkAccess"]=json!(source["networkAccess"].as_bool()?),
        "externalSandbox"=>{
            let access=source["networkAccess"].as_str()?;
            if !matches!(access,"restricted"|"enabled") {return None;}
            sandbox["networkAccess"]=json!(access);
        },
        "workspaceWrite"=>{
            for key in ["networkAccess","excludeTmpdirEnvVar","excludeSlashTmp"] {sandbox[key]=json!(source[key].as_bool()?);}
            let roots=source["writableRoots"].as_array()?;
            if roots.len()>128 || roots.iter().any(|root|root.as_str().is_none_or(|v|v.len()>4096)) {return None;}
            sandbox["writableRoots"]=json!(roots);
        },
        _=>return None,
    }
    let profile=value.get("activePermissionProfile")?;
    let profile=if profile.is_null() {Value::Null} else {
        let id=profile["id"].as_str()?;
        if id.is_empty() || id.len()>256 {return None;}
        let parent=match profile.get("extends") {None|Some(Value::Null)=>None,Some(value)=>Some(value.as_str()?)};
        if parent.is_some_and(|v|v.len()>256) {return None;}
        json!({"id":id,"extends":parent})
    };
    Some(json!({"approvalPolicy":approval,"approvalsReviewer":reviewer,"sandboxPolicy":sandbox,"activePermissionProfile":profile}))
}

fn write<S: Read+Write>(control: &Control,state: &mut State,rpc: &mut Rpc<S>,thread_id: &str,window: Option<i64>,cancelled: &AtomicBool,deadline: Instant) -> Result<Option<Value>,String> {
    live(control,cancelled)?;check_cancelled(rpc.cancelled.as_ref())?;
    if Instant::now()>=deadline {return Err("컨텍스트 변경 기한이 지나 전송하지 않았습니다.".into());}
    rpc.next_id+=1;let id=rpc.next_id;
    let message=Message::Text(json!({"id":id,"method":SETTINGS,"params":{"threadId":thread_id,"modelContextWindow":window}}).to_string().into());
    // Only an actual send attempt creates an unresolved mutation. Partial
    // writes and lost replies remain unknown; exact protocol rejections do not.
    state.pending=Some(window);
    rpc.ws.send(message).map_err(|error|format!("컨텍스트 변경 송신 결과를 확인할 수 없습니다: {error}"))?;
    while Instant::now()<deadline {
        live(control,cancelled)?;
        if let Some(message)=rpc.read()? {if let Some(result)=response(&message,id) {
            // R36 -32600 invalid thread settings overrides are rejected by
            // preview before submit_core_op; failures after submission use
            // -32603 and must retain the unresolved mutation.
            if matches!(message["error"]["code"].as_i64(),Some(-32600 | -32601 | -32602)) {state.pending=None;return Ok(None);}
            return result.map(Some);
        }}
    }
    Err("컨텍스트 변경 응답 기한이 지나 결과 불명을 보존했습니다. 자동으로 다시 요청하지 않습니다.".into())
}

#[cfg(test)]
#[path = "codex_conversation_tests.rs"]
mod tests;
