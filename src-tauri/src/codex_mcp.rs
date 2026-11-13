//! MCP configuration through the CLI's user-layer CAS API.
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
    pub editable: bool,
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionMcpServers {
    pub items: Vec<ExtensionMcpServer>,
    pub user_file: String,
    pub config_version: String,
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

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerPatch {
    pub transport: Option<McpTransportPatch>,
    pub enabled: Option<bool>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum McpTransportPatch {
    Stdio {
        command: Option<String>, args: Option<Vec<String>>, env: Option<BTreeMap<String, String>>,
        #[serde(default)] clear_env: bool,
    },
    StreamableHttp {
        url: Option<String>, bearer_token_env_var: Option<String>,
        #[serde(default)] clear_bearer_token_env_var: bool,
    },
}
impl std::fmt::Debug for McpServerPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("McpServerPatch(<redacted>)") }
}
impl std::fmt::Debug for McpTransportPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("McpTransportPatch(<redacted>)") }
}

// Core config/src/mcp_types.rs RawMcpServerConfig and TryFrom validation:
// these are the fields rejected by the destination transport, not a general cleanup list.
const HTTP_ONLY: &[&str] = &["url", "bearer_token_env_var", "bearer_token", "http_headers_helper", "http_headers", "env_http_headers", "oauth", "oauth_resource", "auth"];
const STDIO_ONLY: &[&str] = &["command", "args", "env", "env_vars", "cwd"];

fn patched(original: &Value, patch: &McpServerPatch) -> Result<Value, String> {
    let mut value = original.as_object().cloned().ok_or("MCP 서버 구성이 객체가 아닙니다. 변경하지 않았습니다.")?;
    let stdio = match (value.get("command"), value.get("url")) {
        (Some(Value::String(_)), None) => true,
        (None, Some(Value::String(_))) => false,
        _ => return Err("MCP 서버의 전송 방식을 확인할 수 없습니다. 변경하지 않았습니다.".into()),
    };
    if let Some(transport) = &patch.transport {
        match transport {
            McpTransportPatch::Stdio { command, args, env, clear_env } => {
                if *clear_env && env.is_some() { return Err("MCP 환경변수 설정과 삭제를 동시에 요청할 수 없습니다.".into()); }
                if !stdio && command.is_none() { return Err("STDIO 전송으로 변경하려면 새 command가 필요합니다.".into()); }
                if let Some(command) = command { argument(command, "MCP command", 4096)?; }
                if let Some(args) = args { validate_args(args)?; }
                if let Some(env) = env { validate_env(env)?; }
                if !stdio { for key in HTTP_ONLY { value.remove(*key); } }
                if let Some(command) = command { value.insert("command".into(), json!(command)); }
                if let Some(args) = args { value.insert("args".into(), json!(args)); }
                if let Some(env) = env { value.insert("env".into(), json!(env)); }
                if *clear_env { value.remove("env"); }
            }
            McpTransportPatch::StreamableHttp { url, bearer_token_env_var, clear_bearer_token_env_var } => {
                if *clear_bearer_token_env_var && bearer_token_env_var.is_some() { return Err("Bearer 환경변수 설정과 삭제를 동시에 요청할 수 없습니다.".into()); }
                if stdio && url.is_none() { return Err("HTTP 전송으로 변경하려면 새 URL이 필요합니다.".into()); }
                if let Some(url) = url { validate_url(url)?; }
                if bearer_token_env_var.as_ref().is_some_and(|variable| !env_name(variable)) { return Err("Bearer token에는 값 대신 올바른 환경변수 이름을 입력하세요.".into()); }
                if stdio { for key in STDIO_ONLY { value.remove(*key); } }
                if let Some(url) = url { value.insert("url".into(), json!(url)); }
                if let Some(variable) = bearer_token_env_var { value.insert("bearer_token_env_var".into(), json!(variable)); }
                if *clear_bearer_token_env_var { value.remove("bearer_token_env_var"); }
            }
        }
    }
    if let Some(enabled) = patch.enabled { value.insert("enabled".into(), json!(enabled)); }
    Ok(Value::Object(value))
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
fn validate_args(args: &[String]) -> Result<(), String> {
    if args.len() > 256 || args.iter().any(|arg| arg.len() > 4096 || arg.contains('\0')) {
        return Err("MCP 인수는 각 4096바이트 이내, 최대 256개이며 NUL을 포함할 수 없습니다.".into());
    }
    Ok(())
}
fn validate_env(env: &BTreeMap<String, String>) -> Result<(), String> {
    if env.len() > 128 || env.iter().any(|(key, value)| !env_name(key) || value.len() > 16 * 1024 || value.contains('\0')) {
        return Err("MCP 환경변수 이름·값 또는 개수 제한을 확인하세요.".into());
    }
    Ok(())
}
fn validate_url(url: &str) -> Result<(), String> {
    argument(url, "MCP URL", 8192)?;
    let uri: tungstenite::http::Uri = url.parse().map_err(|_| "MCP URL은 유효한 HTTP(S) 주소여야 합니다.")?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none_or(str::is_empty)
        || uri.authority().is_some_and(|authority| authority.as_str().contains('@')) || url.contains('#') {
        return Err("MCP URL은 사용자 정보와 fragment가 없는 HTTP(S) 주소여야 합니다.".into());
    }
    Ok(())
}
fn transport_value(transport: &McpTransport) -> Result<Value, String> {
    match transport {
        McpTransport::Stdio { command, args, env } => {
            argument(command, "MCP command", 4096)?;
            validate_args(args)?;
            validate_env(env)?;
            let mut value = json!({"command":command,"args":args});
            if !env.is_empty() { value["env"] = json!(env); }
            Ok(value)
        }
        McpTransport::StreamableHttp { url, bearer_token_env_var } => {
            validate_url(url)?;
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
    let (file, version) = user_layer(config)?;
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
        let editable = editable_server(config, server_name).is_ok();
        items.push(ExtensionMcpServer { name: server_name.clone(), transport, enabled, configured_in, editable });
    }
    Ok(ExtensionMcpServers { items, user_file: file.into(), config_version: version.into() })
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
    // No reload: this helper owns only a proxy to the shared Core.
    Ok(())
}

fn editable_server<'a>(config: &'a Value, server_name: &str) -> Result<&'a Value, String> {
    user_layer(config)?;
    let mut found = None;
    for layer in layers(config)? {
        if let Some(server) = table(&layer["config"])?.and_then(|servers| servers.get(server_name)) {
            if layer["name"]["type"] != "user" || !layer["name"]["profile"].is_null() || !layer["disabledReason"].is_null() || found.is_some() {
                return Err("사용자 base 단독 MCP 서버만 수정·삭제할 수 있습니다. 변경하지 않았습니다.".into());
            }
            found = Some(server);
        }
    }
    let saved = found.filter(|value| value.is_object()).ok_or("사용자 base MCP 서버를 확인할 수 없습니다. 변경하지 않았습니다.")?;
    let source = saved.as_object().unwrap();
    let effective = table(&config["config"])?.and_then(|servers| servers.get(server_name)).and_then(Value::as_object);
    // ceiling: accept only the three additive defaults observed in actual Core1.0.1.
    // Keep the raw user object for patches/readback; never persist projected defaults.
    let matches = effective.is_some_and(|effective| {
        source.iter().all(|(key, value)| effective.get(key) == Some(value))
            && effective.iter().all(|(key, value)| source.contains_key(key) || match key.as_str() {
                "enabled" => value.as_bool() == Some(true),
                "environment_id" => value.as_str() == Some("local"),
                "tool_timeout_sec" => value.is_null(),
                _ => false,
            })
    });
    if !matches {
        return Err("MCP 실제 구성과 사용자 base 구성이 다릅니다. 변경하지 않았습니다.".into());
    }
    Ok(saved)
}

