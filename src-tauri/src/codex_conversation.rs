//! 기존 서버·대화의 구조화 연결. 입력과 응답은 한 작업자가 소유하며 재전송하지 않는다.
use super::*;

#[path = "codex_subagents.rs"]
mod subagents;
#[path = "codex_async_questions.rs"]
mod async_questions;
#[path = "codex_context.rs"]
mod context;
#[path = "codex_context_composition.rs"]
mod composition;
pub use subagents::SubagentRequest;
use std::{collections::{HashMap, HashSet, VecDeque}, path::PathBuf, sync::Weak};

const EVENT_COUNT: usize = 128;
const EVENT_BYTES: usize = 1024 * 1024;
const REQUEST_COUNT: usize = 32;
const TEXT_PREVIEW: usize = 4096;
const DETAIL_BYTES: usize = 16 * 1024;
// Full arguments must fit the existing conversation preview scale to be reviewed.
const MCP_APPROVAL_REVIEW_BYTES: usize = 16 * 1024;
const MCP_APPROVAL_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_CONTEXT_INTEGER: i64 = 9_007_199_254_740_991;
const PASSIVE_SUBSCRIBE: &str = "thread/subscribePassive";
const START_USER_IF_IDLE: &str = "turn/startUserIfIdle";
const COMPACT_IF_IDLE: &str = "thread/compact/startIfIdle";
const HISTORY_UNAVAILABLE: &str = "대화 이력 원문을 아직 조회할 수 없습니다. 빈 이력으로 처리하지 않았습니다.";
const PASSIVE_UNSUPPORTED: &str = "이 서버는 안전한 대화 화면 연결을 지원하지 않습니다. 터미널에서 계속 사용할 수 있습니다.";

fn compact_capability(initialized: &Value) -> bool {
    initialized["opendiyCapabilities"]["compactStartIfIdle"].as_bool() == Some(true)
}

