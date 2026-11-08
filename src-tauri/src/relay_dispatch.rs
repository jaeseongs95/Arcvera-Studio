//! Remote authority is deliberately narrower than the local IPC command surface.
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::Manager;

// opendiy-remote 56c1968, server/remote-server.mjs RELAY_COMMAND_ALLOWLIST.
pub(crate) const COMMANDS: &[&str] = &[
    "get_ui_language", "set_ui_language", "prepare_runtime", "get_codex_settings",
    "runtime_compatibility", "runtime_pair_status", "runtime_transition_preview",
    "load_layout", "save_layout", "list_projects", "import_project", "create_project",
    "rename_project", "set_project_favorite", "delete_project", "reorder_project",
    "start_session", "list_sessions", "session_write", "session_resize", "session_stop",
    "session_output_ack", "move_session_to_project", "list_external", "catalog_list",
    "catalog_preview", "extensions_list", "extensions_mutate", "mobile_control",
    "subagents_request", "cron_request", "saved_cron_list", "saved_cron_save",
    "saved_cron_delete", "saved_cron_register", "conversation_request", "list_pending_questions",
    "respond_pending_question", "choose_conversation_attachments", "conversation_attachment_storage",
    "stage_conversation_image", "cleanup_conversation_attachment", "weekly_usage",
    "choose_project_folder", "choose_skill_folder", "choose_project", "list_directory",
];

fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|_| format!("원격 명령 인수가 올바르지 않습니다: {key}"))
}
fn value<T: Serialize>(result: Result<T, String>) -> Result<Value, String> {
    serde_json::to_value(result?).map_err(|_| "원격 명령 결과를 직렬화하지 못했습니다.".into())
}
fn directory(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if !path.is_absolute() || !path.is_dir() { return Err("존재하는 절대 폴더 경로를 지정하세요.".into()); }
    path.canonicalize().map_err(|_| "원격 폴더 경로를 확인하지 못했습니다.".into())
}

pub(crate) fn paths(command: &str, args: &Value) -> Result<Value, String> {
    if command == "choose_conversation_attachments" {
        let selected: Vec<String> = arg(args, "paths")?;
        if selected.len() > 32 { return Err("첨부는 한 번에 32개까지 선택할 수 있습니다.".into()); }
        let mut items = Vec::new();
        for raw in selected {
            let file = Path::new(&raw);
            if !file.is_absolute() || !file.is_file() { return Err("존재하는 절대 파일 경로를 지정하세요.".into()); }
            let file = file.canonicalize().map_err(|_| "첨부 파일 경로를 확인하지 못했습니다.")?;
            let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            items.push(json!({"type":if ["png","jpg","jpeg","gif","webp"].contains(&ext.as_str()) {"localImage"} else {"file"},
                "path":file.to_str().ok_or("첨부 경로를 표시할 수 없습니다.")?, "name":file.file_name().and_then(|s|s.to_str()).ok_or("첨부 이름을 표시할 수 없습니다.")?}));
        }
        return Ok(Value::Array(items));
    }
    let raw: Option<String> = arg(args, "path")?;
    let cwd: Option<String> = arg(args, "cwd")?;
    let raw = raw.or(cwd).filter(|s| !s.trim().is_empty());
    if command != "list_directory" && raw.is_none() { return Ok(Value::Null); }
    let path = directory(raw.as_deref().unwrap_or(if cfg!(windows) { "C:\\" } else { "/" }))?;
    if command != "list_directory" { return value(Ok(path.to_str().ok_or("폴더 경로를 표시할 수 없습니다.")?.to_owned())); }
    let include_files: Option<bool> = arg(args, "includeFiles")?;
    let mut entries = Vec::new();
    for item in std::fs::read_dir(&path).map_err(|_| "폴더 목록을 읽지 못했습니다.")? {
        let item = item.map_err(|_| "폴더 항목을 읽지 못했습니다.")?;
        let name = item.file_name().into_string().map_err(|_| "표시할 수 없는 파일 이름입니다.")?;
        if name.starts_with('.') { continue; }
        let kind = item.file_type().map_err(|_| "폴더 항목 종류를 확인하지 못했습니다.")?;
        let is_directory = if kind.is_symlink() { item.path().is_dir() } else { kind.is_dir() };
        if !is_directory && !include_files.unwrap_or(false) { continue; }
        if entries.len() >= 1000 { return Err("폴더 항목이 1000개를 초과합니다. 더 구체적인 경로를 입력하세요.".into()); }
        entries.push(json!({"name":name,"path":item.path().to_str().ok_or("경로를 표시할 수 없습니다.")?,"isDirectory":is_directory}));
    }
    entries.sort_by(|a,b| b["isDirectory"].as_bool().cmp(&a["isDirectory"].as_bool()).then_with(|| a["name"].as_str().cmp(&b["name"].as_str())));
    Ok(json!({"currentPath":path.to_str(),"parentPath":path.parent().and_then(Path::to_str),"roots":[path.ancestors().last().and_then(Path::to_str)],"entries":entries}))
}

