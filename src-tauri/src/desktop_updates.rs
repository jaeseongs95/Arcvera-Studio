//! Native-only update adapter. Checks never download executable artifacts.
use crate::session::Engine;
use reqwest::{blocking::Client, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, sync::{Mutex, RwLock}, time::{Duration, Instant}};
use tauri::{Manager, State};
use tauri_plugin_updater::UpdaterExt;

const DESKTOP_REPO: &str = "jaeseongs95/opendiy-desktop";
const CODEX_REPO: &str = "jaeseongs95/opendiy-codex";
const METADATA_LIMIT: usize = 256 * 1024;
const INSTALLER_LIMIT: usize = 512 * 1024 * 1024;
const REQUEST_LIMIT: Duration = Duration::from_secs(20);
// ponytail: one non-queued request bounds native update workers and installer ownership.
static UPDATE_REQUEST: Mutex<()> = Mutex::new(());
static NATIVE_OPERATIONS: RwLock<()> = RwLock::new(());

/// lib.rs holds this permit inside each blocking worker until cleanup returns.
/// Never hold the guard across an async await or wait for a competing installer.
pub(crate) fn with_native_operation<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _permit = NATIVE_OPERATIONS.try_read().map_err(|error| match error {
        std::sync::TryLockError::WouldBlock => "Desktop 업데이트 설치 중이어서 새 작업을 시작하지 않았습니다.",
        std::sync::TryLockError::Poisoned(_) => "네이티브 작업 상태가 손상되었습니다. 앱을 다시 시작하세요.",
    })?;
    operation()
}

fn with_install_operation<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _permit = NATIVE_OPERATIONS.try_write().map_err(|error| match error {
        std::sync::TryLockError::WouldBlock => "진행 중인 네이티브 작업이 있어 업데이트를 보류했습니다. 작업이 끝난 뒤 다시 시도하세요.",
        std::sync::TryLockError::Poisoned(_) => "네이티브 작업 상태가 손상되었습니다. 앱을 다시 시작하세요.",
    })?;
    operation()
}

fn admission() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    UPDATE_REQUEST.try_lock().map_err(|error| match error {
        std::sync::TryLockError::WouldBlock => "다른 업데이트 요청이 진행 중입니다.".into(),
        std::sync::TryLockError::Poisoned(_) => "업데이트 요청 상태가 손상되었습니다. 앱을 다시 시작하세요.".into(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferences { pub background_checks: bool }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub status: &'static str,
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    pub install_supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Deserialize)]
struct Release { tag_name: String, draft: bool, prerelease: bool }

fn parse_release(bytes: &[u8], allow_runtime_patch: bool) -> Result<(String, Version), String> {
    if bytes.len() > METADATA_LIMIT { return Err("업데이트 정보가 크기 상한을 초과했습니다.".into()); }
    let release: Release = serde_json::from_slice(bytes).map_err(|_| "업데이트 정보 형식을 확인할 수 없습니다.")?;
    if release.draft || release.prerelease { return Err("정식 릴리스가 아닌 업데이트 정보는 사용하지 않습니다.".into()); }
    let tag = release.tag_name;
    if tag.len() > 128 || !tag.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b)) {
        return Err("업데이트 버전 식별자가 올바르지 않습니다.".into());
    }
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(&tag)).map_err(|_| "업데이트 버전을 확인할 수 없습니다.")?;
    if !allow_runtime_patch && !version.pre.is_empty() { return Err("시험 버전은 정식 업데이트로 제공하지 않습니다.".into()); }
    Ok((tag, version))
}

fn request_client() -> Result<Client, String> {
    Client::builder().user_agent(concat!("OpenDIY-Desktop/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_LIMIT).connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            let trusted = matches!(url.host_str(), Some("github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com"));
            if attempt.previous().len() < 5 && url.scheme() == "https" && trusted && url.username().is_empty() && url.password().is_none() {
                attempt.follow()
            } else { attempt.stop() }
        })).build().map_err(|_| "업데이트 연결을 준비할 수 없습니다.".into())
}

fn bounded_read(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader.by_ref().take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|_| "업데이트 응답을 끝까지 읽지 못했습니다.")?;
    if bytes.len() > limit { return Err("업데이트 응답이 크기 상한을 초과했습니다.".into()); }
    Ok(bytes)
}

fn get_bytes(client: &Client, url: &str, limit: usize, deadline: Instant) -> Result<Vec<u8>, String> {
    let remaining = deadline.checked_duration_since(Instant::now()).ok_or("업데이트 확인 시간이 초과되었습니다.")?;
    let response = client.get(url).timeout(remaining).send().map_err(|error| {
        if error.is_timeout() { "업데이트 연결 시간이 초과되었습니다." } else { "업데이트 서버에 연결할 수 없습니다." }
    })?;
    if !response.status().is_success() { return Err(format!("업데이트 서버가 요청을 처리하지 못했습니다 (HTTP {}).", response.status().as_u16())); }
    if response.content_length().is_some_and(|size| size > limit as u64) { return Err("업데이트 응답이 크기 상한을 초과했습니다.".into()); }
    bounded_read(response, limit)
}