fn passive_capability(initialized: &Value) -> bool {
    initialized["opendiyCapabilities"]["passiveSubscription"].as_bool() == Some(true)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ConversationRequest {
    Attach,
    Detach,
    HandoffToTerminal,
    Poll,
    ContextStatus,
    ContextSettings,
    ContextComposition { expected_thread_id: String, snapshot_id: Option<String>, cursor: Option<u64> },
    PermissionStatus,
    ReloadMcp { expected_thread_id: String },
    McpStatus { expected_thread_id: String, server_name: String, cursor: Option<String> },
    HooksStatus { expected_thread_id: String },
    SetContextWindow { #[serde(deserialize_with="context::nullable_window")] model_context_window: Option<i64>, #[serde(deserialize_with="context::nullable_window")] expected_model_context_window: Option<i64> },
    Compact,
    Page { cursor: Option<String> },
    Items { turn_id: String, cursor: Option<String> },
    Detail { turn_id: String, item_id: String, offset: Option<usize>, snapshot_id: Option<String> },
    Models { cursor: Option<String> },
    Send { request_id: String, text: String, #[serde(default)] attachments: Vec<Attachment>, model: Option<String>, effort: Option<String> },
    Interrupt { turn_id: String },
    Respond { request_id: String, result: Value },
}
impl ConversationRequest {
    pub(crate) fn validate(&self) -> Result<(), String> { validate_request(self) }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attachment { pub r#type: String, pub path: String }

#[derive(Clone)]
pub(super) enum Endpoint {
    Shared(shared::SharedEndpoint),
    #[cfg(test)]
    Unavailable,
}

#[derive(Clone)]
pub struct ConversationConnection(Arc<Control>);
struct Control {
    endpoint: Endpoint,
    stopped: Arc<AtomicBool>,
    // The observer's own initialize response; a GUI connection cannot grant it support.
    passive_supported: bool,
    subagents_supported: bool,
    compact_supported: bool,
    // 동일 잠금 아래 입력 주체를 바꾸고 PTY 쓰기를 시작한다.
    input: Mutex<bool>,
    owned: AtomicBool,
    worker: Mutex<Option<Worker>>,
    events: Mutex<Events>,
    sent: Mutex<HashSet<String>>,
    unknown: AtomicBool,
    deleted: AtomicBool,
    detail: Mutex<Option<DetailSnapshot>>,
    subagents: Mutex<subagents::State>,
    context_settings: Mutex<context::State>,
    observer_compaction: Mutex<ObserverCompaction>,
}
#[derive(Default)]
struct ObserverCompaction { busy: bool, pending: Option<PendingCompact> }
// One bounded observer mutation slot is shared by compaction and async answers.
// Server-request replies retain their original GUI connection ownership.
enum ObserverAction { Compact, AsyncReply { request_id: String, result: Value } }
struct PendingCompact {
    id: Uuid, thread_id: String, deadline: Instant, cancelled: Arc<AtomicBool>,
    reply: mpsc::SyncSender<Result<Value,String>>,
    action: ObserverAction,
}
struct DetailSnapshot { id: String, turn_id: String, item_id: String, text: Arc<str> }
struct Worker {
    sender: mpsc::SyncSender<Work>,
    cancelled: Arc<AtomicBool>,
    transport: Arc<Mutex<Transport>>,
    join: Option<JoinHandle<()>>,
    joined: bool,
    closing: Option<Uuid>,
}
impl Worker {
    fn stop(&mut self) -> Result<(),String> {
        if self.joined {return Ok(());}
        self.cancelled.store(true,Ordering::Release);
        let transport=self.transport.lock().map_err(|_| "대화 연결 정리 상태 오류")?;
        if let Some(socket)=&transport.socket {let _=socket.shutdown(Shutdown::Both);}
        if let Some(job)=transport.proxy.as_ref().and_then(Weak::upgrade) {let _=job.terminate();}
        drop(transport);
        self.join.take().ok_or("대화 작업자의 종료 결과를 다시 확인할 수 없습니다.")?.join().map_err(|_| "대화 작업자가 비정상 종료되었습니다.".to_owned())?;
        self.joined=true;Ok(())
    }
}
#[derive(Default)]
struct Transport { socket: Option<TcpStream>, proxy: Option<Weak<ProcessJob>> }
struct DeadlineWatch { done: mpsc::Sender<()>, join: Option<JoinHandle<()>> }
impl DeadlineWatch {
    fn start(deadline: Arc<Mutex<Option<Instant>>>, transport: Arc<Mutex<Transport>>, cancelled: Arc<AtomicBool>, stopped: Arc<AtomicBool>) -> Result<Self,String> {
        let (done,receiver) = mpsc::channel();
        let join = thread::Builder::new().name("codex-conversation-deadline".into()).spawn(move || {
            loop {
                let expired = deadline.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|value| Instant::now() >= value);
                if expired || cancelled.load(Ordering::Acquire) || stopped.load(Ordering::Acquire) {
                    cancelled.store(true,Ordering::Release);
                    let transport = transport.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(socket) = &transport.socket { let _ = socket.shutdown(Shutdown::Both); }
                    if let Some(job) = transport.proxy.as_ref().and_then(Weak::upgrade) { let _ = job.terminate(); }
                    break;
                }
                match receiver.recv_timeout(Duration::from_millis(100)) {Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,Err(mpsc::RecvTimeoutError::Timeout) => {}}
            }
        }).map_err(|e| format!("대화 연결 기한 감시자를 시작할 수 없습니다: {e}"))?;
        Ok(Self{done,join:Some(join)})
    }
}
impl Drop for DeadlineWatch {
    fn drop(&mut self) { let _ = self.done.send(()); if let Some(join) = self.join.take() { let _ = join.join(); } }
}
struct SendContext { session_id: String, store: crate::conversation_attachments::ConversationAttachments }
struct Work { request: ConversationRequest, reply: mpsc::SyncSender<Result<Value, String>>, deadline: Instant, send_context: Option<SendContext> }
fn finish_worker(control: &Control, result: Result<(), String>, bootstrap_reply: &mpsc::SyncSender<Result<Value, String>>) {
    {
        let mut events = control.events.lock().unwrap_or_else(|e| e.into_inner());
        events.connected = false;events.gui_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;events.context_revision+=1;
        if let Err(error) = &result { events.error = Some(error.clone()); }
    }
    if let Err(error) = result { let _ = bootstrap_reply.try_send(Err(error)); }
}
#[derive(Default)]
struct CompactProgress { turn: Option<(String,bool)> }
impl CompactProgress {
    fn observe(&mut self,thread_id: &str,message: &Value) {
        let params=&message["params"];if params["threadId"].as_str()!=Some(thread_id) {return;}
        let Some(turn)=params["turn"]["id"].as_str().or_else(||params["turnId"].as_str()).filter(|id|valid_id(id).is_ok()) else {return;};
        match message["method"].as_str() {
            Some("turn/started")=>{if self.turn.as_ref().is_some_and(|(id,done)|id==turn&&*done){return;}self.turn=Some((turn.into(),false));},
            Some("turn/completed")=>{if self.turn.as_ref().is_none_or(|(id,_)|id==turn){self.turn=Some((turn.into(),true));}},
            Some("item/started" | "item/completed") if params["item"]["type"]=="contextCompaction"=>{
                if self.turn.as_ref().is_some_and(|(id,done)|id==turn&&*done){return;}
                if params["item"]["id"].as_str().is_some_and(|id|valid_id(id).is_ok()) {self.turn=Some((turn.into(),false));}
            },
            _=>{},
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ContextTokenUsage {
    total: ContextTokenBreakdown,
    last: ContextTokenBreakdown,
    model_context_window: Option<i64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ContextTokenBreakdown {
    total_tokens: i64, input_tokens: i64, cached_input_tokens: i64,
    #[serde(default)] cache_write_input_tokens: i64,
    output_tokens: i64, reasoning_output_tokens: i64,
}
impl ContextTokenBreakdown {
    fn valid(&self) -> bool {
        [self.total_tokens,self.input_tokens,self.cached_input_tokens,self.cache_write_input_tokens,self.output_tokens,self.reasoning_output_tokens].into_iter().all(|value|(0..=MAX_CONTEXT_INTEGER).contains(&value))
    }
}
#[derive(Default)]
struct Events {
    async_questions: async_questions::State,
    queue: VecDeque<(Value, usize)>, bytes: usize, seq: u64, gap: bool,
    pending: HashMap<String, Value>, connected: bool, error: Option<String>,
    answered: HashSet<String>,
    observed_pending: HashMap<String,Value>, observer_connected: bool,
    requests_unavailable: bool,
    token_usage: Option<ContextTokenUsage>, context_active: Option<bool>, token_usage_fresh: bool,
    context_compaction: Option<(String,String)>, context_revision: u64,
    context_activity_revision: u64, context_turn: Option<String>, gui_compact_supported: bool, observer_compact_supported: bool,
    gui_interrupt_supported: bool, interrupt_diagnostic: Option<&'static str>,
}
impl Events {
    fn blocks_attach(&self,owned: bool,unknown: bool) -> bool {
        owned || unknown || self.blocks_release(true) || self.pending.values().any(|request|request["guiOwned"]!=false)
    }
    fn blocks_release(&self,handoff: bool) -> bool {
        self.requests_unavailable || self.async_questions.has_attempt() || self.pending.iter().chain(self.observed_pending.iter()).any(|(id,request)| {
            !handoff || request["responded"]==true || self.answered.contains(id)
                || !matches!(request["method"].as_str(),Some("item/commandExecution/requestApproval" | "item/fileChange/requestApproval" | "item/tool/requestUserInput" | "item/permissions/requestApproval"))
        })
    }
    fn push(&mut self, mut event: Value) {
        self.seq += 1;
        event["seq"] = json!(self.seq);
        let bytes = event.to_string().len();
        if bytes > EVENT_BYTES { self.gap = true; return; }
        while self.queue.len() >= EVENT_COUNT || self.bytes + bytes > EVENT_BYTES {
            if let Some((_, size)) = self.queue.pop_front() { self.bytes -= size; self.gap = true; }
            else { break; }
        }
        self.bytes += bytes;
        self.queue.push_back((event, bytes));
    }
    fn poll(&mut self) -> Value {
        let events: Vec<_> = self.queue.drain(..).map(|(value, _)| value).collect();
        self.bytes = 0;
        let gap = std::mem::take(&mut self.gap);
        json!({"events":events,"gap":gap,"pendingRequests":self.pending.values().collect::<Vec<_>>(),"connected":self.connected,"error":self.error})
    }
}
impl ConversationConnection {
    pub(crate) fn mcp_tool_approval(params: &Value) -> Option<Value> {
        // Publisher identity is supplied by Core, never inferred from server metadata.
        if params["origin"].as_str() != Some("core") || params["mode"] != "form"
            || params["_meta"]["codex_approval_kind"] != "mcp_tool_call" { return None; }
        let thread=params["threadId"].as_str()?;let turn=params["turnId"].as_str()?;
        let server=params["serverName"].as_str()?;let tool=params["_meta"]["tool_name"].as_str()?;
        if [thread,turn,server,tool].into_iter().any(|value|valid_id(value).is_err()) {return None;}
        let arguments=&params["_meta"]["tool_params"];
        if !arguments.is_object() || arguments.to_string().len()>MCP_APPROVAL_REVIEW_BYTES || !mcp_arguments_reviewable(arguments,0) {return None;}
        Some(json!({"origin":"core","threadId":thread,"turnId":turn,"serverName":server,"toolName":tool,"arguments":arguments}))
    }
    pub(crate) fn approval_decisions(params: &Value) -> Vec<&'static str> {
        match params.get("availableDecisions") {
            None=>vec!["accept","decline","cancel"],
            Some(Value::Array(advertised))=>["accept","acceptForSession","decline","cancel"].into_iter()
                .filter(|choice|advertised.iter().any(|value|value.as_str()==Some(*choice))).collect(),
            Some(_)=>Vec::new(),
        }
    }
    pub(super) fn from_initialize(endpoint: Endpoint, stopped: Arc<AtomicBool>, initialized: &Value) -> Self {
        Self(Arc::new(Control {endpoint,stopped,passive_supported:passive_capability(initialized),subagents_supported:initialized["supportsSubagentManagement"].as_bool()==Some(true),compact_supported:compact_capability(initialized),input:Mutex::new(false),owned:AtomicBool::new(false),worker:Mutex::new(None),events:Mutex::new(Events::default()),sent:Mutex::new(HashSet::new()),unknown:AtomicBool::new(false),deleted:AtomicBool::new(false),detail:Mutex::new(None),subagents:Mutex::new(subagents::State::default()),context_settings:Mutex::new(context::State::default()),observer_compaction:Mutex::new(ObserverCompaction::default())}))
    }
    #[cfg(test)]
    pub(super) fn new(endpoint: Endpoint, stopped: Arc<AtomicBool>) -> Self {
        // Existing protocol fixtures explicitly exercise supported extension methods.
        Self::from_initialize(endpoint, stopped, &json!({"opendiyCapabilities":{"passiveSubscription":true,"compactStartIfIdle":true}}))
    }
    pub(crate) fn terminal_write<T>(&self, write: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let gui = self.0.input.lock().map_err(|_| "대화 입력 소유 상태를 확인할 수 없습니다.")?;
        if *gui { return Err("대화 화면이 입력을 소유하고 있습니다. 터미널로 전환한 뒤 입력하세요.".into()); }
        write()
    }
    pub(crate) fn question_snapshot(&self) -> Result<(bool,bool,Vec<Value>,bool),String> {
        let owned = self.0.owned.load(Ordering::Acquire);
        let events = self.0.events.lock().map_err(|_| "대화 승인 상태 오류")?;
        let mut requests = events.observed_pending.clone();
        requests.extend(events.pending.clone());
        for (id,request) in &mut requests {request["metadataAvailable"]=json!(!events.requests_unavailable&&((events.connected&&events.pending.contains_key(id))||(events.observer_connected&&events.observed_pending.contains_key(id))));}
        let mut requests = requests.into_values().collect::<Vec<_>>();
        requests.extend(events.async_questions.snapshot(events.observer_connected).into_iter().map(|mut request| {
            request["inputAvailable"]=json!(async_reply_available(&self.0,&events,!owned).is_ok());request
        }));
        let complete=self.0.passive_supported && events.observer_connected && !events.requests_unavailable && events.async_questions.complete();
        Ok((owned,events.connected,requests,complete))
    }
    #[cfg(test)]
    pub(crate) fn pending_snapshot(&self) -> Result<(bool,bool,Vec<Value>),String> {
        self.question_snapshot().map(|(owned,connected,requests,_)|(owned,connected,requests))
    }
    #[cfg(test)]
    pub(crate) fn question_metadata_complete(&self) -> Result<bool,String> {
        self.question_snapshot().map(|(_,_,_,complete)|complete)
    }
    fn context_status(&self, thread_id: &str) -> Result<Value,String> {
        // 작업자 종료 join을 기다리지 않는 읽기다. 연결·이력·이벤트 큐를 건드리지 않는다.
        let closing=match self.0.worker.try_lock() {
            Ok(worker)=>worker.as_ref().is_some_and(|worker|worker.closing.is_some()),
            Err(std::sync::TryLockError::WouldBlock)=>true,
            Err(std::sync::TryLockError::Poisoned(_))=>return Err("대화 연결 상태 오류".into()),
        } || self.0.stopped.load(Ordering::Acquire);
        let events=self.0.events.lock().map_err(|_| "대화 문맥 상태 오류")?;
        let reason=context_block_reason(&self.0,&events,closing);
        Ok(json!({"threadId":thread_id,"connected":events.connected||events.observer_connected,"guiConnected":events.connected,"passiveSubscriptionSupported":self.0.passive_supported,
            "subagentsSupported":self.0.subagents_supported,"compactSupported":self.0.compact_supported,
            "tokenUsage":events.token_usage,"tokenUsageFresh":events.token_usage_fresh&&events.observer_connected&&!self.0.unknown.load(Ordering::Acquire),"compacting":events.context_compaction.is_some(),"active":events.context_active,
            "canCompact":reason.is_none(),"reason":reason,"revision":events.context_revision}))
    }
    pub(crate) fn is_deleted(&self) -> bool { self.0.deleted.load(Ordering::Acquire) }
    pub(super) fn observer_message(&self,thread_id: &str,message: Value) -> Result<(),String> {record_message(&self.0,thread_id,message,true)}
    pub(super) fn observer_call<S: Read+Write>(&self,rpc: &mut Rpc<S>,thread_id: &str,method: &str,params: Value) -> Result<Value,String> {
        call_observed(&self.0,rpc,thread_id,method,params,self.0.stopped.as_ref(),true,None,None,None,None)
    }
    fn request_observer_compact(&self,thread_id: &str) -> Result<Value,String> {
        self.request_observer_action(thread_id,ObserverAction::Compact)
    }
    fn request_observer_action(&self,thread_id: &str,action: ObserverAction) -> Result<Value,String> {
        valid_id(thread_id)?;live(&self.0,&AtomicBool::new(false))?;
        let compact=matches!(&action,ObserverAction::Compact);
        let id=Uuid::new_v4();let cancelled=Arc::new(AtomicBool::new(false));
        let (reply,response)=mpsc::sync_channel(1);
        {
            // Match Attach's worker -> slot lock order; no mutex crosses network I/O.
            let worker=self.0.worker.lock().map_err(|_|"대화 연결 상태 오류")?;
            if worker.is_some() || self.0.owned.load(Ordering::Acquire) {return if compact {Ok(json!({"status":"notStarted","reason":"inputOwnerChanged"}))}else{Err("입력 소유권이 바뀌어 질문 답변을 보내지 않았습니다.".into())};}
            let mut slot=self.0.observer_compaction.lock().map_err(|_|"압축 요청 상태 오류")?;
            if slot.busy {return if compact {Ok(json!({"status":"notStarted","reason":"compacting"}))}else{Err("다른 관찰자 요청을 확인 중이라 질문 답변을 보내지 않았습니다.".into())};}
            let events=self.0.events.lock().map_err(|_|"대화 문맥 상태 오류")?;
            match &action {
                ObserverAction::Compact=>if let Some(reason)=context_block_reason(&self.0,&events,false) {return Ok(json!({"status":if reason=="unsupported"{"unsupported"}else{"notStarted"},"reason":reason}));},
                ObserverAction::AsyncReply{request_id,result}=>{
                    async_reply_available(&self.0,&events,true)?;
                    events.async_questions.reply_payload(request_id,result)?;
                },
            }
            slot.busy=true;
            slot.pending=Some(PendingCompact{id,thread_id:thread_id.to_owned(),deadline:Instant::now()+RPC_TIMEOUT,cancelled:cancelled.clone(),reply,action});
        }
        match response.recv_timeout(RPC_TIMEOUT+Duration::from_secs(2)) {
            Ok(result)=>result,
            Err(_)=>{
                cancelled.store(true,Ordering::Release);
                let mut slot=self.0.observer_compaction.lock().map_err(|_|"압축 요청 상태 오류")?;
                if slot.pending.as_ref().is_some_and(|pending|pending.id==id) {slot.pending=None;slot.busy=false;}
                else {self.0.unknown.store(true,Ordering::Release);}
                Err("관찰자 요청 응답을 확인하지 못했습니다. 자동으로 다시 요청하지 않았습니다.".into())
            },
        }
    }
    pub(super) fn observer_compact<S: Read+Write>(&self,rpc: &mut Rpc<S>,thread_id: &str) {
        let pending=self.0.observer_compaction.lock().unwrap_or_else(|e|e.into_inner()).pending.take();
        let Some(pending)=pending else {return;};
        let previous_deadline=rpc.deadline;
        rpc.deadline=Some(previous_deadline.map_or(pending.deadline,|deadline|deadline.min(pending.deadline)));
        let result=if pending.thread_id!=thread_id || Instant::now()>=pending.deadline {
            Err("원래 대화 또는 압축 요청 기한을 확인하지 못해 전송하지 않았습니다.".into())
        } else {match pending.action {
            ObserverAction::Compact=>execute_compact(&self.0,rpc,thread_id,&pending.cancelled,true),
            ObserverAction::AsyncReply{request_id,result}=>execute_async_reply(&self.0,rpc,thread_id,&request_id,&result,&pending.cancelled,true),
        }};
        rpc.deadline=previous_deadline;
        self.0.observer_compaction.lock().unwrap_or_else(|e|e.into_inner()).busy=false;
        let _=pending.reply.send(result);
    }
    // false는 실제 JSON-RPC의 정확한 준비 오류에서만 반환한다. 문자열이나
    // 성공 응답의 유사 필드로 재시도를 지시하지 않는다.
    pub(super) fn observer_subscribe<S: Read+Write>(&self,rpc: &mut Rpc<S>,thread_id: &str,cwd: &Path) -> Result<bool,String> {
        if !self.0.passive_supported { return Err(PASSIVE_UNSUPPORTED.into()); }
        let question_revision=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?.async_questions.revision;
        {let mut events=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?;events.observer_compact_supported=false;events.context_revision+=1;}
        let mut preparing=false;
        let result=match call_observed(&self.0,rpc,thread_id,PASSIVE_SUBSCRIBE,json!({"threadId":thread_id}),self.0.stopped.as_ref(),true,None,Some(&mut preparing),None,None) {
            Ok(value)=>value,Err(_) if preparing=>return Ok(false),Err(error)=>return Err(error),
        };
        verify_passive(&result,thread_id,cwd)?;
        let mut events=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?;
        if events.requests_unavailable {return Err("요청 관측을 중단한 세션의 구독을 복구하지 않았습니다.".into());}
        events.context_active=context_active(&result["thread"]["status"]);
        events.context_turn=None;events.context_activity_revision+=1;
        events.observer_compact_supported=self.0.compact_supported&&result["supportsCompactIfIdle"].as_bool()==Some(true);
        events.token_usage_fresh=false;events.context_compaction=None;
        events.context_revision+=1;
        events.observer_connected=true;
        events.async_questions.unavailable("현재 작업의 비동기 질문 이력을 확인하고 있습니다.");
        let revision=events.async_questions.revision;
        let active=events.context_active;
        drop(events);
        // Passive subscribe deliberately returns no history. Only the active turn is
        // hydrated, with bounded pages/bytes; no transcript cache is retained.
        if active==Some(false) {
            let mut events=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?;
            if events.async_questions.revision==question_revision {events.async_questions.idle();}
            else {events.async_questions.unavailable("질문 구독 중 작업 상태가 바뀌었습니다. 실시간 관측 내용을 보존했습니다.");}
        } else {
            // One retry is reserved for a live-event race, within the same total
            // deadline. Protocol/read/size failures are never retried here.
            let previous_deadline=rpc.deadline;
            rpc.deadline=Some(previous_deadline.unwrap_or_else(||Instant::now()+RPC_TIMEOUT).min(Instant::now()+RPC_TIMEOUT));
            let mut revision=revision;
            for _ in 0..2 {
                let observed_turn=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?.async_questions.active_turn_id().map(str::to_owned);
                match restore_async_questions(&self.0,rpc,thread_id,revision) {
                    Ok(true)=>break,
                    Ok(false)=>{revision=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?.async_questions.revision;},
                    Err(error)=>{
                        let mut events=self.0.events.lock().map_err(|_| "질문 관측 상태 오류")?;
                        // Context status may have no turn identity after subscribe.
                        // Only the exact previously observed question turn's end
                        // can turn a failed history read into a confirmed empty inbox.
                        if observed_turn.as_deref().is_some_and(|turn|events.async_questions.closed_turn_matches(turn)) {events.async_questions.idle();}
                        else {events.async_questions.unavailable(&error);}
                        break;
                    },
                }
            }
            rpc.deadline=previous_deadline;
        }
        Ok(true)
    }
    pub(super) fn observer_disconnected(&self) {
        let mut events=self.0.events.lock().unwrap_or_else(|e|e.into_inner());
        events.observer_connected=false;events.token_usage_fresh=false;events.context_revision+=1;
        events.observer_compact_supported=false;
        events.context_active=None;events.context_compaction=None;
        events.context_turn=None;events.context_activity_revision+=1;
        events.async_questions.unavailable("질문 관측 연결이 끊겨 마지막 확인 내용을 표시합니다.");
        drop(events);
        let mut slot=self.0.observer_compaction.lock().unwrap_or_else(|e|e.into_inner());
        if let Some(pending)=slot.pending.take() {slot.busy=false;let _=pending.reply.send(Err("압축 요청 전 관측 연결이 끊겼습니다. 재요청하지 않았습니다.".into()));}
    }
    pub(crate) fn cancel(&self) {
        let mut slot = self.0.worker.lock().unwrap_or_else(|e| e.into_inner());
        let worker = slot.take();
        if let Some(mut worker) = worker {
            if let Err(error)=worker.stop() {self.0.events.lock().unwrap_or_else(|e|e.into_inner()).error=Some(error);}
        }
        {let mut events=self.0.events.lock().unwrap_or_else(|e|e.into_inner());events.gui_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;}
        *self.0.input.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.0.owned.store(false,Ordering::Release);
        self.0.detail.lock().unwrap_or_else(|e|e.into_inner()).take();
    }
    fn release(&self,finished_only: bool,handoff: bool) -> Result<bool,String> {
        let mut slot=self.0.worker.lock().map_err(|_| "대화 연결 상태 오류")?;
        let Some(worker)=slot.as_mut() else {return if self.0.owned.load(Ordering::Acquire) {Err("대화 입력 소유권의 종료 근거가 없습니다.".into())} else {Ok(true)};};
        if finished_only && !worker.joined && !worker.join.as_ref().is_some_and(JoinHandle::is_finished) {return Ok(false);}
        // worker의 종료 콜백도 events를 잠그므로 join 중에는 events/input을 잡지 않는다.
        if let Err(error)=worker.stop() {
            let mut events=self.0.events.lock().map_err(|_| "대화 승인 상태 오류")?;
            events.connected=false;events.gui_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;events.error=Some(error.clone());return Err(error);
        }
        let mut events=self.0.events.lock().map_err(|_| "대화 승인 상태 오류")?;
        if events.blocks_release(handoff) || self.0.unknown.load(Ordering::Acquire) {
            return Err("미확인 요청 또는 승인 대기가 남아 대화 입력 소유권을 유지했습니다.".into());
        }
        // join 뒤 두 맵의 최신 사본을 같은 잠금에서 재검사한다. 명시 인계의
        // 일반 미응답 pending은 해결/삭제하지 않고 원래 TUI의 응답을 기다린다.
        *self.0.input.lock().map_err(|_| "대화 입력 소유 상태 오류")?=false;
        self.0.owned.store(false,Ordering::Release);
        events.gui_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;
        for request in events.pending.values_mut() {request["guiOwned"]=json!(false);}
        slot.take();
        self.0.detail.lock().map_err(|_| "대화 상세 스냅샷 상태 오류")?.take();
        Ok(true)
    }
    fn runtime_extension_request(&self, thread_id: &str, expected_thread_id: &str, cwd: &Path, request: runtime_extensions::RuntimeExtensionRequest) -> Result<Value, String> {
        if thread_id != expected_thread_id { return Err("검토한 대화가 변경되어 확장 요청을 보내지 않았습니다. 현재 대화를 다시 확인하세요.".into()); }
        runtime_extensions::validate(thread_id, &request)?;
        let mut state = self.0.context_settings.try_lock().map_err(|_| "이 세션의 설정 또는 확장 조회·변경 요청이 이미 진행 중입니다.")?;
        state.bind_thread(thread_id)?;
        live(&self.0, &AtomicBool::new(false))?;
        if matches!(request, runtime_extensions::RuntimeExtensionRequest::ReloadMcp {}) {
            let status = self.context_status(thread_id)?;
            let (_, _, pending, complete) = self.question_snapshot()?;
            if status["connected"] != true || status["active"] != false || status["compacting"] != false
                || status["reason"] == "closing" || self.0.unknown.load(Ordering::Acquire) || !complete || !pending.is_empty() {
                return Err("대화의 연결·작업·승인 상태를 확인한 뒤 MCP를 다시 연결할 수 있습니다. 요청하지 않았습니다.".into());
            }
        }
        let deadline = Instant::now() + RPC_TIMEOUT;
        let cancelled = Arc::new(AtomicBool::new(false));
        let transport = Arc::new(Mutex::new(Transport::default()));
        let _watch = DeadlineWatch::start(Arc::new(Mutex::new(Some(deadline))), transport.clone(), cancelled.clone(), self.0.stopped.clone())?;
        match &self.0.endpoint {
            Endpoint::Shared(endpoint) => {
                let mut proxy = endpoint.open_until(Some(cancelled.clone()), Some(deadline))
                    .map_err(|_| "현재 대화의 확장 관리 연결을 확인하지 못했습니다. 요청 결과는 미확인입니다.".to_owned())?;
                transport.lock().map_err(|_| "확장 연결 정리 상태 오류")?.proxy = proxy.job.as_ref().map(Arc::downgrade);
                let result = (|| {
                    let rpc = proxy.rpc.as_mut().ok_or("현재 대화의 확장 연결을 찾을 수 없습니다.")?;
                    rpc.cancelled = Some(cancelled.clone()); rpc.deadline = Some(deadline);
                    let mut init = initialize(); init["capabilities"] = json!({"experimentalApi":true});
                    rpc.call("initialize", init).map_err(|_| "확장 관리 연결 초기화를 확인하지 못했습니다.".to_owned())?;
                    rpc.send(json!({"method":"initialized","params":{}})).map_err(|_| "확장 관리 연결 초기화를 확인하지 못했습니다.".to_owned())?;
                    live(&self.0, &cancelled)?;
                    let value = if matches!(request, runtime_extensions::RuntimeExtensionRequest::HooksStatus {}) {
                        runtime_extensions::exchange_hooks(rpc, thread_id, cwd)?
                    } else { runtime_extensions::exchange(rpc, thread_id, &request)? };
                    live(&self.0, &cancelled)?;
                    Ok(value)
                })();
                match (result, proxy.finish()) {
                    (result, Ok(())) => result,
                    (_, Err(_)) => Err("확장 관리 연결 정리를 확인하지 못했습니다. 요청 결과를 자동 재시도하지 않습니다.".into()),
                }
            }
            #[cfg(test)]
            Endpoint::Unavailable => Err("Test fixture has no MCP runtime endpoint".into()),
        }
    }
    #[cfg(test)]
    pub(crate) fn request(&self, thread_id: &str, cwd: &Path, request: ConversationRequest) -> Result<Value, String> {
        self.request_context(thread_id,cwd,request,None)
    }
    pub(crate) fn request_for_session(&self,thread_id: &str,cwd: &Path,request: ConversationRequest,session_id: &str,store: crate::conversation_attachments::ConversationAttachments) -> Result<Value,String> {
        let context=matches!(request,ConversationRequest::Send{..}).then(||SendContext{session_id:session_id.to_owned(),store});
        self.request_context(thread_id,cwd,request,context)
    }
    fn request_context(&self, thread_id: &str, cwd: &Path, request: ConversationRequest, send_context: Option<SendContext>) -> Result<Value, String> {
        validate_request(&request)?;
        if let ConversationRequest::McpStatus { expected_thread_id, server_name, cursor } = &request {
            return self.runtime_extension_request(thread_id, expected_thread_id, cwd, runtime_extensions::RuntimeExtensionRequest::McpStatus { server_name: server_name.clone(), cursor: cursor.clone() });
        }
        if let ConversationRequest::ReloadMcp { expected_thread_id } = &request {
            return self.runtime_extension_request(thread_id, expected_thread_id, cwd, runtime_extensions::RuntimeExtensionRequest::ReloadMcp {});
        }
        if let ConversationRequest::HooksStatus { expected_thread_id } = &request {
            return self.runtime_extension_request(thread_id, expected_thread_id, cwd, runtime_extensions::RuntimeExtensionRequest::HooksStatus {});
        }
        if matches!(request, ConversationRequest::ContextSettings | ConversationRequest::ContextComposition {..} | ConversationRequest::PermissionStatus | ConversationRequest::SetContextWindow {..}) {
            return self.context_settings_request(thread_id, request);
        }
        if matches!(request, ConversationRequest::ContextStatus) {
            return self.context_status(thread_id);
        }
        if matches!(request,ConversationRequest::Compact) && !self.0.owned.load(Ordering::Acquire) {
            return self.request_observer_compact(thread_id);
        }
        if let ConversationRequest::Respond{request_id,result}=&request {
            if async_questions::is_question_id(request_id) && !self.0.owned.load(Ordering::Acquire) {
                return self.request_observer_action(thread_id,ObserverAction::AsyncReply{request_id:request_id.clone(),result:result.clone()});
            }
        }
        if matches!(request, ConversationRequest::Poll) {
            let mut poll = self.0.events.lock().map_err(|_| "대화 이벤트 상태를 확인할 수 없습니다.")?.poll();
            poll["unknownResult"] = json!(self.0.unknown.load(Ordering::Acquire)); return Ok(poll);
        }
        // 해제도 같은 단일 작업자의 요청으로 직렬화한다. 승인/결과 불명 상태에서는
        // 연결과 입력 소유권을 보존해 터미널의 두 번째 응답을 막는다.
        if matches!(request, ConversationRequest::Detach | ConversationRequest::HandoffToTerminal) {
            let handoff=matches!(request,ConversationRequest::HandoffToTerminal);
            if self.release(true,handoff)? {return Ok(json!({}));}
            let (reply, receiver) = mpsc::sync_channel(1);
            let claim=Uuid::new_v4();
            {
                let mut workers = self.0.worker.lock().map_err(|_| "대화 연결 상태 오류")?;
                if let Some(worker) = workers.as_mut() {
                    if worker.closing.is_some() {return Err("대화 입력 인계를 이미 확인 중입니다. 입력 소유권을 유지했습니다.".into());}
                    worker.closing=Some(claim);
                    match worker.sender.try_send(Work {request,reply,deadline:Instant::now()+RPC_TIMEOUT,send_context:None}) {
                        Ok(()) => {},
                        Err(mpsc::TrySendError::Full(_)) => {
                            worker.closing=None;return Err("대화 해제 요청을 전달할 수 없습니다. 입력 소유권을 유지했습니다.".into());
                        },
                        Err(mpsc::TrySendError::Disconnected(_)) => {
                            // A dropped receiver is not proof of safe release. Bootstrap
                            // may have just failed: join and recheck pending/unknown.
                            drop(workers);
                            if !self.release(false,handoff)? {return Err("대화 입력 소유권의 해제를 확인할 수 없습니다.".into());}
                            return Ok(json!({}));
                        },
                    }
                } else { return Ok(json!({})); }
            }
            let result = receiver.recv_timeout(RPC_TIMEOUT+Duration::from_secs(2)).map_err(|_| "대화 해제 결과를 확인할 수 없어 입력 소유권을 유지했습니다.")?;
            let result=match result {Ok(value)=>value,Err(error)=>{
                let mut worker=self.0.worker.lock().map_err(|_| "대화 연결 상태 오류")?;
                if let Some(worker)=worker.as_mut() {
                    if worker.closing==Some(claim) && !worker.joined && !worker.cancelled.load(Ordering::Acquire) && worker.join.as_ref().is_some_and(|join|!join.is_finished()) {worker.closing=None;}
                }
                return Err(error);
            }};
            if !self.release(false,handoff)? {return Err("대화 입력 소유권의 해제를 확인할 수 없습니다.".into());}
            return Ok(result);
        }
        if self.0.stopped.load(Ordering::Acquire) { return Err("대화 세션이 종료되었습니다.".into()); }
        if self.0.deleted.load(Ordering::Acquire) { return Err("원래 대화가 삭제되었습니다. 다른 대화를 만들거나 재개하지 않았습니다.".into()); }
        let (reply, response) = mpsc::sync_channel(1);
        let attaching = matches!(request, ConversationRequest::Attach);
        let mut worker = self.0.worker.lock().map_err(|_| "대화 연결 상태를 확인할 수 없습니다.")?;
        if attaching {
            if !self.0.passive_supported {return Err(PASSIVE_UNSUPPORTED.into());}
            if worker.is_some() { return Err("이미 대화 화면이 연결되어 있습니다.".into()); }
            if self.0.observer_compaction.lock().map_err(|_|"압축 요청 상태 오류")?.busy {return Err("컨텍스트 압축 요청을 확인 중입니다. 입력 소유권을 바꾸지 않았습니다.".into());}
            {
                let mut events = self.0.events.lock().map_err(|_| "대화 이벤트 상태 오류")?;
                if events.blocks_attach(self.0.owned.load(Ordering::Acquire),self.0.unknown.load(Ordering::Acquire)) {return Err("이전 연결의 미확인 요청을 보존하고 있어 새 입력 소유권을 만들지 않았습니다.".into());}
                events.queue.clear(); events.bytes=0; events.gap=false; events.error=None;
                events.gui_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;
                events.context_revision+=1;
            }
            *self.0.input.lock().map_err(|_| "대화 입력 소유 상태를 확인할 수 없습니다.")? = true;
            self.0.owned.store(true,Ordering::Release);
            let (sender, receiver) = mpsc::sync_channel(8);
            // Enqueue before spawn so even immediate initialization failure reaches
            // this request's reply, rather than failing a later enqueue generically.
            sender.try_send(Work {request:ConversationRequest::Attach,reply:reply.clone(),deadline:Instant::now()+RPC_TIMEOUT,send_context:None})
                .map_err(|_| "대화 연결의 초기 요청을 준비할 수 없습니다.")?;
            let cancelled = Arc::new(AtomicBool::new(false));
            let transport = Arc::new(Mutex::new(Transport::default()));
            let control = self.0.clone();
            let thread_id = thread_id.to_owned(); let cwd = cwd.to_path_buf();
            let worker_cancelled = cancelled.clone(); let worker_transport = transport.clone();
            let bootstrap_reply = reply.clone();
            let join = match thread::Builder::new().name("codex-conversation".into()).spawn(move || {
                let result = run(control.clone(), &thread_id, &cwd, receiver, worker_cancelled, worker_transport);
                finish_worker(&control, result, &bootstrap_reply);
            }) { Ok(join) => join, Err(error) => { *self.0.input.lock().unwrap_or_else(|e| e.into_inner()) = false; self.0.owned.store(false,Ordering::Release); return Err(format!("대화 연결 작업자를 시작할 수 없습니다: {error}")); } };
            *worker = Some(Worker {sender,cancelled,transport,join:Some(join),joined:false,closing:None});
        }
        if worker.as_ref().is_some_and(|worker|worker.closing.is_some()) {return Err("대화 입력을 터미널로 인계 중이라 새 요청을 받지 않았습니다.".into());}
        if let ConversationRequest::Send {request_id,..} = &request {
            Uuid::parse_str(request_id).map_err(|_| "전송 식별자는 UUID여야 합니다.")?;
            let mut sent = self.0.sent.lock().map_err(|_| "대화 전송 기록을 확인할 수 없습니다.")?;
            if sent.contains(request_id) { return Err("이미 처리한 전송 식별자입니다. 결과가 불명확한 요청을 재전송하지 않았습니다.".into()); }
            // 삭제한 식별자의 재전송을 허용하지 않으므로 상한에 도달하면 명시적으로 거부한다.
            if sent.len() >= 1024 { return Err("이 세션의 전송 식별자 상한에 도달했습니다. 세션을 다시 여세요.".into()); }
            sent.insert(request_id.clone());
        }
        if !attaching {
            worker.as_ref().ok_or("대화 화면을 먼저 연결하세요.")?.sender.try_send(Work {request,reply,deadline:Instant::now() + RPC_TIMEOUT,send_context})
                .map_err(|error| match error {mpsc::TrySendError::Full(_) => "대화 요청 대기열이 가득 찼습니다. 요청을 보내지 않았습니다.",mpsc::TrySendError::Disconnected(_) => "대화 연결이 끊겼습니다. 전송 결과는 확인되지 않았습니다."})?;
        }
        drop(worker);
        response.recv_timeout(RPC_TIMEOUT + Duration::from_secs(2)).map_err(|_| "대화 요청 응답을 확인할 수 없습니다. 요청을 재전송하지 않았습니다.".to_string())?
    }
}

fn run(control: Arc<Control>, thread_id: &str, cwd: &Path, receiver: mpsc::Receiver<Work>, cancelled: Arc<AtomicBool>, transport: Arc<Mutex<Transport>>) -> Result<(), String> {
    let deadline = Arc::new(Mutex::new(Some(Instant::now()+RPC_TIMEOUT)));
    let _watch = DeadlineWatch::start(deadline.clone(),transport.clone(),cancelled.clone(),control.stopped.clone())?;
    let result = match &control.endpoint {
        Endpoint::Shared(endpoint) => {
            let mut proxy = endpoint.open(Some(cancelled.clone()))?;
            transport.lock().map_err(|_| "대화 연결 정리 상태 오류")?.proxy = proxy.job.as_ref().map(Arc::downgrade);
            // 취소 감시자는 세션 연결 수명 동안 유지해 포화된 파이프 쓰기도 회수한다.
            let mut rpc = proxy.rpc.take().unwrap();
            let result = serve(&control, &mut rpc, thread_id, cwd, receiver, &cancelled, &deadline);
            drop(rpc); drop(proxy); result
        }
        #[cfg(test)]
        Endpoint::Unavailable => Err("Test fixture has no runtime endpoint".into()),
    };
    let mut transport = transport.lock().map_err(|_| "대화 연결 정리 상태 오류")?;
    if let Some(socket) = transport.socket.take() { let _ = socket.shutdown(Shutdown::Both); }
    transport.proxy.take();
    result
}
fn live(control: &Control, cancelled: &AtomicBool) -> Result<(), String> {
    if control.deleted.load(Ordering::Acquire) { Err("원래 대화가 삭제되어 연결과 이력 조회를 종료했습니다.".into()) }
    else if control.stopped.load(Ordering::Acquire) || cancelled.load(Ordering::Acquire) { Err("대화 연결이 종료되었습니다. 진행 중 요청 결과는 확인되지 않았습니다.".into()) }
    else if control.events.lock().map_err(|_| "대화 승인 상태 오류")?.requests_unavailable {Err("해결 기록 상한으로 요청 관측을 중단했습니다. 마지막 확인 내용을 보존했습니다.".into())} else { Ok(()) }
}
fn verify_passive(result: &Value,thread_id: &str,cwd: &Path) -> Result<(),String> {
    verified_thread(&result["thread"],thread_id,cwd)?;
    interrupt_capability(result)?;
    if result.get("supportsCompactIfIdle").is_some_and(|value|!value.is_boolean()) {
        return Err("수동 구독 응답의 압축 지원 정보가 bool이 아니어서 연결을 중단했습니다.".into());
    }
    if !result["thread"]["turns"].as_array().is_some_and(Vec::is_empty)
        || !result["initialTurnsPage"].is_null() || !result["turnsBackwardsCursor"].is_null() || !result["itemsBackwardsCursor"].is_null() {
        return Err("수동 구독 응답에 이력이 포함되었거나 형식이 올바르지 않아 연결을 중단했습니다.".into());
    }
    Ok(())
}
fn interrupt_capability(result: &Value) -> Result<bool,String> {
    match result.get("supportsExpectedTurnInterrupt") {
        None=>Ok(false),Some(Value::Bool(supported))=>Ok(*supported),
        Some(_)=>Err("서버 응답의 안전한 작업 중단 지원 정보 형식이 올바르지 않아 연결을 중단했습니다.".into()),
    }
}
fn call<S: Read + Write>(control: &Control, rpc: &mut Rpc<S>, thread_id: &str, method: &str, params: Value, cancelled: &AtomicBool) -> Result<Value, String> {
    call_observed(control,rpc,thread_id,method,params,cancelled,false,None,None,None,None)
}
fn call_observed<S: Read+Write>(control: &Control,rpc: &mut Rpc<S>,thread_id: &str,method: &str,mut params: Value,cancelled: &AtomicBool,metadata_only: bool,send_context: Option<(&SendContext,&[Attachment])>,preparing: Option<&mut bool>,mut compact_progress: Option<&mut CompactProgress>,async_reply: Option<(&str,&str)>) -> Result<Value,String> {
    live(control, cancelled)?;
    check_cancelled(rpc.cancelled.as_ref())?;
    if rpc.deadline.is_some_and(|deadline|Instant::now()>=deadline) {return Err("대화 요청 기한이 지나 전송하지 않았습니다.".into());}
    rpc.bind_core_version(method, &mut params)?;
    // Last check after draining the writer's notifications and before marking
    // the wire attempt unknown. Never send an answer for an obsolete question.
    if let Some((id,turn))=async_reply {
        let events=control.events.lock().map_err(|_|"질문 관측 상태 오류")?;
        async_reply_available(control,&events,metadata_only)?;
        if !events.async_questions.reply_attempt_current(id,turn) {return Err("질문 또는 작업 상태가 바뀌어 답변을 보내지 않았습니다.".into());}
    }
    let mutation = matches!(method,START_USER_IF_IDLE | COMPACT_IF_IDLE | "turn/interrupt" | "turn/steer");
    let previous_unknown = if mutation {control.unknown.swap(true,Ordering::AcqRel)} else {false};
    // 입력·작업 상태·큐·취소·기한 검사가 끝난 실제 송신 시도 경계에서만 보존한다.
    // 이 뒤의 실패는 일부 바이트 송신 가능성이 있어 파일과 결과 불명을 보존한다.
    let mut retention=if let Some((context,attachments))=send_context {
        match context.store.retain_for_send(&context.session_id,&attachments.iter().map(|item|item.path.clone()).collect::<Vec<_>>()) {
            Ok(receipt)=>Some((context,receipt)),
            Err(error)=>{control.unknown.store(previous_unknown,Ordering::Release);return Err(error);},
        }
    } else {None};
    rpc.next_id += 1; let id = rpc.next_id;
    // 최종 사전 검사 뒤의 송신 시도다. retain 뒤에 Rpc::send의 사전 검사를
    // 다시 실행해 확정 미전송 오류로 바꾸지 않는다. 진행 기한 감시자는 이
    // 경계 뒤의 포화 쓰기·취소를 소유 transport 종료로 해제한다.
    rpc.ws.send(Message::Text(json!({"id":id,"method":method,"params":params}).to_string().into()))
        .map_err(|error|format!("대화 {method} 송신 시도에 실패했습니다. 요청 결과는 확인되지 않았습니다: {error}"))?;
    let deadline = rpc.deadline.unwrap_or_else(|| Instant::now()+RPC_TIMEOUT).min(Instant::now()+RPC_TIMEOUT);
    while Instant::now() < deadline {
        live(control, cancelled)?;
        if let Some(message) = rpc.read()? {
            if let Some(result) = response(&message, id) {
                // 송신 후 internal 오류는 수락 뒤 실패일 수도 있다. 명시 미시작 또는
                // 메서드/파라미터 검증 거절 외에는 결과 불명과 첨부를 보존한다.
                let rejected=matches!(message["error"]["code"].as_i64(),Some(-32601 | -32602))
                    || (method=="turn/steer" && message["error"]["code"].as_i64()==Some(-32600))
                    || (method==START_USER_IF_IDLE && matches!(message["result"]["status"].as_str(),Some("skippedBusy" | "notStarted")))
                    || (method==COMPACT_IF_IDLE && message.get("error").is_none() && (message["result"]["status"]=="skippedBusy" || (message["result"]["status"]=="notStarted"&&message["result"]["reason"].is_string())));
                let accepted=if matches!(method,START_USER_IF_IDLE | COMPACT_IF_IDLE) {
                    (method!=COMPACT_IF_IDLE||message.get("error").is_none()) && message["result"]["status"]=="started" && message["result"]["turn"]["id"].as_str().is_some_and(|id|!id.is_empty()&&(method!=COMPACT_IF_IDLE||valid_id(id).is_ok()))
                } else if method=="turn/steer" {
                    message.get("error").is_none() && message["result"]["turnId"].as_str().is_some_and(|turn|Some(turn)==params["expectedTurnId"].as_str())
                } else {message.get("result").is_some()};
                if mutation && (rejected || accepted) { control.unknown.store(previous_unknown,Ordering::Release); }
                if rejected {
                    if let Some((context,receipt))=retention.take() {context.store.rollback_known_rejection(&context.session_id,receipt)?;}
                }
                if method==COMPACT_IF_IDLE && matches!(message["error"]["code"].as_i64(),Some(-32601 | -32602)) {
                    return Ok(if message["error"]["code"]==-32601 {json!({"status":"unsupported","reason":"methodUnsupported"})}else{json!({"status":"notStarted","reason":"invalidParams"})});
                }
                if metadata_only && method==PASSIVE_SUBSCRIBE && message["error"]["code"].as_i64()==Some(-32600)
                    && message["error"]["data"]["kind"].as_str()==Some("threadSubscriptionUnavailable")
                    && matches!(message["error"]["data"]["reason"].as_str(),Some("notLoaded" | "listenerNotReady")) {
                    if let Some(preparing)=preparing {*preparing=true;}
                    return result;
                }
                if message["error"]["code"] == -32601 && matches!(method,PASSIVE_SUBSCRIBE | START_USER_IF_IDLE) {
                    return Err(format!("현재 Codex 서버가 안전한 대화 기능({method})을 지원하지 않습니다. 호환되는 서버가 필요합니다. 기존 재개·전송 방식으로 대체하지 않았습니다."));
                }
                if method==PASSIVE_SUBSCRIBE && message["error"]["code"].as_i64()==Some(-32600) && message["error"]["data"].is_null() {
                    return Err(format!("현재 서버의 안전한 수동 구독을 확인할 수 없습니다. 호환되는 서버가 필요합니다. 재시도하거나 기존 재개 방식으로 대체하지 않았습니다. {}",result.as_ref().err().map(String::as_str).unwrap_or("응답 오류")));
                }
                if message["error"]["code"].as_i64()==Some(-32600) && message["error"]["data"]["kind"].as_str()==Some("historyProjectionUnavailable")
                    && message["error"]["data"]["reason"].as_str()==Some("unmaterialized") {
                    if matches!(method,"thread/turns/list" | "thread/items/list" | "thread/items/read") {
                        return Ok(json!({"historyUnavailable":true,"reason":message["error"]["data"]["reason"]}));
                    }
                    return Err(HISTORY_UNAVAILABLE.into());
                }
                if matches!(method,"thread/turns/list" | "thread/items/list" | "thread/items/read") && message["result"].get("historyUnavailable").is_some() {
                    return Err("서버 성공 응답의 이력 대기 표식을 거부했습니다.".into());
                }
                return result;
            }
            if let Some(progress)=compact_progress.as_deref_mut() {progress.observe(thread_id,&message);}
            record_message(control, thread_id, message, metadata_only)?;
        }
    }
    Err(format!("대화 {method} 응답 시간이 초과되었습니다. 요청 결과는 불명확하며 재전송하지 않았습니다."))
}
fn restore_async_questions<S: Read+Write>(control: &Control, rpc: &mut Rpc<S>, thread_id: &str, revision: u64) -> Result<bool,String> {
    let previous_deadline=rpc.deadline;
    rpc.deadline=Some(previous_deadline.unwrap_or_else(||Instant::now()+RPC_TIMEOUT).min(Instant::now()+RPC_TIMEOUT));
    let result=(|| {
        let turns=call_observed(control,rpc,thread_id,"thread/turns/list",json!({"threadId":thread_id,"limit":1,"sortDirection":"desc","itemsView":"summary"}),control.stopped.as_ref(),true,None,None,None,None)?;
        if turns["historyUnavailable"]==true {return Err(HISTORY_UNAVAILABLE.into());}
        let turns=turns["data"].as_array().filter(|turns|turns.len()<=1).ok_or("현재 질문 작업 목록의 형식 또는 크기가 올바르지 않습니다.")?;
        let current=turns.first().filter(|turn|turn["status"]=="inProgress");
        if current.is_none() {return Err("진행 중인 작업의 질문 이력을 확인하지 못했습니다. 빈 질문 목록으로 처리하지 않았습니다.".into());}
        let turn=current.map(|turn|turn["id"].as_str().ok_or("현재 질문 작업 식별자가 없습니다.")).transpose()?;
        if let Some(turn)=turn {valid_id(turn)?;}
        let mut items=Vec::new();let mut bytes=0;let mut inspected=0;let mut pages=0;let mut cursors=HashSet::new();let mut cursor: Option<String>=None;
        if let Some(turn)=turn {
            loop {
                let page=call_observed(control,rpc,thread_id,"thread/items/list",json!({"threadId":thread_id,"turnId":turn,"cursor":cursor,"limit":32,"sortDirection":"desc"}),control.stopped.as_ref(),true,None,None,None,None)?;
                pages+=1;
                if page["historyUnavailable"]==true {return Err(HISTORY_UNAVAILABLE.into());}
                bytes+=page.to_string().len();
                let data=page["data"].as_array().filter(|data|data.len()<=32).ok_or("비동기 질문 이력 페이지의 형식 또는 크기가 올바르지 않습니다.")?;
                inspected+=data.len();
                if bytes>async_questions::HISTORY_BYTES || inspected>async_questions::HISTORY_ITEMS {return Err("현재 작업의 질문 이력 조회 상한에 도달했습니다. 전체 미응답 상태를 확인하지 못했습니다.".into());}
                // R36 returns ThreadItemEntry {turnId,item,...}. The reducer
                // validates every envelope before replacing any previous state.
                items.extend(data.iter().cloned());
                match page.get("nextCursor") {
                    None|Some(Value::Null)=>break,
                    Some(Value::String(next)) if !next.is_empty()&&next.len()<=1024&&cursors.insert(next.clone())=>cursor=Some(next.clone()),
                    _=>return Err("비동기 질문 이력의 다음 위치 형식이 올바르지 않습니다.".into()),
                }
                // Count every inspected item, including irrelevant tool items.
                if pages>=4 || inspected>=async_questions::HISTORY_ITEMS {return Err("현재 작업의 질문 이력 조회 상한에 도달했습니다.".into());}
            }
        }
        control.events.lock().map_err(|_| "질문 관측 상태 오류")?.async_questions.restore(revision,turn,&items)
    })();
    rpc.deadline=previous_deadline;
    result
}
fn serve<S: Read + Write>(control: &Control, rpc: &mut Rpc<S>, thread_id: &str, cwd: &Path, receiver: mpsc::Receiver<Work>, cancelled: &AtomicBool, deadline: &Mutex<Option<Instant>>) -> Result<(), String> {
    {let mut events=control.events.lock().map_err(|_| "대화 문맥 상태 오류")?;events.gui_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;}
    rpc.deadline = *deadline.lock().map_err(|_| "대화 요청 기한 상태 오류")?;
    let mut init = initialize(); init["capabilities"] = json!({"experimentalApi":true});
    let initialized=call(control, rpc, thread_id, "initialize", init, cancelled)?;
    if !passive_capability(&initialized) {return Err(PASSIVE_UNSUPPORTED.into());}
    let initialize_interrupt=interrupt_capability(&initialized)?;
    rpc.send(json!({"method":"initialized","params":{}}))?;
    // loaded 동일 runtime의 수동 구독만 허용한다. resume는 idle 작업을 실행할 수 있어 대체 경로로 쓰지 않는다.
    let mut resumed = call(control, rpc, thread_id, PASSIVE_SUBSCRIBE, json!({"threadId":thread_id}), cancelled)?;
    verify_passive(&resumed, thread_id, cwd)?;
    // CLI는 응답 뒤 복사된 pending을 replay한다. 기존 Attach 전체기한 안에서
    // 실제 동일 원문 또는 resolved를 기다리며 별도 큐/재송신은 만들지 않는다.
    let replay_deadline=rpc.deadline.unwrap_or_else(||Instant::now()+RPC_TIMEOUT);
    loop {
        live(control,cancelled)?;
        let events=control.events.lock().map_err(|_| "대화 승인 상태 오류")?;
        if events.blocks_release(true) || control.unknown.load(Ordering::Acquire) {return Err("재구독 중 미확인 요청이 확인되어 입력 소유권을 유지했습니다.".into());}
        let waiting=events.pending.values().any(|request|request["guiOwned"]==false);drop(events);
        if !waiting {break;}
        if Instant::now()>=replay_deadline {return Err("기한 안에 인계한 요청의 동일 원문을 확인하지 못했습니다. 이전 요청을 보존했습니다.".into());}
        if let Some(message)=rpc.read()? {observe_message(control,thread_id,message)?;}
    }
    resumed["threadId"] = json!(thread_id); resumed["owner"] = json!("gui");
    let current = call(control,rpc,thread_id,"thread/turns/list",json!({"threadId":thread_id,"limit":1,"sortDirection":"desc","itemsView":"summary"}),cancelled)?;
    if current["historyUnavailable"] == true {
        resumed["historyUnavailable"] = json!(true); resumed["activeTurnKnown"] = json!(false); resumed["activeTurnId"] = Value::Null;
    } else {
        let current = current["data"].as_array().filter(|values| values.len() <= 1).ok_or("현재 대화 작업 목록이 요청 상한을 넘었습니다.")?;
        resumed["activeTurnKnown"] = json!(true);
        resumed["activeTurnId"] = current.first().filter(|turn| turn["status"] == "inProgress").and_then(|turn| turn["id"].as_str()).map_or(Value::Null,|id|json!(id));
    }
    { let mut events = control.events.lock().map_err(|_| "대화 이벤트 상태 오류")?; if events.requests_unavailable {return Err("요청 관측을 중단한 세션의 연결을 복구하지 않았습니다.".into());} events.gui_compact_supported=control.compact_supported&&compact_capability(&initialized)&&resumed["supportsCompactIfIdle"].as_bool()==Some(true);
        let passive_interrupt=interrupt_capability(&resumed)?;
        // 이 GUI 연결에서 두 응답을 모두 읽었으므로 양쪽의 명시 지원만 채택한다.
        // 다른 관찰자 연결의 값이나 압축 지원 값은 중단 권한이 아니다.
        events.gui_interrupt_supported=initialize_interrupt&&passive_interrupt;
        events.interrupt_diagnostic=(initialize_interrupt!=passive_interrupt).then_some("현재 연결의 안전한 작업 중단 지원 응답이 서로 일치하지 않아 요청을 보내지 않았습니다.");
        events.connected = true;events.context_revision+=1; events.error = None; }
    rpc.deadline = None; *deadline.lock().map_err(|_| "대화 요청 기한 상태 오류")? = None;
    loop {
        live(control, cancelled)?;
        match receiver.try_recv() {
            Ok(work) => {
                if Instant::now() >= work.deadline { let _ = work.reply.send(Err("대기 중 대화 요청의 기한이 지나 전송하지 않았습니다.".into())); continue; }
                if matches!(work.request,ConversationRequest::Detach | ConversationRequest::HandoffToTerminal) {
                    let handoff=matches!(work.request,ConversationRequest::HandoffToTerminal);
                    let pending = control.events.lock().map_err(|_| "대화 승인 상태 오류")?.blocks_release(handoff);
                    if pending || control.unknown.load(Ordering::Acquire) {
                        let _ = work.reply.send(Err("승인 대기 또는 결과 불명 요청이 있어 대화 입력 소유권을 유지했습니다. 현재 화면에서 요청을 처리하세요.".into()));
                        continue;
                    }
                    let _ = work.reply.send(Ok(json!({}))); return Ok(());
                }
                let result = match work.request {
                    ConversationRequest::Attach => Ok(resumed.clone()),
                    request => {
                        rpc.deadline = Some(work.deadline); *deadline.lock().map_err(|_| "대화 요청 기한 상태 오류")? = Some(work.deadline);
                        let result = execute_context(control, rpc, thread_id, request, cancelled, work.send_context.as_ref());
                        rpc.deadline = None; *deadline.lock().map_err(|_| "대화 요청 기한 상태 오류")? = None;
                        result
                    },
                };
                let _ = work.reply.send(result);
            }
            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Err(mpsc::TryRecvError::Empty) => if let Some(message) = rpc.read()? { observe_message(control, thread_id, message)?; },
        }
    }
}
fn valid_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) { Err("대화 항목 식별자가 올바르지 않습니다.".into()) } else { Ok(()) }
}
fn cursor(value: &Option<String>) -> Result<(), String> {
    if value.as_ref().is_some_and(|v| v.len() > 4096) { Err("대화 페이지 위치가 상한을 넘었습니다.".into()) } else { Ok(()) }
}
fn validate_request(request: &ConversationRequest) -> Result<(), String> {
    match request {
        ConversationRequest::ContextComposition { expected_thread_id, snapshot_id, cursor } => {
            valid_id(expected_thread_id)?;
            composition::validate(snapshot_id.as_deref(), cursor.unwrap_or(0))
        },
        ConversationRequest::ReloadMcp { expected_thread_id } | ConversationRequest::McpStatus { expected_thread_id, .. } | ConversationRequest::HooksStatus { expected_thread_id } => valid_id(expected_thread_id),
        ConversationRequest::SetContextWindow {model_context_window,expected_model_context_window} => {context::validate_window(*model_context_window)?;context::validate_window(*expected_model_context_window)},
        ConversationRequest::Page {cursor: value} | ConversationRequest::Models {cursor: value} => cursor(value),
        ConversationRequest::Items {turn_id,cursor: value} => {valid_id(turn_id)?;cursor(value)},
        ConversationRequest::Detail {turn_id,item_id,snapshot_id,..} => {valid_id(turn_id)?;valid_id(item_id)?;if let Some(id)=snapshot_id {valid_id(id)?;}Ok(())},
        ConversationRequest::Interrupt {turn_id} => valid_id(turn_id),
        ConversationRequest::Send {request_id,text,attachments,model,effort} => {
            Uuid::parse_str(request_id).map_err(|_| "전송 식별자는 UUID여야 합니다.")?;
            if text.len() > 64*1024 || attachments.len() > 16 || attachments.iter().any(|item| item.path.len() > 32768 || item.r#type.len() > 32)
                || model.as_ref().is_some_and(|v| v.is_empty() || v.len() > 256) || effort.as_ref().is_some_and(|v| v.len() > 64) {
                return Err("대화 전송 요청이 허용 크기를 넘었습니다. 대기열에 넣지 않았습니다.".into());
            }
            Ok(())
        },
        ConversationRequest::Respond {request_id,result} => {
            if request_id.len()>512 {return Err("대화 응답 식별자가 허용 크기를 넘었습니다.".into());}
            let limit=if async_questions::is_question_id(request_id) {512}else{256};
            if request_id.len() > limit || !result.is_object() || result.to_string().len() > 64*1024 {return Err("대화 응답 요청이 허용 크기를 넘었습니다. 대기열에 넣지 않았습니다.".into());}
            Ok(())
        },
        _ => Ok(()),
    }
}
fn bounded_page(mut value: Value, limit: usize) -> Result<Value, String> {
    if value["historyUnavailable"] == true { return Err(HISTORY_UNAVAILABLE.into()); }
    if value["data"].as_array().is_none_or(|data| data.len() > limit) { return Err("대화 페이지가 요청 상한을 넘었거나 형식이 올바르지 않습니다.".into()); }
    summarize(&mut value);
    Ok(value)
}
fn summarize(value: &mut Value) {
    match value {
        Value::String(text) if text.len() > TEXT_PREVIEW => *value = json!({"__detail":true,"bytes":text.len()}),
        Value::Array(values) => for value in values { summarize(value); },
        Value::Object(values) => for value in values.values_mut() { summarize(value); },
        _ => {},
    }
}
#[cfg(test)]
fn execute<S: Read + Write>(control: &Control, rpc: &mut Rpc<S>, thread_id: &str, request: ConversationRequest, cancelled: &AtomicBool) -> Result<Value, String> {
    execute_context(control,rpc,thread_id,request,cancelled,None)
}
fn async_reply_available(control: &Control,events: &Events,observer: bool) -> Result<(),String> {
    if !control.passive_supported || control.stopped.load(Ordering::Acquire) || control.deleted.load(Ordering::Acquire)
        || !events.observer_connected || events.requests_unavailable || !events.async_questions.complete()
        || control.unknown.load(Ordering::Acquire) || events.context_compaction.is_some()
        || control.owned.load(Ordering::Acquire)==observer || (!observer && !events.connected) {
        return Err("현재 입력 소유권과 질문 관측 연결을 확인하지 못해 답변을 보내지 않았습니다.".into());
    }
    Ok(())
}
fn execute_async_reply<S: Read+Write>(control: &Control,rpc: &mut Rpc<S>,thread_id: &str,id: &str,result: &Value,cancelled: &AtomicBool,observer: bool) -> Result<Value,String> {
    live(control,cancelled)?;
    {
        let events=control.events.lock().map_err(|_|"질문 관측 상태 오류")?;
        async_reply_available(control,&events,observer)?;
        events.async_questions.reply_payload(id,result)?;
    }
    // The writer consumes already queued notifications before committing an
    // answer. Continuous traffic has an explicit bound; it cannot starve the
    // terminal or turn an incomplete drain into permission to send.
    let mut drained=false;
    for _ in 0..EVENT_COUNT {
        live(control,cancelled)?;
        check_cancelled(rpc.cancelled.as_ref())?;
        if rpc.deadline.is_some_and(|deadline|Instant::now()>=deadline) {return Err("질문 답변 기한이 지나 전송하지 않았습니다.".into());}
        match rpc.ws.read() {
            Ok(Message::Text(text))=>{
                if text.len()>MAX_FRAME {return Err("Codex protocol frame exceeded limit".into());}
                let message=serde_json::from_str(&text).map_err(|error|format!("Invalid Codex JSON: {error}"))?;
                record_message(control,thread_id,message,observer)?;
            },
            Ok(Message::Close(_))=>return Err("Codex app-server disconnected".into()),
            Ok(_)=>{},
            Err(error) if timed_out(&error)=>{drained=true;break;},
            Err(error)=>return Err(format!("Codex transport read failed: {error}")),
        }
    }
    if !drained {return Err("질문 알림 처리 상한에 도달해 현재 상태를 확인한 뒤 다시 답변하세요. 전송하지 않았습니다.".into());}
    let (turn,text)={
        let mut events=control.events.lock().map_err(|_|"질문 관측 상태 오류")?;
        async_reply_available(control,&events,observer)?;
        events.async_questions.prepare_reply(id,result)?
    };
    let response=call_observed(control,rpc,thread_id,"turn/steer",json!({"threadId":thread_id,"expectedTurnId":turn,"clientUserMessageId":Uuid::new_v4().to_string(),"input":[{"type":"text","text":text,"text_elements":[]}]}),cancelled,observer,None,None,None,Some((id,&turn)))?;
    if response["turnId"].as_str()!=Some(turn.as_str()) {return Err("비동기 질문 답변의 접수 결과를 확인할 수 없습니다. 다시 보내지 않았습니다.".into());}
    Ok(json!({"status":"accepted","resolved":false,"turnId":turn}))
}
fn execute_compact<S: Read+Write>(control: &Control,rpc: &mut Rpc<S>,thread_id: &str,cancelled: &AtomicBool,metadata_only: bool) -> Result<Value,String> {
    live(control,cancelled)?;
    let events=control.events.lock().map_err(|_| "대화 문맥 상태 오류")?;
    if let Some(reason)=context_block_reason(control,&events,false) {return Ok(json!({"status":if reason=="unsupported" {"unsupported"}else{"notStarted"},"reason":reason}));}
    let activity=events.context_activity_revision;drop(events);
    let mut progress=CompactProgress::default();
    let result=call_observed(control,rpc,thread_id,COMPACT_IF_IDLE,json!({"threadId":thread_id}),cancelled,metadata_only,None,None,Some(&mut progress),None);
    let mut events=control.events.lock().map_err(|_| "대화 문맥 상태 오류")?;
    let value=match result {Ok(value)=>value,Err(error)=>{events.token_usage_fresh=false;events.context_revision+=1;return Err(error);}};
    match value["status"].as_str() {
        Some("unsupported") if !control.unknown.load(Ordering::Acquire)=>{if metadata_only {events.observer_compact_supported=false;}else{events.gui_compact_supported=false;}events.context_revision+=1;Ok(value)},
        Some("skippedBusy")=>Ok(value),
        Some("notStarted") if value["reason"].is_string()=>Ok(value),
        Some("started") if value["turn"]["id"].as_str().is_some_and(|id|valid_id(id).is_ok())=>{
            let turn=value["turn"]["id"].as_str().unwrap();
            // 같은 관찰자의 더 최근 상태를 접수 응답으로 되돌리지 않는다.
            // GUI에서 먼저 온 알림은 이번 RPC 동안의 작은 요약으로만 대조한다.
            if events.context_activity_revision==activity {
                match progress.turn {
                    // 화면에서 먼저 온 알림의 활동/완료를 관찰자 전역 상태로
                    // 승격하지 않는다. 원래 관찰자가 확인할 때까지 미확인이다.
                    Some(_)=>{events.context_active=None;events.context_turn=None;events.context_compaction=None;},
                    None=>{events.context_active=Some(true);events.context_turn=Some(turn.into());events.context_compaction=Some((turn.into(),String::new()));},
                }
                events.token_usage_fresh=false;events.context_activity_revision+=1;
            }
            events.context_revision+=1;Ok(value)
        },
        _=>{control.unknown.store(true,Ordering::Release);events.token_usage_fresh=false;events.context_revision+=1;Err("압축 접수 결과를 확인할 수 없습니다. 자동으로 다시 요청하지 않았습니다.".into())},
    }
}
fn execute_context<S: Read + Write>(control: &Control, rpc: &mut Rpc<S>, thread_id: &str, request: ConversationRequest, cancelled: &AtomicBool,send_context: Option<&SendContext>) -> Result<Value, String> {
    match request {
        ConversationRequest::Page {cursor: page_cursor} => {
            cursor(&page_cursor)?;
            let value=call(control,rpc,thread_id,"thread/turns/list",json!({"threadId":thread_id,"cursor":page_cursor,"limit":20,"sortDirection":"desc","itemsView":"summary"}),cancelled)?;
            if page_cursor.is_none() && value["historyUnavailable"]==true && value["reason"]=="unmaterialized" {Ok(value)} else {bounded_page(value,20)}
        },
        ConversationRequest::Items {turn_id,cursor: page_cursor} => { valid_id(&turn_id)?; cursor(&page_cursor)?; bounded_page(call(control,rpc,thread_id,"thread/items/list",json!({"threadId":thread_id,"turnId":turn_id,"cursor":page_cursor,"limit":20,"sortDirection":"desc"}),cancelled)?,20) },
        ConversationRequest::Detail {turn_id,item_id,offset,snapshot_id} => {
            valid_id(&turn_id)?; valid_id(&item_id)?;
            let offset=offset.unwrap_or(0);
            if offset==0 {
                control.detail.lock().map_err(|_| "대화 상세 스냅샷 상태 오류")?.take();
                let result = call(control,rpc,thread_id,"thread/items/read",json!({"threadId":thread_id,"turnId":turn_id,"itemIds":[item_id]}),cancelled)?;
                if result["historyUnavailable"] == true { return Err(HISTORY_UNAVAILABLE.into()); }
                let data = result["data"].as_array().filter(|data| data.len() == 1).ok_or("상세 대화 항목을 찾을 수 없습니다.")?;
                if data[0]["turnId"] != turn_id || data[0]["item"]["id"] != item_id { return Err("다른 대화 항목의 상세 응답을 거부했습니다.".into()); }
                let text=serde_json::to_string(&data[0]["item"]).map_err(|e|e.to_string())?;
                if text.len()>MAX_FRAME {return Err("상세 항목이 8MiB 프로토콜 상한을 넘었습니다. 원문을 잘라내지 않았습니다.".into());}
                *control.detail.lock().map_err(|_| "대화 상세 스냅샷 상태 오류")?=Some(DetailSnapshot{id:Uuid::new_v4().to_string(),turn_id:turn_id.clone(),item_id:item_id.clone(),text:Arc::from(text)});
            }
            let (id,text)={
                let detail=control.detail.lock().map_err(|_| "대화 상세 스냅샷 상태 오류")?;
                let detail=detail.as_ref().filter(|detail| detail.turn_id==turn_id&&detail.item_id==item_id&&(offset==0||snapshot_id.as_ref()==Some(&detail.id))).ok_or("상세 스냅샷이 교체되었거나 만료되었습니다. 처음부터 다시 조회하세요.")?;
                (detail.id.clone(),detail.text.clone())
            };
            let mut page=detail_chunk(&text,offset)?; page["snapshotId"]=json!(id);
            if page["nextOffset"].is_null() {control.detail.lock().map_err(|_| "대화 상세 스냅샷 상태 오류")?.take();}
            Ok(page)
        },
        ConversationRequest::Compact => execute_compact(control,rpc,thread_id,cancelled,false),
        ConversationRequest::Models {cursor: page_cursor} => { cursor(&page_cursor)?; bounded_page(call(control,rpc,thread_id,"model/list",json!({"cursor":page_cursor,"limit":50,"includeHidden":false}),cancelled)?,50) },
        ConversationRequest::Send {request_id,text,attachments,model,effort} => {
            if control.unknown.load(Ordering::Acquire) { return Err("이전 전송 결과가 불명확하여 새 전송을 차단했습니다. 원래 대화 상태를 먼저 확인하세요.".into()); }
            let input = user_input(&text, &attachments)?;
            if model.as_ref().is_some_and(|v| v.is_empty() || v.len() > 256) || effort.as_ref().is_some_and(|v| v.len() > 64) { return Err("모델 또는 추론 설정이 올바르지 않습니다.".into()); }
            // 서버의 원자적 사용자 입장만 사용한다. 상태 조회와 start 사이의 외부 입력 경쟁을 만들지 않는다.
            let mut params = json!({"threadId":thread_id,"input":input,"clientUserMessageId":request_id});
            if let Some(model) = model { params["model"] = json!(model); }
            if let Some(effort) = effort { params["effort"] = json!(effort); }
            let result=call_observed(control,rpc,thread_id,START_USER_IF_IDLE,params,cancelled,false,send_context.map(|context|(context,attachments.as_slice())),None,None,None)?;
            match result["status"].as_str() {
                Some("started") if result["turn"]["id"].as_str().is_some_and(|id| !id.is_empty()) => Ok(result),
                Some("skippedBusy") => Err("다른 입력이 먼저 시작되어 새 메시지를 보내지 않았습니다. 초안을 보존했습니다.".into()),
                Some("notStarted") => Err(format!("새 메시지가 수락되지 않았습니다. 초안을 보존했습니다. {}",result["reason"].as_str().unwrap_or("사유 미확인"))),
                _ => {control.unknown.store(true,Ordering::Release);Err("서버의 새 메시지 접수 결과 형식이 올바르지 않아 실행 여부를 확인할 수 없습니다. 재전송하지 않았습니다.".into())},
            }
        },
        ConversationRequest::Interrupt {turn_id} => {
            valid_id(&turn_id)?;live(control,cancelled)?;
            {let events=control.events.lock().map_err(|_| "대화 중단 지원 상태 오류")?;
                if !events.connected || !events.gui_interrupt_supported {return Err(events.interrupt_diagnostic.unwrap_or("현재 연결에서 안전한 작업 중단을 지원하는지 확인되지 않아 요청을 보내지 않았습니다.").into());}
            }
            call(control,rpc,thread_id,"turn/interrupt",json!({"threadId":thread_id,"turnId":turn_id}),cancelled)
        },
        ConversationRequest::Respond {request_id,result} => {
            if async_questions::is_question_id(&request_id) {
                return execute_async_reply(control,rpc,thread_id,&request_id,&result,cancelled,false);
            }
            let mut events = control.events.lock().map_err(|_| "대화 승인 상태 오류")?;
            let request = events.pending.get(&request_id).ok_or("이미 해결되었거나 이 연결이 소유하지 않은 요청입니다.")?;
            if request["responded"] == true { return Err("이미 응답을 보낸 요청입니다. 같은 응답을 다시 보내지 않았습니다.".into()); }
            if request["method"]=="mcpServer/elicitation/request" && (control.deleted.load(Ordering::Acquire)
                || control.stopped.load(Ordering::Acquire) || cancelled.load(Ordering::Acquire) || events.requests_unavailable) {
                return Err("도구 승인 연결이 종료되었거나 요청 상태가 미확인이므로 응답하지 않았습니다.".into());
            }
            if request["method"]=="mcpServer/elicitation/request" && (request["params"]["threadId"]!=thread_id || request["guiOwned"]!=true) {
                return Err("검토한 도구 승인의 대화 또는 입력 소유권이 변경되어 응답하지 않았습니다.".into());
            }
            validate_response(request, &result)?;
            if events.answered.len() >= 1024 { return Err("서버 요청 응답 기록 상한에 도달해 추가 응답을 보내지 않았습니다.".into()); }
            // 전송 전에 소유권을 소비한다. 쓰기 실패·연결 단절 후 같은 응답을 재전송하지 않는다.
            events.pending.get_mut(&request_id).unwrap()["responded"] = json!(true);
            events.answered.insert(request_id.clone()); drop(events);
            let id: Value = serde_json::from_str(&request_id).map_err(|_| "승인 요청 식별자 오류")?;
            rpc.send(json!({"id":id,"result":result})).map_err(|_| "승인 응답 전송 결과를 확인할 수 없습니다. 다시 보내지 않았습니다.".to_string())?;
            Ok(json!({}))
        },
        _ => Err("이 대화 요청은 현재 연결에서 실행할 수 없습니다.".into()),
    }
}
fn user_input(text: &str, attachments: &[Attachment]) -> Result<Vec<Value>, String> {
    if text.len() > 64 * 1024 || attachments.len() > 16 { return Err("메시지 또는 첨부 개수가 상한을 넘었습니다.".into()); }
    let mut input = Vec::new();
    if !text.trim().is_empty() { input.push(json!({"type":"text","text":text,"text_elements":[]})); }
    for attachment in attachments {
        if !matches!(attachment.r#type.as_str(), "localImage" | "file") { return Err("이 첨부 형식은 지원하지 않습니다. 로컬 이미지 또는 파일 참조만 사용할 수 있습니다.".into()); }
        if attachment.path.len() > 32768 || attachment.path.chars().any(char::is_control) { return Err("첨부 경로가 올바르지 않습니다.".into()); }
        let path = PathBuf::from(&attachment.path);
        if !path.is_absolute() || !path.is_file() { return Err("첨부는 존재하는 로컬 파일의 절대 경로여야 합니다.".into()); }
        if attachment.r#type == "localImage" {
            if !path.extension().and_then(|v| v.to_str()).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif")) { return Err("지원하지 않는 로컬 이미지 형식입니다.".into()); }
            input.push(json!({"type":"localImage","path":path}));
        } else {
            input.push(json!({"type":"mention","name":path.file_name().and_then(|s| s.to_str()).unwrap_or("file"),"path":attachment.path}));
        }
    }
    if input.is_empty() { return Err("전송할 메시지 또는 첨부가 없습니다.".into()); }
    Ok(input)
}
fn detail_chunk(text: &str, offset: usize) -> Result<Value, String> {
    if offset > text.len() || !text.is_char_boundary(offset) { return Err("상세 조회 위치가 올바르지 않습니다.".into()); }
    let mut end = (offset + DETAIL_BYTES).min(text.len());
    while !text.is_char_boundary(end) { end -= 1; }
    Ok(json!({"text":&text[offset..end],"nextOffset":if end < text.len() {Some(end)} else {None},"totalBytes":text.len()}))
}
fn context_active(status: &Value) -> Option<bool> {
    match status["type"].as_str() {Some("idle")=>Some(false),Some("active")=>Some(true),_=>None}
}
fn context_block_reason(control: &Control, events: &Events, closing: bool) -> Option<&'static str> {
    if control.deleted.load(Ordering::Acquire) {Some("deleted")}
    else if closing {Some("closing")}
    else if control.unknown.load(Ordering::Acquire) {Some("unknownResult")}
    else if events.requests_unavailable || !events.pending.is_empty() || !events.observed_pending.is_empty() {Some("pendingRequest")}
    else if events.context_compaction.is_some() {Some("compacting")}
    else if !control.compact_supported {Some("unsupported")}
    else if !events.connected && !events.observer_connected {Some("disconnected")}
    else if !events.connected && !events.observer_compact_supported {Some("unsupported")}
    else if events.context_active==Some(true) {Some("active")}
    else if events.context_active.is_none() {Some("statusUnknown")}
    else if events.connected && !events.gui_compact_supported {Some("unsupported")}
    else {None}
}
fn observe_message(control: &Control, thread_id: &str, message: Value) -> Result<(), String> {
    record_message(control,thread_id,message,false)
}
fn record_message(control: &Control, thread_id: &str, message: Value, metadata_only: bool) -> Result<(), String> {
    let Some(method) = message["method"].as_str() else { return Ok(()); };
    if metadata_only && message.get("id").is_none() && !matches!(method,"serverRequest/resolved" | "thread/deleted" | "thread/tokenUsage/updated" | "thread/settings/updated" | "model/rerouted" | "thread/status/changed" | "turn/started" | "turn/completed" | "item/started" | "item/completed") {return Ok(());}
    let params = &message["params"];
    let selected = params["threadId"].as_str().or_else(|| params["thread"]["id"].as_str());
    if selected != Some(thread_id) { return Ok(()); }
    let mut events = control.events.lock().map_err(|_| "대화 이벤트 상태 오류")?;
    if method == "thread/deleted" {
        if control.deleted.swap(true,Ordering::AcqRel) { return Err("원래 대화가 이미 삭제되었습니다.".into()); }
        events.pending.clear(); events.observed_pending.clear(); events.connected=false; events.observer_connected=false;events.gui_compact_supported=false;events.observer_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;
        events.token_usage=None;events.context_active=None;events.context_compaction=None;events.context_turn=None;events.context_activity_revision+=1;events.context_revision+=1;
        events.queue.clear(); events.bytes=0;
        events.async_questions.clear();
        events.error=Some("원래 대화가 삭제되었습니다. 이전 이력과 상세 캐시를 사용하지 않습니다.".into());
        events.push(json!({"method":method,"params":params}));
        drop(events);
        control.detail.lock().map_err(|_| "대화 상세 스냅샷 상태 오류")?.take();
        // 요청 결과 불명은 삭제 알림만으로 해결됐다고 가정하지 않는다. 소유권은 worker join 후 해제한다.
        return Err("원래 대화가 삭제되어 수동 구독을 종료했습니다.".into());
    }
    // The observer is the canonical ordered stream. A GUI copy must not mix
    // arrival order with it; when disconnected, retained metadata stays stale.
    if metadata_only && message.get("id").is_none() {
        events.async_questions.observe(method,params);
    }
    // 사용량과 그 무효화는 기존 관찰 스트림 하나만 환원한다. GUI 복사본을
    // 섞지 않아 서로 다른 연결의 도착 순번을 전역 최신 순서로 오인하지 않는다.
    if metadata_only && message.get("id").is_none() {
        match method {
            "thread/tokenUsage/updated" => {
                let usage=params["turnId"].as_str().filter(|id|valid_id(id).is_ok()).and_then(|_|serde_json::from_value::<ContextTokenUsage>(params["tokenUsage"].clone()).ok())
                    .filter(|usage|usage.total.valid()&&usage.last.valid()&&usage.model_context_window.map_or(true,|window|(1..=MAX_CONTEXT_INTEGER).contains(&window)));
                events.token_usage_fresh=usage.as_ref().is_some_and(|usage|usage.model_context_window.is_some())&&events.context_compaction.is_none();
                events.token_usage=usage;events.context_revision+=1;
            },
            "thread/settings/updated" | "model/rerouted" => {events.token_usage_fresh=false;events.context_revision+=1;},
            "thread/status/changed" => {events.context_active=context_active(&params["status"]);events.context_activity_revision+=1;events.context_revision+=1;},
            "turn/started" => {events.context_active=Some(true);events.context_turn=params["turn"]["id"].as_str().filter(|id|valid_id(id).is_ok()).map(str::to_owned);events.context_activity_revision+=1;events.context_revision+=1;},
            "turn/completed" => {
                let turn=params["turn"]["id"].as_str().or_else(||params["turnId"].as_str());
                if events.context_compaction.as_ref().is_some_and(|(id,_)|Some(id.as_str())==turn) {events.token_usage_fresh=false;events.context_compaction=None;}
                if events.context_compaction.is_none() && events.context_turn.as_deref().is_none_or(|id|Some(id)==turn) {events.context_active=Some(false);}
                events.context_activity_revision+=1;events.context_revision+=1;
            },
            "item/started" | "item/completed" if params["item"]["type"]=="contextCompaction" => {
                if let (Some(turn),Some(item))=(params["turnId"].as_str(),params["item"]["id"].as_str()) {
                    if valid_id(turn).is_ok()&&valid_id(item).is_ok() {
                        let key=(turn.to_owned(),item.to_owned());
                        events.token_usage_fresh=false;
                        if method=="item/started" {events.context_compaction=Some(key);}
                        else if events.context_compaction.as_ref().is_some_and(|(id,old_item)|id==turn&&(old_item.is_empty()||old_item==item)) {events.context_compaction=None;}
                        events.context_activity_revision+=1;events.context_revision+=1;
                    }
                }
            },
            _=>{},
        }
    }
    if metadata_only && message.get("id").is_none() && method!="serverRequest/resolved" {return Ok(());}
    if method == "serverRequest/resolved" {
        if let Some(id) = params.get("requestId") {
            let key=id.to_string();
            if key.len()>256 || (!id.is_string()&&!id.is_i64()&&!id.is_u64()) {return Err("해결 요청 식별자 형식을 확인할 수 없어 관측을 중단했습니다.".into());}
            events.pending.remove(&key);events.observed_pending.remove(&key);
            // replay는 서버가 복사한 뒤 이미 해결됐을 수 있다. 안전한 eviction 근거가
            // 없으므로 해결 ID를 세션 동안 보존하고 상한에서는 명시 중단한다.
            if !events.answered.contains(&key) && events.answered.len()>=1024 {
                let error="해결 요청 기록 상한에 도달해 늦은 사본을 구분할 수 없습니다. 관측을 중단했습니다.";
                events.requests_unavailable=true;events.connected=false;events.observer_connected=false;events.gui_compact_supported=false;events.observer_compact_supported=false;events.gui_interrupt_supported=false;events.interrupt_diagnostic=None;events.error=Some(error.into());return Err(error.into());
            }
            events.answered.insert(key);
        }
    }
    if events.requests_unavailable {return Err("서버 요청 관측을 중단한 세션입니다. 마지막 확인 내용을 보존했습니다.".into());}
    let mut event = json!({"method":method,"params":params});
    if let Some(id) = message.get("id") {
        let key = id.to_string();
        if events.answered.contains(&key) { return Ok(()); }
        if key.len()>256 {return Err("서버 요청 식별자가 허용 크기를 넘었습니다. 응답을 보내지 않았습니다.".into());}
        if !id.is_string() && !id.is_i64() && !id.is_u64() { return Err("서버 요청 식별자 형식이 올바르지 않습니다.".into()); }
        let count = events.pending.keys().chain(events.observed_pending.keys()).collect::<HashSet<_>>().len();
        if !events.pending.contains_key(&key) && !events.observed_pending.contains_key(&key) && count >= REQUEST_COUNT { return Err("처리 대기 서버 요청이 상한을 넘었습니다. 응답을 자동 승인하지 않았습니다.".into()); }
        if event.to_string().len() > 64 * 1024 { return Err("서버 승인 요청이 표시 상한을 넘었습니다. 응답을 보내지 않았습니다.".into()); }
        event["requestId"] = json!(key);
        event["guiOwned"] = json!(!metadata_only);
        if method=="mcpServer/elicitation/request" {
            // Keep the original wire-size check above; add only eligibility, not a second argument copy.
            event["mcpToolApprovalAvailable"]=json!(ConversationConnection::mcp_tool_approval(params).is_some());
        }
        if metadata_only {events.observed_pending.entry(key).or_insert(event);return Ok(());}
        if let Some(previous)=events.pending.get_mut(&key) {
            if previous["guiOwned"]==false {
                if previous["method"]!=event["method"] || previous["params"]!=event["params"] {return Err("인계한 요청과 재구독 원문이 달라 이전 요청을 보존했습니다.".into());}
                previous["guiOwned"]=json!(true);
            }
            return Ok(());
        }
        events.pending.insert(key,event.clone());
    } else {
        summarize(&mut event);
    }
    if !metadata_only {events.push(event);}
    Ok(())
}
fn mcp_arguments_reviewable(value: &Value, depth: usize) -> bool {
    if depth>64 {return false;}
    match value {
        Value::Number(number)=>{
            // Inspect original integer variants before any magnitude comparison through f64.
            // safety ceiling: finite fractions remain supported, but unsafe magnitudes and -0 do not.
            let Some(float)=number.as_f64() else {return false;};
            if !float.is_finite() || (float==0.0 && float.is_sign_negative()) {false}
            else if let Some(integer)=number.as_u64() {integer<=MCP_APPROVAL_MAX_SAFE_INTEGER}
            else if let Some(integer)=number.as_i64() {
                (-(MCP_APPROVAL_MAX_SAFE_INTEGER as i64)..=MCP_APPROVAL_MAX_SAFE_INTEGER as i64).contains(&integer)
            }
            else {float.abs()<=MCP_APPROVAL_MAX_SAFE_INTEGER as f64}
        },
        Value::Object(fields)=>fields.get("__detail")!=Some(&json!(true)) && fields.get("truncated")!=Some(&json!(true)) && fields.get("hasMore")!=Some(&json!(true))
            && fields.values().all(|value|mcp_arguments_reviewable(value,depth+1)),
        Value::Array(values)=>values.iter().all(|value|mcp_arguments_reviewable(value,depth+1)),
        _=>true,
    }
}
fn validate_response(request: &Value, result: &Value) -> Result<(), String> {
    if result.to_string().len() > 64 * 1024 || !result.is_object() { return Err("서버 요청 응답 형식 또는 크기가 올바르지 않습니다.".into()); }
    match request["method"].as_str() {
        Some("mcpServer/elicitation/request") => {
            if ConversationConnection::mcp_tool_approval(&request["params"]).is_some()
                && result.as_object().is_some_and(|value|value.len()==3 && value.contains_key("content") && value.contains_key("_meta"))
                && matches!(result["action"].as_str(),Some("accept"|"decline"|"cancel")) && result["content"].is_null() && result["_meta"].is_null() {Ok(())}
            else {Err("검토 가능한 MCP 도구 요청의 일회 허용·거절·취소만 응답할 수 있습니다. 저장 또는 권한 확대 응답은 보내지 않았습니다.".into())}
        },
        Some("item/commandExecution/requestApproval" | "item/fileChange/requestApproval") => {
            if result.as_object().is_some_and(|v| v.len() == 1) && result["decision"].as_str().is_some_and(|decision|ConversationConnection::approval_decisions(&request["params"]).contains(&decision)) { Ok(()) }
            else { Err("지원하지 않는 승인 결정입니다. 권한·정책 변경 응답은 보내지 않았습니다.".into()) }
        },
        Some("item/tool/requestUserInput") => {
            let answers = result["answers"].as_object().ok_or("질문 응답 목록이 올바르지 않습니다.")?;
            let questions = request["params"]["questions"].as_array().ok_or("질문 목록이 올바르지 않습니다.")?;
            if answers.len() != questions.len() || questions.iter().any(|question| {
                question["id"].as_str().and_then(|id| answers.get(id)).and_then(|value| value["answers"].as_array())
                    .is_none_or(|values| values.is_empty() || values.iter().any(|value| !value.is_string()))
            }) { return Err("모든 질문에 문자열 답변을 제공해야 합니다.".into()); }
            Ok(())
        },
        _ => Err("현재 화면에서 지원하지 않는 서버 요청입니다. 자동 응답하지 않았습니다.".into()),
    }
}

#[cfg(test)]
#[path = "codex_conversation_protocol_tests.rs"]
mod tests;
