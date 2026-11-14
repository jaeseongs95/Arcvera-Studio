//! Narrow extension runtime wire adapters. The caller owns session admission and transport lifetime.
use super::*;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_THREAD_BYTES: usize = 256;
const MAX_SERVER_BYTES: usize = 128;
const MAX_CURSOR_BYTES: usize = 256;
const MAX_TOOL_BYTES: usize = 256;
const MAX_TOOLS: usize = 256;
const MAX_JSON_TEXT: usize = 8192;
const MAX_JSON_NODES: usize = 8192;
const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_ARRAY: usize = 256;
const MAX_JSON_FIELDS: usize = 512;
const INVALID_RESPONSE: &str = "MCP runtime 응답 형식 또는 상한을 확인할 수 없습니다. 요청 결과는 확인되지 않았습니다.";
const INVALID_HOOK_RESPONSE: &str = "현재 대화 Hook 응답 형식·대상 또는 상한을 확인할 수 없습니다. 조회 결과는 미확인입니다.";

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub(super) enum RuntimeExtensionRequest {
    ReloadMcp {},
    McpStatus { server_name: String, cursor: Option<String> },
    HooksStatus {},
}

/// Validate before the caller opens the admitted vendor connection.
pub(super) fn validate(thread_id: &str, request: &RuntimeExtensionRequest) -> Result<(), String> {
    identifier(thread_id, MAX_THREAD_BYTES)
        .map_err(|_| if matches!(request, RuntimeExtensionRequest::HooksStatus {}) {
            "현재 대화의 Hook 조회 대상을 확인할 수 없습니다.".to_owned()
        } else { "현재 대화의 MCP 요청 대상을 확인할 수 없습니다.".to_owned() })?;
    if let RuntimeExtensionRequest::McpStatus { server_name, cursor } = request {
        identifier(server_name, MAX_SERVER_BYTES)
            .map_err(|_| "MCP 서버 이름 형식을 확인할 수 없습니다.".to_owned())?;
        if let Some(cursor) = cursor {
            identifier(cursor, MAX_CURSOR_BYTES)
                .map_err(|_| "MCP 상태 페이지 식별자 형식을 확인할 수 없습니다.".to_owned())?;
        }
    }
    Ok(())
}

/// Never creates a connection or retries a mutation. Use the owner's already admitted Rpc.
pub(super) fn exchange<S: Read + Write>(
    rpc: &mut Rpc<S>, thread_id: &str, request: &RuntimeExtensionRequest,
) -> Result<Value, String> {
    validate(thread_id, request)?;
    let (method, params) = match request {
        RuntimeExtensionRequest::ReloadMcp {} => ("config/mcpServer/reload", json!({"threadId":thread_id})),
        RuntimeExtensionRequest::McpStatus { server_name, cursor } => {
            let mut params = json!({"threadId":thread_id,"serverName":server_name,"detail":"toolsAndAuthOnly","limit":1});
            if let Some(cursor) = cursor { params["cursor"] = json!(cursor); }
            ("mcpServerStatus/list", params)
        }
        RuntimeExtensionRequest::HooksStatus {} => return Err("현재 대화 Hook 조회에는 결속된 프로젝트 경로가 필요합니다.".into()),
    };
    let value = rpc.call(method, params).map_err(|error| safe_rpc_error(request, &error))?;
    same_thread(&value, thread_id)?;
    let result = match request {
        RuntimeExtensionRequest::ReloadMcp {} => reload_result(&value, thread_id),
        RuntimeExtensionRequest::McpStatus { server_name, cursor } => status_result(&value, thread_id, server_name, cursor.as_deref()),
        RuntimeExtensionRequest::HooksStatus {} => Err("현재 대화 Hook 조회에는 결속된 프로젝트 경로가 필요합니다.".into()),
    }?;
    // Raw tools may contain large descriptions/schemas inside the shared Rpc frame limit.
    // Bound only the validated renderer projection; never forward unused tool content.
    bounded_response(&result)?;
    Ok(result)
}

/// Existing hooks/list thread mode, not a configuration query or execution-gate probe.
/// The caller owns the expected thread/cwd binding, admission, cancellation and Rpc lifetime.
pub(super) fn exchange_hooks<S: Read + Write>(
    rpc: &mut Rpc<S>, thread_id: &str, cwd: &Path,
) -> Result<Value, String> {
    validate(thread_id, &RuntimeExtensionRequest::HooksStatus {})?;
    let directory = cwd.to_str().ok_or(INVALID_HOOK_RESPONSE)?;
    if !cwd.is_absolute() || identifier(directory, 128 * 1024).is_err() {
        return Err("현재 대화 Hook 조회의 프로젝트 경로를 확인할 수 없습니다.".into());
    }
    let value = rpc.call("hooks/list", json!({"threadId":thread_id}))
        .map_err(|error| safe_hooks_rpc_error(&error))?;
    // The existing wire does not echo threadId. Reject a conflicting explicit echo,
    // but do not pretend that its absence acknowledges thread identity or a gate.
    same_thread(&value, thread_id).map_err(|_| INVALID_HOOK_RESPONSE.to_owned())?;
    let hooks = extensions::parse_runtime_hooks(&value, cwd).map_err(|_| INVALID_HOOK_RESPONSE.to_owned())?;
    let runtime = hooks_runtime_metadata(&value)?;
    // Explicit allowlist: opaque keys, hashes, paths, commands and diagnostics may
    // contain sensitive content even after length/type validation. Never serialize
    // ExtensionHooks here; future fields must not cross this read-only boundary.
    let items: Vec<Value> = hooks.items.iter().map(|hook| json!({
        "id":format!("hook-{:x}",Sha256::digest(hook.key.as_bytes())),
        "eventName":hook.event_name,"handlerType":hook.handler_type,"source":hook.source,
        "enabled":hook.enabled,"isManaged":hook.is_managed,"trustStatus":hook.trust_status,
    })).collect();
    let result = json!({
        "operation":"hooksStatus","threadId":thread_id,"provenance":"loadedThreadHooksList",
        "executionGate":"unknown","applicationState":"unknown",
        "runtime":runtime,
        "hooks":{"items":items,"warningCount":hooks.warnings.len(),"errorCount":hooks.errors.len()},
    });
    bounded_hooks_response(&result)?;
    Ok(result)
}

fn hooks_runtime_metadata(value: &Value) -> Result<Value, String> {
    let current_thread = match value.get("scope") {
        None | Some(Value::Null) => false,
        Some(Value::String(scope)) if scope == "currentThread" => true,
        _ => return Err(INVALID_HOOK_RESPONSE.into()),
    };
    let runtime = match value.get("runtime") {
        None | Some(Value::Null) => return Ok(Value::Null),
        Some(runtime) => runtime.as_object().ok_or(INVALID_HOOK_RESPONSE)?,
    };
    let feature_enabled = runtime.get("featureEnabled").and_then(Value::as_bool).ok_or(INVALID_HOOK_RESPONSE)?;
    let base_handler_count = runtime.get("baseHandlerCount").and_then(Value::as_u64)
        .filter(|count| *count <= 9_007_199_254_740_991).ok_or(INVALID_HOOK_RESPONSE)?;
    // Ordinary feature configuration is not an execution gate. Base handlers exclude
    // per-turn overlays. Only explicit current-thread scope makes these values evidence.
    if !current_thread { return Ok(Value::Null); }
    Ok(json!({"featureEnabled":feature_enabled,"baseHandlerCount":base_handler_count}))
}