fn release(client: &Client, repo: &str, deadline: Instant) -> Result<(String, Version), String> {
    parse_release(&get_bytes(client, &format!("https://api.github.com/repos/{repo}/releases/latest"), METADATA_LIMIT, deadline)?, repo == CODEX_REPO)
}

fn public_key() -> Option<&'static str> {
    // Only a build-time public key is accepted. No runtime secret/config discovery.
    option_env!("OPENDIY_UPDATER_PUBLIC_KEY").map(str::trim).filter(|key| !key.is_empty())
}

fn manifest(client: &Client, tag: &str, version: &Version, deadline: Instant) -> Result<Value, String> {
    let bytes = get_bytes(client, &format!("https://github.com/{DESKTOP_REPO}/releases/download/{tag}/latest.json"), METADATA_LIMIT, deadline)?;
    validate_manifest(&bytes, tag, version)
}

fn validate_manifest(bytes: &[u8], tag: &str, version: &Version) -> Result<Value, String> {
    if bytes.len() > METADATA_LIMIT { return Err("서명 업데이트 정보가 크기 상한을 초과했습니다.".into()); }
    let raw: Value = serde_json::from_slice(bytes).map_err(|_| "서명 업데이트 정보 형식이 올바르지 않습니다.")?;
    let declared = raw["version"].as_str().ok_or("서명 업데이트 버전이 없습니다.")?;
    let declared = Version::parse(declared.strip_prefix('v').unwrap_or(declared)).map_err(|_| "서명 업데이트 버전 형식이 올바르지 않습니다.")?;
    if &declared != version { return Err("서명 업데이트 정보와 확인한 릴리스 버전이 다릅니다.".into()); }
    let platforms = raw["platforms"].as_object().ok_or("업데이트 플랫폼 정보가 없습니다.")?;
    if platforms.is_empty() || platforms.len() > 32 { return Err("업데이트 플랫폼 정보가 올바르지 않습니다.".into()); }
    for platform in platforms.values() {
        let signature = platform["signature"].as_str().ok_or("업데이트 서명이 없습니다.")?;
        if signature.trim().is_empty() || signature.len() > 16 * 1024 { return Err("업데이트 서명이 올바르지 않습니다.".into()); }
        let url = platform["url"].as_str().ok_or("업데이트 파일 주소가 없습니다.")?;
        let url = Url::parse(url).map_err(|_| "업데이트 파일 주소 형식이 올바르지 않습니다.")?;
        if url.scheme() != "https" || url.host_str() != Some("github.com") || !url.username().is_empty() || url.password().is_some()
            || url.port().is_some() || url.query().is_some() || url.fragment().is_some()
            || !url.path().starts_with(&format!("/{DESKTOP_REPO}/releases/download/{tag}/")) {
            return Err("업데이트 파일 주소가 고정된 릴리스 저장소와 일치하지 않습니다.".into());
        }
    }
    Ok(raw)
}

fn restore(app: &tauri::AppHandle, raw: Value) -> Result<tauri_plugin_updater::Update, String> {
    let key = public_key().ok_or("업데이트 설치 미구성: 신뢰 공개 키가 지정되지 않았습니다.")?;
    let handle = app.clone();
    app.updater_builder().pubkey(key).timeout(REQUEST_LIMIT)
        .endpoints(vec![format!("https://github.com/{DESKTOP_REPO}/releases/latest/download/latest.json").parse().map_err(|_| "업데이트 주소 구성 오류")?])
        .map_err(|_| "업데이트 설치 주소를 구성할 수 없습니다.")?
        .on_before_exit(move || {
            // Windows updater exits directly, bypassing RunEvent::ExitRequested.
            // Engine's update gate is still held and no owned PTY/management worker exists.
            handle.state::<crate::ExitState>().0.store(true, std::sync::atomic::Ordering::Release);
            handle.cleanup_before_exit();
        }).build().map_err(|_| "업데이트 설치 환경을 구성할 수 없습니다.")?
        .restore_update(raw).map_err(|_| "서명 업데이트 대상이나 설치 환경을 확인할 수 없습니다.")?
        .ok_or_else(|| "설치할 새 Desktop 버전이 없습니다.".into())
}

