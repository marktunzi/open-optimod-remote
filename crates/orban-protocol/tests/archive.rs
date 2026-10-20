use orban_protocol::archive::{decrypt, encrypt};
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
#[test]
fn encrypt_is_the_inverse_of_decrypt_for_every_tail_length() {
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/archive-vectors.json")).unwrap();
    for vector in vectors {
        let iv: [u8; 16] = vector.cipher[..16].try_into().unwrap();
        assert_eq!(encrypt(&vector.plain, "9876", iv).unwrap(), vector.cipher);
    }
    for len in 16..80 {
        let plain: Vec<u8> = (0..len).map(|i| (i * 37 % 251) as u8).collect();
        let sealed = encrypt(&plain, "ADMIN", [7; 16]).unwrap();
        assert_eq!(sealed.len(), len + 16);
        assert_eq!(decrypt(&sealed, "ADMIN").unwrap(), plain, "length {len}");
    }
}
