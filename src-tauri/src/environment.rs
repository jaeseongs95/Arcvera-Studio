//! Public environment settings retain their previous names as read-only aliases.
use std::ffi::OsString;

pub(crate) fn prefer(current: Option<OsString>, legacy: Option<OsString>) -> Option<OsString> {
    // An explicit empty/invalid current value must reach the caller's validator.
    current.or(legacy)
}

pub(crate) fn setting(current: &str, legacy: &str) -> Option<OsString> {
    prefer(std::env::var_os(current), std::env::var_os(legacy))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renamed_setting_preserves_legacy_and_never_hides_an_explicit_current_value() {
        assert_eq!(prefer(None, None), None);
        assert_eq!(prefer(None, Some("old".into())), Some("old".into()));
        assert_eq!(prefer(Some("new".into()), Some("old".into())), Some("new".into()));
        assert_eq!(prefer(Some("".into()), Some("old".into())), Some("".into()));
        assert_eq!(prefer(Some("relative-invalid".into()), Some("old".into())), Some("relative-invalid".into()));
    }
}
