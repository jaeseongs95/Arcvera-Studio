//! New-name-only MCP configuration through the CLI's user-layer CAS API.
//! Raw config and credentials stay on this side of the renderer boundary.
use super::*;
use std::collections::BTreeMap;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionMcpServer {
    pub name: String,
    pub transport: &'static str,
    pub enabled: bool,
    pub configured_in: Vec<String>,
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionMcpServers {
    pub items: Vec<ExtensionMcpServer>,
    pub user_file: String,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum McpTransport {
    Stdio { command: String, args: Vec<String>, #[serde(default)] env: BTreeMap<String, String> },
    StreamableHttp { url: String, bearer_token_env_var: Option<String> },
}
impl std::fmt::Debug for McpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("McpTransport(<redacted>)") }
}

fn name(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128 || !value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '@' | '/' | '.')) {
        return Err("MCP 이름은 영문·숫자와 - _ : @ / . 문자로 128자 이내여야 합니다.".into());
    }
    Ok(())
}
fn env_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().enumerate().all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit())
}
fn transport_value(transport: &McpTransport) -> Result<Value, String> {
    match transport {
        McpTransport::Stdio { command, args, env } => {
            argument(command, "MCP command", 4096)?;
            if args.len() > 256 || args.iter().any(|arg| arg.len() > 4096 || arg.contains('\0')) {
                return Err("MCP 인수는 각 4096바이트 이내, 최대 256개이며 NUL을 포함할 수 없습니다.".into());
            }
            if env.len() > 128 || env.iter().any(|(key, value)| !env_name(key) || value.len() > 16 * 1024 || value.contains('\0')) {
                return Err("MCP 환경변수 이름·값 또는 개수 제한을 확인하세요.".into());
            }
            let mut value = json!({"command":command,"args":args});
            if !env.is_empty() { value["env"] = json!(env); }
            Ok(value)
        }
        McpTransport::StreamableHttp { url, bearer_token_env_var } => {
            argument(url, "MCP URL", 8192)?;
            let uri: tungstenite::http::Uri = url.parse().map_err(|_| "MCP URL은 유효한 HTTP(S) 주소여야 합니다.")?;
            if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none_or(str::is_empty)
                || uri.authority().is_some_and(|authority| authority.as_str().contains('@')) || url.contains('#') {
                return Err("MCP URL은 사용자 정보와 fragment가 없는 HTTP(S) 주소여야 합니다.".into());
            }
            let mut value = json!({"url":url});
            if let Some(variable) = bearer_token_env_var {
                if !env_name(variable) { return Err("Bearer token에는 값 대신 올바른 환경변수 이름을 입력하세요.".into()); }
                value["bearer_token_env_var"] = json!(variable);
            }
            Ok(value)
        }
    }
}