fn desktop_check(app: &tauri::AppHandle) -> Result<UpdateStatus, String> {
    let client = request_client()?;
    let deadline = Instant::now() + REQUEST_LIMIT;
    let (tag, latest) = release(&client, DESKTOP_REPO, deadline)?;
    let current = app.package_info().version.clone();
    let mut status = UpdateStatus { status: if latest > current { "available" } else { "current" }, current_version: current.to_string(),
        latest_version: Some(latest.to_string()), release_url: Some(format!("https://github.com/{DESKTOP_REPO}/releases/tag/{tag}")), install_supported: false, reason: None };
    if public_key().is_none() {
        status.reason = Some("업데이트 설치 미구성: 신뢰 공개 키가 지정되지 않았습니다.".into());
    } else if !cfg!(windows) {
        status.reason = Some("이 빌드의 앱 내 업데이트 설치는 Windows에서만 지원합니다.".into());
    } else if latest > current {
        match manifest(&client, &tag, &latest, deadline).and_then(|raw| restore(app, raw)) {
            Ok(_) => status.install_supported = true,
            Err(error) => status.reason = Some(format!("업데이트 설치 미구성: {error}")),
        }
    }
    Ok(status)
}

#[tauri::command]
pub async fn get_update_preferences(engine: State<'_, Engine>) -> Result<UpdatePreferences, String> {
    let engine = engine.inner().clone();
    crate::native_worker(move || engine.update_preferences()).await.map_err(|_| "업데이트 설정 조회 작업자가 종료되었습니다.")?
}

#[tauri::command]
pub async fn set_update_preferences(engine: State<'_, Engine>, background_checks: bool) -> Result<UpdatePreferences, String> {
    let engine = engine.inner().clone();
    crate::native_worker(move || engine.set_update_preferences(background_checks)).await.map_err(|_| "업데이트 설정 저장 작업자가 종료되었습니다.")?
}

#[tauri::command]
pub async fn check_desktop_update(app: tauri::AppHandle) -> Result<UpdateStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _request = admission()?;
        desktop_check(&app)
    }).await.map_err(|_| "Desktop 업데이트 확인 작업자가 종료되었습니다.")?
}

#[tauri::command]
pub async fn install_desktop_update(app: tauri::AppHandle, engine: State<'_, Engine>, confirmed: bool, version: String) -> Result<(), String> {
    if !confirmed { return Err("업데이트 설치에는 명시적인 동의가 필요합니다.".into()); }
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _request = admission()?;
        if !cfg!(windows) { return Err("이 빌드의 앱 내 업데이트 설치는 Windows에서만 지원합니다.".into()); }
        if public_key().is_none() { return Err("업데이트 설치 미구성: 신뢰 공개 키가 지정되지 않았습니다.".into()); }
        with_install_operation(|| {
        let client = request_client()?;
        let deadline = Instant::now() + REQUEST_LIMIT;
        let (tag, latest) = release(&client, DESKTOP_REPO, deadline)?;
        if version != latest.to_string() || latest <= app.package_info().version { return Err("확인한 업데이트 버전이 바뀌었습니다. 다시 확인하세요.".into()); }
        let update = restore(&app, manifest(&client, &tag, &latest, deadline)?)?;
        // Hold admission through download and installer launch. A late session cannot
        // race Windows' direct exit; an active/unknown session is never stopped.
        engine.with_desktop_update_gate(|| {
            let bytes = get_bytes(&client, update.download_url.as_str(), INSTALLER_LIMIT, deadline)?;
            let result = update.install(bytes).map_err(|_| "업데이트 서명 검증 또는 설치 시작에 실패했습니다. 설치 완료로 처리하지 않았습니다.".to_owned());
            if result.is_err() { app.state::<crate::ExitState>().0.store(false, std::sync::atomic::Ordering::Release); }
            result
        })
        })
    }).await.map_err(|_| "Desktop 업데이트 설치 작업자가 종료되었습니다. 결과를 확인하세요.")?
}

#[tauri::command]
pub async fn check_codex_update(engine: State<'_, Engine>) -> Result<UpdateStatus, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _request = admission()?;
        let deadline = Instant::now() + REQUEST_LIMIT;
        let current = engine.codex_update_version(deadline)?;
        let client = request_client()?;
        let (tag, latest) = release(&client, CODEX_REPO, deadline)?;
        // A plain SemVer does not prove OpenDIY runtime identity or revision order.
        Ok(UpdateStatus { status: "unknown", current_version: current, latest_version: Some(latest.to_string()),
            release_url: Some(format!("https://github.com/{CODEX_REPO}/releases/tag/{tag}")), install_supported: false,
            reason: Some("선택한 CLI 버전은 조회했습니다. OpenDIY 릴리스 식별·설치 계약이 확인되지 않아 업데이트 필요 여부와 앱 내 설치는 미구성입니다.".into()) })
    }).await.map_err(|_| "Codex 업데이트 확인 작업자가 종료되었습니다.")?
}

