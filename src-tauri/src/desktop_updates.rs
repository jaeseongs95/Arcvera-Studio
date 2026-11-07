//! Native-only update adapter. Checks never download executable artifacts.
use crate::session::Engine;
use reqwest::{blocking::Client, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, sync::{Mutex, RwLock}, time::{Duration, Instant}};
use tauri::State;
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

fn parse_release_tag(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > METADATA_LIMIT { return Err("업데이트 정보가 크기 상한을 초과했습니다.".into()); }
    let release: Release = serde_json::from_slice(bytes).map_err(|_| "업데이트 정보 형식을 확인할 수 없습니다.")?;
    if release.draft || release.prerelease { return Err("정식 릴리스가 아닌 업데이트 정보는 사용하지 않습니다.".into()); }
    let tag = release.tag_name;
    if tag.len() > 128 || !tag.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b)) {
        return Err("업데이트 버전 식별자가 올바르지 않습니다.".into());
    }
    Ok(tag)
}

fn parse_release(bytes: &[u8], allow_runtime_patch: bool) -> Result<(String, Version), String> {
    let tag = parse_release_tag(bytes)?;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(&tag)).map_err(|_| "업데이트 버전을 확인할 수 없습니다.")?;
    if !allow_runtime_patch && !version.pre.is_empty() { return Err("시험 버전은 정식 업데이트로 제공하지 않습니다.".into()); }
    Ok((tag, version))
}

#[derive(Debug, PartialEq, Eq)]
struct CodexReleaseIdentity { revision: i32, tag: String }

