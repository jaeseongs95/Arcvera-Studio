//! Outbound-only relay transport. No local listener, stored secret or independent session engine.
use reqwest::{header::{HeaderMap, HeaderValue}, Client, Url};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, VecDeque}, future::Future, pin::Pin, sync::{atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}, Arc, Mutex}, time::Duration};
use tauri::{Emitter, State};
use tokio::sync::{mpsc, Semaphore};

const MAX_FRAME: usize = 16 * 1024 * 1024;
const MAX_REPLY: usize = 4 * 1024 * 1024;
const MAX_CACHE: usize = 16 * 1024 * 1024;
const MAX_EVENTS: usize = 4 * 1024 * 1024;
type Dispatch = Arc<dyn Fn(String, Value) -> Pin<Box<dyn Future<Output=Result<Value,String>> + Send>> + Send + Sync>;
type Notify = Arc<dyn Fn(RelayStatus) + Send + Sync>;

#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct RelayStatus {
    generation: u64,
    revision: u64,
    state: &'static str,
    url: Option<String>,
    error: Option<String>,
    release_confirmed: Option<bool>,
    pending_commands: usize,
}
impl Default for RelayStatus {
    fn default() -> Self { Self { generation:0,revision:0,state:"disconnected",url:None,error:None,release_confirmed:None,pending_commands:0 } }
}
struct Active {
    connection: Arc<Connection>,
    stream: tauri::async_runtime::JoinHandle<()>,
    events: tauri::async_runtime::JoinHandle<()>,
    sender: mpsc::Sender<Vec<u8>>,
}
pub(crate) struct RelayState {
    active: Mutex<Option<Active>>,
    status: Arc<Mutex<RelayStatus>>,
    generation: AtomicU64,
    // Shared across reconnects: disconnecting cannot bypass the native work cap.
    permits: Arc<Semaphore>,
    lifecycle: tokio::sync::Mutex<()>,
}
impl Default for RelayState {
    fn default() -> Self { Self { active:Mutex::new(None),status:Arc::new(Mutex::new(RelayStatus::default())),generation:AtomicU64::new(0),permits:Arc::new(Semaphore::new(4)),lifecycle:tokio::sync::Mutex::new(()) } }
}

pub(crate) fn normalize_url(raw: &str) -> Result<Url,String> {
    let raw=raw.trim();
    if raw.len()>4096 { return Err("중계기 주소가 너무 깁니다.".into()); }
    let mut url=Url::parse(raw).map_err(|_| "올바른 중계기 주소를 입력하세요.")?;
    let scheme=match url.scheme() { "wss"=>"https", "ws"=>"http", "http"=>"http", "https"=>"https", _=>return Err("https:// 또는 루프백 http:// 주소를 입력하세요.".into()) };
    // URL cannot change between special and non-special schemes, so reparse ws/wss.
    if url.scheme()!=scheme {
        url=Url::parse(&format!("{scheme}:{}",raw.split_once(':').ok_or("잘못된 주소")?.1)).map_err(|_| "중계기 주소를 변환하지 못했습니다.")?;
    }
    if !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() || url.port()==Some(0) {
        return Err("사용자 정보가 없는 중계기 주소를 입력하세요.".into());
    }
    let host=url.host_str().unwrap().trim_matches(['[',']']);
    let local=host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback());
    if url.scheme()=="http" && !local { return Err("외부 중계기는 HTTPS가 필요합니다.".into()); }
    url.set_query(None); url.set_fragment(None);
    let pathname=url.path().to_owned();
    let path=pathname.to_ascii_lowercase().find("/api/relay").map(|i|&pathname[..i]).unwrap_or(&pathname).trim_end_matches('/');
    if !path.is_empty() { return Err("중계기 기본 주소를 입력하세요. 별도 경로는 지원하지 않습니다.".into()); }
    url.set_path("/");
    Ok(url)
}

