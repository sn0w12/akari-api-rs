use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rand::RngCore;
use rand::rngs::OsRng;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_128_GCM};
use ring::agreement::{ECDH_P256, EphemeralPrivateKey, UnparsedPublicKey, agree_ephemeral};
use ring::hkdf::{HKDF_SHA256, Salt};
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};

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

    STANDARD.encode(&out)
}

/// Decrypts ciphertext encrypted with AES-256-CBC by encrypt().
/// Reads first 16 bytes as IV, decrypts the rest.
pub fn decrypt(cipher_text: &str, key: &[u8; 32]) -> Result<String, String> {
    let full = STANDARD
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

/// Errors produced while delivering a push notification.
#[derive(Debug)]
pub enum PushError {
    /// The push service reports the subscription as gone (404/410) — remove it.
    Expired,
    /// The push service rejected the request with another status.
    Request(String),
    /// Crypto, key material, or endpoint URL is invalid.
    Crypto(String),
    /// Network/transport failure.
    Http(reqwest::Error),
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushError::Expired => write!(f, "subscription expired"),
            PushError::Request(s) => write!(f, "push service error: {}", s),
            PushError::Crypto(s) => write!(f, "push crypto error: {}", s),
            PushError::Http(e) => write!(f, "push transport error: {}", e),
        }
    }
}

impl std::error::Error for PushError {}

/// Decodes base64url, tolerating both padded and unpadded encodings.
fn b64url_decode(input: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(input.trim())
        .or_else(|_| {
            URL_SAFE_NO_PAD
                .decode(format!("{}===", input.trim()))
                .map_err(|_| ())
        })
        .or_else(|_| {
            base64::engine::general_purpose::URL_SAFE
                .decode(input.trim())
                .map_err(|_| ())
        })
        .map_err(|_| "invalid base64url value".to_string())
}

/// VAPID signing key material, parsed from the standard base64url format
/// produced by `web-push generate-vapid-keys`.
#[derive(Debug, Clone)]
pub struct VapidKeys {
    pub subject: String,
    /// 65-byte uncompressed P-256 public key.
    public_key: Vec<u8>,
    /// 32-byte P-256 private scalar.
    private_key: Vec<u8>,
}

impl VapidKeys {
    pub fn parse(
        subject: &str,
        public_key_b64: &str,
        private_key_b64: &str,
    ) -> Result<Self, String> {
        if subject.is_empty() || public_key_b64.is_empty() || private_key_b64.is_empty() {
            return Err("VAPID keys not configured".to_string());
        }
        let public_key = b64url_decode(public_key_b64)?;
        let private_key = b64url_decode(private_key_b64)?;
        if public_key.len() != 65 || public_key[0] != 0x04 {
            return Err(
                "VAPID_PUBLIC_KEY must be a 65-byte uncompressed P-256 point (base64url)".into(),
            );
        }
        if private_key.len() != 32 {
            return Err("VAPID_PRIVATE_KEY must be a 32-byte P-256 scalar (base64url)".into());
        }
        Ok(Self {
            subject: subject.to_string(),
            public_key,
            private_key,
        })
    }
}

