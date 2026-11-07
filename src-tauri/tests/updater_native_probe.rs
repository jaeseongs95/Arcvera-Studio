#![cfg(windows)]
// Test-only MockRuntime. PN01-05 are offline; PN06-07 use an owned loopback server.
// No updater install, restart, external requests or private key access.
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri_plugin_updater::{Error, Update, Updater, UpdaterExt};
use windows_sys::Win32::Security::Cryptography::{
    CryptBinaryToStringW, CryptStringToBinaryW, CRYPT_STRING_BASE64, CRYPT_STRING_NOCRLF,
};

const INSTALLER: &str = "OpenDIYDesktop-1.6.1-windows-x64-setup.exe";
const PUBLIC_SHA: &str = "a8c985f62b8182b7fc54d892fb36a27140c3f27e590ae5e891d42ff494b01803";
const INPUTS: [(&str, &str); 3] = [
    ("latest.json", "2f4e6f02c8c9caab682bd1110e7a40ce60ebc2e23df742cba27f6aa0456f15fc"),
    (INSTALLER, "5c458a9d88ebd7ce335362e754060c3fe04bc41ff931be2e974ee9a65cf6e414"),
    ("OpenDIYDesktop-1.6.1-windows-x64-setup.exe.sig", "8b0480686861faf6ebe871a25bb401388c0fbdb19e6d3c3e7633788548a02067"),
];

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// Reuse the existing Windows dependency for base64 encoding only, never key access.
fn decode_base64(value: &str) -> Vec<u8> {
    let wide: Vec<u16> = value.encode_utf16().collect();
    let mut length = 0;
    unsafe {
        assert_ne!(CryptStringToBinaryW(wide.as_ptr(), wide.len().try_into().unwrap(), CRYPT_STRING_BASE64, std::ptr::null_mut(), &mut length, std::ptr::null_mut(), std::ptr::null_mut()), 0);
        let mut bytes = vec![0; length as usize];
        assert_ne!(CryptStringToBinaryW(wide.as_ptr(), wide.len().try_into().unwrap(), CRYPT_STRING_BASE64, bytes.as_mut_ptr(), &mut length, std::ptr::null_mut(), std::ptr::null_mut()), 0);
        bytes.truncate(length as usize);
        bytes
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    let mut length = 0;
    let flags = CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF;
    unsafe {
        assert_ne!(CryptBinaryToStringW(bytes.as_ptr(), bytes.len().try_into().unwrap(), flags, std::ptr::null_mut(), &mut length), 0);
        let mut wide = vec![0; length as usize];
        assert_ne!(CryptBinaryToStringW(bytes.as_ptr(), bytes.len().try_into().unwrap(), flags, wide.as_mut_ptr(), &mut length), 0);
        let used = wide.iter().position(|value| *value == 0).unwrap_or(wide.len());
        String::from_utf16(&wide[..used]).unwrap()
    }
}

fn read_regular(file: &Path) -> Vec<u8> {
    let metadata = fs::symlink_metadata(file).expect("exact artifact metadata");
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert!(metadata.len() <= 512 * 1024 * 1024);
    fs::read(file).expect("exact artifact bytes")
}

struct Fixture {
    _app: tauri::App<tauri::test::MockRuntime>,
    updater: Updater,
    update: Update,
    bytes: Vec<u8>,
    hook_calls: Arc<AtomicUsize>,
    root: PathBuf,
    network: bool,
}

impl Fixture {
    fn load() -> Self {
        let root = PathBuf::from(std::env::var_os("OD_UPDATER_ARTIFACT_DIR").expect("artifact directory"));
        assert!(root.is_absolute());
        for (name, expected) in INPUTS {
            assert_eq!(sha(&read_regular(&root.join(name))), expected, "artifact pin: {name}");
        }
        let public = std::env::var("OD_UPDATER_PUBLIC_KEY").expect("public key variable, never private key");
        // The verified public file is exactly 152 UTF-8 bytes with no BOM/whitespace/newline.
        assert_eq!(public.as_bytes().len(), 152);
        assert_eq!(sha(public.as_bytes()), PUBLIC_SHA, "exact public key byte pin");
        let metadata: serde_json::Value = serde_json::from_slice(&read_regular(&root.join("latest.json"))).unwrap();
        assert_eq!(metadata["version"], "1.6.1");
        let signature = String::from_utf8(read_regular(&root.join(format!("{INSTALLER}.sig")))).unwrap();
        assert_eq!(metadata["platforms"]["windows-x86_64"]["signature"], signature.trim());
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        // PluginApi deserializes its config before UpdaterExt can override the trust key.
        context.config_mut().plugins.0.insert("updater".into(), serde_json::json!({ "pubkey": "" }));
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .expect("MockRuntime plugin state");
        let hook_calls = Arc::new(AtomicUsize::new(0));
        let hook = hook_calls.clone();
        let updater = app.updater_builder()
            .pubkey(public)
            .target("windows-x86_64")
            .endpoints(vec!["https://github.com/jaeseongs95/opendiy-desktop/releases/latest/download/latest.json".parse().unwrap()]).unwrap()
            .configure_client(|_| panic!("network forbidden in offline native probe"))
            .on_before_exit(move || { hook.fetch_add(1, Ordering::SeqCst); })
            .build().expect("offline updater context");
        let update = updater.restore_update(metadata).expect("actual frozen metadata").expect("actual 1.6.1 exceeds mock 0.1.0");
        assert_eq!(update.version, "1.6.1");
        assert_eq!(update.current_version, "0.1.0");
        let bytes = read_regular(&root.join(INSTALLER));
        Self { _app: app, updater, update, bytes, hook_calls, root, network: false }
    }

    fn load_for_download() -> Self {
        let mut fixture = Self::load();
        let hook = fixture.hook_calls.clone();
        // A separate builder preserves the panic-on-network guard in PN01-05.
        fixture.updater = fixture._app.updater_builder()
            .pubkey(std::env::var("OD_UPDATER_PUBLIC_KEY").unwrap())
            .target("windows-x86_64")
            .endpoints(vec!["https://github.com/jaeseongs95/opendiy-desktop/releases/latest/download/latest.json".parse().unwrap()]).unwrap()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .on_before_exit(move || { hook.fetch_add(1, Ordering::SeqCst); })
            .build().expect("loopback-only download context");
        fixture.update = fixture.updater.restore_update(fixture.update.raw_json.clone())
            .expect("unchanged actual metadata").expect("actual update");
        fixture.network = true;
        fixture
    }

    fn finish(&self, id: &str) {
        assert_eq!(self.hook_calls.load(Ordering::SeqCst), 0, "verification must not enter installer hook");
        for (name, expected) in INPUTS {
            assert_eq!(sha(&read_regular(&self.root.join(name))), expected, "original changed: {name}");
        }
        println!("{}", serde_json::json!({"case": id, "actualPluginVerify": true, "actualPluginDownload": self.network, "mockRuntime": true, "network": self.network, "networkScope": if self.network { "loopback" } else { "none" }, "install": false, "restart": false, "beforeExitCalls": 0, "originalsUnchanged": true}));
    }
}

fn mutated_signature(signature: &str, packet: bool) -> String {
    let text = String::from_utf8(decode_base64(signature.trim())).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[2].starts_with("trusted comment: "));
    if packet {
        let mut bytes = decode_base64(&lines[1]);
        assert_eq!(bytes.len(), 74);
        bytes[10] ^= 1;
        lines[1] = encode_base64(&bytes);
    } else {
        lines[2].push('X');
    }
    encode_base64((lines.join("\n") + "\n").as_bytes())
}

