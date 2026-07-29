use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine;
use rand::RngCore;
use rand::rngs::OsRng;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Encrypts plaintext with AES-256-CBC using a 32-byte key.
/// IV is randomly generated and prepended to the ciphertext, matching C# EncryptionHelper.
pub fn encrypt(plain_text: &str, key: &[u8; 32]) -> String {
    let mut iv = [0u8; 16];
    OsRng.fill_bytes(&mut iv);

    let pt = plain_text.as_bytes();
    let buf_len = pt.len() + 16 + 16;
    let mut buf = vec![0u8; buf_len];
    buf[..pt.len()].copy_from_slice(pt);

    let ct = Aes256CbcEnc::new(key.into(), &iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pt.len())
        .expect("encrypt failed");

    let mut out = Vec::with_capacity(16 + ct.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(ct);

    base64::engine::general_purpose::STANDARD.encode(&out)
}

/// Decrypts ciphertext encrypted with AES-256-CBC by encrypt().
/// Reads first 16 bytes as IV, decrypts the rest.
pub fn decrypt(cipher_text: &str, key: &[u8; 32]) -> Result<String, String> {
    let full = base64::engine::general_purpose::STANDARD
        .decode(cipher_text)
        .map_err(|e| format!("base64 decode error: {}", e))?;

    if full.len() < 17 {
        return Err("ciphertext too short".to_string());
    }

    let (iv, ct) = full.split_at(16);
    let mut buf = ct.to_vec();
    let pt = Aes256CbcDec::new(key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("decrypt error: {:?}", e))?;

    String::from_utf8(pt.to_vec()).map_err(|e| format!("utf8 error: {}", e))
}
