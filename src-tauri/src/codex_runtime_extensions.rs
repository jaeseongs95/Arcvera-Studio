//! Narrow MCP runtime wire adapter. The caller owns session admission and transport lifetime.
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

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub(super) enum RuntimeExtensionRequest {
    ReloadMcp {},
    McpStatus { server_name: String, cursor: Option<String> },
}

/// Validate before the caller opens the admitted vendor connection.
pub(super) fn validate(thread_id: &str, request: &RuntimeExtensionRequest) -> Result<(), String> {
    identifier(thread_id, MAX_THREAD_BYTES)
        .map_err(|_| "현재 대화의 MCP 요청 대상을 확인할 수 없습니다.".to_owned())?;
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
    };
    let value = rpc.call(method, params).map_err(|error| safe_rpc_error(request, &error))?;
    same_thread(&value, thread_id)?;
    let result = match request {
        RuntimeExtensionRequest::ReloadMcp {} => reload_result(&value, thread_id),
        RuntimeExtensionRequest::McpStatus { server_name, cursor } => status_result(&value, thread_id, server_name, cursor.as_deref()),
    }?;
    // Raw tools may contain large descriptions/schemas inside the shared Rpc frame limit.
    // Bound only the validated renderer projection; never forward unused tool content.
    bounded_response(&result)?;
    Ok(result)
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
}