fn assert_signature_rejected(result: tauri_plugin_updater::Result<()>) {
    assert!(matches!(result, Err(Error::Minisign(_))), "expected native Minisign rejection, got {result:?}");
}

#[test]
fn pn01_actual_final_artifact_verifies() {
    let fixture = Fixture::load();
    fixture.update.verify(&fixture.bytes).expect("actual final signed bytes");
    fixture.finish("PN01");
}

#[test]
fn pn02_actual_plugin_rejects_installer_mutation() {
    let fixture = Fixture::load();
    let mut bytes = fixture.bytes.clone();
    bytes[0] ^= 1;
    assert_signature_rejected(fixture.update.verify(&bytes));
    fixture.finish("PN02");
}

#[test]
fn pn03_actual_plugin_rejects_signature_mutation() {
    let fixture = Fixture::load();
    let mut update = fixture.update.clone();
    update.signature = mutated_signature(&update.signature, true);
    assert_signature_rejected(update.verify(&fixture.bytes));
    fixture.finish("PN03");
}

#[test]
fn pn04_actual_plugin_rejects_trusted_comment_mutation() {
    let fixture = Fixture::load();
    let mut update = fixture.update.clone();
    update.signature = mutated_signature(&update.signature, false);
    assert_signature_rejected(update.verify(&fixture.bytes));
    fixture.finish("PN04");
}