#[derive(Default)]
pub(crate) struct SseDecoder { line:Vec<u8>, data:Vec<u8> }
impl SseDecoder {
    pub(crate) fn push(&mut self, bytes:&[u8]) -> Result<Vec<Value>,String> {
        let mut messages=Vec::new();
        for byte in bytes {
            if *byte!=b'\n' {
                self.line.push(*byte);
                if self.line.len()+self.data.len()>MAX_FRAME { return Err("중계기 프레임 크기 상한을 초과했습니다.".into()); }
                continue;
            }
            if self.line.last()==Some(&b'\r') { self.line.pop(); }
            if self.line.is_empty() {
                if !self.data.is_empty() {
                    self.data.pop();
                    messages.push(serde_json::from_slice(&self.data).map_err(|_| "중계기 SSE 데이터가 올바른 JSON이 아닙니다.")?);
                    self.data.clear();
                    if messages.len()>64 { return Err("한 번에 수신한 원격 요청이 너무 많습니다.".into()); }
                }
            } else if let Some(data)=self.line.strip_prefix(b"data:") {
                let data=data.strip_prefix(b" ").unwrap_or(data);
                self.data.extend_from_slice(data); self.data.push(b'\n');
            }
            self.line.clear();
        }
        Ok(messages)
    }
}

#[derive(Default)]
pub(crate) struct Calls {
    in_flight: HashMap<String,(Vec<u8>,usize)>,
    completed: VecDeque<(String,Vec<u8>,Value,usize)>,
    bytes:usize,
}
pub(crate) enum Admission { New, Duplicate, Replay(Value), Refused }
impl Calls {
    pub(crate) fn admit(&mut self,id:&str,fingerprint:&[u8],bytes:usize)->Admission {
        if let Some((old,_))=self.in_flight.get(id) { return if old==fingerprint {Admission::Duplicate} else {Admission::Refused}; }
        if let Some((_,old,reply,_))=self.completed.iter().find(|(key,_,_,_)|key==id) {
            return if old==fingerprint {Admission::Replay(reply.clone())} else {Admission::Refused};
        }
        if self.in_flight.len()>=4 || self.in_flight.values().map(|(_,size)|size).sum::<usize>()+bytes>MAX_FRAME { return Admission::Refused; }
        self.in_flight.insert(id.to_owned(),(fingerprint.to_vec(),bytes)); Admission::New
    }
    pub(crate) fn finish(&mut self,id:String,reply:Value) {
        let Some((fingerprint,_))=self.in_flight.remove(&id) else {return};
        let bytes=serde_json::to_vec(&reply).map(|s|s.len()).unwrap_or(MAX_CACHE+1);
        while self.completed.len()>=256 || self.bytes+bytes>MAX_CACHE {
            if let Some((_,_,_,size))=self.completed.pop_front() {self.bytes-=size;} else {break;}
        }
        if bytes<=MAX_CACHE { self.bytes+=bytes;self.completed.push_back((id,fingerprint,reply,bytes)); }
    }
}