#[tauri::command]
pub async fn install_codex_update(confirmed: bool, version: String) -> Result<(), String> {
    let _ = version;
    if !confirmed { return Err("업데이트 설치에는 명시적인 동의가 필요합니다.".into()); }
    Err("Codex 업데이트 설치 미구성: 선택한 런타임을 보존하는 설치 계약이 확인되지 않았습니다.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_updates_install_operation_excludes_inflight_and_late_native_workers() {
        let _serial = admission().unwrap();
        with_native_operation(|| {
            assert!(with_install_operation::<()>(|| panic!("existing native worker must win")).is_err());
            Ok(())
        }).unwrap();
        with_install_operation(|| {
            let result = std::thread::spawn(|| with_native_operation::<()>(|| panic!("late worker must not start"))).join().unwrap();
            assert!(result.is_err());
            assert!(with_install_operation::<()>(|| panic!("second installer must not start")).is_err());
            Ok(())
        }).unwrap();
        with_native_operation(|| Ok(())).unwrap();
    }
    #[test]
    fn desktop_updates_preferences_survive_reopen_and_reject_corruption() {
        let path = std::env::temp_dir().join(format!("opendiy-update-preferences-{}.sqlite3", uuid::Uuid::new_v4()));
        {
            let store = crate::storage::Store::open(&path).unwrap();
            assert!(store.background_update_checks().unwrap());
            store.set_background_update_checks(false).unwrap();
        }
        {
            let store = crate::storage::Store::open(&path).unwrap();
            assert!(!store.background_update_checks().unwrap());
            store.set_background_update_checks(true).unwrap();
        }
        {
            let store = crate::storage::Store::open(&path).unwrap();
            assert!(store.background_update_checks().unwrap());
        }
        {
            let connection = rusqlite::Connection::open(&path).unwrap();
            connection.execute("UPDATE app_settings SET value='invalid' WHERE key='background_update_checks'", []).unwrap();
        }
        {
            let store = crate::storage::Store::open(&path).unwrap();
            assert!(store.background_update_checks().is_err());
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn desktop_updates_release_rejects_unstable_and_invalid_tags() {
        for body in [r#"{"tag_name":"v1.7.0","draft":true,"prerelease":false}"#, r#"{"tag_name":"v1.7.0","draft":false,"prerelease":true}"#,
            r#"{"tag_name":"v1.7.0-beta.1","draft":false,"prerelease":false}"#, r#"{"tag_name":"../v1.7.0","draft":false,"prerelease":false}"#] {
            assert!(parse_release(body.as_bytes(), false).is_err());
        }
        let (tag, version) = parse_release(br#"{"tag_name":"v1.7.0","draft":false,"prerelease":false,"body":"untrusted text"}"#, false).unwrap();
        assert_eq!(tag, "v1.7.0"); assert_eq!(version, Version::new(1, 7, 0));
        let (_, patched) = parse_release(br#"{"tag_name":"v0.166.0-r37.20261104","draft":false,"prerelease":false}"#, true).unwrap();
        assert_eq!(patched.to_string(), "0.166.0-r37.20261104");
        assert!(version > Version::new(1, 6, 0)); assert!(version <= Version::new(1, 7, 0));
        assert!(parse_release(&vec![b' '; METADATA_LIMIT + 1], false).is_err());
    }
    #[test]
    fn desktop_updates_response_reader_rejects_overflow() {
        assert_eq!(bounded_read(&b"abc"[..], 3).unwrap(), b"abc");
        assert!(bounded_read(&b"abcd"[..], 3).is_err());
    }
    #[test]
    fn desktop_updates_manifest_requires_same_release_and_signature() {
        let version = Version::new(1, 7, 0);
        let fixture = |url: &str, signature: &str, version: &str| serde_json::to_vec(&serde_json::json!({"version":version,"platforms":{"windows-x86_64":{"url":url,"signature":signature}}})).unwrap();
        let own = "https://github.com/jaeseongs95/opendiy-desktop/releases/download/v1.7.0/app.exe";
        assert!(validate_manifest(&fixture(own, "fixture-signature", "1.7.0"), "v1.7.0", &version).is_ok());
        for url in ["http://github.com/jaeseongs95/opendiy-desktop/releases/download/v1.7.0/app.exe", "https://example.com/app.exe", "https://github.com/other/repo/releases/download/v1.7.0/app.exe", "https://github.com/jaeseongs95/opendiy-desktop/releases/download/v1.6.0/app.exe"] {
            assert!(validate_manifest(&fixture(url, "fixture-signature", "1.7.0"), "v1.7.0", &version).is_err());
        }
        assert!(validate_manifest(&fixture(own, "", "1.7.0"), "v1.7.0", &version).is_err());
        assert!(validate_manifest(&fixture(own, "fixture-signature", "1.8.0"), "v1.7.0", &version).is_err());
    }
}