#[test]
fn pn05_actual_plugin_rejects_announced_version_mismatch() {
    let fixture = Fixture::load();
    let mut metadata = fixture.update.raw_json.clone();
    metadata["version"] = serde_json::json!("1.6.2");
    let update = fixture.updater.restore_update(metadata).unwrap().unwrap();
    assert_eq!(update.signature, fixture.update.signature, "original signature required");
    match update.verify(&fixture.bytes) {
        Err(Error::SignedVersionMismatch { signed, announced }) => {
            assert_eq!(signed, "1.6.1");
            assert_eq!(announced, "1.6.2");
        }
        other => panic!("expected semantic signed-version mismatch, got {other:?}"),
    }
    fixture.finish("PN05");
}

fn download_case(tampered: bool, id: &str) {
    let fixture = Fixture::load_for_download();
    let base: reqwest::Url = std::env::var("OD_UPDATER_LOOPBACK_BASE")
        .expect("owned local HTTP fixture required").parse().unwrap();
    assert_eq!(base.scheme(), "http");
    assert_eq!(base.host_str(), Some("127.0.0.1"));
    assert!(base.port().is_some_and(|port| port > 0));
    assert!(base.username().is_empty() && base.password().is_none());
    assert!(base.query().is_none() && base.fragment().is_none());
    let nonce = base.path().trim_matches('/');
    assert_eq!(nonce.len(), 32);
    assert!(nonce.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(base.path().ends_with('/'));
    let mut update = fixture.update.clone();
    update.download_url = base.join(if tampered { "tampered.exe" } else { "valid.exe" }).unwrap();
    let mut received = 0usize;
    let mut finished = false;
    let result = tauri::async_runtime::block_on(update.download(|chunk, length| {
        assert_eq!(length, Some(fixture.bytes.len() as u64));
        received = received.checked_add(chunk).expect("download length overflow");
        assert!(received <= fixture.bytes.len());
    }, || { finished = true; }));
    assert_eq!(received, fixture.bytes.len(), "actual complete HTTP body required");
    assert!(finished, "body finish callback required; this does not imply verification");
    if tampered {
        assert_signature_rejected(result.map(|_| ()));
    } else {
        let bytes = result.expect("actual download and signature verification");
        assert_eq!(bytes, fixture.bytes);
        assert_eq!(sha(&bytes), INPUTS[1].1);
    }
    fixture.finish(id);
}

#[test]
fn pn06_actual_plugin_downloads_signed_installer() {
    download_case(false, "PN06");
}

#[test]
fn pn07_actual_download_rejects_tampered_installer() {
    download_case(true, "PN07");
}
