/// Legacy password obfuscation, required by the device. It is not encryption.
/// The caller must keep the resulting buffer out of diagnostics and logs.
pub fn login_request(code: &str) -> Result<Vec<u8>, String> {
    if code.is_empty() || code.len() > 64 || !code.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Use an ASCII alphanumeric remote access code".into());
    }
    let mut state = 123456_u32;
    let mut result = b"connect\n".to_vec();
    for value in code.bytes().map(|c| c.to_ascii_uppercase()) {
        state = state.wrapping_mul(419).wrapping_add(6173) % 29282;
        let mask = state as u8;
        let encoded = if value == mask { value } else { value ^ mask };
        if encoded == b'\n' || encoded == 0 {
            return Err("This code cannot be represented by the verified login codec".into());
        }
        result.push(encoded);
    }
    result.extend_from_slice(b"\n\0");
    Ok(result)
}
