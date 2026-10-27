//! Optional passphrase lock (docs/09-passphrase-lock.md).
//!
//! The locked file stays JSON:
//! { "version": 1, "encrypted": true, "kdf": {...}, "cipher": {...}, "ciphertext": "<base64>" }
//! There is no recovery if the passphrase is forgotten.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

const MEMORY_KIB: u32 = 65_536;
const ITERATIONS: u32 = 3;
const PARALLELISM: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Kdf {
    name: String,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    salt: String,
}

#[derive(Serialize, Deserialize)]
struct Cipher {
    name: String,
    nonce: String,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    encrypted: bool,
    kdf: Kdf,
    cipher: Cipher,
    ciphertext: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    /// The text is not a locked file.
    NotLocked,
    /// Wrong passphrase, or the file was changed.
    WrongPassphraseOrDamaged,
    Other(String),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::NotLocked => write!(f, "not locked"),
            LockError::WrongPassphraseOrDamaged => write!(f, "wrong passphrase, or the file is damaged"),
            LockError::Other(s) => write!(f, "{}", s),
        }
    }
}

/// True if the text is a locked envelope.
pub fn is_locked(text: &str) -> bool {
    serde_json::from_str::<Envelope>(text).map(|e| e.encrypted).unwrap_or(false)
}

