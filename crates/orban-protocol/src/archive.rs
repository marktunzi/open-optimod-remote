//! Legacy archive interoperability. CBC-CTS provides no integrity protection;
//! callers must validate the decrypted document before using its contents.
use aes::{
    Aes256,
    cipher::{BlockDecrypt, KeyInit},
};
use zeroize::Zeroizing;

pub fn decrypt(bytes: &[u8], code: &str) -> Result<Vec<u8>, String> {
    if bytes.len() < 32 || bytes.len() > 65535 || code.is_empty() || !code.is_ascii() {
        return Err("Invalid archive length or access code".into());
    }
    let mut key = Zeroizing::new([0u8; 32]);
    for (i, b) in key.iter_mut().enumerate() {
        *b = code.as_bytes()[i % code.len()];
    }
    let aes = Aes256::new_from_slice(&*key).map_err(|_| "Invalid key")?;
    let mut previous: [u8; 16] = bytes[..16].try_into().unwrap();
    let ciphertext = &bytes[16..];
    let remainder = ciphertext.len() % 16;
    let main = if remainder == 0 {
        ciphertext.len()
    } else {
        ciphertext.len() - 16 - remainder
    };
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    for chunk in ciphertext[..main].chunks_exact(16) {
        let mut block = aes::Block::clone_from_slice(chunk);
        aes.decrypt_block(&mut block);
        plaintext.extend(block.iter().zip(previous).map(|(a, b)| a ^ b));
        previous.copy_from_slice(chunk);
    }
    if remainder != 0 {
        let mut last = aes::Block::clone_from_slice(&ciphertext[main..main + 16]);
        aes.decrypt_block(&mut last);
        let short = &ciphertext[main + 16..];
        let mut penultimate = last;
        penultimate[..remainder].copy_from_slice(short);
        aes.decrypt_block(&mut penultimate);
        plaintext.extend(penultimate.iter().zip(previous).map(|(a, b)| a ^ b));
        plaintext.extend(last[..remainder].iter().zip(short).map(|(a, b)| a ^ b));
    }
    Ok(plaintext)
}
