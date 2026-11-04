/// Keep OS launch policy at the native boundary, including callers bypassing TS.
fn validated_url(value: &str) -> Result<String, String> {
    let invalid = || "HTTP(S) 웹 주소만 열 수 있습니다.".to_string();
    if value.is_empty() || value.len() > 8192 || value.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return Err(invalid());
    }
    let lower = value.to_ascii_lowercase();
    let authority = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")).ok_or_else(invalid)?;
    if authority.is_empty() || authority.starts_with('/') || authority.starts_with('?') || authority.starts_with('#') { return Err(invalid()); }
    if lower.as_bytes().windows(3).any(|part| part[0] == b'%' && u8::from_str_radix(std::str::from_utf8(&part[1..]).unwrap_or(""), 16).is_ok_and(|byte| byte <= 0x1f || byte == 0x7f)) {
        return Err(invalid());
    }
    if lower.as_bytes().windows(6).any(|part| &part[..4] == b"%c2%" && matches!(part[4], b'8' | b'9') && part[5].is_ascii_hexdigit()) { return Err(invalid()); }
    let url = tauri::Url::parse(value).map_err(|_| "올바른 웹 주소가 아닙니다.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() { return Err(invalid()); }
    Ok(url.to_string())
}

#[cfg(windows)]
fn launch_url(url: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    // ShellExecute receives the URL as one native argument; no command shell.
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), operation.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), 1) } as isize;
    if result <= 32 { return Err(format!("기본 브라우저 실행에 실패했습니다 (Windows 오류 {result}).")); }
    Ok(())
}

#[cfg(not(windows))]
fn launch_url(url: &str) -> Result<(), String> {
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let result = std::process::Command::new(program).arg(url).status().map_err(|error| format!("기본 브라우저 실행에 실패했습니다: {error}"))?;
    if !result.success() { return Err(format!("기본 브라우저 실행에 실패했습니다 ({result}).")); }
    Ok(())
}

#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_url_with(&url, launch_url)).await.map_err(|error| format!("브라우저 실행 작업자 오류: {error}"))?
}

fn open_url_with(value: &str, launch: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
    launch(&validated_url(value)?)
}

#[cfg(test)]
mod tests {
    use super::{open_url_with, validated_url};
    #[test]
    fn native_launch_boundary_rejects_before_launch_and_preserves_failure() {
        assert!(open_url_with("javascript:alert(1)", |_| panic!("invalid URL launched")).is_err());
        let result = open_url_with("https://example.com/경로?q=한글", |url| {
            assert_eq!(url, "https://example.com/%EA%B2%BD%EB%A1%9C?q=%ED%95%9C%EA%B8%80");
            Err("native launch failed (test fixture)".to_string())
        });
        assert_eq!(result.unwrap_err(), "native launch failed (test fixture)");
    }
    #[test]
    fn external_url_boundary_accepts_http_and_unicode_query() {
        assert_eq!(validated_url("https://community.openai.com/c/codex/37").unwrap(), "https://community.openai.com/c/codex/37");
        assert_eq!(validated_url("http://example.com/a?b=1&c=2#part").unwrap(), "http://example.com/a?b=1&c=2#part");
        let unicode = validated_url("https://예시.한국/경로?q=한글").unwrap();
        assert_eq!(validated_url(&unicode).unwrap(), unicode);
        assert!(unicode.starts_with("https://xn--"));
        assert!(unicode.contains("/%EA%B2%BD%EB%A1%9C?q="));
    }
    #[test]
    fn external_url_boundary_rejects_protocols_controls_and_missing_authority() {
        for value in ["javascript:alert(1)", "file:///C:/Windows", "data:text/html,x", "mailto:a@example.com", " https://example.com", "https://example.com\n", "https://example.com/\0x", "https://example.com/%00", "https://example.com/%0a", "https://example.com/%7F", "https://example.com/%c2%9f", "https://example.com\\path", "https:///path", "https://?q=a", "https://", "https://[invalid", "https://example.com:99999/"] {
            assert!(validated_url(value).is_err(), "unexpected accepted fixture {value:?}");
        }
        assert!(validated_url(&format!("https://example.com/{}", "a".repeat(8192))).is_err());
    }
}
