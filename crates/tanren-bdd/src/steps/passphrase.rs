//! Unit-equivalent BDD coverage for `CredentialSealPassphrase::parse`.
//!
//! These tests verify that the zxcvbn-backed strength estimator accepts
//! high-entropy passphrases and rejects patterned strings that a unique-byte
//! heuristic would miss. They live in `tanren-bdd` because this is the only
//! crate permitted to define `#[test]` items (per the workspace test-surface
//! policy enforced by `xtask check-rust-test-surface`).

use secrecy::SecretString;
use tanren_configuration_secrets::{ConfigSecretsError, CredentialSealPassphrase};

#[test]
fn positive_high_entropy_random_passphrase_accepted() {
    let result = CredentialSealPassphrase::parse(SecretString::from("KZ7j9#mLp2$xQ4wN".to_owned()));
    assert!(
        result.is_ok(),
        "high-entropy random passphrase should be accepted, got {result:?}"
    );
}

#[test]
fn positive_diceware_phrase_accepted() {
    let result = CredentialSealPassphrase::parse(SecretString::from(
        "correct horse battery staple".to_owned(),
    ));
    assert!(
        result.is_ok(),
        "diceware-style passphrase should be accepted, got {result:?}"
    );
}

#[test]
fn falsification_repeated_characters_rejected() {
    let result = CredentialSealPassphrase::parse(SecretString::from("aaaaaaaaaaaa".to_owned()));
    assert!(
        matches!(result, Err(ConfigSecretsError::PassphraseTooWeak)),
        "repeated 'a' string should be rejected as too weak, got {result:?}"
    );
}

#[test]
fn falsification_dictionary_word_repeat_rejected() {
    let result = CredentialSealPassphrase::parse(SecretString::from("passwordpassword".to_owned()));
    assert!(
        matches!(result, Err(ConfigSecretsError::PassphraseTooWeak)),
        "dictionary-word repeat should be rejected as too weak, got {result:?}"
    );
}

#[test]
fn falsification_leet_speak_rejected() {
    let result = CredentialSealPassphrase::parse(SecretString::from("p4ssw0rd".to_owned()));
    assert!(
        matches!(result, Err(ConfigSecretsError::PassphraseTooWeak)),
        "l33t-speak 'p4ssw0rd' should be rejected, got {result:?}"
    );
}

#[test]
fn falsification_keyboard_walk_rejected() {
    let result = CredentialSealPassphrase::parse(SecretString::from("qwertyuiop".to_owned()));
    assert!(
        matches!(result, Err(ConfigSecretsError::PassphraseTooWeak)),
        "keyboard walk should be rejected, got {result:?}"
    );
}

#[test]
fn falsification_empty_passphrase_rejected() {
    let result = CredentialSealPassphrase::parse(SecretString::from(String::new()));
    assert!(
        matches!(result, Err(ConfigSecretsError::PassphraseValidationFailed)),
        "empty passphrase should fail validation, got {result:?}"
    );
}
