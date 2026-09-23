use orban_protocol::archive::decrypt;
use serde::Deserialize;
#[derive(Deserialize)]
struct Vector {
    plain: Vec<u8>,
    cipher: Vec<u8>,
}
#[test]
fn decrypts_independent_aes_cbc_cts_vectors_including_partial_final_blocks() {
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/archive-vectors.json")).unwrap();
    for vector in vectors {
        assert_eq!(decrypt(&vector.cipher, "9876").unwrap(), vector.plain);
    }
}
#[test]
fn rejects_missing_iv_or_password() {
    assert!(decrypt(&[0; 31], "9876").is_err());
    assert!(decrypt(&[0; 32], "").is_err());
}