fn derive_key(passphrase: &str, salt: &[u8], memory_kib: u32, iterations: u32, parallelism: u32) -> Result<[u8; 32], LockError> {
    let params = Params::new(memory_kib, iterations, parallelism, Some(32)).map_err(|e| LockError::Other(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon.hash_password_into(passphrase.as_bytes(), salt, &mut key).map_err(|e| LockError::Other(e.to_string()))?;
    Ok(key)
}

/// Encrypt `plaintext` with a passphrase. A fresh salt and nonce every time.
pub fn lock(plaintext: &str, passphrase: &str) -> Result<String, LockError> {
    if passphrase.is_empty() {
        return Err(LockError::Other("the passphrase is empty".into()));
    }
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    getrandom::getrandom(&mut salt).map_err(|e| LockError::Other(e.to_string()))?;
    getrandom::getrandom(&mut nonce).map_err(|e| LockError::Other(e.to_string()))?;
    let key = derive_key(passphrase, &salt, MEMORY_KIB, ITERATIONS, PARALLELISM)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| LockError::Other("encryption failed".into()))?;
    let env = Envelope {
        version: 1,
        encrypted: true,
        kdf: Kdf { name: "argon2id".into(), memory_kib: MEMORY_KIB, iterations: ITERATIONS, parallelism: PARALLELISM, salt: B64.encode(salt) },
        cipher: Cipher { name: "xchacha20poly1305".into(), nonce: B64.encode(nonce) },
        ciphertext: B64.encode(ct),
    };
    serde_json::to_string_pretty(&env).map_err(|e| LockError::Other(e.to_string()))
}

/// Decrypt a locked envelope.
pub fn unlock(envelope: &str, passphrase: &str) -> Result<String, LockError> {
    let env: Envelope = serde_json::from_str(envelope).map_err(|_| LockError::NotLocked)?;
    if !env.encrypted || env.kdf.name != "argon2id" || env.cipher.name != "xchacha20poly1305" {
        return Err(LockError::NotLocked);
    }
    let salt = B64.decode(&env.kdf.salt).map_err(|_| LockError::WrongPassphraseOrDamaged)?;
    let nonce = B64.decode(&env.cipher.nonce).map_err(|_| LockError::WrongPassphraseOrDamaged)?;
    let ct = B64.decode(&env.ciphertext).map_err(|_| LockError::WrongPassphraseOrDamaged)?;
    if nonce.len() != 24 {
        return Err(LockError::WrongPassphraseOrDamaged);
    }
    let key = derive_key(passphrase, &salt, env.kdf.memory_kib, env.kdf.iterations, env.kdf.parallelism)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let pt = cipher.decrypt(XNonce::from_slice(&nonce), ct.as_ref()).map_err(|_| LockError::WrongPassphraseOrDamaged)?;
    String::from_utf8(pt).map_err(|_| LockError::WrongPassphraseOrDamaged)
}

// ---------- a key kept in memory, for files that are opened often (journal, photos) ----------
//
// Running Argon2 for every photo would be far too slow. Instead a random 32-byte "vault key" is made once
// per person; the passcode only wraps that key (`wrap_key` / `unwrap_key`, which use `lock` / `unlock`). The
// person's files are then sealed with the key itself (`seal` / `open`, XChaCha20-Poly1305). Changing the
// passcode re-wraps the key and touches no file. A forgotten passcode cannot be recovered.

const SEAL_MAGIC: &[u8; 4] = b"CXL1";

/// A fresh random 32-byte key.
pub fn new_key() -> Result<[u8; 32], LockError> {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).map_err(|e| LockError::Other(e.to_string()))?;
    Ok(key)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn from_hex(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The key, locked with a passcode (a normal locked envelope, so it can be kept in a text file).
pub fn wrap_key(key: &[u8; 32], passcode: &str) -> Result<String, LockError> {
    lock(&to_hex(key), passcode)
}

/// The key again, or `WrongPassphraseOrDamaged` for a wrong passcode.
pub fn unwrap_key(envelope: &str, passcode: &str) -> Result<[u8; 32], LockError> {
    let hex = unlock(envelope, passcode)?;
    from_hex(&hex).ok_or(LockError::WrongPassphraseOrDamaged)
}

/// True if these bytes are a sealed file.
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.len() >= 4 + 24 + 16 && &bytes[..4] == SEAL_MAGIC
}

/// Seal bytes with the key: magic, a fresh 24-byte nonce, then the ciphertext.
pub fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, LockError> {
    let mut nonce = [0u8; 24];
    getrandom::getrandom(&mut nonce).map_err(|e| LockError::Other(e.to_string()))?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ct = cipher.encrypt(XNonce::from_slice(&nonce), plaintext).map_err(|_| LockError::Other("encryption failed".into()))?;
    let mut out = Vec::with_capacity(4 + 24 + ct.len());
    out.extend_from_slice(SEAL_MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Open sealed bytes. A wrong key or a changed file gives `WrongPassphraseOrDamaged`.
pub fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>, LockError> {
    if !is_sealed(sealed) {
        return Err(LockError::NotLocked);
    }
    let nonce = &sealed[4..28];
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher.decrypt(XNonce::from_slice(nonce), &sealed[28..]).map_err(|_| LockError::WrongPassphraseOrDamaged)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "{\"people\":[{\"name\":\"Sample\"}]}";

    #[test]
    fn round_trip() {
        let locked = lock(SECRET, "correct horse").unwrap();
        assert!(is_locked(&locked));
        assert!(!locked.contains("Sample"), "plaintext must not appear in the locked file");
        assert_eq!(unlock(&locked, "correct horse").unwrap(), SECRET);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let locked = lock(SECRET, "right").unwrap();
        assert_eq!(unlock(&locked, "wrong"), Err(LockError::WrongPassphraseOrDamaged));
    }

    #[test]
    fn tampering_is_detected() {
        let locked = lock(SECRET, "pw").unwrap();
        let mut env: serde_json::Value = serde_json::from_str(&locked).unwrap();
        let ct = env["ciphertext"].as_str().unwrap().to_string();
        let mut bytes = B64.decode(ct).unwrap();
        bytes[0] ^= 1;
        env["ciphertext"] = serde_json::Value::String(B64.encode(bytes));
        assert_eq!(unlock(&env.to_string(), "pw"), Err(LockError::WrongPassphraseOrDamaged));
    }

    #[test]
    fn fresh_salt_and_nonce_each_time() {
        assert_ne!(lock(SECRET, "pw").unwrap(), lock(SECRET, "pw").unwrap());
    }

    #[test]
    fn plain_text_is_not_locked() {
        assert!(!is_locked(SECRET));
        assert_eq!(unlock(SECRET, "pw"), Err(LockError::NotLocked));
    }

    #[test]
    fn a_key_is_wrapped_by_a_passcode_and_files_are_sealed_by_the_key() {
        let key = new_key().unwrap();
        assert_ne!(key, new_key().unwrap());
        let wrapped = wrap_key(&key, "a long passcode").unwrap();
        assert!(is_locked(&wrapped));
        assert_eq!(unwrap_key(&wrapped, "a long passcode").unwrap(), key);
        assert_eq!(unwrap_key(&wrapped, "wrong"), Err(LockError::WrongPassphraseOrDamaged));
        let sealed = seal(&key, b"Dear diary, secret").unwrap();
        assert!(is_sealed(&sealed));
        assert!(!String::from_utf8_lossy(&sealed).contains("diary"));
        assert_eq!(open(&key, &sealed).unwrap(), b"Dear diary, secret");
        assert_ne!(seal(&key, b"x").unwrap(), seal(&key, b"x").unwrap());
        let other = new_key().unwrap();
        assert_eq!(open(&other, &sealed), Err(LockError::WrongPassphraseOrDamaged));
        let mut bad = sealed.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert_eq!(open(&key, &bad), Err(LockError::WrongPassphraseOrDamaged));
        assert_eq!(open(&key, b"plain text file"), Err(LockError::NotLocked));
        assert!(seal(&key, b"").is_ok() && open(&key, &seal(&key, b"").unwrap()).unwrap().is_empty());
    }

    #[test]
    fn empty_passphrase_refused() {
        assert!(lock(SECRET, "").is_err());
    }
}