/// Encrypts `plaintext` for a WebPush subscription (RFC 8291, aes128gcm content coding).
///
/// `p256dh` and `auth` are the base64url values from the push subscription.
/// Returns the full aes128gcm payload: header (salt || rs || idlen || keyid)
/// followed by ciphertext and tag.
pub fn encrypt_message(p256dh: &str, auth: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let ua_public = b64url_decode(p256dh)?;
    if ua_public.len() != 65 {
        return Err("p256dh must be a 65-byte uncompressed P-256 public key".to_string());
    }
    let auth_secret = b64url_decode(auth)?;
    if auth_secret.len() != 16 {
        return Err("auth must be a 16-byte secret".to_string());
    }

    let rng = SystemRandom::new();
    let as_private = EphemeralPrivateKey::generate(&ECDH_P256, &rng)
        .map_err(|e| format!("ephemeral key generation failed: {:?}", e))?;
    let as_public = as_private
        .compute_public_key()
        .map_err(|e| format!("ephemeral public key failed: {:?}", e))?;

    // ECDH shared secret (RFC 8291 step 1).
    let peer = UnparsedPublicKey::new(&ECDH_P256, &ua_public);
    let ikm = agree_ephemeral(as_private, &peer, |secret| secret.to_vec())
        .map_err(|e| format!("ECDH failed: {:?}", e))?;

    // PRK = HKDF-Extract(salt = auth_secret, ikm)
    let prk = Salt::new(HKDF_SHA256, &auth_secret).extract(&ikm);

    // key = HKDF-Expand(PRK, "WebPush: info\0" || ua_public || as_public, 16)
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(&ua_public);
    key_info.extend_from_slice(as_public.as_ref());
    let mut key_okm = [0u8; 32];
    prk.expand(&[&key_info], HKDF_SHA256)
        .map_err(|e| format!("HKDF expand (key) failed: {:?}", e))?
        .fill(&mut key_okm)
        .map_err(|e| format!("HKDF fill (key) failed: {:?}", e))?;

    // nonce = HKDF-Expand(PRK, "Content-Encoding: aes128gcm\0", 12)
    let nonce_info = b"Content-Encoding: aes128gcm\0";
    let mut nonce_okm = [0u8; 32];
    prk.expand(&[nonce_info], HKDF_SHA256)
        .map_err(|e| format!("HKDF expand (nonce) failed: {:?}", e))?
        .fill(&mut nonce_okm)
        .map_err(|e| format!("HKDF fill (nonce) failed: {:?}", e))?;

    // aes128gcm header: salt(16) || rs(4, BE) || idlen(1) || keyid = as_public(65).
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let rs = 4096u32;
    let mut header = Vec::with_capacity(16 + 4 + 1 + 65);
    header.extend_from_slice(&salt);
    header.extend_from_slice(&rs.to_be_bytes());
    header.push(65);
    header.extend_from_slice(as_public.as_ref());

    // First (only) record: AES-128-GCM with nonce = derived nonce || 0x00000000,
    // AAD = header.
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&nonce_okm[..12]);
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_128_GCM, &key_okm[..16])
            .map_err(|e| format!("AEAD key construction failed: {:?}", e))?,
    );

    let mut in_out = plaintext.to_vec();
    key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce_bytes), Aad::from(&header[..]), &mut in_out)
        .map_err(|e| format!("AES-GCM seal failed: {:?}", e))?;

    let mut payload = header;
    payload.extend_from_slice(&in_out);
    Ok(payload)
}

/// Builds a VAPID JWT (ES256) for the given endpoint origin, valid for 12 hours.
fn create_vapid_jwt(
    subject: &str,
    public_key: &[u8],
    private_key: &[u8],
    audience: &str,
) -> Result<String, String> {
    let rng = SystemRandom::new();
    let key_pair = EcdsaKeyPair::from_private_key_and_public_key(
        &ECDSA_P256_SHA256_FIXED_SIGNING,
        private_key,
        public_key,
        &rng,
    )
    .map_err(|e| format!("invalid VAPID key pair: {:?}", e))?;

    let header = URL_SAFE_NO_PAD.encode(b"{\"typ\":\"JWT\",\"alg\":\"ES256\"}");
    let exp = chrono::Utc::now().timestamp() + 12 * 3600;
    let payload_json = serde_json::json!({ "aud": audience, "exp": exp, "sub": subject });
    let payload = URL_SAFE_NO_PAD.encode(payload_json.to_string().as_bytes());
    let signing_input = format!("{}.{}", header, payload);

    let signature = key_pair
        .sign(&rng, signing_input.as_bytes())
        .map_err(|e| format!("VAPID signing failed: {:?}", e))?;

    Ok(format!(
        "{}.{}",
        signing_input,
        URL_SAFE_NO_PAD.encode(signature.as_ref())
    ))
}

/// Extracts the origin (scheme://host[:port]) from a push endpoint URL.
fn origin_of(endpoint: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(endpoint).map_err(|e| format!("invalid endpoint URL: {}", e))?;
    let host = url.host_str().ok_or_else(|| "endpoint URL has no host".to_string())?;
    let port = match url.port() {
        Some(p) => format!(":{}", p),
        None => String::new(),
    };
    Ok(format!("{}://{}{}", url.scheme(), host, port))
}