fn change(directory: &Path, server_name: &str, expected_version: &str, patch: Option<&McpServerPatch>,
    rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    name(server_name)?;
    if expected_version.is_empty() || expected_version.len() > 4096 { return Err("검토한 MCP 설정 버전이 필요합니다. 새로 고침하세요.".into()); }
    let before = read(directory, rpc)?;
    let (file, version) = user_layer(&before)?;
    if version != expected_version { return Err("설정 버전 충돌입니다. 덮어쓰지 않았습니다. 새로 고침 후 다시 검토하세요.".into()); }
    let original = editable_server(&before, server_name)?;
    let value = match patch { Some(patch) => patched(original, patch)?, None => Value::Null };
    let key = format!("mcp_servers.{}", serde_json::to_string(server_name).map_err(|_| "MCP 이름 인코딩에 실패했습니다.")?);
    let response = call(rpc, "config/value/write", json!({"keyPath":key,"value":value,"mergeStrategy":"replace","filePath":file,"expectedVersion":expected_version}))?;
    if response["status"] == "okOverridden" || !response["overriddenMetadata"].is_null() {
        return Err("MCP 변경을 저장했지만 상위 설정이 덮어씁니다. 새로 고침으로 실제 구성을 확인하세요.".into());
    }
    let written_version = response["version"].as_str().filter(|v| !v.is_empty() && v.len() <= 4096);
    if response["status"] != "ok" || response["filePath"].as_str() != Some(file) || written_version.is_none() {
        return Err("MCP 쓰기 응답을 확인하지 못했습니다. 저장되었을 수 있으니 재시도 전에 새로 고침하세요.".into());
    }
    let after = read(directory, rpc).map_err(|_| "MCP 변경 후 재조회에 실패했습니다. 저장되었을 수 있으니 재시도 전에 새로 고침하세요.")?;
    let (after_file, after_version) = user_layer(&after).map_err(|_| "MCP 변경 후 사용자 계층을 확인하지 못했습니다. 재시도 전에 새로 고침하세요.")?;
    if after_file != file || Some(after_version) != written_version {
        return Err("MCP 변경 후 설정 대상 또는 버전이 달라졌습니다. 재시도 전에 새로 고침하세요.".into());
    }
    let user = |config: &Value| -> Result<Value, String> {
        let layer = layers(config)?.iter().find(|layer| layer["name"]["type"] == "user" && layer["name"]["profile"].is_null()).ok_or("사용자 계층을 확인하지 못했습니다.")?;
        Ok(layer["config"].clone())
    };
    let mut expected_user = user(&before)?;
    let servers = expected_user.get_mut("mcp_servers").and_then(Value::as_object_mut).ok_or("사용자 MCP 설정을 확인하지 못했습니다.")?;
    if value.is_null() { servers.remove(server_name); } else { servers.insert(server_name.into(), value.clone()); }
    let mut actual_user = user(&after)?;
    // Core may remove the now-empty table when deleting its final entry.
    for config in [&mut expected_user, &mut actual_user] {
        if config.get("mcp_servers").and_then(Value::as_object).is_some_and(|servers| servers.is_empty()) { config.as_object_mut().unwrap().remove("mcp_servers"); }
    }
    if actual_user != expected_user { return Err("MCP 변경 후 사용자 구성의 정확한 보존을 확인하지 못했습니다. 새로 고침하세요.".into()); }
    if value.is_null() {
        if table(&after["config"])?.is_some_and(|servers| servers.contains_key(server_name))
            || layers(&after)?.iter().map(|layer| table(&layer["config"])).collect::<Result<Vec<_>, _>>()?.into_iter().flatten().any(|servers| servers.contains_key(server_name)) {
            return Err("MCP 삭제 후 같은 이름의 구성이 남아 있습니다. 새로 고침으로 실제 구성을 확인하세요.".into());
        }
    } else if editable_server(&after, server_name).map_err(|_| "MCP 변경 후 사용자 단독 구성을 확인하지 못했습니다. 새로 고침하세요.")? != &value {
        return Err("MCP 변경 후 실제 구성이 일치하지 않습니다. 새로 고침하세요.".into());
    }
    Ok(())
}