// Arbitrary vendor diagnostics may quote credentials. Preserve known failure
// classes, but never return the diagnostic text or the submitted payload.
pub(super) fn safe_error(method: &str, error: &str) -> String {
    let class = if error.contains("configVersionConflict") || error.contains("Configuration was modified since last read") {
        "설정 버전 충돌입니다. 덮어쓰지 않았습니다"
    } else if error.contains("configRequirementReadonly") || error.contains("configLayerReadonly") || error.contains("Only writes to the user config are allowed") || error.contains("is managed by requirements") {
        "관리 정책 또는 읽기 전용 설정이 쓰기를 거부했습니다"
    } else if error.contains("-32601") || error.contains("Method not found") || error.contains("method not found") {
        "선택한 CLI가 MCP 설정 API를 지원하지 않습니다"
    } else if error.contains("configValidationError") || error.contains("configSchemaUnknownKey") || error.contains("-32602") {
        "CLI가 MCP 설정 입력을 거부했습니다"
    } else if error.contains("timed out") || error.contains("기한") {
        "시간 초과로 결과를 확인하지 못했습니다"
    } else if error.contains("transport") || error.contains("disconnect") || error.contains("connection") {
        "CLI 연결 실패로 결과를 확인하지 못했습니다"
    } else { "CLI 요청 실패로 결과를 확인하지 못했습니다" };
    format!("{method}: {class}. 새로 고침으로 실제 설정을 확인하세요.")
}
fn call(rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>, method: &str, params: Value) -> Result<Value, String> {
    rpc(method, params).map_err(|error| safe_error(method, &error))
}
fn table(value: &Value) -> Result<Option<&serde_json::Map<String, Value>>, String> {
    match value.get("mcp_servers") {
        None => Ok(None),
        Some(value) => value.as_object().map(Some).ok_or_else(|| "CLI의 MCP 설정이 객체가 아닙니다. 변경하지 않았습니다.".into()),
    }
}
fn layers(config: &Value) -> Result<&Vec<Value>, String> {
    let layers = rows(config, "layers")?;
    if layers.iter().any(|layer| !layer["config"].is_object()) { return Err("CLI 설정 계층의 구성 객체를 확인할 수 없습니다.".into()); }
    Ok(layers)
}
fn user_layer(config: &Value) -> Result<(&str, &str), String> {
    let mut users = layers(config)?.iter().filter(|layer| layer["name"]["type"] == "user" && layer["name"]["profile"].is_null());
    let layer = users.next().ok_or("MCP 설정을 쓸 사용자 base 계층이 없습니다. 변경하지 않았습니다.")?;
    if users.next().is_some() { return Err("MCP 사용자 base 계층이 중복되었습니다. 변경하지 않았습니다.".into()); }
    if !layer["disabledReason"].is_null() { return Err("MCP 사용자 base 계층이 비활성 상태입니다. 변경하지 않았습니다.".into()); }
    let file = layer["name"]["file"].as_str().filter(|s| !s.is_empty() && s.len() <= 128 * 1024 && Path::new(s).is_absolute())
        .ok_or("MCP 사용자 설정의 절대 경로를 확인할 수 없습니다. 변경하지 않았습니다.")?;
    let version = layer["version"].as_str().filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or("MCP 사용자 설정 버전을 확인할 수 없습니다. 변경하지 않았습니다.")?;
    Ok((file, version))
}
fn read(directory: &Path, rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>) -> Result<Value, String> {
    let value = call(rpc, "config/read", json!({"includeLayers":true,"cwd":directory}))?;
    if !value["config"].is_object() { return Err("CLI의 실제 설정 객체를 확인할 수 없습니다.".into()); }
    Ok(value)
}
fn inventory(config: &Value) -> Result<ExtensionMcpServers, String> {
    let (file, _) = user_layer(config)?;
    let configured = table(&config["config"])?;
    if configured.is_some_and(|servers| servers.len() > MAX_ROWS) { return Err("MCP 서버 목록 상한을 초과했습니다.".into()); }
    let mut items = Vec::new();
    for (server_name, value) in configured.into_iter().flatten() {
        name(server_name)?;
        if !value.is_object() { return Err("MCP 서버 구성을 확인할 수 없습니다.".into()); }
        let transport = match (value.get("command"), value.get("url")) {
            (Some(Value::String(_)), None) => "stdio",
            (None, Some(Value::String(_))) => "streamableHttp",
            _ => "unknown",
        };
        let enabled = match value.get("enabled") { None => true, Some(Value::Bool(value)) => *value, _ => return Err("MCP 활성 상태가 올바르지 않습니다.".into()) };
        let mut configured_in = Vec::new();
        for layer in layers(config)? {
            if table(&layer["config"])?.is_some_and(|servers| servers.contains_key(server_name)) {
                let scope = match layer["name"]["type"].as_str() {
                    Some("user") if !layer["name"]["profile"].is_null() => "profile",
                    Some("user") => "user", Some("project") => "project", _ => "managedOrOther",
                };
                if !configured_in.iter().any(|s| s == scope) { configured_in.push(scope.to_string()); }
            }
        }
        items.push(ExtensionMcpServer { name: server_name.clone(), transport, enabled, configured_in });
    }
    Ok(ExtensionMcpServers { items, user_file: file.into() })
}
pub(super) fn list(directory: &Path, rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>) -> Result<ExtensionMcpServers, String> {
    inventory(&read(directory, rpc)?)
}
pub(super) fn add(directory: &Path, server_name: &str, transport: &McpTransport,
    rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    name(server_name)?;
    let value = transport_value(transport)?;
    let before = read(directory, rpc)?;
    let (file, version) = user_layer(&before)?;
    if table(&before["config"])?.is_some_and(|servers| servers.contains_key(server_name))
        || layers(&before)?.iter().map(|layer| table(&layer["config"])).collect::<Result<Vec<_>, _>>()?.into_iter().flatten().any(|servers| servers.contains_key(server_name)) {
        return Err("같은 이름의 MCP 서버가 이미 설정되어 있습니다. 기존 서버는 덮어쓰지 않았습니다.".into());
    }
    let key = format!("mcp_servers.{}", serde_json::to_string(server_name).map_err(|_| "MCP 이름 인코딩에 실패했습니다.")?);
    let response = call(rpc, "config/value/write", json!({"keyPath":key,"value":value,"mergeStrategy":"upsert","filePath":file,"expectedVersion":version}))?;
    if response["status"] == "okOverridden" || !response["overriddenMetadata"].is_null() {
        return Err("MCP 설정을 저장했지만 상위 설정이 덮어씁니다. 새로 고침으로 실제 구성을 확인하세요.".into());
    }
    if response["status"] != "ok" || response["filePath"].as_str() != Some(file) || response["version"].as_str().is_none_or(str::is_empty) {
        return Err("MCP 쓰기 응답을 확인하지 못했습니다. 저장되었을 수 있으니 재시도 전에 새로 고침하세요.".into());
    }
    let after = read(directory, rpc).map_err(|_| "MCP 설정 저장 후 재조회에 실패했습니다. 저장되었을 수 있으니 재시도 전에 새로 고침하세요.")?;
    let (after_file, _) = user_layer(&after).map_err(|_| "MCP 설정 저장 후 사용자 계층을 확인하지 못했습니다. 재시도 전에 새로 고침하세요.")?;
    if after_file != file { return Err("MCP 저장 후 사용자 설정 대상이 바뀌었습니다. 재시도 전에 새로 고침하세요.".into()); }
    let mut saved_in_user = false;
    for layer in layers(&after)? {
        if let Some(saved) = table(&layer["config"])?.and_then(|servers| servers.get(server_name)) {
            if layer["name"]["type"] != "user" || !layer["name"]["profile"].is_null() {
                return Err("MCP 설정 저장 후 같은 이름이 다른 계층에 나타났습니다. 새로 고침으로 실제 구성을 확인하세요.".into());
            }
            saved_in_user = value.as_object().unwrap().iter().all(|(key, value)| saved.get(key) == Some(value));
        }
    }
    let actual = table(&after["config"])?.and_then(|servers| servers.get(server_name));
    let matched = actual.is_some_and(|actual| value.as_object().unwrap().iter().all(|(key, value)| actual.get(key) == Some(value))
        && actual.get("enabled").is_none_or(|enabled| enabled == &json!(true))
        && if value.get("command").is_some() { actual.get("url").is_none() } else { actual.get("command").is_none() });
    if !saved_in_user || !matched { return Err("MCP 설정을 저장했지만 사용자 파일과 실제 구성을 확인하지 못했습니다. 새로 고침으로 확인하세요.".into()); }
    // No reload: this connection owns a short-lived server, not existing sessions.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Value { json!({"config":{},"layers":[{"name":{"type":"user","profile":null,"file":file()},"version":"v1","config":{}}]}) }
    fn file() -> String { env::temp_dir().join("mcp-fixture-config.toml").to_string_lossy().into_owned() }
    fn stdio() -> McpTransport { McpTransport::Stdio { command:"fixture-tool".into(),args:vec!["literal \"argument\"".into(),"".into()],env:BTreeMap::new() } }
    fn write_ok() -> Value { json!({"status":"ok","filePath":file(),"version":"v2","overriddenMetadata":null}) }
    fn run(before: Value, result: Result<Value, String>, after: Value) -> (Result<(),String>,Vec<(String,Value)>) {
        let mut responses = vec![Ok(before),result,Ok(after)].into_iter(); let mut requests = Vec::new();
        let result = add(Path::new("."),"server.name",&stdio(),&mut |method,params| { requests.push((method.into(),params)); responses.next().expect("unexpected RPC") });
        (result,requests)
    }
    #[test]
    fn new_name_uses_single_quoted_key_cas_and_literal_args_without_rewriting_siblings() {
        let mut before=config(); before["config"]["mcp_servers"]=json!({"kept":{"command":"other","env":{"TOKEN":"SENTINEL"},"unknown":"kept"}});
        let mut after=before.clone(); after["config"]["mcp_servers"]["server.name"]=transport_value(&stdio()).unwrap();
        after["layers"][0]["config"]["mcp_servers"]["server.name"]=transport_value(&stdio()).unwrap();
        let (result,requests)=run(before,Ok(write_ok()),after); result.unwrap();
        assert_eq!(requests.len(),3); assert_eq!(requests[1].0,"config/value/write");
        assert_eq!(requests[1].1,json!({"keyPath":"mcp_servers.\"server.name\"","value":{"command":"fixture-tool","args":["literal \"argument\"",""]},"mergeStrategy":"upsert","filePath":file(),"expectedVersion":"v1"}));
        assert!(!serde_json::to_string(&requests[1]).unwrap().contains("SENTINEL"));
    }
    #[test]
    fn duplicate_in_effective_or_any_layer_never_writes() {
        for in_layer in [false,true] { let mut before=config(); if in_layer { before["layers"][0]["config"]["mcp_servers"]=json!({"server.name":{"command":"kept"}}); } else { before["config"]["mcp_servers"]=json!({"server.name":{"command":"kept"}}); }
            let (result,requests)=run(before,Ok(write_ok()),config()); assert!(result.unwrap_err().contains("이미")); assert_eq!(requests.len(),1);
        }
    }
    #[test]
    fn missing_duplicate_relative_profile_or_disabled_user_layer_never_writes() {
        let mut variants=Vec::new(); let mut missing=config();missing["layers"]=json!([]);variants.push(missing);
        let mut duplicate=config();let layer=duplicate["layers"][0].clone();duplicate["layers"].as_array_mut().unwrap().push(layer);variants.push(duplicate);
        let mut relative=config();relative["layers"][0]["name"]["file"]=json!("relative.toml");variants.push(relative);
        let mut profile=config();profile["layers"][0]["name"]["profile"]=json!("other");variants.push(profile);
        let mut disabled=config();disabled["layers"][0]["disabledReason"]=json!("fixture");variants.push(disabled);
        for before in variants { let (result,requests)=run(before,Ok(write_ok()),config());assert!(result.is_err());assert_eq!(requests.len(),1); }
    }
    #[test]
    fn sanitized_inventory_and_errors_never_return_raw_secrets() {
        let mut input=config();input["config"]["mcp_servers"]=json!({"safe":{"url":"https://host/SENTINEL","env":{"TOKEN":"SENTINEL"},"oauth":{"client_secret":"SENTINEL"}}});
        let serialized=serde_json::to_string(&inventory(&input).unwrap()).unwrap();assert!(!serialized.contains("SENTINEL"));assert!(serialized.contains("streamableHttp"));
        for (message,class) in [("configVersionConflict SENTINEL","충돌"),("configLayerReadonly SENTINEL","읽기 전용"),("-32601 SENTINEL","지원"),("timed out SENTINEL","시간 초과"),("disconnect SENTINEL","연결 실패")] {
            let (result,requests)=run(config(),Err(message.into()),config());let error=result.unwrap_err();assert!(error.contains(class));assert!(!error.contains("SENTINEL"));assert_eq!(requests.len(),2);
        }
    }
    #[test]
    fn overridden_unknown_reply_or_mismatched_readback_is_not_success() {
        for response in [json!({"status":"okOverridden"}),json!({"status":"ok","version":"v2","filePath":"wrong"}),write_ok()] {
            let (result,_)=run(config(),Ok(response),config());assert!(result.is_err());
        }
        let mut responses=vec![Ok(config()),Ok(write_ok()),Err("disconnect SENTINEL".into())].into_iter();
        let error=add(Path::new("."),"server.name",&stdio(),&mut |_,_|responses.next().unwrap()).unwrap_err();
        assert!(error.contains("저장 후 재조회"));assert!(!error.contains("SENTINEL"));
    }
    #[test]
    fn http_secret_values_stay_in_one_new_key_and_are_verified_in_the_user_layer() {
        let transport=McpTransport::StreamableHttp {url:"https://fixture.invalid/mcp?token=SENTINEL".into(),bearer_token_env_var:Some("FIXTURE_TOKEN".into())};
        let mut after=config();let value=transport_value(&transport).unwrap();
        after["config"]["mcp_servers"]["fixture-http"]=value.clone();after["layers"][0]["config"]["mcp_servers"]["fixture-http"]=value;
        let mut responses=vec![Ok(config()),Ok(write_ok()),Ok(after.clone())].into_iter();let mut writes=Vec::new();
        add(Path::new("."),"fixture-http",&transport,&mut |method,params| { if method=="config/value/write" { writes.push(params); } responses.next().unwrap() }).unwrap();
        assert_eq!(writes.len(),1);assert_eq!(writes[0]["keyPath"],"mcp_servers.\"fixture-http\"");
        assert!(!serde_json::to_string(&inventory(&after).unwrap()).unwrap().contains("SENTINEL"));
        let project=json!({"name":{"type":"project","dotCodexFolder":env::temp_dir()},"config":{"mcp_servers":{"fixture-http":{"url":"https://fixture.invalid/mcp?token=SENTINEL"}}},"version":"project-v1"});
        after["layers"].as_array_mut().unwrap().push(project);
        let mut responses=vec![Ok(config()),Ok(write_ok()),Ok(after)].into_iter();
        assert!(add(Path::new("."),"fixture-http",&transport,&mut |_,_|responses.next().unwrap()).unwrap_err().contains("다른 계층"));
    }
    #[test]
    fn invalid_inputs_are_rejected_before_vendor_io_and_transport_fields_cannot_mix() {
        for invalid in ["","space name","quote\"","line\n",&"x".repeat(129)] { assert!(add(Path::new("."),invalid,&stdio(),&mut |_,_|panic!("invalid input reached RPC")).is_err()); }
        for input in [json!({"type":"stdio","command":"tool","args":[],"url":"https://host"}),json!({"type":"streamableHttp","url":"https://host","env":{}})] { assert!(serde_json::from_value::<McpTransport>(input).is_err()); }
        for url in ["file:///tmp/a","https://user:secret@host/path","https://host/path#fragment","not-a-url"] { assert!(transport_value(&McpTransport::StreamableHttp {url:url.into(),bearer_token_env_var:None}).is_err()); }
        let transport=McpTransport::StreamableHttp {url:"https://fixture.invalid/mcp?key=SENTINEL".into(),bearer_token_env_var:Some("FIXTURE_TOKEN".into())};
        assert_eq!(transport_value(&transport).unwrap(),json!({"url":"https://fixture.invalid/mcp?key=SENTINEL","bearer_token_env_var":"FIXTURE_TOKEN"}));
        assert!(!format!("{transport:?}").contains("SENTINEL"));
    }
}