struct Connection {
    epoch:u64, url:Url, client:Client, stopped:AtomicBool, connected:AtomicBool,
    queued:AtomicUsize, status:Arc<Mutex<RelayStatus>>, notify:Notify, dispatch:Dispatch,
    permits:Arc<Semaphore>, calls:Mutex<Calls>,
    host_id:String,
    network_abort:Mutex<Vec<tokio::task::AbortHandle>>,
}
impl Connection {
    fn status(&self,state:&'static str,error:Option<String>) {
        if state!="error" && self.stopped.load(Ordering::Acquire) {return;}
        if let Ok(mut status)=self.status.lock() {
            if status.generation!=self.epoch {return;}
            status.revision+=1;status.state=state;status.error=error;
            status.pending_commands=4-self.permits.available_permits();
            (self.notify)(status.clone());
        }
    }
    fn fatal(&self,error:String) {
        // A late POST failure must not overwrite explicit disconnection.
        if self.stopped.swap(true,Ordering::AcqRel) {return;}
        self.connected.store(false,Ordering::Release);
        self.status("error",Some(error));
        if let Ok(tasks)=self.network_abort.lock() {for task in tasks.iter() {task.abort();}}
    }
    async fn post(&self,endpoint:&str,bytes:Vec<u8>)->Result<(),String> {
        let target=self.url.join(endpoint).map_err(|_| "중계기 요청 주소 오류")?;
        let mut response=self.client.post(target).header("Content-Type","application/json; charset=utf-8")
            .timeout(Duration::from_secs(5)).body(bytes).send().await.map_err(|_| "중계기 전송 결과를 확인하지 못했습니다.")?;
        if response.status()!=200 { return Err(format!("중계기 요청 실패 (HTTP {})",response.status().as_u16())); }
        let mut body=Vec::new();
        while let Some(chunk)=response.chunk().await.map_err(|_| "중계기 응답 수신 실패")? {
            if body.len()+chunk.len()>65536 {return Err("중계기 응답 크기 초과".into());}body.extend_from_slice(&chunk);
        }
        let value:Value=serde_json::from_slice(&body).map_err(|_| "중계기 응답 형식 오류")?;
        if value.get("ok")!=Some(&Value::Bool(true)) {return Err("중계기가 요청을 수락하지 않았습니다.".into());} Ok(())
    }
    async fn reply(&self,reply:Value) {
        if self.stopped.load(Ordering::Acquire) || !self.connected.load(Ordering::Acquire) {return;}
        let body=match serde_json::to_vec(&reply) {Ok(body)=>body,Err(_)=>{self.fatal("원격 응답 직렬화 실패".into());return;}};
        if let Err(error)=self.post("/api/relay/reply",body).await {self.fatal(error);}
    }
    async fn invoke(self:&Arc<Self>,message:Value) -> Result<(),String> {
        let id=message.get("reqId").and_then(Value::as_str).filter(|id|uuid::Uuid::parse_str(id).is_ok()).ok_or("원격 요청 식별자 오류")?.to_owned();
        let command=message.get("command").and_then(Value::as_str).ok_or("원격 명령 이름 오류")?.to_owned();
        let args=message.get("args").cloned().unwrap_or(json!({}));
        if !args.is_object() {return Err("원격 명령 인수 형식 오류".into());}
        let input=serde_json::to_vec(&json!([command,args])).map_err(|_| "원격 요청 직렬화 실패")?;
        let fingerprint=Sha256::digest(&input).to_vec();
        let admission=self.calls.lock().map_err(|_| "원격 요청 상태 잠금 실패")?.admit(&id,&fingerprint,input.len());
        match admission {
            Admission::Duplicate=>return Ok(()),
            Admission::Replay(reply)=>{self.reply(reply).await;return Ok(());}
            Admission::Refused=>{self.reply(json!({"reqId":id,"ok":false,"error":"원격 요청이 충돌했거나 처리 상한에 도달했습니다. 새 작업은 실행하지 않았습니다."})).await;return Ok(());}
            Admission::New=>(),
        }
        let permit=self.permits.clone().try_acquire_owned();
        let connection=self.clone();
        tauri::async_runtime::spawn(async move {
            let result=match permit {
                Err(_)=>Err("이전 원격 요청이 아직 진행 중입니다. 새 작업은 실행하지 않았습니다.".into()),
                Ok(_permit)=>{
                    if connection.stopped.load(Ordering::Acquire) {Err("연결이 해제되어 실행하지 않았습니다.".into())}
                    else if !super::relay_dispatch::COMMANDS.contains(&command.as_str()) {Err("허용되지 않은 원격 명령입니다.".into())}
                    else {(connection.dispatch)(command,args).await}
                }
            };
            let mut reply=match result {Ok(value)=>json!({"reqId":id,"ok":true,"result":value}),Err(error)=>json!({"reqId":id,"ok":false,"error":error})};
            if serde_json::to_vec(&reply).map_or(true,|bytes|bytes.len()>MAX_REPLY) {
                reply=json!({"reqId":id,"ok":false,"error":"명령은 실행됐지만 응답 크기가 상한을 초과했습니다. 재실행하지 말고 상태를 조회하세요."});
            }
            if let Ok(mut calls)=connection.calls.lock() {calls.finish(id,reply.clone());}
            connection.reply(reply).await;
        });
        Ok(())
    }
    async fn stream(self:Arc<Self>) {
        let mut delay=2;
        while !self.stopped.load(Ordering::Acquire) {
            let response=self.client.get(self.url.join("/api/relay/stream").unwrap()).header("Accept","text/event-stream").send().await;
            let mut response=match response {
                Ok(response) if response.status()==200=>response,
                Ok(response)=>{
                    let status=response.status().as_u16();
                    if [401,403,409,429].contains(&status) || !(500..600).contains(&status) {self.fatal(format!("중계기 연결 거부 (HTTP {status}). 주소·비밀키·다른 호스트 연결을 확인하세요."));return;}
                    self.status("reconnecting",Some(format!("중계기 일시 오류 (HTTP {status})")));
                    tokio::time::sleep(Duration::from_secs(delay)).await;delay=(delay*2).min(30);continue;
                }
                Err(_)=>{self.status("reconnecting",Some("중계기에 연결하지 못했습니다. 자동 재접속을 기다리거나 연결을 해제하세요.".into()));tokio::time::sleep(Duration::from_secs(delay)).await;delay=(delay*2).min(30);continue;}
            };
            if !response.headers().get("content-type").and_then(|v|v.to_str().ok()).is_some_and(|v|v.to_ascii_lowercase().starts_with("text/event-stream")) {
                self.fatal("중계기가 SSE 스트림을 반환하지 않았습니다.".into());return;
            }
            let mut decoder=SseDecoder::default();let mut registered=false;
            loop {
                let chunk=match response.chunk().await {Ok(Some(chunk))=>chunk,_=>break};
                if self.stopped.load(Ordering::Acquire) {return;}
                let messages=match decoder.push(&chunk) {Ok(messages)=>messages,Err(error)=>{self.fatal(error);return;}};
                for message in messages {
                    match message.get("type").and_then(Value::as_str) {
                        Some("registered") if !registered && message["hostId"]==self.host_id && message["protocolVersion"]=="1.0.0"=>{
                            registered=true;delay=2;self.connected.store(true,Ordering::Release);self.status("connected",None);
                        }
                        Some("invoke") if registered=>if let Err(error)=self.invoke(message).await {self.fatal(error);return;},
                        _=>{self.fatal("중계기 등록 또는 프로토콜 버전이 일치하지 않습니다.".into());return;}
                    }
                }
            }
            self.connected.store(false,Ordering::Release);
            if !self.stopped.load(Ordering::Acquire) {self.status("reconnecting",Some("중계기 연결이 끊겼습니다. 기존 명령을 재실행하지 않고 재접속합니다.".into()));tokio::time::sleep(Duration::from_secs(delay)).await;delay=(delay*2).min(30);}
        }
    }
}

