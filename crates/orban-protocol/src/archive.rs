//! Legacy archive interoperability. CBC-CTS provides no integrity protection;
//! callers must validate the decrypted document before using its contents.
use aes::{
    Aes256,
    cipher::{BlockDecrypt, BlockEncrypt, KeyInit},
};
use zeroize::Zeroizing;

fn cipher(code: &str) -> Result<Aes256, String> {
    let mut key = Zeroizing::new([0u8; 32]);
    for (i, b) in key.iter_mut().enumerate() {
        *b = code.as_bytes()[i % code.len()];
    }
    Aes256::new_from_slice(&*key).map_err(|_| "Invalid key".into())
}

/// The inverse of `decrypt`: `iv` followed by the CBC-CTS ciphertext, the
/// last two blocks swapped as the firmware expects.
pub fn encrypt(plain: &[u8], code: &str, iv: [u8; 16]) -> Result<Vec<u8>, String> {
    if plain.len() < 16 || plain.len() + 16 > 65535 || code.is_empty() || !code.is_ascii() {
        return Err("Invalid archive length or access code".into());
    }
    let aes = cipher(code)?;
    let mut out = iv.to_vec();
    let mut previous = iv;
    let full = plain.len() / 16 * 16;
    for chunk in plain[..full].chunks_exact(16) {
        let mut block = aes::Block::clone_from_slice(chunk);
        block.iter_mut().zip(previous).for_each(|(a, b)| *a ^= b);
        aes.encrypt_block(&mut block);
        previous.copy_from_slice(&block);
        out.extend_from_slice(&block);
    }
    let remainder = plain.len() - full;
    if remainder != 0 {
        // Encrypt the zero-padded tail after the last full block, then emit
        // it first and the first `remainder` bytes of that last block after.
        let last_full = out.split_off(out.len() - 16);
        let mut tail = aes::Block::default();
        tail[..remainder].copy_from_slice(&plain[full..]);
        tail.iter_mut().zip(previous).for_each(|(a, b)| *a ^= b);
        aes.encrypt_block(&mut tail);
        out.extend_from_slice(&tail);
        out.extend_from_slice(&last_full[..remainder]);
    }
    Ok(out)
}

/// The start of an archive that arrived cut: every complete block after the
/// IV, decrypted as plain CBC. Valid only for a prefix that ends before the
/// last two blocks of the whole archive, where CTS swaps them.
pub fn decrypt_prefix(bytes: &[u8], code: &str) -> Result<Vec<u8>, String> {
    if bytes.len() < 32 || bytes.len() > 65535 || code.is_empty() || !code.is_ascii() {
        return Err("Invalid archive length or access code".into());
    }
    let aes = cipher(code)?;
    let mut previous: [u8; 16] = bytes[..16].try_into().unwrap();
    let mut plaintext = Vec::with_capacity(bytes.len());
    for chunk in bytes[16..].chunks_exact(16) {
        let mut block = aes::Block::clone_from_slice(chunk);
        aes.decrypt_block(&mut block);
        plaintext.extend(block.iter().zip(previous).map(|(a, b)| a ^ b));
        previous.copy_from_slice(chunk);
    }
    Ok(plaintext)
}

pub fn decrypt(bytes: &[u8], code: &str) -> Result<Vec<u8>, String> {
    if bytes.len() < 32 || bytes.len() > 65535 || code.is_empty() || !code.is_ascii() {
        return Err("Invalid archive length or access code".into());
    }
    let aes = cipher(code)?;
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