fn canonical_decimal(text: &str) -> bool {
    !text.is_empty() && (text.len() == 1 || !text.starts_with('0')) && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn codex_cargo_base(base: &str) -> bool {
    let (core, alpha) = match base.split_once("-alpha") {
        Some((core, alpha)) => (core, Some(alpha)),
        None => (base, None),
    };
    let core: Vec<_> = core.split('.').collect();
    if core.len() != 3 || !core.iter().all(|part| canonical_decimal(part)) { return false; }
    match alpha {
        None | Some("") => true,
        Some(suffix) => suffix.strip_prefix('.').is_some_and(|numbers| {
            let parts: Vec<_> = numbers.split('.').collect();
            (1..=2).contains(&parts.len()) && parts.iter().all(|part| canonical_decimal(part))
        }),
    }
}

// Match the pinned Windows updater's stable/alpha + native-r grammar. Do not
// infer chronology from Cargo base, alpha components or the eight date digits.
fn parse_codex_version(text: &str) -> Option<CodexReleaseIdentity> {
    if text.len() > 256 { return None; }
    let version = text.strip_prefix("codex-cli ").unwrap_or(text);
    let (base, patch) = version.split_once("+native-r")?;
    if !codex_cargo_base(base) { return None; }
    let (revision, date) = patch.split_once('.')?;
    if !canonical_decimal(revision) || date.len() != 8 || !date.bytes().all(|byte| byte.is_ascii_digit()) { return None; }
    let revision: i32 = revision.parse().ok()?;
    if revision <= 0 { return None; }
    Some(CodexReleaseIdentity { revision, tag: format!("v{base}-r{patch}") })
}

fn parse_codex_release_tag(tag: &str) -> Option<CodexReleaseIdentity> {
    if tag.len() > 128 { return None; }
    let (base, patch) = tag.strip_prefix('v')?.rsplit_once("-r")?;
    let identity = parse_codex_version(&format!("{base}+native-r{patch}"))?;
    (identity.tag == tag).then_some(identity)
}

#[derive(Debug, PartialEq, Eq)]
enum CodexComparison { Current, Available, UnsupportedRuntime, InvalidReleaseIdentity, ReleaseIdentityMismatch, DowngradeRefused }

fn compare_codex_versions(current: &str, tag: &str) -> CodexComparison {
    let Some(current) = parse_codex_version(current) else { return CodexComparison::UnsupportedRuntime; };
    let Some(latest) = parse_codex_release_tag(tag) else { return CodexComparison::InvalidReleaseIdentity; };
    match latest.revision.cmp(&current.revision) {
        std::cmp::Ordering::Greater => CodexComparison::Available,
        std::cmp::Ordering::Less => CodexComparison::DowngradeRefused,
        std::cmp::Ordering::Equal if latest.tag == current.tag => CodexComparison::Current,
        std::cmp::Ordering::Equal => CodexComparison::ReleaseIdentityMismatch,
    }
}

fn codex_status(current: String, tag: String) -> UpdateStatus {
    let comparison = compare_codex_versions(&current, &tag);
    let (status, reason) = match comparison {
        CodexComparison::Current => ("current", "현재 OpenDIY 릴리스와 일치합니다. 앱 내 설치는 대상 고정 계약이 없어 미구성입니다."),
        CodexComparison::Available => ("available", "더 높은 R의 OpenDIY 업데이트 후보가 있습니다. 앱 내 설치는 대상 고정 계약이 없어 미구성입니다."),
        CodexComparison::UnsupportedRuntime => ("unknown", "unsupported_runtime: 선택한 CLI가 확인된 OpenDIY native 버전 형식과 일치하지 않아 비교하지 않았습니다."),
        CodexComparison::InvalidReleaseIdentity => ("unknown", "invalid_release_identity: 공개 릴리스 태그가 확인된 OpenDIY 버전 형식과 일치하지 않습니다."),
        CodexComparison::ReleaseIdentityMismatch => ("unknown", "release_identity_mismatch: 같은 R이지만 전체 릴리스 식별자가 달라 최신 여부를 확정하지 않았습니다."),
        CodexComparison::DowngradeRefused => ("unknown", "downgrade_refused: 공개 릴리스의 R이 현재 CLI보다 낮아 업데이트 대상으로 제공하지 않습니다."),
    };
    let valid_tag = parse_codex_release_tag(&tag).is_some();
    UpdateStatus { status, current_version: current,
        latest_version: valid_tag.then(|| tag.strip_prefix('v').unwrap_or(&tag).to_owned()),
        release_url: valid_tag.then(|| format!("https://github.com/{CODEX_REPO}/releases/tag/{tag}")),
        install_supported: false, reason: Some(reason.into()) }
}

fn request_client() -> Result<Client, String> {
    // The updater registers ring only in its own check path; our reads run first.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        // Another caller may win registration. Keep that process-wide provider.
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
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

// UpdaterExt injects cleanup_before_exit by default. Replace that callback:
// Windows invokes it BEFORE ShellExecuteW, which can still fail or be refused.
// Keep the UI intact on Err; after successful launch the plugin exits directly.
fn preserve_app_before_installer_launch() {}

/// Explicitly enabled native component, never a product offer or UI-consent test.
#[cfg(all(windows, feature = "actual-updater-probe"))]
pub(crate) mod actual_probe {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::windows::fs::MetadataExt,
        path::Path,
        sync::{atomic::{AtomicUsize, Ordering}, Arc},
    };

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Pin { name: String, sha256: String }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Inputs {
        schema_version: u32,
        nonce: String,
        fixture_root: String,
        mode: String,
        // Component transport only. Never rewrite the signed release metadata URL.
        transport_url: String,
        version: String,
        product_name: String,
        identifier: String,
        main_binary: String,
        installed_exe_sha256: String,
        installer: Pin,
        signature: Pin,
        manifest: Pin,
        public_key: Pin,
        merged_config: Pin,
        package_receipt: Pin,
    }

    fn require(ok: bool, reason: &str) -> Result<(), String> {
        if ok { Ok(()) } else { Err(reason.into()) }
    }

    fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

    fn valid_sha(value: &str) -> bool {
        value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    fn validate_transport(value: &str) -> Result<Url, String> {
        require(!value.is_empty() && value.len() <= 16 * 1024 && value.trim() == value
            && !value.chars().any(char::is_control), "invalid component transport URL")?;
        let url = Url::parse(value).map_err(|_| "invalid component transport URL")?;
        require(url.scheme() == "https" && url.host_str().is_some() && url.username().is_empty()
            && url.password().is_none() && url.fragment().is_none(), "transport requires HTTPS without credentials or fragment")?;
        Ok(url)
    }

    fn transport_observation(input: &str, transport: &Url, metadata: &Url) -> Value {
        // Never serialize either full URL: a temporary transport query can be sensitive.
        serde_json::json!({
            "componentOnly":true,"transportDiffersFromMetadata":transport != metadata,
            "transportHost":transport.host_str(),"transportPath":transport.path(),
            "transportUrlSHA256":digest(input.as_bytes()),"requestUrlSHA256":digest(transport.as_str().as_bytes()),
            "metadataHost":metadata.host_str(),"metadataPath":metadata.path(),
            "metadataUrlSHA256":digest(metadata.as_str().as_bytes())
        })
    }

    // No normalization: reject aliases, traversal, whitespace and shell/NSIS delimiters.
    fn safe_path(path: &str) -> bool {
        path.starts_with("D:/codex/build/") && path.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
            && path.bytes().enumerate().all(|(i, b)| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.') || (i == 1 && b == b':'))
    }

    fn no_reparse(path: &Path, directory: bool) -> Result<(), String> {
        for ancestor in path.ancestors() {
            let metadata = fs::symlink_metadata(ancestor).map_err(|e| format!("exact path metadata: {e}"))?;
            require(metadata.file_attributes() & 0x400 == 0, "reparse point refused")?;
        }
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        require(if directory { metadata.is_dir() } else { metadata.is_file() }, "unexpected path type")
    }

    fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
        no_reparse(path, false)?;
        let file = File::open(path).map_err(|e| e.to_string())?;
        require(file.metadata().map_err(|e| e.to_string())?.len() <= limit as u64, "input size limit")?;
        bounded_read(file, limit)
    }

    fn pinned(root: &str, pin: &Pin, limit: usize) -> Result<Vec<u8>, String> {
        require(!pin.name.is_empty() && pin.name != "." && !pin.name.contains("..")
            && pin.name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            && valid_sha(&pin.sha256), "invalid file pin")?;
        let bytes = read_file(&Path::new(root).join("inputs").join(&pin.name), limit)?;
        require(digest(&bytes) == pin.sha256, "input SHA256 mismatch")?;
        Ok(bytes)
    }

    fn validate_identity(input: &Inputs) -> Result<(), String> {
        require(input.schema_version == 1 && matches!(input.mode.as_str(), "tamper-only" | "install"), "schema/mode refused")?;
        require(input.nonce.len() == 32 && input.nonce.bytes().all(|b| b.is_ascii_hexdigit()), "nonce must be 32 hex bytes")?;
        // The real CLI control socket is 103 bytes with this exact TEST root;
        // the previous prefix produced a 122-byte socket and exceeded SUN_LEN.
        require(input.fixture_root == format!("D:/codex/build/u-{}", input.nonce) && safe_path(&input.fixture_root), "own fixture path refused")?;
        require(input.identifier == "io.agentgrid.desktop.test" && input.product_name.ends_with(" TEST")
            && input.product_name.len() <= 80 && input.product_name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_')), "TEST identity required")?;
        require(!input.main_binary.is_empty() && input.main_binary.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            && valid_sha(&input.installed_exe_sha256), "installed TEST executable pin required")?;
        let version = Version::parse(&input.version).map_err(|e| e.to_string())?;
        require(version > Version::new(0, 1, 0) && version.pre.is_empty() && version.build.is_empty(), "actual stable signed version required")
    }

    fn verify_installer(input: &Inputs) -> Result<(), String> {
        require(!input.installer.name.is_empty() && !input.installer.name.contains("..")
            && input.installer.name.ends_with(".exe") && input.installer.name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            && valid_sha(&input.installer.sha256), "invalid bare installer pin")?;
        let path = Path::new(&input.fixture_root).join("inputs").join(&input.installer.name);
        no_reparse(&path, false)?;
        let mut file = File::open(&path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut magic = [0; 2];
        file.read_exact(&mut magic).map_err(|e| e.to_string())?;
        require(magic == *b"MZ", "bare NSIS executable required")?;
        hasher.update(magic);
        let mut chunk = [0; 65536];
        let mut size = 2usize;
        loop {
            let n = file.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            size += n;
            require(size <= INSTALLER_LIMIT, "original installer size limit")?;
            hasher.update(&chunk[..n]);
        }
        require(size >= 2 && format!("{:x}", hasher.finalize()) == input.installer.sha256, "original installer SHA256 mismatch")
    }

    fn event(file: &mut File, phase: &str, details: Value) -> Result<(), String> {
        let line = serde_json::json!({"phase":phase,"details":details,"pid":std::process::id()});
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        println!("{line}");
        std::io::stdout().flush().map_err(|e| e.to_string())
    }

    fn validate_package(input: &Inputs) -> Result<Value, String> {
        let root = &input.fixture_root;
        let config: Value = serde_json::from_slice(&pinned(root, &input.merged_config, METADATA_LIMIT)?).map_err(|e| e.to_string())?;
        require(config["productName"] == input.product_name && config["identifier"] == input.identifier && config["version"] == input.version
            && config["bundle"]["windows"]["nsis"]["installMode"] == "currentUser", "merged TEST config mismatch")?;
        for value in [&config["bundle"]["fileAssociations"], &config["plugins"]["deep-link"]] {
            require(value.is_null() || value.as_array().is_some_and(|v| v.is_empty()), "associations/deep links outside TEST install scope")?;
        }
        for value in [&config["bundle"]["windows"]["minimumWebview2Version"], &config["bundle"]["windows"]["nsis"]["minimumWebview2Version"]] {
            require(value.is_null() || value == "", "shared WebView2 update minimum must be absent")?;
        }
        let windows = config["app"]["windows"].as_array().ok_or("TEST window config missing")?;
        require(!windows.is_empty() && windows.iter().all(|w| w["title"].as_str().is_some_and(|s| s.ends_with(" TEST"))), "TEST window title required")?;
        // The parent's pinned CI/package receipt is the binary-to-config binding.
        // JSON config alone cannot prove the NSIS executable's embedded identity.
        let receipt: Value = serde_json::from_slice(&pinned(root, &input.package_receipt, METADATA_LIMIT)?).map_err(|e| e.to_string())?;
        for (field, expected) in [
            ("kind", "signed-test-package-binding.v1"), ("productName", input.product_name.as_str()),
            ("identifier", input.identifier.as_str()), ("version", input.version.as_str()),
            ("installerSHA256", input.installer.sha256.as_str()), ("mergedConfigSHA256", input.merged_config.sha256.as_str()),
            ("installedExeSHA256", input.installed_exe_sha256.as_str()), ("mainBinary", input.main_binary.as_str()),
        ] { require(receipt[field] == expected, "parent package receipt mismatch")?; }
        require(receipt["sourceCommit"].as_str().is_some_and(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            && receipt["ciRunId"].as_str().is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())), "CI/source binding missing")?;
        let raw = serde_json::from_slice::<Value>(&pinned(root, &input.manifest, METADATA_LIMIT)?).map_err(|e| e.to_string())?;
        let signature = String::from_utf8(pinned(root, &input.signature, 16 * 1024)?).map_err(|e| e.to_string())?;
        require(raw["platforms"]["windows-x86_64"]["signature"] == signature.trim(), "original signature metadata mismatch")?;
        validate_manifest(&serde_json::to_vec(&raw).map_err(|e| e.to_string())?, &format!("v{}", input.version), &Version::parse(&input.version).map_err(|e| e.to_string())?)
    }

    fn nsis_install_arg(root: &str) -> String {
        format!("/D={}\\install", root.replace('/', "\\"))
    }

    pub fn run() -> Result<(), String> {
        require(std::env::args_os().count() == 1, "argc must be 1; never forward libtest arguments")?;
        let path = std::env::var("OD_UPDATER_ACTUAL_CONFIG").map_err(|_| "missing parent config")?;
        let expected = std::env::var("OD_UPDATER_ACTUAL_CONFIG_SHA256").map_err(|_| "missing parent config SHA256")?;
        require(safe_path(&path) && valid_sha(&expected), "invalid config path/SHA256")?;
        let bytes = read_file(Path::new(&path), METADATA_LIMIT)?;
        require(digest(&bytes) == expected, "config SHA256 mismatch")?;
        let input: Inputs = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        validate_identity(&input)?;
        let transport = validate_transport(&input.transport_url)?;
        require(path == format!("{}/inputs/probe.json", input.fixture_root), "config outside own exact path")?;
        let root = Path::new(&input.fixture_root);
        no_reparse(root, true)?;
        require(read_file(&root.join("OWNER.txt"), 128)? == format!("od161-updater-actual:{}", input.nonce).as_bytes(), "own marker mismatch")?;
        for (name, suffix) in [("HOME", "home"), ("CODEX_HOME", "home/codex"), ("APPDATA", "appdata"),
            ("LOCALAPPDATA", "localappdata"), ("TEMP", "temp"), ("TMP", "temp"),
            ("AGENT_GRID_WORKSPACE_DIR", "workspace"), ("WEBVIEW2_USER_DATA_FOLDER", "webview")] {
            let own = format!("{}/{suffix}", input.fixture_root);
            require(std::env::var(name).ok().as_deref() == Some(own.as_str()), "own environment binding missing")?;
            no_reparse(Path::new(&own), true)?;
        }
        no_reparse(&root.join("install"), true)?;
        no_reparse(&root.join("evidence"), true)?;
        let raw = validate_package(&input)?;
        let public = String::from_utf8(pinned(&input.fixture_root, &input.public_key, 16 * 1024)?).map_err(|e| e.to_string())?;
        require(public.trim() == public_key().ok_or("probe has no build-time configured public key")?, "public key differs from build-time product trust")?;
        verify_installer(&input)?;
        let mut log = OpenOptions::new().write(true).create_new(true).open(root.join("evidence/probe-events.jsonl")).map_err(|e| e.to_string())?;
        event(&mut log, "inputs_validated", serde_json::json!({"mode":input.mode,"configSHA256":expected,"originalSHA256":input.installer.sha256,"componentCurrentVersion":"0.1.0","normalProductOffer":false}))?;
        crate::process_job::install_app_job()?;
        event(&mut log, "app_job_installed", Value::Null)?;
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert("updater".into(), serde_json::json!({"pubkey":""}));
        let app = tauri::test::mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).map_err(|e| e.to_string())?;
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = calls.clone();
        let mut hook_log = log.try_clone().map_err(|e| e.to_string())?;
        let hook_log = Mutex::new(move || {
            preserve_app_before_installer_launch();
            let count = hook_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if event(&mut hook_log, "before_exit", serde_json::json!({"calls":count})).is_err() { std::process::abort(); }
        });
        let update = app.updater_builder().pubkey(public.trim()).target("windows-x86_64")
            .endpoints(vec![format!("https://github.com/{DESKTOP_REPO}/releases/latest/download/latest.json").parse().map_err(|e| format!("endpoint: {e}"))?]).map_err(|e| e.to_string())?
            .configure_client(|_| panic!("probe must reuse product get_bytes, never Update.download/check"))
            .clear_installer_args().installer_arg(nsis_install_arg(&input.fixture_root))
            .on_before_exit(move || match hook_log.lock() { Ok(mut hook) => hook(), Err(_) => std::process::abort() })
            .build().map_err(|e| e.to_string())?.restore_update(raw).map_err(|e| e.to_string())?.ok_or("no component update")?;
        require(update.current_version == "0.1.0" && update.version == input.version, "mock/actual version boundary mismatch")?;
        let client = request_client()?;
        event(&mut log, "download_started", transport_observation(&input.transport_url, &transport, &update.download_url))?;
        let mut downloaded = get_bytes(&client, transport.as_str(), INSTALLER_LIMIT, Instant::now() + REQUEST_LIMIT)?;
        require(digest(&downloaded) == input.installer.sha256, "download SHA256 differs from preserved signed original")?;
        update.verify(&downloaded).map_err(|e| format!("original signature: {e}"))?;
        event(&mut log, "download_verified", serde_json::json!({"bytes":downloaded.len(),"sha256":digest(&downloaded)}))?;
        downloaded[0] ^= 1;
        let rejection = update.install(&downloaded);
        downloaded[0] ^= 1;
        require(matches!(rejection, Err(tauri_plugin_updater::Error::Minisign(_))), "tamper must return native Minisign error")?;
        require(calls.load(Ordering::SeqCst) == 0, "tamper entered before-exit hook")?;
        update.verify(&downloaded).map_err(|e| e.to_string())?;
        verify_installer(&input)?;
        validate_package(&input)?;
        pinned(&input.fixture_root, &input.public_key, 16 * 1024)?;
        require(digest(&read_file(Path::new(&path), METADATA_LIMIT)?) == expected, "parent config changed")?;
        event(&mut log, "tamper_rejected", serde_json::json!({"beforeExitCalls":0,"originalsUnchanged":true}))?;
        if input.mode == "tamper-only" { return event(&mut log, "tamper_only_complete", Value::Null); }
        event(&mut log, "install_armed", serde_json::json!({"installerArgLast":nsis_install_arg(&input.fixture_root),"expectedExe":format!("{}/install/{}.exe",input.fixture_root,input.main_binary),"expectedExeSHA256":input.installed_exe_sha256,"installerComplete":"UNKNOWN","restart":"UNKNOWN","parentOracleRequired":true}))?;
        // Windows success exits inside the plugin. No child-side PASS can follow.
        let result = update.install(downloaded);
        event(&mut log, "install_returned", serde_json::json!({"beforeExitCalls":calls.load(Ordering::SeqCst),"error":format!("{result:?}")}))?;
        Err("install returned; installer completion/restart not established".into())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn probe_nsis_install_argument_uses_native_windows_directory() {
            assert_eq!(
                nsis_install_arg("D:/codex/build/u-0123456789abcdef0123456789abcdef"),
                r"/D=D:\codex\build\u-0123456789abcdef0123456789abcdef\install"
            );
        }
        #[test]
        fn probe_paths_reject_aliases_and_nsis_delimiters() {
            assert!(safe_path("D:/codex/build/u-abc/install"));
            for bad in ["C:/Users/Someone", "D:/codex/build/../production", "D:/codex/build/a b", "D:/codex/build/a/", "D:/codex/build/a\\b", "D:/codex/build/a:stream", "D:/codex/build/a\"/R"] { assert!(!safe_path(bad), "{bad}"); }
        }
        #[test]
        fn probe_pins_require_exact_lowercase_sha256() {
            assert!(valid_sha(&"ab".repeat(32)));
            for bad in ["", "ABC", &"g".repeat(64), &"a".repeat(63)] { assert!(!valid_sha(bad)); }
        }
        #[test]
        fn probe_identity_refuses_production_and_forged_current_version() {
            let pin = serde_json::json!({"name":"pending","sha256":"ab".repeat(32)});
            let mut input: Inputs = serde_json::from_value(serde_json::json!({
                "schemaVersion":1,"nonce":"a".repeat(32),"fixtureRoot":format!("D:/codex/build/u-{}","a".repeat(32)),
                "transportUrl":"https://artifact.example.invalid/parser-only.exe",
                "mode":"tamper-only","version":"1.6.2","productName":"OpenDIY Desktop TEST","identifier":"io.agentgrid.desktop.test",
                "mainBinary":"opendiy-desktop","installedExeSha256":"ab".repeat(32),"installer":pin,"signature":pin,"manifest":pin,
                "publicKey":pin,"mergedConfig":pin,"packageReceipt":pin
            })).unwrap();
            assert_eq!(format!("{}/home/codex/app-server-control/app-server-control.sock", input.fixture_root).len(), 103);
            assert!(validate_identity(&input).is_ok());
            let short_root = input.fixture_root.clone();
            input.fixture_root = format!("D:/codex/build/od161-updater-actual-{}", input.nonce);
            assert!(validate_identity(&input).is_err());
            input.fixture_root = short_root;
            input.identifier = "io.agentgrid.desktop".into();
            assert!(validate_identity(&input).is_err());
            input.identifier = "io.agentgrid.desktop.test".into();
            input.version = "0.1.0".into();
            assert!(validate_identity(&input).is_err());
            input.version = "1.6.2-beta.1".into();
            assert!(validate_identity(&input).is_err());
        }
        #[test]
        fn probe_transport_rejects_scheme_credentials_and_fragment() {
            for bad in ["http://artifact.example.invalid/a.exe", "file:///a.exe", "https://user@artifact.example.invalid/a.exe",
                "https://user:password@artifact.example.invalid/a.exe", "https://artifact.example.invalid/a.exe#fragment",
                "https://artifact.example.invalid/a.exe#", " https://artifact.example.invalid/a.exe", "https://artifact.example.invalid/a.exe\n?token=x"] {
                assert!(validate_transport(bad).is_err());
            }
            // Parser-only synthetic examples. No endpoint existence/download assertion.
            assert!(validate_transport("https://artifact.example.invalid/a.exe").is_ok());
            assert!(validate_transport("https://artifact.example.invalid/a.exe?token=PARSER_ONLY_QUERY").is_ok());
        }
        #[test]
        fn probe_transport_observation_redacts_query_and_marks_component_boundary() {
            let raw = "https://artifact.example.invalid/a.exe?token=PARSER_ONLY_QUERY";
            let transport = validate_transport(raw).unwrap();
            let metadata = Url::parse("https://github.com/jaeseongs95/opendiy-desktop/releases/download/v1.6.2/a.exe").unwrap();
            let observed = transport_observation(raw, &transport, &metadata);
            assert_eq!(observed["componentOnly"], true);
            assert_eq!(observed["transportDiffersFromMetadata"], true);
            assert_eq!(observed["transportHost"], "artifact.example.invalid");
            assert_eq!(observed["transportPath"], "/a.exe");
            assert_eq!(observed["transportUrlSHA256"], digest(raw.as_bytes()));
            assert!(!observed.to_string().contains("PARSER_ONLY_QUERY"));
            assert!(!observed.to_string().contains("?token="));
        }
    }
}

fn restore(app: &tauri::AppHandle, raw: Value) -> Result<tauri_plugin_updater::Update, String> {
    let key = public_key().ok_or("업데이트 설치 미구성: 신뢰 공개 키가 지정되지 않았습니다.")?;
    app.updater_builder().pubkey(key).timeout(REQUEST_LIMIT)
        .endpoints(vec![format!("https://github.com/{DESKTOP_REPO}/releases/latest/download/latest.json").parse().map_err(|_| "업데이트 주소 구성 오류")?])
        .map_err(|_| "업데이트 설치 주소를 구성할 수 없습니다.")?
        .on_before_exit(preserve_app_before_installer_launch)
        .build().map_err(|_| "업데이트 설치 환경을 구성할 수 없습니다.")?
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
            update.install(bytes).map_err(|_| "업데이트 서명 검증 또는 설치 시작에 실패했습니다. 설치 완료로 처리하지 않았습니다.".to_owned())
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
        // The Codex updater's date grammar allows any eight digits. A SemVer
        // prerelease parser would add an unsupported leading-zero restriction.
        let tag = parse_release_tag(&get_bytes(&client, &format!("https://api.github.com/repos/{CODEX_REPO}/releases/latest"), METADATA_LIMIT, deadline)?)?;
        Ok(codex_status(current, tag))
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

    fn in_fresh_tls_process(test_name: &str, scenario: impl FnOnce()) {
        const CHILD: &str = "OPENDIY_UPDATE_TLS_TEST_CHILD";
        let witness = format!("fresh TLS case completed: {test_name}");
        if std::env::var(CHILD).as_deref() == Ok(test_name) {
            assert!(rustls::crypto::CryptoProvider::get_default().is_none(),
                "fresh child must not inherit a provider installed by another test");
            scenario();
            println!("\n{witness}");
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD, test_name).output().expect("start isolated TLS test process");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "isolated TLS case failed: {}\n{stdout}\n{stderr}", output.status);
        assert!(stdout.lines().any(|line| line == witness), "isolated TLS case did not run\n{stdout}\n{stderr}");
    }

    #[test]
    fn desktop_updates_tls_client_starts_without_preinstalled_provider() {
        in_fresh_tls_process("desktop_updates::tests::desktop_updates_tls_client_starts_without_preinstalled_provider", || {
            drop(request_client().expect("first actual Client must initialize its provider"));
            let provider = rustls::crypto::CryptoProvider::get_default().unwrap().clone();
            drop(request_client().expect("actual Client must reuse an installed provider"));
            assert!(std::sync::Arc::ptr_eq(&provider, rustls::crypto::CryptoProvider::get_default().unwrap()));
        });
    }

    #[test]
    fn desktop_updates_tls_clients_initialize_concurrently() {
        in_fresh_tls_process("desktop_updates::tests::desktop_updates_tls_clients_initialize_concurrently", || {
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
            let workers: Vec<_> = (0..4).map(|_| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    drop(request_client().expect("concurrent actual Client must initialize"));
                    let provider = rustls::crypto::CryptoProvider::get_default().unwrap().clone();
                    for _ in 0..2 {
                        drop(request_client().expect("repeated actual Client must initialize"));
                        assert!(std::sync::Arc::ptr_eq(&provider, rustls::crypto::CryptoProvider::get_default().unwrap()));
                    }
                    provider
                })
            }).collect();
            for worker in workers {
                let provider = worker.join().expect("TLS initialization worker must not panic");
                assert!(std::sync::Arc::ptr_eq(&provider, rustls::crypto::CryptoProvider::get_default().unwrap()));
            }
        });
    }

    #[test]
    fn desktop_updates_packaged_config_initializes_without_install_credentials() {
        let app_config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let config: tauri_plugin_updater::Config =
            serde_json::from_value(app_config["plugins"]["updater"].clone())
                .expect("the shipped app must initialize its registered updater plugin");
        assert!(config.pubkey.is_empty(), "trust is supplied only by the build-time key");
        assert!(config.endpoints.is_empty(), "checks use the existing fixed repository path");
        assert!(!config.dangerous_insecure_transport_protocol);
        assert!(!config.dangerous_accept_invalid_certs);
        assert!(!config.dangerous_accept_invalid_hostnames);
    }

    #[test]
    fn desktop_updates_codex_version_parser_preserves_contract_identity() {
        for (version, tag, revision) in [
            ("codex-cli 0.166.0+native-r38.20261106", "v0.166.0-r38.20261106", 38),
            ("0.166.0-alpha+native-r38.20261106", "v0.166.0-alpha-r38.20261106", 38),
            ("0.166.0-alpha.1+native-r38.20261106", "v0.166.0-alpha.1-r38.20261106", 38),
            ("0.166.0-alpha.1.2+native-r38.20261106", "v0.166.0-alpha.1.2-r38.20261106", 38),
            ("0.0.0-alpha.0.0+native-r2147483647.00000000", "v0.0.0-alpha.0.0-r2147483647.00000000", i32::MAX),
        ] {
            assert_eq!(parse_codex_version(version), Some(CodexReleaseIdentity { revision, tag: tag.into() }), "{version}");
        }
    }

    #[test]
    fn desktop_updates_codex_version_parser_rejects_other_runtimes_and_malformed_versions() {
        for version in [
            "codex-cli 0.166.0", "0.166.0+other-r38.20261106", "0.166.0-r38.20261106", "Codex-cli 0.166.0+native-r38.20261106",
            "0.166+native-r38.20261106", "01.166.0+native-r38.20261106", "0.166.00+native-r38.20261106",
            "0.166.0-beta.1+native-r38.20261106", "0.166.0-alpha.01+native-r38.20261106", "0.166.0-alpha.1.2.3+native-r38.20261106",
            "0.166.0-alpha.+native-r38.20261106", "0.166.0+native-r0.20261106", "0.166.0+native-r038.20261106",
            "0.166.0+native-r2147483648.20261106", "0.166.0+native-r-38.20261106", "0.166.0+native-r38.2026116",
            "0.166.0+native-r38.202611066", "0.166.0+native-r38.2026x106", "0.166.0+native-r38.20261106.extra",
            "0.166.0+native-r38.20261106\n", " codex-cli 0.166.0+native-r38.20261106",
        ] {
            assert_eq!(parse_codex_version(version), None, "{version:?}");
        }
        assert!(parse_codex_version(&"1".repeat(257)).is_none());
    }

    #[test]
    fn desktop_updates_codex_release_tag_requires_exact_canonical_form() {
        assert_eq!(parse_codex_release_tag("v0.166.0-r38.20261106"), Some(CodexReleaseIdentity { revision: 38, tag: "v0.166.0-r38.20261106".into() }));
        assert_eq!(parse_codex_release_tag("v0.166.0-alpha.1.2-r2147483647.00000000").unwrap().revision, i32::MAX);
        for tag in ["0.166.0-r38.20261106", "V0.166.0-r38.20261106", "v0.166.0+native-r38.20261106", "v0.166.0-r038.20261106",
            "v0.166.0-beta-r38.20261106", "v0.166.0-r0.20261106", "v0.166.0-r2147483648.20261106", "v0.166.0-r38.20261106.extra"] {
            assert!(parse_codex_release_tag(tag).is_none(), "{tag}");
        }
        assert!(parse_codex_release_tag(&"v".repeat(129)).is_none());
        let tag = parse_release_tag(br#"{"tag_name":"v0.1.0-alpha.0-r39.00000000","draft":false,"prerelease":false}"#).unwrap();
        assert_eq!(compare_codex_versions("0.166.0+native-r38.20261106", &tag), CodexComparison::Available,
            "metadata intake must preserve the updater's date grammar without SemVer ordering");
    }

    #[test]
    fn desktop_updates_codex_comparison_uses_revision_and_exact_same_revision_identity() {
        let current = "codex-cli 0.166.0+native-r38.20261106";
        for (tag, expected) in [
            ("v0.166.0-r38.20261106", CodexComparison::Current),
            ("v0.166.0-r39.20261107", CodexComparison::Available),
            ("v0.1.0-alpha.0-r39.00000000", CodexComparison::Available),
            ("v0.166.0-r38.20261107", CodexComparison::ReleaseIdentityMismatch),
            ("v0.167.0-r38.20261106", CodexComparison::ReleaseIdentityMismatch),
            ("v0.166.0-alpha-r38.20261106", CodexComparison::ReleaseIdentityMismatch),
            ("v0.166.0-r37.20261105", CodexComparison::DowngradeRefused),
            ("v9.999.999-r37.99999999", CodexComparison::DowngradeRefused),
            ("v0.166.0+native-r39.20261107", CodexComparison::InvalidReleaseIdentity),
        ] {
            assert_eq!(compare_codex_versions(current, tag), expected, "{tag}");
        }
        assert_eq!(compare_codex_versions("codex-cli 0.166.0", "v0.166.0-r39.20261107"), CodexComparison::UnsupportedRuntime);
    }

    #[test]
    fn desktop_updates_codex_status_exposes_candidates_without_enabling_installation() {
        let current = "codex-cli 0.166.0+native-r38.20261106";
        for (version, tag, expected, reason_code) in [
            (current, "v0.166.0-r38.20261106", "current", None),
            (current, "v0.166.0-r39.20261107", "available", None),
            (current, "v0.166.0-r38.20261107", "unknown", Some("release_identity_mismatch:")),
            (current, "v0.166.0-r37.20261105", "unknown", Some("downgrade_refused:")),
            (current, "v0.166.0+native-r39.20261107", "unknown", Some("invalid_release_identity:")),
            ("codex-cli 0.166.0", "v0.166.0-r39.20261107", "unknown", Some("unsupported_runtime:")),
        ] {
            let result = codex_status(version.into(), tag.into());
            assert_eq!(result.status, expected); assert_eq!(result.current_version, version);
            assert!(!result.install_supported, "metadata comparison never authorizes installation");
            if let Some(code) = reason_code { assert!(result.reason.as_deref().unwrap().starts_with(code)); }
            if reason_code == Some("invalid_release_identity:") {
                assert!(result.latest_version.is_none()); assert!(result.release_url.is_none());
            } else {
                assert_eq!(result.latest_version.as_deref(), tag.strip_prefix('v'));
                assert_eq!(result.release_url, Some(format!("https://github.com/{CODEX_REPO}/releases/tag/{tag}")));
            }
        }
    }

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
        // Inject the installer's Err at the real native admission boundary.
        // This proves permit release, not real ShellExecute/UAC or UI survival.
        let launch_failure = with_install_operation::<()>(|| {
            assert!(with_native_operation::<()>(|| panic!("installer still owns admission")).is_err());
            Err("fixture: installer launch was refused".into())
        });
        assert_eq!(launch_failure.unwrap_err(), "fixture: installer launch was refused");
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