pub(crate) async fn dispatch(app: tauri::AppHandle, command: &str, args: Value) -> Result<Value, String> {
    if !COMMANDS.contains(&command) || !args.is_object() { return Err("허용되지 않거나 잘못된 원격 명령입니다.".into()); }
    // All business operations retain their existing worker/update-admission and
    // Engine validation. Remote path selection never invokes an OS modal.
    macro_rules! invoke { ($name:ident $(, $key:literal)* ) => { value(super::$name(app.state() $(, arg(&args,$key)?)*).await) }; }
    match command {
        "get_ui_language" => invoke!(get_ui_language),
        "set_ui_language" => value(super::set_ui_language(app.state(),app.state(),arg(&args,"language")?).await),
        "prepare_runtime" => invoke!(prepare_runtime),
        "get_codex_settings" => invoke!(get_codex_settings),
        "runtime_compatibility" => invoke!(runtime_compatibility),
        "runtime_pair_status" => invoke!(runtime_pair_status),
        "runtime_transition_preview" => invoke!(runtime_transition_preview,"replaceExisting"),
        "load_layout" => value(super::load_layout(app.state())),
        "save_layout" => value(super::save_layout(app.state(),arg(&args,"layout")?)),
        "list_projects" => invoke!(list_projects),
        "import_project" => invoke!(import_project,"cwd"),
        "create_project" => invoke!(create_project,"name","cwd"),
        "rename_project" => invoke!(rename_project,"id","name"),
        "set_project_favorite" => invoke!(set_project_favorite,"id","favorite"),
        "delete_project" => invoke!(delete_project,"id"),
        "reorder_project" => invoke!(reorder_project,"id","targetId","after"),
        "start_session" => invoke!(start_session,"request"),
        "list_sessions" => value(super::list_sessions(app.state())),
        "session_write" => invoke!(session_write,"id","generation","data","binary"),
        "session_resize" => invoke!(session_resize,"id","generation","cols","rows"),
        "session_stop" => invoke!(session_stop,"id","generation"),
        "session_output_ack" => value(super::session_output_ack(app.state(),arg(&args,"id")?,arg(&args,"generation")?,arg(&args,"seq")?)),
        "move_session_to_project" => invoke!(move_session_to_project,"id","generation","projectId"),
        "list_external" => invoke!(list_external),
        "catalog_list" => invoke!(catalog_list,"request"),
        "catalog_preview" => invoke!(catalog_preview,"threadId","cursor"),
        "extensions_list" => invoke!(extensions_list,"cwd","catalogQuery"),
        "extensions_mutate" => invoke!(extensions_mutate,"cwd","request"),
        "mobile_control" => invoke!(mobile_control,"id","generation","action"),
        "subagents_request" => invoke!(subagents_request,"id","generation","request"),
        "cron_request" => invoke!(cron_request,"id","generation","request"),
        "saved_cron_list" => invoke!(saved_cron_list,"threadId"),
        "saved_cron_save" => invoke!(saved_cron_save,"id","threadId","prompt","schedule"),
        "saved_cron_delete" => invoke!(saved_cron_delete,"id","threadId"),
        "saved_cron_register" => invoke!(saved_cron_register,"id","generation","definitionId","threadId"),
        "conversation_request" => invoke!(conversation_request,"id","generation","request"),
        "list_pending_questions" => invoke!(list_pending_questions),
        "respond_pending_question" => invoke!(respond_pending_question,"sessionId","generation","requestId","answers","decision"),
        "conversation_attachment_storage" => invoke!(conversation_attachment_storage),
        "stage_conversation_image" => invoke!(stage_conversation_image,"id","generation","dataUrl"),
        "cleanup_conversation_attachment" => invoke!(cleanup_conversation_attachment,"id","generation","path"),
        "weekly_usage" => invoke!(weekly_usage,"manual"),
        "choose_project" => {
            let folder = super::native_worker(move || paths("choose_project",&args)).await.map_err(|_| "원격 경로 검사 작업 실패".to_owned())??;
            if folder.is_null() { Ok(folder) } else { value(super::import_project(app.state(),folder.as_str().unwrap().to_owned()).await) }
        }
        "choose_project_folder" | "choose_skill_folder" | "choose_conversation_attachments" | "list_directory" => {
            let command=command.to_owned();
            super::native_worker(move || paths(&command,&args)).await.map_err(|_| "원격 경로 검사 작업 실패".to_owned())?
        }
        _ => Err("지원하지 않는 원격 명령입니다.".into()),
    }
}