fn safe_hooks_rpc_error(raw: &str) -> String {
    let class = if raw.contains("-32601") || raw.contains("Method not found") || raw.contains("method not found") {
        "선택한 Core가 현재 대화 Hook 조회를 지원하지 않습니다."
    } else if raw.contains("timed out") || raw.contains("기한") {
        "현재 대화 Hook 조회 시간이 초과되었습니다."
    } else if raw.contains("cancelled") || raw.contains("canceled") {
        "현재 대화 Hook 조회가 취소되었습니다."
    } else if raw.contains("transport") || raw.contains("disconnect") {
        "현재 대화 Hook 연결 응답을 확인하지 못했습니다."
    } else { "현재 대화 Hook 조회가 실패했습니다." };
    format!("{class} 조회 결과와 실행 gate는 미확인입니다.")
}

fn bounded_hooks_response(value: &Value) -> Result<(), String> {
    // ceiling: reuse the shared 8 MiB budget after an explicit, typed projection.
    // Hook's existing 4096-row/64 KiB field limits are not MCP's 256-row/8 KiB limits.
    serde_json::to_writer(SizeLimit { remaining: MAX_FRAME }, value)
        .map_err(|_| INVALID_HOOK_RESPONSE.to_owned())
}

fn identifier(value: &str, maximum: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        Err(INVALID_RESPONSE.into())
    } else { Ok(()) }
}

fn safe_rpc_error(request: &RuntimeExtensionRequest, raw: &str) -> String {
    // RPC diagnostics can contain config, URL credentials, schemas, and server output.
    let class = if raw.contains("-32601") || raw.contains("Method not found") || raw.contains("method not found") {
        "선택한 Core가 이 MCP runtime 요청을 지원하지 않습니다."
    } else if raw.contains("timed out") || raw.contains("기한") {
        "MCP runtime 요청 시간이 초과되었습니다."
    } else if raw.contains("cancelled") || raw.contains("canceled") {
        "MCP runtime 요청이 취소되었습니다."
    } else if raw.contains("transport") || raw.contains("disconnect") {
        "MCP runtime 연결 응답을 확인하지 못했습니다."
    } else {
        "MCP runtime 요청이 실패했습니다."
    };
    match request {
        RuntimeExtensionRequest::ReloadMcp {} => format!("{class} 재연결 요청 결과는 미확인입니다. 자동 재시도하지 않습니다."),
        RuntimeExtensionRequest::McpStatus { .. } => format!("{class} 현재 연결과 도구 목록은 미확인입니다."),
        RuntimeExtensionRequest::HooksStatus {} => safe_hooks_rpc_error(raw),
    }
}

struct SizeLimit { remaining: usize }
impl Write for SizeLimit {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "MCP response limit"));
        }
        self.remaining -= bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

