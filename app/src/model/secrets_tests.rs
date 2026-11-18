//! Tests for sealing the API key: format, binding and the file's permissions.

use super::secrets::*;
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "folio-secrets-{}-{}-{}",
        std::process::id(),
        name,
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temporary data folder");
    dir
}

fn material() -> Material {
    Material {
        machine: "machine-A".to_string(),
        user: "author".to_string(),
    }
}

fn other_machine() -> Material {
    Material {
        machine: "machine-B".to_string(),
        user: "author".to_string(),
    }
}

fn other_user() -> Material {
    Material {
        machine: "machine-A".to_string(),
        user: "someone-else".to_string(),
    }
}

#[test]
fn a_sealed_key_opens_again() {
    let dir = temp_dir("roundtrip");
    let sealed = seal_with(&dir, "sk-round-trip", &material()).expect("seal the key");
    assert!(
        sealed.starts_with("v1:"),
        "the format is versioned: {sealed}"
    );
    assert!(
        !sealed.contains("sk-round-trip"),
        "the plain key must not survive in the seal"
    );
    assert_eq!(
        open_with(&dir, &sealed, &material()).expect("open the seal"),
        "sk-round-trip"
    );
}

#[test]
fn every_seal_uses_a_fresh_nonce() {
    let dir = temp_dir("nonce");
    let first = seal_with(&dir, "sk-same", &material()).expect("first seal");
    let second = seal_with(&dir, "sk-same", &material()).expect("second seal");
    assert_ne!(
        first, second,
        "the same key must not produce the same blob twice"
    );
    assert_eq!(open_with(&dir, &first, &material()).unwrap(), "sk-same");
    assert_eq!(open_with(&dir, &second, &material()).unwrap(), "sk-same");
}

#[test]
fn another_machine_cannot_open_it() {
    let dir = temp_dir("foreign-machine");
    let sealed = seal_with(&dir, "sk-bound", &material()).expect("seal");
    assert_eq!(
        open_with(&dir, &sealed, &other_machine()),
        Err(SealError::WrongMachine)
    );
}

#[test]
fn another_user_cannot_open_it() {
    let dir = temp_dir("foreign-user");
    let sealed = seal_with(&dir, "sk-bound", &material()).expect("seal");
    assert_eq!(
        open_with(&dir, &sealed, &other_user()),
        Err(SealError::WrongMachine)
    );
}

#[test]
fn a_foreign_pepper_cannot_open_it() {
    let source = temp_dir("pepper-source");
    let target = temp_dir("pepper-target");
    let sealed = seal_with(&source, "sk-bound", &material()).expect("seal");
    // The second folder gets a pepper of its own, as a fresh install would.
    seal_with(&target, "sk-other", &material()).expect("second pepper");
    assert_eq!(
        open_with(&target, &sealed, &material()),
        Err(SealError::WrongMachine)
    );
}

#[test]
fn a_missing_pepper_is_named_and_not_invented() {
    let source = temp_dir("no-pepper-source");
    let target = temp_dir("no-pepper-target");
    let sealed = seal_with(&source, "sk-bound", &material()).expect("seal");
    assert!(!target.join(PEPPER_FILE).exists());
    assert_eq!(
        open_with(&target, &sealed, &material()),
        Err(SealError::NoPepper)
    );
    assert!(
        !target.join(PEPPER_FILE).exists(),
        "opening must never mint a new pepper: that would lock the key out"
    );
}

#[test]
fn text_that_is_not_a_seal_is_rejected() {
    let dir = temp_dir("garbage");
    for text in [
        "",
        "sk-plain",
        "v1:",
        "v1:zz",
        "v1:00",
        "v2:000000000000000000000000",
    ] {
        assert_eq!(
            open_with(&dir, text, &material()),
            Err(SealError::Format),
            "accepted {text:?}"
        );
    }
}

#[test]
fn the_real_machine_seals_and_opens() {
    // Uses the platform machine id, so this is the check that the binding is
    // available at all on the machine running the tests.
    let dir = temp_dir("system");
    let sealed = seal(&dir, "sk-system").expect("seal with this machine");
    assert_eq!(open(&dir, &sealed).expect("open"), "sk-system");
}

