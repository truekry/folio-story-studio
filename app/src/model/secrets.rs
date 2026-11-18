//! The API key as it is stored in `config.json`.
//!
//! Only the key is sealed; every other setting stays readable JSON. The seal is
//! bound to three things: the machine's id, the user and a random pepper file
//! next to `config.json` (`secrets.pepper`). A copied `config.json` therefore
//! opens on no other machine and in no other account.
//!
//! The machine id and the pepper are not secrets on their own: every program on
//! the machine can read both. This protects the file at rest and against being
//! carried away, not against malware running as the same user.

use super::*;
use chacha20poly1305::aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use std::fmt;

/// The pepper lives beside `config.json` and never inside the repository.
pub(crate) const PEPPER_FILE: &str = "secrets.pepper";

/// What the author reads when the seal cannot be opened here.
pub(crate) const UNREADABLE: &str = "API key could not be read on this PC; enter it again.";

/// The format marker. A later version may add another one without guessing.
const PREFIX: &str = "v1:";

/// The context the derived key is used for.
const INFO: &[u8] = b"folio/api-key/v1";

/// The nonce ChaCha20-Poly1305 expects.
const NONCE_LEN: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SealError {
    /// There is a sealed key, but no pepper to open it with.
    NoPepper,
    /// The stored text is not a seal this version understands.
    Format,
    /// The seal belongs to another machine, user or pepper.
    WrongMachine,
    /// The pepper exists but cannot be read, or cannot be written.
    Storage,
}

impl fmt::Display for SealError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SealError::NoPepper => "the pepper file is missing",
            SealError::Format => "the stored key is not in a known format",
            SealError::WrongMachine => "the key belongs to another machine, user or data folder",
            SealError::Storage => "the pepper file could not be read or written",
        };
        formatter.write_str(text)
    }
}

/// What the key is bound to. Kept as a value so a test can play another machine
/// without touching the real one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Material {
    pub machine: String,
    pub user: String,
}

/// The machine id of this computer. `None` when the platform cannot supply one;
/// the seal then rests on the pepper and the user alone.
pub(crate) fn machine_id() -> Option<String> {
    match machine_uid::get() {
        Ok(id) => {
            let id = id.trim().to_string();
            (!id.is_empty()).then_some(id)
        }
        Err(_) => None,
    }
}

fn user_id() -> String {
    for name in ["USERNAME", "USER", "LOGNAME"] {
        if let Ok(value) = std::env::var(name) {
            let value = value.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    String::new()
}

pub(crate) fn system_material() -> Material {
    Material {
        machine: machine_id().unwrap_or_default(),
        user: user_id(),
    }
}

/// Seal `key` for this data folder. Creates the pepper on first use.
pub(crate) fn seal(dir: &Path, key: &str) -> Result<String, SealError> {
    seal_with(dir, key, &system_material())
}

/// Open a sealed key from this data folder.
pub(crate) fn open(dir: &Path, sealed: &str) -> Result<String, SealError> {
    open_with(dir, sealed, &system_material())
}

pub(crate) fn seal_with(dir: &Path, key: &str, material: &Material) -> Result<String, SealError> {
    let pepper = pepper(dir, true)?;
    let derived = derive(material, &pepper)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&derived));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, key.as_bytes())
        .map_err(|_| SealError::Format)?;
    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(nonce.as_slice());
    blob.extend_from_slice(&ciphertext);
    Ok(format!("{PREFIX}{}", hex_encode(&blob)))
}

pub(crate) fn open_with(
    dir: &Path,
    sealed: &str,
    material: &Material,
) -> Result<String, SealError> {
    let body = sealed
        .trim()
        .strip_prefix(PREFIX)
        .ok_or(SealError::Format)?;
    let blob = hex_decode(body).ok_or(SealError::Format)?;
    if blob.len() < NONCE_LEN + 16 {
        return Err(SealError::Format);
    }
    let pepper = pepper(dir, false)?;
    let derived = derive(material, &pepper)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&derived));
    let nonce = Nonce::from_slice(&blob[..NONCE_LEN]);
    let plain = cipher
        .decrypt(nonce, &blob[NONCE_LEN..])
        .map_err(|_| SealError::WrongMachine)?;
    String::from_utf8(plain).map_err(|_| SealError::Format)
}

/// The key material: HKDF over the machine id and the user, salted with the
/// pepper. The pepper being public is fine; the binding is what protects it.
fn derive(material: &Material, pepper: &[u8]) -> Result<[u8; 32], SealError> {
    let mut input = String::with_capacity(material.machine.len() + material.user.len() + 1);
    input.push_str(&material.machine);
    input.push('\n');
    input.push_str(&material.user);
    let hkdf = Hkdf::<Sha256>::new(Some(pepper), input.as_bytes());
    let mut key = [0u8; 32];
    hkdf.expand(INFO, &mut key)
        .map_err(|_| SealError::Storage)?;
    Ok(key)
}

/// Read the pepper, creating it once when allowed. A missing pepper with a
/// sealed key is an error: silently minting a new one would lock the key out.
fn pepper(dir: &Path, create: bool) -> Result<Vec<u8>, SealError> {
    let path = dir.join(PEPPER_FILE);
    for _ in 0..2 {
        match fs::read(&path) {
            Ok(bytes) if bytes.len() >= 16 => return Ok(bytes),
            Ok(_) => return Err(SealError::Storage),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Err(SealError::NoPepper);
                }
                create_pepper(&path)?;
            }
            Err(_) => return Err(SealError::Storage),
        }
    }
    Err(SealError::Storage)
}

/// Write 32 random bytes with owner-only permissions where the platform has
/// them. A concurrent first start is not an error: the other file wins.
fn create_pepper(path: &Path) -> Result<(), SealError> {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(&bytes).map_err(|_| SealError::Storage)?;
            file.flush().map_err(|_| SealError::Storage)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(SealError::Storage),
    }
}

/// Fill the readable key from the sealed one. Returns the sentence for the
/// author when the seal cannot be opened on this machine.
pub(crate) fn unlock(dir: &Path, settings: &mut AiSettings) -> Option<String> {
    let stored = settings.api_key_sealed.trim().to_string();
    if stored.is_empty() || !settings.api_key.is_empty() {
        return None;
    }
    match open(dir, &stored) {
        Ok(key) => {
            settings.api_key = key;
            None
        }
        Err(_) => Some(UNREADABLE.to_string()),
    }
}

/// Prepare the settings for writing: never a plain key, and a seal that is only
/// replaced when the key itself changed.
pub(crate) fn lock(dir: &Path, settings: &mut AiSettings) -> Option<String> {
    let stored = settings.api_key_sealed.trim().to_string();
    if settings.api_key.trim().is_empty() {
        // No key in memory. If an old seal exists it is kept unless the author
        // cleared the field on this machine, so a missing pepper cannot destroy
        // a key that may still be recoverable.
        match open(dir, &stored) {
            Ok(_) if !stored.is_empty() => settings.api_key_sealed.clear(),
            _ => settings.api_key_sealed = stored,
        }
        return None;
    }
    if !stored.is_empty() {
        if let Ok(old) = open(dir, &stored) {
            if old == settings.api_key {
                settings.api_key_sealed = stored;
                return None;
            }
        }
    }
    match seal(dir, &settings.api_key) {
        Ok(sealed) => {
            settings.api_key_sealed = sealed;
            None
        }
        Err(error) => {
            settings.api_key_sealed.clear();
            Some(format!(
                "The API key could not be stored safely ({error}). It stays in memory for this session and must be entered again next time."
            ))
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push(((high << 4) | low) as u8);
    }
    Some(out)
}
