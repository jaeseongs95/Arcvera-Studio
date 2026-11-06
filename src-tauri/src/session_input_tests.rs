use super::*;

#[test]
fn binary_input_preserves_every_byte_without_utf8_expansion() {
    let byte_string: String = (0u8..=255).map(char::from).collect();
    assert_eq!(binary_input_bytes(&byte_string).unwrap(), (0u8..=255).collect::<Vec<u8>>());
}

#[test]
fn binary_input_enforces_the_byte_limit_before_writing() {
    assert_eq!(binary_input_bytes(&"\u{ff}".repeat(MAX_WRITE_BYTES)).unwrap(), vec![255; MAX_WRITE_BYTES]);
    assert_eq!(binary_input_bytes(&"\u{ff}".repeat(MAX_WRITE_BYTES + 1)).unwrap_err(), "terminal write exceeds 64 KiB limit");
}

#[test]
fn binary_input_rejects_non_byte_characters_without_truncating_them() {
    for input in ["\u{100}", "abc한글", "\u{1f600}"] {
        assert_eq!(binary_input_bytes(input).unwrap_err(), "terminal binary input contains a non-byte character");
    }
}

#[test]
fn binary_input_uses_the_existing_session_admission() {
    let engine = Engine::new(Path::new(":memory:"), Arc::new(|_| Ok(()))).unwrap();
    let ordinary = engine.write("not-owned", 1, "input").unwrap_err();
    assert_eq!(engine.write_binary("not-owned", 1, "\u{80}\u{ff}").unwrap_err(), ordinary);
}