impl RelayState {
    pub(crate) fn snapshot(&self)->Result<RelayStatus,String> {
        let mut value=self.status.lock().map_err(|_| "원격 연결 상태 잠금 실패")?.clone();
        value.pending_commands=4-self.permits.available_permits();Ok(value)
    }
    fn start(&self,url:&str,secret:String,dispatch:Dispatch,notify:Notify)->Result<RelayStatus,String> {
        let url=normalize_url(url)?;
        let mut header=HeaderValue::from_str(secret.trim()).map_err(|_| "HostSecret 형식이 올바르지 않습니다.")?;
        if secret.trim().is_empty() || secret.len()>4096 {return Err("HostSecret을 입력하세요 (최대4096자).".into());}header.set_sensitive(true);
        let mut active=self.active.lock().map_err(|_| "원격 연결 잠금 실패")?;
        if active.is_some() {return Err("기존 연결을 해제한 뒤 다시 연결하세요.".into());}
        let host_id=uuid::Uuid::new_v4().to_string();
        let mut headers=HeaderMap::new();headers.insert("X-OpenDIY-Host-Secret",header);headers.insert("X-OpenDIY-Host-Id",HeaderValue::from_str(&host_id).unwrap());
        // reqwest uses rustls-no-provider via the updater's dependency graph.
        // First relay use must also work before any updater request. Preserve
        // a provider another caller already installed (including a race).
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _=rustls::crypto::ring::default_provider().install_default();
        }
        let client=Client::builder().default_headers(headers).redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10)).read_timeout(Duration::from_secs(45)).build().map_err(|_| "원격 연결 클라이언트 준비 실패")?;
        let epoch=self.generation.fetch_add(1,Ordering::AcqRel)+1;
        let status=RelayStatus{generation:epoch,revision:0,state:"connecting",url:Some(url.to_string()),error:None,release_confirmed:None,pending_commands:4-self.permits.available_permits()};
        *self.status.lock().map_err(|_| "원격 연결 상태 잠금 실패")?=status.clone();
        let connection=Arc::new(Connection{epoch,url,client,stopped:AtomicBool::new(false),connected:AtomicBool::new(false),queued:AtomicUsize::new(0),status:self.status.clone(),notify:notify.clone(),dispatch,permits:self.permits.clone(),calls:Mutex::new(Calls::default()),host_id,network_abort:Mutex::new(Vec::new())});
        let (sender,mut receiver)=mpsc::channel::<Vec<u8>>(32);
        let worker=connection.clone();
        let events=tauri::async_runtime::spawn(async move {
            while let Some(bytes)=receiver.recv().await {
                worker.queued.fetch_sub(bytes.len(),Ordering::AcqRel);
                if worker.stopped.load(Ordering::Acquire) {break;}
                if let Err(error)=worker.post("/api/relay/event",bytes).await {worker.fatal(error);break;}
            }
        });
        let stream=tauri::async_runtime::spawn(connection.clone().stream());
        if let Ok(mut tasks)=connection.network_abort.lock() {tasks.extend([stream.inner().abort_handle(),events.inner().abort_handle()]);if connection.stopped.load(Ordering::Acquire) {for task in tasks.iter(){task.abort();}}}
        *active=Some(Active{connection,stream,events,sender});
        notify(status.clone());Ok(status)
    }
    pub(crate) fn forward(&self,event:&str,payload:Value) {
        if !["terminal-output","session-output","session-changed","runtime-updated"].contains(&event) {return;}
        let Ok(active)=self.active.lock() else {return};let Some(active)=active.as_ref() else {return};
        let connection=&active.connection;
        if connection.stopped.load(Ordering::Acquire) || !connection.connected.load(Ordering::Acquire) {return;}
        let bytes=match serde_json::to_vec(&json!({"event":event,"payload":payload})) {Ok(bytes)=>bytes,Err(_)=>{connection.fatal("원격 이벤트 직렬화 실패".into());return;}};
        let size=bytes.len();
        if size>1024*1024 || connection.queued.fetch_update(Ordering::AcqRel,Ordering::Acquire,|n|n.checked_add(size).filter(|total|*total<=MAX_EVENTS)).is_err() {
            connection.fatal("원격 출력 대기 상한을 초과했습니다. 연결 해제 후 다시 연결하고 세션을 새로고침하세요.".into());active.stream.abort();active.events.abort();return;
        }
        if active.sender.try_send(bytes).is_err() {
            connection.queued.fetch_sub(size,Ordering::AcqRel);connection.fatal("원격 이벤트 대기열이 가득 찼습니다. 다시 연결하고 세션을 새로고침하세요.".into());active.stream.abort();active.events.abort();
        }
    }
    async fn stop(&self)->Result<RelayStatus,String> {
        let active=self.active.lock().map_err(|_| "원격 연결 잠금 실패")?.take();
        let Some(active)=active else {return self.snapshot();};
        let connection=active.connection;
        connection.stopped.store(true,Ordering::Release);connection.connected.store(false,Ordering::Release);
        active.stream.abort();active.events.abort();
        // Join cancellation of network tasks; accepted native commands keep
        // their original ownership and are not falsely reported as cancelled.
        let _=active.stream.await;let _=active.events.await;
        let released=connection.post("/api/relay/disconnect",b"{}".to_vec()).await.is_ok();
        let mut status=self.status.lock().map_err(|_| "원격 연결 상태 잠금 실패")?;
        status.revision+=1;status.state="disconnected";status.release_confirmed=Some(released);status.pending_commands=4-self.permits.available_permits();
        status.error=if released {None} else {Some("로컬 연결은 닫혔지만 중계기의 연결 해제 응답을 확인하지 못했습니다.".into())};
        (connection.notify)(status.clone());Ok(status.clone())
    }
}

#[tauri::command]
pub(crate) fn relay_status(relay:State<'_,RelayState>)->Result<RelayStatus,String>{relay.snapshot()}
#[tauri::command]
pub(crate) async fn relay_connect(app:tauri::AppHandle,relay:State<'_,RelayState>,url:String,secret:String)->Result<RelayStatus,String>{
    let _lifecycle=relay.lifecycle.lock().await;
    let dispatcher=app.clone();let notifier=app.clone();
    let dispatch:Dispatch=Arc::new(move |command,args|{let app=dispatcher.clone();Box::pin(async move {super::relay_dispatch::dispatch(app,&command,args).await})});
    let notify:Notify=Arc::new(move |status|{let _=notifier.emit("relay-status",status);});
    relay.start(&url,secret,dispatch,notify)
}
#[tauri::command]
pub(crate) async fn relay_disconnect(relay:State<'_,RelayState>)->Result<RelayStatus,String>{let _lifecycle=relay.lifecycle.lock().await;relay.stop().await}

#[cfg(test)]
#[path="relay_tests.rs"]
mod tests;