pub(super) fn edit(directory: &Path, server_name: &str, expected_version: &str, patch: &McpServerPatch,
    rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    change(directory, server_name, expected_version, Some(patch), rpc)
}
pub(super) fn remove(directory: &Path, server_name: &str, expected_version: &str,
    rpc: &mut impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    change(directory, server_name, expected_version, None, rpc)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "actual-core-fixture")]
    #[test]
    fn actual_core_user_layer_semantics() {
        let expected_home = std::path::PathBuf::from(env::var_os("ARCVERA_STUDIO_ACTUAL_CORE_HOME").expect("explicit owned Core fixture home required"));
        assert!(expected_home.is_absolute());
        assert_eq!(std::fs::canonicalize(&expected_home).unwrap(), std::fs::canonicalize(env::var_os("ARCVERA_HOME").expect("isolated Core home required")).unwrap());
        let cli = env::var_os("ARCVERA_STUDIO_ACTUAL_CORE_EXE").expect("explicit public Core executable required");
        let cwd = expected_home.parent().unwrap().join("project");
        super::super::with_rpc(&cli, &cwd, |rpc| {
            let config = read(&cwd, &mut |method, params| rpc.call(method, params))?;
            let inventory = inventory(&config)?;
            let targets: Vec<_> = inventory.items.iter().filter(|item| item.name.starts_with("desktop210_probe_")).collect();
            assert!(!targets.is_empty(), "the isolated actual fixture must contain its prepared user MCP server");
            for item in targets {
                let saved = layers(&config)?.iter().find(|layer| layer["name"]["type"] == "user" && layer["name"]["profile"].is_null()).unwrap()["config"]["mcp_servers"][&item.name].as_object().unwrap();
                let effective = config["config"]["mcp_servers"][&item.name].as_object().unwrap();
                let keys: std::collections::BTreeSet<_> = saved.keys().chain(effective.keys()).collect();
                let differences: Vec<_> = keys.into_iter().filter(|key| saved.get(*key) != effective.get(*key)).collect();
                // Only field names and bounded DTO flags; never raw config values.
                println!("actual Core MCP projection {}", json!({"name":item.name,"sourceKeys":saved.keys().collect::<Vec<_>>(),"effectiveKeys":effective.keys().collect::<Vec<_>>(),"differentKeys":differences,"transport":item.transport,"editable":item.editable}));
                assert!(item.editable && item.transport == "stdio", "own user-only STDIO configuration must remain editable");
            }
            Ok(())
        }).unwrap();
    }
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

    fn saved(server: Value) -> Value {
        let mut config = config();
        config["config"] = json!({"mcp_servers":{"server.name":server.clone(),"sibling":{"command":"sibling","env":{"TOKEN":"SIBLING_SENTINEL"}}},"unrelated":{"keep":"yes"}});
        config["layers"][0]["config"] = config["config"].clone();
        config
    }
    fn after_change(before: &Value, server: Option<Value>) -> Value {
        let mut after = before.clone();
        after["layers"][0]["version"] = json!("v2");
        let servers = after["config"]["mcp_servers"].as_object_mut().unwrap();
        match server { Some(value) => { servers.insert("server.name".into(), value); }, None => { servers.remove("server.name"); } }
        after["layers"][0]["config"] = after["config"].clone();
        after
    }
    fn patch(value: Value) -> McpServerPatch { serde_json::from_value(value).unwrap() }
    fn run_change(before: Value, expected_version: &str, patch: Option<&McpServerPatch>, write: Result<Value, String>, after: Result<Value, String>) -> (Result<(), String>, Vec<(String, Value)>) {
        let mut responses = vec![Ok(before), write, after].into_iter();
        let mut requests = Vec::new();
        let mut rpc = |method: &str, params: Value| { requests.push((method.into(), params)); responses.next().expect("unexpected RPC") };
        let result = match patch {
            Some(patch) => edit(Path::new("."), "server.name", expected_version, patch, &mut rpc),
            None => remove(Path::new("."), "server.name", expected_version, &mut rpc),
        };
        (result, requests)
    }
    fn with_observed_defaults(mut config: Value) -> Value {
        let effective = config["config"]["mcp_servers"]["server.name"].as_object_mut().unwrap();
        for (key, value) in [("enabled", json!(true)), ("environment_id", json!("local")), ("tool_timeout_sec", Value::Null)] {
            effective.entry(key.to_string()).or_insert(value);
        }
        config
    }
    #[test]
    fn observed_core_defaults_allow_inventory_edit_and_remove_without_persisting_defaults() {
        let original = json!({"command":"fixture","args":["kept"],"env":{"TOKEN":"SECRET_SENTINEL"},"unknown":{"keep":true}});
        let before = with_observed_defaults(saved(original.clone()));
        let inventory = inventory(&before).unwrap();
        let row = inventory.items.iter().find(|row| row.name == "server.name").unwrap();
        assert!(row.editable && row.transport == "stdio");
        let mut expected = original.clone(); expected["args"] = json!(["new"]);
        let after = with_observed_defaults(after_change(&before, Some(expected.clone())));
        let (result, requests) = run_change(before.clone(), "v1", Some(&patch(json!({"transport":{"type":"stdio","args":["new"]}}))), Ok(write_ok()), Ok(after));
        result.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1], ("config/value/write".into(), json!({"keyPath":"mcp_servers.\"server.name\"","value":expected,"mergeStrategy":"replace","filePath":file(),"expectedVersion":"v1"})));
        assert!(!serde_json::to_string(&requests[1]).unwrap().contains("SIBLING_SENTINEL"));
        let (result, requests) = run_change(before.clone(), "v1", None, Ok(write_ok()), Ok(after_change(&before, None)));
        result.unwrap(); assert_eq!(requests.len(), 3); assert!(requests[1].1["value"].is_null());
        assert_eq!(before["layers"][0]["config"]["mcp_servers"]["server.name"], original);
    }
    #[test]
    fn observed_defaults_do_not_allow_other_differences_or_missing_source_values() {
        let original = with_observed_defaults(saved(json!({"command":"fixture","args":["kept"],"env":{"TOKEN":"SECRET_SENTINEL"},"unknown":"UNKNOWN_SENTINEL"})));
        let mut variants = Vec::new();
        for (key, value) in [("enabled", json!(false)), ("enabled", json!("true")), ("enabled", Value::Null),
            ("environment_id", json!("remote")), ("environment_id", json!(true)), ("environment_id", Value::Null),
            ("tool_timeout_sec", json!(0)), ("tool_timeout_sec", json!("null")), ("tool_timeout_sec", json!(false)),
            ("arbitrary", Value::Null), ("required", json!(false)), ("args", json!([])),
            ("env", json!({"TOKEN":"CHANGED_SENTINEL"})), ("unknown", json!("CHANGED_SENTINEL"))] {
            let mut config = original.clone(); config["config"]["mcp_servers"]["server.name"][key] = value; variants.push(config);
        }
        for key in ["command", "args", "env", "unknown"] {
            let mut config = original.clone(); config["config"]["mcp_servers"]["server.name"].as_object_mut().unwrap().remove(key); variants.push(config);
        }
        for (key, value) in [("enabled", json!(false)), ("environment_id", json!("remote")), ("tool_timeout_sec", json!(3))] {
            let mut config = original.clone(); config["layers"][0]["config"]["mcp_servers"]["server.name"][key] = value; variants.push(config);
        }
        for (key, value) in [("enabled", json!(true)), ("environment_id", json!("local")), ("tool_timeout_sec", Value::Null)] {
            let mut config = original.clone();
            config["layers"][0]["config"]["mcp_servers"]["server.name"][key] = value;
            config["config"]["mcp_servers"]["server.name"].as_object_mut().unwrap().remove(key);
            variants.push(config);
        }
        let mut missing_args = with_observed_defaults(saved(json!({"command":"fixture"})));
        missing_args["config"]["mcp_servers"]["server.name"]["args"] = json!([]); variants.push(missing_args);
        let patch = patch(json!({"enabled":false}));
        for before in variants {
            assert!(editable_server(&before, "server.name").is_err());
            if let Ok(inventory) = inventory(&before) { assert!(!inventory.items.iter().find(|row| row.name == "server.name").unwrap().editable); }
            for operation in [Some(&patch), None] {
                let (result, requests) = run_change(before.clone(), "v1", operation, Ok(write_ok()), Ok(config()));
                let error = result.unwrap_err(); assert!(!error.contains("SENTINEL")); assert_eq!(requests.len(), 1);
            }
        }
    }
    #[test]
    fn observed_defaults_preserve_scope_version_and_complete_readback_guards() {
        let original = json!({"command":"fixture","args":[],"env":{"TOKEN":"SECRET_SENTINEL"},"unknown":"UNKNOWN_SENTINEL"});
        let before = with_observed_defaults(saved(original.clone()));
        let patch = patch(json!({"transport":{"type":"stdio","args":["new"]}}));
        let mut variants = Vec::new();
        for kind in ["project", "system", "sessionFlags", "unknown"] {
            let mut config = before.clone(); config["layers"].as_array_mut().unwrap().push(json!({"name":{"type":kind},"version":"other","config":{"mcp_servers":{"server.name":original}}})); variants.push(config);
        }
        let mut profile = before.clone(); profile["layers"][0]["name"]["profile"] = json!("other"); variants.push(profile);
        let mut disabled = before.clone(); disabled["layers"][0]["disabledReason"] = json!("managed"); variants.push(disabled);
        let mut duplicate = before.clone(); let layer = duplicate["layers"][0].clone(); duplicate["layers"].as_array_mut().unwrap().push(layer); variants.push(duplicate);
        for config in variants {
            for operation in [Some(&patch), None] {
                let (result, requests) = run_change(config.clone(), "v1", operation, Ok(write_ok()), Ok(config.clone()));
                assert!(result.is_err()); assert_eq!(requests.len(), 1);
            }
        }
        for operation in [Some(&patch), None] {
            let (result, requests) = run_change(before.clone(), "stale", operation, Ok(write_ok()), Ok(config()));
            assert!(result.is_err()); assert_eq!(requests.len(), 1);
        }
        let mut expected = original; expected["args"] = json!(["new"]);
        let after = with_observed_defaults(after_change(&before, Some(expected)));
        let mut variants = Vec::new();
        for key in ["env", "unknown"] {
            let mut config = after.clone(); config["layers"][0]["config"]["mcp_servers"]["server.name"].as_object_mut().unwrap().remove(key); variants.push(config);
            let mut config = after.clone(); config["config"]["mcp_servers"]["server.name"].as_object_mut().unwrap().remove(key); variants.push(config);
        }
        let mut sibling = after.clone(); sibling["layers"][0]["config"]["mcp_servers"].as_object_mut().unwrap().remove("sibling"); variants.push(sibling);
        let mut default = after; default["config"]["mcp_servers"]["server.name"]["enabled"] = json!(false); variants.push(default);
        for after in variants {
            let (result, requests) = run_change(before.clone(), "v1", Some(&patch), Ok(write_ok()), Ok(after));
            let error = result.unwrap_err(); assert!(!error.contains("SENTINEL")); assert_eq!(requests.len(), 3);
        }
    }
    #[test]
    fn reviewed_version_conflict_blocks_edit_and_delete_without_a_write() {
        let before = saved(json!({"command":"fixture"}));
        let patch = patch(json!({"enabled":false}));
        for operation in [Some(&patch), None] {
            let (result, requests) = run_change(before.clone(), "stale", operation, Ok(write_ok()), Ok(config()));
            assert!(result.unwrap_err().contains("버전 충돌"));
            assert_eq!(requests.len(), 1);
        }
    }
    #[test]
    fn edit_replaces_only_one_quoted_key_and_preserves_raw_secrets_unknowns_and_siblings() {
        let before = saved(json!({"url":"https://fixture.invalid/mcp?token=SENTINEL","bearer_token_env_var":"EXISTING_TOKEN","http_headers":{"Authorization":"SENTINEL"},"oauth":{"client_secret":"SENTINEL"},"unknown":{"nested":[1,2]},"enabled":true}));
        let expected = json!({"url":"https://fixture.invalid/mcp?token=SENTINEL","bearer_token_env_var":"EXISTING_TOKEN","http_headers":{"Authorization":"SENTINEL"},"oauth":{"client_secret":"SENTINEL"},"unknown":{"nested":[1,2]},"enabled":false});
        let after = after_change(&before, Some(expected.clone()));
        let (result, requests) = run_change(before, "v1", Some(&patch(json!({"enabled":false}))), Ok(write_ok()), Ok(after.clone()));
        result.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1], ("config/value/write".into(), json!({"keyPath":"mcp_servers.\"server.name\"","value":expected,"mergeStrategy":"replace","filePath":file(),"expectedVersion":"v1"})));
        assert!(!serde_json::to_string(&requests[1]).unwrap().contains("SIBLING_SENTINEL"));
        let inventory = inventory(&after).unwrap();
        assert_eq!(inventory.config_version, "v2");
        assert!(inventory.items.iter().find(|row| row.name == "server.name").unwrap().editable);
        assert!(!serde_json::to_string(&inventory).unwrap().contains("SENTINEL"));
    }
    #[test]
    fn patch_omission_empty_and_explicit_clear_have_distinct_results() {
        let before = saved(json!({"command":"fixture","args":["kept"],"env":{"TOKEN":"SENTINEL"},"cwd":"kept","unknown":true}));
        for (input, expected) in [
            (json!({"transport":{"type":"stdio","args":[]}}), json!({"command":"fixture","args":[],"env":{"TOKEN":"SENTINEL"},"cwd":"kept","unknown":true})),
            (json!({"transport":{"type":"stdio","env":{}}}), json!({"command":"fixture","args":["kept"],"env":{},"cwd":"kept","unknown":true})),
            (json!({"transport":{"type":"stdio","clearEnv":true}}), json!({"command":"fixture","args":["kept"],"cwd":"kept","unknown":true})),
        ] {
            let after = after_change(&before, Some(expected.clone()));
            let (result, requests) = run_change(before.clone(), "v1", Some(&patch(input)), Ok(write_ok()), Ok(after));
            result.unwrap(); assert_eq!(requests[1].1["value"], expected);
        }
        let http = saved(json!({"url":"https://fixture.invalid","bearer_token_env_var":"TOKEN","http_headers":{"X-Key":"SENTINEL"}}));
        let expected = json!({"url":"https://fixture.invalid","http_headers":{"X-Key":"SENTINEL"}});
        let after = after_change(&http, Some(expected.clone()));
        let (result, requests) = run_change(http, "v1", Some(&patch(json!({"transport":{"type":"streamableHttp","clearBearerTokenEnvVar":true}}))), Ok(write_ok()), Ok(after));
        result.unwrap(); assert_eq!(requests[1].1["value"], expected);
    }
    #[test]
    fn transport_switch_removes_core_rejected_fields_but_keeps_common_and_unknown_fields() {
        let http = saved(json!({"url":"https://fixture.invalid?secret=SENTINEL","bearer_token_env_var":"TOKEN","bearer_token":"SENTINEL","http_headers_helper":"SENTINEL","http_headers":{"X-Key":"SENTINEL"},"env_http_headers":{"X-Key":"TOKEN"},"oauth":{"client_secret":"SENTINEL"},"oauth_resource":"resource","auth":"ema_auth","scopes":["scope"],"environment_id":"local","tool_timeout_sec":7,"unknown":{"secret":"PRESERVED"}}));
        let stdio_expected = json!({"command":"new-command","args":["literal argument",""],"scopes":["scope"],"environment_id":"local","tool_timeout_sec":7,"unknown":{"secret":"PRESERVED"}});
        let after = after_change(&http, Some(stdio_expected.clone()));
        let (result, requests) = run_change(http, "v1", Some(&patch(json!({"transport":{"type":"stdio","command":"new-command","args":["literal argument",""]}}))), Ok(write_ok()), Ok(after));
        result.unwrap(); assert_eq!(requests[1].1["value"], stdio_expected);
        let stdio = saved(json!({"command":"fixture","args":["SENTINEL"],"env":{"TOKEN":"SENTINEL"},"env_vars":["TOKEN"],"cwd":"kept","enabled":false,"required":true,"unknown":"PRESERVED"}));
        let http_expected = json!({"url":"https://new.invalid/mcp","bearer_token_env_var":"NEW_TOKEN","enabled":false,"required":true,"unknown":"PRESERVED"});
        let after = after_change(&stdio, Some(http_expected.clone()));
        let (result, requests) = run_change(stdio, "v1", Some(&patch(json!({"transport":{"type":"streamableHttp","url":"https://new.invalid/mcp","bearerTokenEnvVar":"NEW_TOKEN"}}))), Ok(write_ok()), Ok(after));
        result.unwrap(); assert_eq!(requests[1].1["value"], http_expected);
    }
    #[test]
    fn non_user_or_overlaid_config_is_not_editable_and_never_mutates() {
        let patch = patch(json!({"enabled":false}));
        let original = saved(json!({"command":"fixture"}));
        let mut variants = Vec::new();
        for kind in ["project", "system", "sessionFlags", "unknown"] {
            let mut before = original.clone();
            before["layers"].as_array_mut().unwrap().push(json!({"name":{"type":kind},"config":{"mcp_servers":{"server.name":{"command":"fixture"}}},"version":"other"}));
            variants.push(before);
        }
        let mut profile = original.clone(); profile["layers"][0]["name"]["profile"] = json!("profile"); variants.push(profile);
        let mut disabled = original.clone(); disabled["layers"][0]["disabledReason"] = json!("managed"); variants.push(disabled);
        let mut missing = original.clone(); missing["layers"][0]["config"] = json!({}); variants.push(missing);
        let mut duplicate = original.clone(); let base = duplicate["layers"][0].clone(); duplicate["layers"].as_array_mut().unwrap().push(base); variants.push(duplicate);
        let mut relative = original.clone(); relative["layers"][0]["name"]["file"] = json!("relative.toml"); variants.push(relative);
        let mut no_user = original.clone(); no_user["layers"] = json!([]); variants.push(no_user);
        let mut overridden = original; overridden["config"]["mcp_servers"]["server.name"]["command"] = json!("overridden"); variants.push(overridden);
        for before in variants {
            if let Ok(inventory) = inventory(&before) { assert!(!inventory.items.iter().find(|row| row.name == "server.name").unwrap().editable); }
            for operation in [Some(&patch), None] {
                let (result, requests) = run_change(before.clone(), "v1", operation, Ok(write_ok()), Ok(config()));
                assert!(result.is_err()); assert_eq!(requests.len(), 1);
            }
        }
    }
    #[test]
    fn remove_uses_null_cas_and_requires_absence_in_every_returned_layer() {
        let before = saved(json!({"command":"fixture","env":{"TOKEN":"SENTINEL"}}));
        let after = after_change(&before, None);
        let (result, requests) = run_change(before.clone(), "v1", None, Ok(write_ok()), Ok(after.clone()));
        result.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1].1, json!({"keyPath":"mcp_servers.\"server.name\"","value":null,"mergeStrategy":"replace","filePath":file(),"expectedVersion":"v1"}));
        assert!(!serde_json::to_string(&requests[1]).unwrap().contains("SENTINEL"));
        let mut reappeared = after;
        reappeared["layers"].as_array_mut().unwrap().push(json!({"name":{"type":"project"},"version":"v2","config":{"mcp_servers":{"server.name":{"command":"other"}}}}));
        let (result, requests) = run_change(before, "v1", None, Ok(write_ok()), Ok(reappeared));
        assert!(result.is_err()); assert_eq!(requests.len(), 3);
        let mut last = saved(json!({"command":"fixture"}));
        last["config"]["mcp_servers"].as_object_mut().unwrap().remove("sibling");
        last["layers"][0]["config"] = last["config"].clone();
        let mut after = after_change(&last, None);
        after["config"].as_object_mut().unwrap().remove("mcp_servers");
        after["layers"][0]["config"] = after["config"].clone();
        run_change(last, "v1", None, Ok(write_ok()), Ok(after)).0.unwrap();
    }
    #[test]
    fn edit_and_remove_never_retry_or_report_partial_or_unknown_results_as_success() {
        let before = saved(json!({"command":"fixture","env":{"TOKEN":"SENTINEL"},"unknown":"kept"}));
        let patch = patch(json!({"enabled":false}));
        for operation in [Some(&patch), None] {
            for error in ["configVersionConflict SENTINEL", "configLayerReadonly SENTINEL", "disconnect SENTINEL", "timed out SENTINEL"] {
                let (result, requests) = run_change(before.clone(), "v1", operation, Err(error.into()), Ok(config()));
                assert!(!result.unwrap_err().contains("SENTINEL")); assert_eq!(requests.len(), 2);
            }
            for reply in [json!({"status":"okOverridden"}), json!({"status":"ok","filePath":file(),"version":"v2","overriddenMetadata":{}}), json!({"status":"ok","filePath":"other","version":"v2"}), json!({"status":"ok","filePath":file()})] {
                let (result, requests) = run_change(before.clone(), "v1", operation, Ok(reply), Ok(config()));
                assert!(result.is_err()); assert_eq!(requests.len(), 2);
            }
            let (result, requests) = run_change(before.clone(), "v1", operation, Ok(write_ok()), Err("disconnect SENTINEL".into()));
            assert!(!result.unwrap_err().contains("SENTINEL")); assert_eq!(requests.len(), 3);
            let (result, requests) = run_change(before.clone(), "v1", operation, Ok(write_ok()), Ok(before.clone()));
            assert!(result.is_err()); assert_eq!(requests.len(), 3);
        }
        let mut partial = after_change(&before, Some(json!({"command":"fixture","enabled":false})));
        let (result, _) = run_change(before.clone(), "v1", Some(&patch), Ok(write_ok()), Ok(partial.clone()));
        assert!(result.is_err(), "secret and unknown-field loss must fail readback");
        partial = after_change(&before, Some(json!({"command":"fixture","env":{"TOKEN":"SENTINEL"},"unknown":"kept","enabled":false})));
        partial["config"]["mcp_servers"]["server.name"]["enabled"] = json!(true);
        let (result, _) = run_change(before.clone(), "v1", Some(&patch), Ok(write_ok()), Ok(partial));
        assert!(result.is_err(), "effective mismatch must fail readback");
        let mut sibling_loss = after_change(&before, None);
        sibling_loss["layers"][0]["config"]["mcp_servers"].as_object_mut().unwrap().remove("sibling");
        let (result, _) = run_change(before, "v1", None, Ok(write_ok()), Ok(sibling_loss));
        assert!(result.is_err(), "sibling loss must fail readback");
    }
    #[test]
    fn invalid_patch_or_incomplete_transport_switch_never_writes_and_debug_is_redacted() {
        for input in [json!({"transport":{"type":"stdio","env":{},"clearEnv":true}}), json!({"transport":{"type":"streamableHttp","bearerTokenEnvVar":"TOKEN","clearBearerTokenEnvVar":true}}), json!({"transport":{"type":"streamableHttp"}}), json!({"transport":{"type":"stdio","command":""}}), json!({"transport":{"type":"streamableHttp","url":"file:///SENTINEL"}}), json!({"transport":{"type":"stdio","args":["bad\0arg"]}}), json!({"transport":{"type":"stdio","env":{"bad-name":"SENTINEL"}}}), json!({"transport":{"type":"streamableHttp","url":"https://fixture.invalid","bearerTokenEnvVar":"bad-name"}})] {
            let before = saved(json!({"command":"fixture"}));
            let (result, requests) = run_change(before, "v1", Some(&patch(input)), Ok(write_ok()), Ok(config()));
            assert!(result.is_err()); assert_eq!(requests.len(), 1);
        }
        let before = saved(json!({"url":"https://fixture.invalid"}));
        let (result, requests) = run_change(before, "v1", Some(&patch(json!({"transport":{"type":"stdio"}}))), Ok(write_ok()), Ok(config()));
        assert!(result.is_err()); assert_eq!(requests.len(), 1);
        for input in [json!({"filePath":"arbitrary"}), json!({"transport":{"type":"stdio","url":"https://fixture.invalid"}}), json!({"transport":{"type":"streamableHttp","env":{}}})] {
            assert!(serde_json::from_value::<McpServerPatch>(input).is_err());
        }
        let patch = patch(json!({"transport":{"type":"stdio","env":{"TOKEN":"SENTINEL"}}}));
        assert!(!format!("{patch:?}").contains("SENTINEL"));
        assert!(!format!("{:?}", patch.transport.unwrap()).contains("SENTINEL"));
        let mut unreachable = |_: &str, _: Value| -> Result<Value, String> { panic!("invalid request reached RPC") };
        assert!(remove(Path::new("."), "space name", "v1", &mut unreachable).is_err());
        assert!(remove(Path::new("."), "valid", "", &mut unreachable).is_err());
    }
}