fn bounded_response(value: &Value) -> Result<(), String> {
    // ceiling: the shared Rpc has already parsed its 8 MiB frame. Bound this projection
    // to 1 MiB without allocating a second serialized response or returning a prefix.
    serde_json::to_writer(SizeLimit { remaining: MAX_RESPONSE_BYTES }, value)
        .map_err(|_| INVALID_RESPONSE.to_owned())?;
    fn visit(value: &Value, depth: usize, left: &mut usize) -> Result<(), String> {
        if depth > MAX_JSON_DEPTH || *left == 0 { return Err(INVALID_RESPONSE.into()); }
        *left -= 1;
        match value {
            Value::String(text) if text.len() > MAX_JSON_TEXT => Err(INVALID_RESPONSE.into()),
            Value::Array(items) => {
                if items.len() > MAX_JSON_ARRAY { return Err(INVALID_RESPONSE.into()); }
                for item in items { visit(item, depth + 1, left)?; }
                Ok(())
            }
            Value::Object(fields) => {
                if fields.len() > MAX_JSON_FIELDS || fields.keys().any(|key| key.len() > MAX_TOOL_BYTES) {
                    return Err(INVALID_RESPONSE.into());
                }
                for item in fields.values() { visit(item, depth + 1, left)?; }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    let mut remaining = MAX_JSON_NODES;
    visit(value, 0, &mut remaining)
}

fn same_thread(value: &Value, thread_id: &str) -> Result<(), String> {
    let fields = value.as_object().ok_or(INVALID_RESPONSE)?;
    if fields.get("threadId").is_some_and(|value| value.as_str() != Some(thread_id)) {
        return Err("다른 대화의 MCP runtime 응답입니다. 현재 대화에 적용하지 않았습니다.".into());
    }
    Ok(())
}

fn reload_result(value: &Value, thread_id: &str) -> Result<Value, String> {
    let (outcome, reason) = match value.get("reconnectQueuedForThreadId") {
        Some(Value::String(id)) => {
            identifier(id, MAX_THREAD_BYTES)?;
            if id == thread_id { ("queued", None) } else { ("unconfirmed", Some("targetMismatch")) }
        }
        None | Some(Value::Null) => ("unconfirmed", Some("ackMissing")),
        _ => return Err(INVALID_RESPONSE.into()),
    };
    let mut result = json!({"operation":"reloadMcp","threadId":thread_id,"outcome":outcome});
    if let Some(reason) = reason { result["reason"] = json!(reason); }
    Ok(result)
}

fn nullable_enum<'a>(value: Option<&'a Value>, allowed: &[&str]) -> Result<&'a str, String> {
    match value {
        None | Some(Value::Null) => Ok("unknown"),
        Some(Value::String(text)) if allowed.contains(&text.as_str()) => Ok(text),
        _ => Err(INVALID_RESPONSE.into()),
    }
}

fn status_result(value: &Value, thread_id: &str, server_name: &str, cursor: Option<&str>) -> Result<Value, String> {
    let rows = value.get("data").and_then(Value::as_array).ok_or(INVALID_RESPONSE)?;
    if rows.len() > 1 { return Err("MCP 상태 응답에 요청하지 않은 서버 또는 중복 서버가 있습니다.".into()); }
    let next_cursor = match value.get("nextCursor") {
        Some(Value::Null) => None,
        Some(Value::String(next)) => {
            identifier(next, MAX_CURSOR_BYTES)?;
            if Some(next.as_str()) == cursor { return Err("MCP 상태 페이지가 진행되지 않았습니다. 자동으로 다시 조회하지 않습니다.".into()); }
            Some(next.as_str())
        }
        _ => return Err(INVALID_RESPONSE.into()),
    };
    let mut result = json!({
        "operation":"mcpStatus","threadId":thread_id,"serverName":server_name,"found":!rows.is_empty(),
        "runtimeStatus":"unknown","authStatus":"unknown","toolsState":"unknown","toolNames":[],"nextCursor":next_cursor,
    });
    let Some(row) = rows.first() else { return Ok(result); };
    same_thread(row, thread_id)?;
    if row.get("name").and_then(Value::as_str) != Some(server_name) {
        return Err("MCP 상태 응답의 서버가 요청한 서버와 다릅니다.".into());
    }
    let runtime_status = nullable_enum(row.get("runtimeStatus"), &[
        "notStarted", "starting", "connected", "authenticationRequired", "failed", "cancelled", "disabled",
    ])?;
    let auth_status = nullable_enum(row.get("authStatus"), &["unknown", "unsupported", "notLoggedIn", "bearerToken", "oAuth"])?;
    let tools_error = match row.get("toolsError") {
        None => None,
        Some(Value::Null) => Some(false),
        Some(Value::String(_)) => Some(true),
        _ => return Err(INVALID_RESPONSE.into()),
    };
    let mut names = Vec::new();
    let tools_present = match row.get("tools") {
        None => false,
        Some(Value::Object(tools)) => {
            if tools.len() > MAX_TOOLS { return Err(INVALID_RESPONSE.into()); }
            for (name, tool) in tools {
                identifier(name, MAX_TOOL_BYTES)?;
                // Core returns raw server tool names as both map key and Tool.name.
                // Reject conflicting aliases rather than relabeling executable content.
                if tool.as_object().is_none() || tool.get("name").and_then(Value::as_str) != Some(name.as_str()) {
                    return Err(INVALID_RESPONSE.into());
                }
                names.push(name.clone());
            }
            true
        }
        _ => return Err(INVALID_RESPONSE.into()),
    };
    names.sort();
    let tools_state = match tools_error {
        Some(true) => { names.clear(); "failed" }
        Some(false) if tools_present && !names.is_empty() => "available",
        Some(false) if tools_present => "empty",
        _ => "unknown",
    };
    result["runtimeStatus"] = json!(runtime_status);
    result["authStatus"] = json!(auth_status);
    result["toolsState"] = json!(tools_state);
    result["toolNames"] = json!(names);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const THREAD: &str = "thread-a";
    const SERVER: &str = "server-a";
    const SECRET: &str = "RUNTIME_SECRET_SENTINEL";

    struct MemoryStream { input: Cursor<Vec<u8>>, output: Vec<u8> }
    impl Read for MemoryStream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> { self.input.read(bytes) }
    }
    impl Write for MemoryStream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.output.extend_from_slice(bytes); Ok(bytes.len()) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    fn fixture(reply: Option<Value>) -> Rpc<MemoryStream> {
        let mut bytes = Vec::new();
        if let Some(reply) = reply {
            let payload = reply.to_string().into_bytes();
            bytes.push(0x81); // An unmasked, final server Text frame; no real socket.
            match payload.len() {
                size @ 0..=125 => bytes.push(size as u8),
                size @ 126..=65535 => { bytes.push(126); bytes.extend_from_slice(&(size as u16).to_be_bytes()); }
                size => { bytes.push(127); bytes.extend_from_slice(&(size as u64).to_be_bytes()); }
            }
            bytes.extend_from_slice(&payload);
        }
        Rpc {
            ws: WebSocket::from_raw_socket(MemoryStream { input: Cursor::new(bytes), output: Vec::new() }, tungstenite::protocol::Role::Client, None),
            core_version: None, next_id: 0, cancelled: None, deadline: None,
        }
    }
    fn requests(rpc: &Rpc<MemoryStream>) -> Vec<Value> {
        let stream = MemoryStream { input: Cursor::new(rpc.ws.get_ref().output.clone()), output: Vec::new() };
        let mut ws = WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);
        let mut values = Vec::new();
        while let Ok(Message::Text(text)) = ws.read() { values.push(serde_json::from_str(&text).unwrap()); }
        values
    }
    fn execute(response: Value, request: RuntimeExtensionRequest) -> (Result<Value, String>, Vec<Value>) {
        let mut rpc = fixture(Some(json!({"id":1,"result":response})));
        let result = exchange(&mut rpc, THREAD, &request);
        (result, requests(&rpc))
    }
    fn request(cursor: Option<&str>) -> RuntimeExtensionRequest {
        RuntimeExtensionRequest::McpStatus { server_name: SERVER.into(), cursor: cursor.map(str::to_owned) }
    }
    fn server() -> Value {
        json!({"name":SERVER,"runtimeStatus":"connected","authStatus":"oAuth","toolsError":null,"tools":{"lookup":{"name":"lookup","inputSchema":{"type":"object"}}}})
    }
    fn server_with_large_unused_schema() -> Value {
        let mut row = server();
        let mut properties = serde_json::Map::new();
        for index in 0..=MAX_JSON_FIELDS { properties.insert(format!("field-{index}"), json!({"type":"string"})); }
        row["tools"]["lookup"]["description"] = json!(format!("{SECRET}{}", "x".repeat(MAX_RESPONSE_BYTES + 1)));
        row["tools"]["lookup"]["inputSchema"] = json!({
            "type":"object", "properties":properties,
            "enum":vec!["unused";MAX_JSON_ARRAY + 1],
            "anyOf":vec![json!({"enum":vec!["unused";128]});128],
        });
        row
    }
    fn page(server: Value) -> Value { json!({"data":[server],"nextCursor":null}) }

    #[test]
    fn runtime_extensions_reload_is_exactly_one_queued_request_and_never_ready() {
        let (result, sent) = execute(json!({"reconnectQueuedForThreadId":THREAD}), RuntimeExtensionRequest::ReloadMcp {});
        assert_eq!(sent, vec![json!({"id":1,"method":"config/mcpServer/reload","params":{"threadId":THREAD}})]);
        assert_eq!(result.unwrap(), json!({"operation":"reloadMcp","threadId":THREAD,"outcome":"queued"}));
    }
    #[test]
    fn runtime_extensions_legacy_and_other_thread_ack_are_unconfirmed() {
        for (value, reason) in [(json!({}), "ackMissing"), (json!({"reconnectQueuedForThreadId":null}), "ackMissing"),
            (json!({"reconnectQueuedForThreadId":"other-thread"}), "targetMismatch")] {
            let (result, sent) = execute(value, RuntimeExtensionRequest::ReloadMcp {});
            assert_eq!(result.unwrap(), json!({"operation":"reloadMcp","threadId":THREAD,"outcome":"unconfirmed","reason":reason}));
            assert_eq!(sent.len(), 1);
        }
    }
    #[test]
    fn runtime_extensions_reject_invalid_inputs_without_sending() {
        assert!(validate(THREAD, &RuntimeExtensionRequest::ReloadMcp {}).is_ok());
        assert!(validate(THREAD, &request(Some("1"))).is_ok());
        for thread in ["", "  ", "thread\n", &"x".repeat(MAX_THREAD_BYTES + 1)] {
            let mut rpc = fixture(None);
            let expected = validate(thread, &RuntimeExtensionRequest::ReloadMcp {}).unwrap_err();
            assert_eq!(exchange(&mut rpc, thread, &RuntimeExtensionRequest::ReloadMcp {}).unwrap_err(), expected);
            assert!(requests(&rpc).is_empty());
        }
        for value in [RuntimeExtensionRequest::McpStatus {server_name:"".into(),cursor:None},
            RuntimeExtensionRequest::McpStatus {server_name:"s".repeat(MAX_SERVER_BYTES+1),cursor:None},
            RuntimeExtensionRequest::McpStatus {server_name:SERVER.into(),cursor:Some("".into())},
            RuntimeExtensionRequest::McpStatus {server_name:SERVER.into(),cursor:Some("c".repeat(MAX_CURSOR_BYTES+1))}] {
            let mut rpc = fixture(None);
            let expected = validate(THREAD, &value).unwrap_err();
            assert_eq!(exchange(&mut rpc, THREAD, &value).unwrap_err(), expected);
            assert!(requests(&rpc).is_empty());
        }
        assert!(serde_json::from_value::<RuntimeExtensionRequest>(json!({"operation":"reloadMcp","method":"arbitrary"})).is_err());
        assert!(serde_json::from_value::<RuntimeExtensionRequest>(json!({"operation":"mcpStatus","serverName":SERVER,"env":{}})).is_err());
    }
    #[test]
    fn runtime_extensions_status_is_one_scoped_page_with_only_safe_projection() {
        let mut row = server();
        row["httpOrigin"] = json!(format!("https://example.invalid/?token={SECRET}"));
        row["env"] = json!({"TOKEN":SECRET}); row["headers"] = json!({"Authorization":SECRET});
        row["oauth"] = json!({"secret":SECRET}); row["serverInfo"] = json!({"description":SECRET});
        row["tools"]["lookup"]["inputSchema"] = json!({"description":SECRET});
        row["tools"]["lookup"]["description"] = json!(SECRET);
        for cursor in [None, Some("1")] {
            let mut response = page(row.clone()); response["nextCursor"] = json!("2");
            let (result, sent) = execute(response, request(cursor));
            let mut params = json!({"threadId":THREAD,"serverName":SERVER,"detail":"toolsAndAuthOnly","limit":1});
            if let Some(cursor) = cursor { params["cursor"] = json!(cursor); }
            assert_eq!(sent, vec![json!({"id":1,"method":"mcpServerStatus/list","params":params})]);
            let result = result.unwrap();
            assert_eq!(result, json!({"operation":"mcpStatus","threadId":THREAD,"serverName":SERVER,"found":true,
                "runtimeStatus":"connected","authStatus":"oAuth","toolsState":"available","toolNames":["lookup"],"nextCursor":"2"}));
            assert!(!result.to_string().contains(SECRET));
        }
    }
    #[test]
    fn runtime_extensions_empty_failed_and_unknown_catalogs_stay_distinct() {
        let mut empty = server(); empty["tools"] = json!({});
        let (result, _) = execute(page(empty.clone()), request(None));
        assert_eq!(result.unwrap()["toolsState"], "empty");
        for status in ["notStarted", "starting", "connected", "authenticationRequired", "failed", "cancelled", "disabled"] {
            let mut failed = empty.clone(); failed["runtimeStatus"] = json!(status); failed["toolsError"] = json!(SECRET);
            let result = execute(page(failed), request(None)).0.unwrap();
            assert_eq!(result["runtimeStatus"], status); assert_eq!(result["toolsState"], "failed");
            assert_eq!(result["toolNames"], json!([])); assert!(!result.to_string().contains(SECRET));
        }
        for missing in ["runtimeStatus", "toolsError", "tools", "authStatus"] {
            let mut row = empty.clone(); row.as_object_mut().unwrap().remove(missing);
            let result = execute(page(row), request(None)).0.unwrap();
            match missing {
                "runtimeStatus" | "authStatus" => assert_eq!(result[missing], "unknown"),
                _ => assert_eq!(result["toolsState"], "unknown"),
            }
        }
        for auth in ["unknown", "unsupported", "notLoggedIn", "bearerToken", "oAuth"] {
            let mut row = empty.clone(); row["authStatus"] = json!(auth);
            assert_eq!(execute(page(row), request(None)).0.unwrap()["authStatus"], auth);
        }
        let mut unavailable = empty; unavailable["runtimeStatus"] = Value::Null;
        assert_eq!(execute(page(unavailable), request(None)).0.unwrap()["runtimeStatus"], "unknown");
        let absent = execute(json!({"data":[],"nextCursor":null}), request(None)).0.unwrap();
        assert_eq!(absent["found"], false); assert_eq!(absent["toolsState"], "unknown");
    }
    #[test]
    fn runtime_extensions_reject_malformed_duplicates_other_server_and_wrong_thread() {
        let mut cases = vec![json!(null), json!({}), json!({"data":{}}), page(json!(null)), json!({"data":[server()]}),
            json!({"data":[server(),server()]}), json!({"data":[server()],"threadId":"other"}),
            json!({"data":[server()],"threadId":null}), json!({"data":[server()],"nextCursor":42})];
        for (key, value) in [("name",json!("other")),("threadId",json!("other")),("runtimeStatus",json!("ready")),
            ("authStatus",json!("loggedIn")),("toolsError",json!({"message":SECRET})),("tools",json!([])),
            ("tools",json!({"lookup":{"name":"renamed"}})),("tools",json!({"lookup":null}))] {
            let mut row = server(); row[key] = value; cases.push(page(row));
        }
        for value in cases {
            let (result, sent) = execute(value, request(None));
            let error = result.unwrap_err(); assert!(!error.contains(SECRET)); assert_eq!(sent.len(), 1);
        }
        assert!(execute(json!({"reconnectQueuedForThreadId":5}), RuntimeExtensionRequest::ReloadMcp {}).0.is_err());
    }
    #[test]
    fn runtime_extensions_cursor_is_bounded_opaque_and_never_automatically_followed() {
        let mut response = page(server()); response["nextCursor"] = json!("opaque-next");
        let (result, sent) = execute(response.clone(), request(None));
        assert_eq!(result.unwrap()["nextCursor"], "opaque-next"); assert_eq!(sent.len(), 1);
        assert!(execute(response, request(Some("opaque-next"))).0.is_err());
        for cursor in ["", "x\n", &"x".repeat(MAX_CURSOR_BYTES+1)] {
            let mut value = page(server()); value["nextCursor"] = json!(cursor);
            assert!(execute(value, request(None)).0.is_err());
        }
    }
    #[test]
    fn runtime_extensions_used_field_limits_reject_instead_of_returning_partial_data() {
        let mut oversized_tools = server();
        let tools = oversized_tools["tools"].as_object_mut().unwrap(); tools.clear();
        for index in 0..=MAX_TOOLS { let name=format!("tool-{index}"); tools.insert(name.clone(),json!({"name":name})); }
        for name in ["x".repeat(MAX_TOOL_BYTES+1), "가".repeat(MAX_TOOL_BYTES / 3 + 1)] {
            let mut too_long_tool = server(); too_long_tool["tools"] = json!({name.clone():{"name":name}});
            let (result, sent) = execute(page(too_long_tool), request(None));
            assert!(result.is_err()); assert_eq!(sent.len(), 1);
        }
        let (result, sent) = execute(page(oversized_tools), request(None));
        assert!(result.is_err()); assert_eq!(sent.len(), 1);
    }
    #[test]
    fn runtime_extensions_projection_limiter_rejects_oversized_synthetic_dtos() {
        // These are limiter unit inputs, not raw Core responses or current DTO shapes.
        // The fixed output contract cannot currently reach every defensive shape ceiling.
        let mut too_long_string = page(server()); too_long_string["diagnostic"] = json!("x".repeat(MAX_JSON_TEXT+1));
        let mut too_many_rows = page(server()); too_many_rows["ignored"] = json!(vec![Value::Null;MAX_JSON_ARRAY+1]);
        let mut too_many_fields = page(server());
        for index in 0..=MAX_JSON_FIELDS { too_many_fields[format!("ignored-{index}")] = Value::Null; }
        let mut nested = Value::Null;
        for _ in 0..=MAX_JSON_DEPTH { nested = json!([nested]); }
        let mut too_deep = page(server()); too_deep["ignored"] = nested;
        let mut too_many_nodes = page(server()); too_many_nodes["ignored"] = json!(vec![vec![Value::Null;128];128]);
        let mut too_many_bytes = page(server()); too_many_bytes["ignored"] = json!(vec!["x".repeat(8192);129]);
        for value in [too_long_string,too_many_rows,too_many_fields,too_deep,too_many_nodes,too_many_bytes] {
            assert_eq!(bounded_response(&value).unwrap_err(), INVALID_RESPONSE);
        }
    }
    #[test]
    fn runtime_extensions_large_unused_tool_content_is_projected_without_exposure() {
        let response = page(server_with_large_unused_schema());
        let frame = json!({"id":1,"result":response.clone()}).to_string();
        assert!(frame.len() > MAX_RESPONSE_BYTES);
        assert!(frame.len() < MAX_FRAME);
        let (result, sent) = execute(response, request(None));
        assert_eq!(sent.len(), 1);
        let result = result.unwrap();
        assert_eq!(result, json!({"operation":"mcpStatus","threadId":THREAD,"serverName":SERVER,"found":true,
            "runtimeStatus":"connected","authStatus":"oAuth","toolsState":"available","toolNames":["lookup"],"nextCursor":null}));
        assert!(!result.to_string().contains(SECRET));
        assert!(bounded_response(&result).is_ok());
    }
    #[test]
    fn runtime_extensions_large_unused_content_does_not_bypass_used_field_validation() {
        for (key, value) in [("threadId",json!("other-thread")),("name",json!("other-server")),
            ("runtimeStatus",json!("ready")),("authStatus",json!("loggedIn"))] {
            let mut row = server_with_large_unused_schema(); row[key] = value;
            let (result, sent) = execute(page(row), request(None));
            let error = result.unwrap_err();
            assert!(!error.contains(SECRET)); assert_eq!(sent.len(), 1);
        }
        let mut renamed = server_with_large_unused_schema();
        renamed["tools"]["lookup"]["name"] = json!("renamed");
        let mut oversized = server_with_large_unused_schema();
        for index in 0..MAX_TOOLS {
            let name = format!("tool-{index}"); oversized["tools"][name.clone()] = json!({"name":name});
        }
        let mut long_name = server_with_large_unused_schema();
        let tool = long_name["tools"].as_object_mut().unwrap().remove("lookup").unwrap();
        long_name["tools"]["가".repeat(MAX_TOOL_BYTES / 3 + 1)] = tool;
        for row in [renamed,oversized,long_name] {
            let (result, sent) = execute(page(row), request(None));
            let error = result.unwrap_err();
            assert!(!error.contains(SECRET)); assert_eq!(sent.len(), 1);
        }
        let mut response = page(server_with_large_unused_schema());
        response["nextCursor"] = json!("가".repeat(MAX_CURSOR_BYTES / 3 + 1));
        let (result, sent) = execute(response, request(None));
        assert!(result.is_err()); assert_eq!(sent.len(), 1);
    }
    #[test]
    fn runtime_extensions_raw_frame_limit_remains_at_shared_rpc_boundary() {
        let mut response = page(server());
        response["data"][0]["tools"]["lookup"]["description"] = json!("x".repeat(MAX_FRAME));
        let reply = json!({"id":1,"result":response});
        assert!(reply.to_string().len() > MAX_FRAME);
        let mut raw_rpc = fixture(Some(reply.clone()));
        assert_eq!(raw_rpc.read().unwrap_err(), "Codex protocol frame exceeded limit");
        let mut rpc = fixture(Some(reply));
        let error = exchange(&mut rpc, THREAD, &request(None)).unwrap_err();
        assert_eq!(error, "MCP runtime 요청이 실패했습니다. 현재 연결과 도구 목록은 미확인입니다.");
        assert_eq!(requests(&rpc).len(), 1);
    }
    #[test]
    fn runtime_extensions_maximum_tool_catalog_is_kept_complete() {
        let mut row = server();
        let tools = row["tools"].as_object_mut().unwrap(); tools.clear();
        for index in 0..MAX_TOOLS { let name=format!("tool-{index:03}"); tools.insert(name.clone(),json!({"name":name})); }
        let result = execute(page(row), request(None)).0.unwrap();
        let names = result["toolNames"].as_array().unwrap();
        assert_eq!(names.len(), MAX_TOOLS); assert_eq!(names[0], "tool-000"); assert_eq!(names[MAX_TOOLS-1], "tool-255");
        assert_eq!(result["toolsState"], "available");
    }
    #[test]
    fn runtime_extensions_post_send_error_and_disconnect_never_retry_or_expose_rpc_text() {
        for reply in [None, Some(json!({"id":1,"error":{"code":-32601,"message":SECRET}})),
            Some(json!({"id":1,"error":{"code":-32000,"message":format!("transport {SECRET}")}}))] {
            let mut rpc = fixture(reply);
            let error = exchange(&mut rpc, THREAD, &RuntimeExtensionRequest::ReloadMcp {}).unwrap_err();
            assert!(!error.contains(SECRET)); assert!(error.contains("미확인"));
            assert_eq!(requests(&rpc).len(), 1);
        }
    }
    #[test]
    fn runtime_extensions_rpc_error_classes_never_relay_diagnostics() {
        for (raw, class) in [(format!("-32601 {SECRET}"), "지원"), (format!("timed out {SECRET}"), "초과"),
            (format!("cancelled {SECRET}"), "취소"), (format!("transport {SECRET}"), "연결"), (SECRET.to_owned(), "실패")] {
            let error = safe_rpc_error(&request(None), &raw);
            assert!(error.contains(class)); assert!(error.contains("미확인")); assert!(!error.contains(SECRET));
        }
    }

    // Hook wire fixtures exercise the real Rpc framing/parser but no Core process or socket.
    fn hook_directory() -> std::path::PathBuf { env::current_dir().unwrap() }
    fn hook_row(key: &str, handler: &str) -> Value {
        let mut value = json!({"key":key,"currentHash":"opaque-hash","handlerType":handler,
            "eventName":"preToolUse","matcher":null,"timeoutSec":10,"statusMessage":null,
            "additionalContextLimit":null,"sourcePath":hook_directory().join("hooks.json"),
            "source":"project","pluginId":null,"enabled":true,"isManaged":false,"trustStatus":"trusted"});
        match handler {
            "command" => { value["command"] = json!("echo fixture"); value["async"] = json!(false); }
            "mcpTool" => { value["server"] = json!("fixture-server"); value["tool"] = json!("fixture-tool"); }
            _ => {}
        }
        value
    }
    fn hook_inventory(hooks: Vec<Value>) -> Value {
        json!({"data":[{"cwd":hook_directory(),"hooks":hooks,"warnings":[],"errors":[]}]})
    }
    fn execute_hooks(response: Value) -> (Result<Value, String>, Vec<Value>) {
        let mut rpc = fixture(Some(json!({"id":1,"result":response})));
        let result = exchange_hooks(&mut rpc, THREAD, &hook_directory());
        (result, requests(&rpc))
    }

    // HR01: the oracle is the existing public HooksListParams/Response, not a new 1.1 field.
    #[test]
    fn runtime_extensions_hooks_exact_thread_query_and_explicit_readonly_projection() {
        let rows = ["command","mcpTool","prompt","agent"].into_iter()
            .map(|handler| hook_row(handler, handler)).collect();
        let (result, sent) = execute_hooks(hook_inventory(rows));
        assert_eq!(sent, vec![json!({"id":1,"method":"hooks/list","params":{"threadId":THREAD}})]);
        let result = result.unwrap();
        assert_eq!(result["operation"], "hooksStatus"); assert_eq!(result["threadId"], THREAD);
        assert_eq!(result["provenance"], "loadedThreadHooksList");
        assert_eq!(result["executionGate"], "unknown"); assert_eq!(result["applicationState"], "unknown");
        assert_eq!(result["hooks"]["warningCount"], 0); assert_eq!(result["hooks"]["errorCount"], 0);
        let items = result["hooks"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 4);
        for (item, handler) in items.iter().zip(["command","mcpTool","prompt","agent"]) {
            assert_eq!(item.as_object().unwrap().len(), 7);
            assert_eq!(item["eventName"], "preToolUse"); assert_eq!(item["handlerType"], handler);
            assert_eq!(item["source"], "project"); assert_eq!(item["enabled"], true);
            assert_eq!(item["isManaged"], false); assert_eq!(item["trustStatus"], "trusted");
            let id = item["id"].as_str().unwrap();
            assert!(id.starts_with("hook-")); assert_eq!(id.len(), 69);
            assert!(id[5..].bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
        }
    }
    // HR02: pre-admission validation and additional-field rejection must happen before a send.
    #[test]
    fn runtime_extensions_hooks_invalid_inputs_and_missing_cwd_never_send() {
        assert!(serde_json::from_value::<RuntimeExtensionRequest>(json!({"operation":"hooksStatus"})).is_ok());
        for field in ["cwd","threadId","method","enabled"] {
            let mut request = json!({"operation":"hooksStatus"}); request[field] = json!(SECRET);
            assert!(serde_json::from_value::<RuntimeExtensionRequest>(request).is_err());
        }
        for thread in ["".to_owned(),"  ".into(),"thread\n".into(),"x".repeat(MAX_THREAD_BYTES+1),"가".repeat(86)] {
            let mut rpc = fixture(None);
            let error = exchange_hooks(&mut rpc, &thread, &hook_directory()).unwrap_err();
            assert!(!error.contains(&thread) || thread.trim().is_empty());
            assert!(requests(&rpc).is_empty());
        }
        for cwd in [std::path::PathBuf::new(),std::path::PathBuf::from("relative"),
            hook_directory().join("bad\npath"),hook_directory().join("x".repeat(128*1024+1))] {
            let mut rpc = fixture(None);
            assert!(exchange_hooks(&mut rpc, THREAD, &cwd).is_err());
            assert!(requests(&rpc).is_empty());
        }
        let mut rpc = fixture(None);
        assert!(exchange(&mut rpc, THREAD, &RuntimeExtensionRequest::HooksStatus {}).is_err());
        assert!(requests(&rpc).is_empty());
    }
    // HR03: never use an inventory from an unbound project or an explicit other-thread echo.
    #[test]
    fn runtime_extensions_hooks_reject_other_context_and_multiple_projects() {
        let mut other_cwd = hook_inventory(vec![hook_row(SECRET,"command")]);
        other_cwd["data"][0]["cwd"] = json!(hook_directory().join(SECRET));
        let mut multiple = hook_inventory(vec![]);
        let duplicate = multiple["data"][0].clone();
        multiple["data"].as_array_mut().unwrap().push(duplicate);
        let mut other_thread = hook_inventory(vec![]); other_thread["threadId"] = json!(SECRET);
        let mut bad_thread = hook_inventory(vec![]); bad_thread["threadId"] = json!(123);
        for value in [other_cwd,multiple,other_thread,bad_thread] {
            let (result, sent) = execute_hooks(value);
            assert!(!result.unwrap_err().contains(SECRET)); assert_eq!(sent.len(), 1);
        }
    }
    // HR04: the existing bounded parser remains authoritative even for hidden fields.
    #[test]
    fn runtime_extensions_hooks_reject_missing_malformed_and_duplicate_metadata() {
        for value in [Value::Null,json!({}),json!({"data":[]}),json!({"data":{}})] {
            assert!(execute_hooks(value).0.is_err());
        }
        for field in ["cwd","hooks","warnings","errors"] {
            let mut missing = hook_inventory(vec![]); missing["data"][0].as_object_mut().unwrap().remove(field);
            assert!(execute_hooks(missing).0.is_err(), "accepted missing {field}");
            let mut malformed = hook_inventory(vec![]); malformed["data"][0][field] = json!(false);
            assert!(execute_hooks(malformed).0.is_err(), "accepted malformed {field}");
        }
        for (field,value) in [("handlerType",json!("shell")),("eventName",json!("futureEvent")),
            ("source",json!("futureSource")),("trustStatus",json!("futureTrust")),("timeoutSec",json!(-1)),
            ("timeoutSec",json!(9_007_199_254_740_992u64)),("enabled",json!("true")),("isManaged",Value::Null),
            ("sourcePath",json!("relative")),("key",json!("")),("currentHash",json!("")),
            ("command",json!(123))] {
            let mut row = hook_row("key","command"); row[field] = value;
            let (result,sent) = execute_hooks(hook_inventory(vec![row]));
            assert!(result.is_err(), "accepted invalid {field}"); assert_eq!(sent.len(),1);
        }
        assert!(execute_hooks(hook_inventory(vec![hook_row("same","command"),hook_row("same","agent")])).0.is_err());
    }
    // HR05: neither enabled entries nor an empty inventory can prove an execution gate.
    #[test]
    fn runtime_extensions_hooks_legacy_empty_and_uninterpreted_metadata_keep_gate_unknown() {
        let mut value = hook_inventory(vec![]);
        value["runtime"] = json!({"featureEnabled":false,"baseHandlerCount":0,"scope":SECRET});
        let result = execute_hooks(value).0.unwrap();
        assert_eq!(result, json!({"operation":"hooksStatus","threadId":THREAD,"provenance":"loadedThreadHooksList",
            "executionGate":"unknown","applicationState":"unknown","runtime":null,
            "hooks":{"items":[],"warningCount":0,"errorCount":0}}));
        assert!(!result.to_string().contains(SECRET));
    }
    // HR06: Hook cardinality is intentionally independent of MCP's 256-array boundary.
    #[test]
    fn runtime_extensions_hooks_keep_complete_inventory_up_to_existing_4096_limit() {
        for count in [257,4096,4097] {
            let rows = (0..count).map(|index| hook_row(&format!("key-{index}"),"command")).collect();
            let (result,sent) = execute_hooks(hook_inventory(rows));
            assert_eq!(sent.len(),1);
            if count <= 4096 { assert_eq!(result.unwrap()["hooks"]["items"].as_array().unwrap().len(),count); }
            else { assert!(result.is_err()); }
        }
    }
    // HR07: large unused content is not a reason to reject a small safe projection.
    #[test]
    fn runtime_extensions_hooks_large_unused_fields_do_not_change_used_field_limits() {
        let mut row = hook_row("key","command");
        row["ignored"] = json!({"description":format!("{SECRET}{}","x".repeat(MAX_RESPONSE_BYTES+1)),
            "array":vec![SECRET;MAX_JSON_ARRAY+1],"schema":(0..=MAX_JSON_FIELDS).map(|i|(format!("field-{i}"),json!(SECRET))).collect::<serde_json::Map<_,_>>()});
        row["command"] = json!("x".repeat(64*1024));
        let result = execute_hooks(hook_inventory(vec![row.clone()])).0.unwrap();
        assert!(!result.to_string().contains(SECRET)); assert!(result["hooks"]["items"][0].get("command").is_none());
        for (field,value) in [("command",json!("x".repeat(64*1024+1))),("command",json!("가".repeat(64*1024/3+1))),
            ("matcher",json!("x".repeat(64*1024+1))),("statusMessage",json!("x".repeat(64*1024+1))),
            ("key",json!("가".repeat(4096/3+1))),("currentHash",json!("x".repeat(4097)))] {
            let mut invalid = row.clone(); invalid[field] = value;
            assert!(execute_hooks(hook_inventory(vec![invalid])).0.is_err(),"accepted oversized {field}");
        }
    }
    // HR08: length validation alone does not make opaque identity/display strings secret-safe.
    #[test]
    fn runtime_extensions_hooks_allowlist_never_exposes_raw_identity_or_diagnostics() {
        let mut command = hook_row(SECRET,"command");
        for field in ["currentHash","command","matcher","statusMessage","pluginId","environmentId"] { command[field] = json!(SECRET); }
        command["sourcePath"] = json!(hook_directory().join(SECRET));
        command["env"] = json!({"TOKEN":SECRET}); command["prompt"] = json!(SECRET);
        let mut mcp = hook_row("second","mcpTool"); mcp["server"] = json!(SECRET); mcp["tool"] = json!(SECRET);
        let mut value = hook_inventory(vec![command,mcp]);
        value["data"][0]["warnings"] = json!([SECRET,SECRET]);
        value["data"][0]["errors"] = json!([{"path":SECRET,"message":SECRET}]);
        let result = execute_hooks(value.clone()).0.unwrap();
        assert!(!result.to_string().contains(SECRET));
        assert_eq!(result["hooks"]["warningCount"],2); assert_eq!(result["hooks"]["errorCount"],1);
        assert_eq!(result["hooks"].as_object().unwrap().len(),3);
        for item in result["hooks"]["items"].as_array().unwrap() { assert_eq!(item.as_object().unwrap().len(),7); }
        for field in ["warnings","errors"] {
            let mut oversized = value.clone();
            oversized["data"][0][field] = json!(vec![value["data"][0][field][0].clone();129]);
            let error = execute_hooks(oversized).0.unwrap_err(); assert!(!error.contains(SECRET));
        }
    }
    // HR09: errors remain errors, without diagnostic reflection or an additional send.
    #[test]
    fn runtime_extensions_hooks_rpc_failure_and_disconnect_never_retry_or_relay_raw_content() {
        for (reply,class) in [(None,"연결"),(Some(json!({"id":1,"error":{"code":-32601,"message":SECRET}})),"지원"),
            (Some(json!({"id":1,"error":{"code":-32000,"message":format!("transport {SECRET}")}})),"연결"),
            (Some(json!({"id":1,"error":{"code":-32000,"message":format!("timed out {SECRET}")}})),"초과"),
            (Some(json!({"id":1,"error":{"code":-32000,"message":format!("cancelled {SECRET}")}})),"취소")] {
            let mut rpc = fixture(reply);
            let error = exchange_hooks(&mut rpc,THREAD,&hook_directory()).unwrap_err();
            assert!(!error.contains(SECRET)); assert!(error.contains("미확인")); assert!(error.contains(class));
            assert_eq!(requests(&rpc).len(),1);
        }
        let mut rpc = fixture(None); rpc.cancelled = Some(Arc::new(AtomicBool::new(true)));
        assert!(exchange_hooks(&mut rpc,THREAD,&hook_directory()).is_err()); assert!(requests(&rpc).is_empty());
    }
    // HR10: shared transport and the Hook projection have separate, finite byte ceilings.
    #[test]
    fn runtime_extensions_hooks_raw_frame_and_projected_byte_limits_are_preserved() {
        let mut value = hook_inventory(vec![]); value["unused"] = json!("x".repeat(MAX_FRAME));
        let mut rpc = fixture(Some(json!({"id":1,"result":value})));
        let error = exchange_hooks(&mut rpc,THREAD,&hook_directory()).unwrap_err();
        assert!(error.contains("미확인")); assert_eq!(requests(&rpc).len(),1);
        assert!(bounded_hooks_response(&json!("x".repeat(MAX_FRAME))).is_err());
        assert!(bounded_hooks_response(&json!("x".repeat(MAX_RESPONSE_BYTES+1))).is_ok());
    }
    // HR11: SHA256('abc') is an external known vector; no product hashing builds the oracle.
    #[test]
    fn runtime_extensions_hooks_readonly_id_is_stable_distinct_and_not_the_mutation_key() {
        let value = hook_inventory(vec![hook_row("abc","command"),hook_row("abd","command")]);
        let first = execute_hooks(value.clone()).0.unwrap(); let second = execute_hooks(value).0.unwrap();
        let items = first["hooks"]["items"].as_array().unwrap();
        assert_eq!(items[0]["id"],"hook-ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_ne!(items[0]["id"],items[1]["id"]); assert_eq!(first,second);
        for item in items { assert!(item.get("key").is_none()); assert!(item.get("currentHash").is_none()); }
    }

    // HF01: legacy inventory remains usable; unscoped metadata is not runtime evidence.
    #[test]
    fn runtime_extensions_hooks_feature_legacy_scope_keeps_runtime_unknown() {
        for scope in [None, Some(Value::Null)] {
            for runtime in [None, Some(Value::Null), Some(json!({"featureEnabled":false,"baseHandlerCount":3}))] {
                let mut value = hook_inventory(vec![hook_row("abc","command")]);
                if let Some(scope) = &scope { value["scope"] = scope.clone(); }
                if let Some(runtime) = runtime { value["runtime"] = runtime; }
                let (result,sent) = execute_hooks(value);
                let result = result.unwrap();
                assert_eq!(result["runtime"],Value::Null); assert!(result.get("runtime").is_some());
                assert_eq!(result["hooks"]["items"].as_array().unwrap().len(),1);
                assert_eq!(result["executionGate"],"unknown"); assert_eq!(result["applicationState"],"unknown");
                assert_eq!(sent,vec![json!({"id":1,"method":"hooks/list","params":{"threadId":THREAD}})]);
            }
        }
    }

    // HF02: feature configuration and installed base count are not execution permission.
    #[test]
    fn runtime_extensions_hooks_current_thread_feature_metadata_is_exact_and_bounded() {
        for runtime in [json!({"featureEnabled":false,"baseHandlerCount":3}),
            json!({"featureEnabled":true,"baseHandlerCount":0}),
            json!({"featureEnabled":true,"baseHandlerCount":9_007_199_254_740_991u64})] {
            let mut value = hook_inventory(vec![]); value["scope"] = json!("currentThread"); value["runtime"] = runtime.clone();
            let (result,sent) = execute_hooks(value); let result = result.unwrap();
            assert_eq!(result["runtime"],runtime); assert_eq!(result["runtime"].as_object().unwrap().len(),2);
            assert_eq!(result["executionGate"],"unknown"); assert_eq!(result["applicationState"],"unknown");
            assert!(result["hooks"]["items"].as_array().unwrap().is_empty()); assert_eq!(sent.len(),1);
        }
    }

    // HF03: explicit configuration/unknown scope cannot masquerade as current-thread inventory.
    #[test]
    fn runtime_extensions_hooks_feature_scope_mismatch_is_safe_and_never_retried() {
        for scope in [json!("configuration"),json!(SECRET),json!(false),json!({"raw":SECRET}),json!([])] {
            let mut value = hook_inventory(vec![hook_row(SECRET,"command")]); value["scope"] = scope;
            value["runtime"] = json!({"featureEnabled":true,"baseHandlerCount":1});
            let (result,sent) = execute_hooks(value);
            assert_eq!(result.unwrap_err(),INVALID_HOOK_RESPONSE); assert_eq!(sent.len(),1);
        }
    }

    // HF04: optional is unknown, malformed is rejected, including when scope is legacy.
    #[test]
    fn runtime_extensions_hooks_feature_runtime_rejects_invalid_types_and_precision() {
        for runtime in [None,Some(Value::Null)] {
            let mut value = hook_inventory(vec![]); value["scope"] = json!("currentThread");
            if let Some(runtime) = runtime { value["runtime"] = runtime; }
            let result = execute_hooks(value).0.unwrap();
            assert!(result.get("runtime").is_some()); assert_eq!(result["runtime"],Value::Null);
        }
        let invalid = [json!(false),json!(SECRET),json!([]),json!({}),
            json!({"featureEnabled":true}),json!({"baseHandlerCount":1}),
            json!({"featureEnabled":null,"baseHandlerCount":1}),json!({"featureEnabled":SECRET,"baseHandlerCount":1}),
            json!({"featureEnabled":1,"baseHandlerCount":1}),json!({"featureEnabled":true,"baseHandlerCount":null}),
            json!({"featureEnabled":true,"baseHandlerCount":SECRET}),json!({"featureEnabled":true,"baseHandlerCount":false}),
            json!({"featureEnabled":true,"baseHandlerCount":-1}),json!({"featureEnabled":true,"baseHandlerCount":0.5}),
            json!({"featureEnabled":true,"baseHandlerCount":9_007_199_254_740_992u64}),
            json!({"featureEnabled":true,"baseHandlerCount":u64::MAX})];
        for scope in [None,Some(json!("currentThread"))] {
            for runtime in &invalid {
                let mut value = hook_inventory(vec![]);
                if let Some(scope) = &scope { value["scope"] = scope.clone(); }
                value["runtime"] = runtime.clone();
                let (result,sent) = execute_hooks(value);
                assert_eq!(result.unwrap_err(),INVALID_HOOK_RESPONSE); assert_eq!(sent.len(),1);
            }
        }
    }

    // HF05: only the two known fields cross Native; raw additions cannot expand the DTO.
    #[test]
    fn runtime_extensions_hooks_feature_projection_drops_unused_raw_content() {
        let mut value = hook_inventory(vec![hook_row(SECRET,"command")]); value["scope"] = json!("currentThread");
        value["runtime"] = json!({"featureEnabled":false,"baseHandlerCount":3,"raw":SECRET,
            "command":format!("{SECRET}{}","x".repeat(MAX_RESPONSE_BYTES+1)),
            "schema":vec![SECRET;MAX_JSON_ARRAY+1],"executionGate":"allowed"});
        let (result,sent) = execute_hooks(value); let result = result.unwrap();
        assert_eq!(result["runtime"],json!({"featureEnabled":false,"baseHandlerCount":3}));
        assert!(!result.to_string().contains(SECRET)); assert_eq!(result["executionGate"],"unknown");
        assert_eq!(result["applicationState"],"unknown");
        assert_eq!(sent,vec![json!({"id":1,"method":"hooks/list","params":{"threadId":THREAD}})]);
    }
}