#[test]
fn unlocking_fills_the_readable_key() {
    let dir = temp_dir("unlock");
    let sealed = seal_with(&dir, "sk-real", &material()).expect("seal");
    let mut settings = AiSettings {
        api_key_sealed: sealed.clone(),
        ..Default::default()
    };
    // The seal was made with a test machine, so this machine must refuse it.
    assert_eq!(unlock(&dir, &mut settings), Some(UNREADABLE.to_string()));
    assert!(settings.api_key.is_empty());
    assert_eq!(
        settings.api_key_sealed, sealed,
        "a seal that could not be read is kept for a machine that can"
    );
}

#[test]
fn unlocking_a_real_seal_returns_no_notice() {
    let dir = temp_dir("unlock-real");
    let sealed = seal(&dir, "sk-real").expect("seal");
    let mut settings = AiSettings {
        api_key_sealed: sealed,
        ..Default::default()
    };
    assert_eq!(unlock(&dir, &mut settings), None);
    assert_eq!(settings.api_key, "sk-real");
}

#[test]
fn locking_reuses_a_seal_that_still_matches() {
    let dir = temp_dir("lock-reuse");
    let mut settings = AiSettings {
        api_key: "sk-stable".to_string(),
        ..Default::default()
    };
    assert_eq!(lock(&dir, &mut settings), None);
    let first = settings.api_key_sealed.clone();
    assert!(first.starts_with("v1:"));

    assert_eq!(lock(&dir, &mut settings), None);
    assert_eq!(
        settings.api_key_sealed, first,
        "an unchanged key must not rewrite config.json on every save"
    );
}

#[test]
fn a_new_key_gets_a_new_seal() {
    let dir = temp_dir("lock-new");
    let mut settings = AiSettings {
        api_key: "sk-old".to_string(),
        ..Default::default()
    };
    lock(&dir, &mut settings);
    let first = settings.api_key_sealed.clone();

    settings.api_key = "sk-new".to_string();
    assert_eq!(lock(&dir, &mut settings), None);
    assert_ne!(settings.api_key_sealed, first);
    assert_eq!(
        open(&dir, &settings.api_key_sealed).expect("open the new seal"),
        "sk-new"
    );
}

#[test]
fn clearing_the_key_clears_the_seal() {
    let dir = temp_dir("lock-clear");
    let mut settings = AiSettings {
        api_key: "sk-gone".to_string(),
        ..Default::default()
    };
    lock(&dir, &mut settings);
    assert!(!settings.api_key_sealed.is_empty());

    settings.api_key.clear();
    assert_eq!(lock(&dir, &mut settings), None);
    assert!(
        settings.api_key_sealed.is_empty(),
        "the author cleared the field, so the seal goes too"
    );
}

#[test]
fn an_unreadable_seal_survives_a_save() {
    let source = temp_dir("lock-foreign-source");
    let target = temp_dir("lock-foreign-target");
    let sealed = seal_with(&source, "sk-hidden", &material()).expect("seal");
    let mut settings = AiSettings {
        api_key_sealed: sealed.clone(),
        ..Default::default()
    };
    assert_eq!(lock(&target, &mut settings), None);
    assert_eq!(
        settings.api_key_sealed, sealed,
        "a key this machine cannot read must not be thrown away"
    );
}

#[test]
fn the_serialized_settings_never_hold_the_plain_key() {
    let dir = temp_dir("serialize");
    let mut settings = AiSettings {
        api_key: "sk-secret-value".to_string(),
        ..Default::default()
    };
    lock(&dir, &mut settings);
    let text = serde_json::to_string(&settings).expect("serialize the settings");
    assert!(!text.contains("sk-secret-value"), "{text}");
    assert!(!text.contains("api_key\""), "no plain key field: {text}");
    assert!(text.contains("api_key_sealed"), "{text}");
}

#[cfg(unix)]
#[test]
fn the_pepper_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("pepper-permissions");
    seal_with(&dir, "sk", &material()).expect("seal");
    let mode = fs::metadata(dir.join(PEPPER_FILE))
        .expect("pepper file")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the pepper is for this user only");
}