/// Delivers `payload` to a WebPush subscription, signing with VAPID.
///
/// Returns `Err(PushError::Expired)` when the push service reports the
/// subscription as gone (HTTP 404/410); callers should remove it.
pub async fn send_webpush(
    client: &reqwest::Client,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    payload: &[u8],
    vapid: &VapidKeys,
) -> Result<(), PushError> {
    let body = encrypt_message(p256dh, auth, payload).map_err(PushError::Crypto)?;
    let audience = origin_of(endpoint).map_err(PushError::Crypto)?;
    let jwt = create_vapid_jwt(&vapid.subject, &vapid.public_key, &vapid.private_key, &audience)
        .map_err(PushError::Crypto)?;

    let resp = client
        .post(endpoint)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Encoding", "aes128gcm")
        .header("TTL", "86400")
        .header("Urgency", "normal")
        .header(
            "Authorization",
            format!(
                "vapid t={}, k={}",
                jwt,
                URL_SAFE_NO_PAD.encode(&vapid.public_key)
            ),
        )
        .body(body)
        .send()
        .await
        .map_err(PushError::Http)?;

    match resp.status().as_u16() {
        200..=299 => Ok(()),
        404 | 410 => Err(PushError::Expired),
        code => Err(PushError::Request(format!(
            "push service returned status {}",
            code
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};

    #[test]
    fn aes256_round_trip() {
        let key: [u8; 32] = *b"0123456789abcdef0123456789abcdef";
        let secret = "subscription-auth-secret";
        let encrypted = encrypt(secret, &key);
        assert_ne!(encrypted, secret);
        assert_eq!(decrypt(&encrypted, &key).unwrap(), secret);
    }

    #[test]
    fn aes256_rejects_bad_input() {
        let key: [u8; 32] = *b"0123456789abcdef0123456789abcdef";
        assert!(decrypt("!!not-base64!!", &key).is_err());
        assert!(decrypt("c2hvcnQ=", &key).is_err()); // fewer than 16 IV bytes
    }

    /// Browser-side decryptor for aes128gcm, implemented independently from
    /// `encrypt_message` following RFC 8291, to cross-check the framing.
    fn browser_side_decrypt(payload: &[u8], ua_private: EphemeralPrivateKey, ua_public: &[u8], auth: &[u8]) -> Vec<u8> {
        assert!(payload.len() > 21 + 65);
        let rs = u32::from_be_bytes(payload[16..20].try_into().unwrap());
        assert_eq!(rs, 4096);
        let idlen = payload[20] as usize;
        let keyid = &payload[21..21 + idlen];
        let ciphertext = &payload[21 + idlen..];

        let peer = ring::agreement::UnparsedPublicKey::new(&ECDH_P256, keyid);
        let ikm = agree_ephemeral(ua_private, &peer, |s| s.to_vec()).unwrap();
        // RFC 8291 §3.3: the auth secret is the HKDF salt, not the header salt.
        let prk = Salt::new(HKDF_SHA256, auth).extract(&ikm);

        let mut key_info = b"WebPush: info\0".to_vec();
        key_info.extend_from_slice(ua_public);
        key_info.extend_from_slice(keyid);
        let mut key_okm = [0u8; 32];
        prk.expand(&[&key_info], HKDF_SHA256)
            .unwrap()
            .fill(&mut key_okm)
            .unwrap();

        let mut nonce_okm = [0u8; 32];
        prk.expand(&[b"Content-Encoding: aes128gcm\0"], HKDF_SHA256)
            .unwrap()
            .fill(&mut nonce_okm)
            .unwrap();
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_okm[..12]);

        let key = LessSafeKey::new(UnboundKey::new(&AES_128_GCM, &key_okm[..16]).unwrap());
        let mut in_out = ciphertext.to_vec();
        let header = &payload[..21 + idlen];
        let pt = key
            .open_in_place(Nonce::assume_unique_for_key(nonce), Aad::from(header), &mut in_out)
            .unwrap();
        pt.to_vec()
    }

    #[test]
    fn webpush_encryption_round_trip() {
        let rng = SystemRandom::new();
        let ua_private = EphemeralPrivateKey::generate(&ECDH_P256, &rng).unwrap();
        let ua_public = ua_private.compute_public_key().unwrap();
        let mut auth = [0u8; 16];
        OsRng.fill_bytes(&mut auth);

        let p256dh = URL_SAFE_NO_PAD.encode(ua_public.as_ref());
        let auth_b64 = URL_SAFE_NO_PAD.encode(auth);
        let plaintext = b"{\"title\":\"Chapter 5\",\"body\":\"New chapter\",\"badge\":2}";

        let payload = encrypt_message(&p256dh, &auth_b64, plaintext).unwrap();

        let opened = browser_side_decrypt(&payload, ua_private, ua_public.as_ref(), &auth);
        assert_eq!(opened, plaintext);

        // aes128gcm framing: salt(16) || rs(4) || idlen(1) || keyid(65) || ct(16+tag)
        assert_eq!(payload[20], 65);
        assert_eq!(&payload[16..20], &4096u32.to_be_bytes());
        assert_eq!(payload.len(), 21 + 65 + plaintext.len() + 16);
    }

    #[test]
    fn vapid_jwt_signs_and_verifies() {
        let subject = "mailto:admin@example.com";
        let keys = VapidKeys::parse(
            subject,
            "BCn7P1o4MNCFLqqgl7AJiVEf-TsGVFrsvk5BXNb46buxuwBE9kbkXj3iGcZjtw5Dp6hXYRDfnSByFYVPe_8jT_M",
            "0SKqYOAmO5iOI5BLRKcjrKgrD2XubnpSUYurfg65nWI",
        )
        .unwrap();

        let jwt = create_vapid_jwt(&keys.subject, &keys.public_key, &keys.private_key, "https://fcm.googleapis.com")
            .unwrap();

        let mut parts = jwt.split('.');
        let header_b64 = parts.next().unwrap();
        let payload_b64 = parts.next().unwrap();
        let signature_b64 = parts.next().unwrap();
        assert!(parts.next().is_none(), "JWT must have exactly 3 parts");

        // Verify the signature with the public key (raw R||S, JWS ES256).
        let signing_input = format!("{}.{}", header_b64, payload_b64);
        let signature = URL_SAFE_NO_PAD.decode(signature_b64).unwrap();
        let verifier = UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &keys.public_key);
        verifier
            .verify(signing_input.as_bytes(), &signature)
            .expect("VAPID JWT signature must verify");
        assert_eq!(signature.len(), 64);

        // Claims.
        let header: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header_b64).unwrap()).unwrap();
        assert_eq!(header["alg"], "ES256");
        assert_eq!(header["typ"], "JWT");
        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload_b64).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert_eq!(claims["sub"], subject);
        let exp = claims["exp"].as_i64().unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!(
            exp > now + 11 * 3600 && exp < now + 13 * 3600,
            "exp {} must be ~12h ahead of now {}",
            exp,
            now
        );
    }

    #[test]
    fn vapid_keys_parse_validation() {
        assert!(VapidKeys::parse("", "abc", "def").is_err());
        assert!(VapidKeys::parse("sub", "", "def").is_err());
        assert!(VapidKeys::parse("sub", "not-b64!!", "def").is_err());
        // Wrong key lengths.
        assert!(VapidKeys::parse("sub", "YWJj", "0SKqYOAmO5iOI5BLRKcjrKgrD2XubnpSUYurfg65nWI").is_err());
        // Valid round trip.
        assert!(
            VapidKeys::parse(
                "mailto:a@b.c",
                "BCn7P1o4MNCFLqqgl7AJiVEf-TsGVFrsvk5BXNb46buxuwBE9kbkXj3iGcZjtw5Dp6hXYRDfnSByFYVPe_8jT_M",
                "0SKqYOAmO5iOI5BLRKcjrKgrD2XubnpSUYurfg65nWI",
            )
            .is_ok()
        );
    }

    #[test]
    fn endpoint_origin_extraction() {
        assert_eq!(
            origin_of("https://fcm.googleapis.com/fcm/send/abc123").unwrap(),
            "https://fcm.googleapis.com"
        );
        assert_eq!(
            origin_of("https://push.example.com:8443/push").unwrap(),
            "https://push.example.com:8443"
        );
        assert!(origin_of("not a url").is_err());
    }
}
