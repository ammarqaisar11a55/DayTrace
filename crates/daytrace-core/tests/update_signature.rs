//! Checks the updater's trust chain with the same signature scheme and library
//! the Tauri updater uses. The fixture key is a throwaway whose private half
//! was discarded, and it is unrelated to the real signing key.

use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use minisign_verify::{PublicKey, Signature};

const CONFIG: &str = include_str!("../../../src-tauri/tauri.conf.json");
const FIXTURE_KEY: &str = include_str!("../../../tests/fixtures/update-test.pub");
const FIXTURE_PAYLOAD: &[u8] = include_bytes!("../../../tests/fixtures/update-payload.txt");
const FIXTURE_SIGNATURE: &str = include_str!("../../../tests/fixtures/update-payload.txt.sig");

fn decode(base64_text: &str) -> String {
    String::from_utf8(STANDARD.decode(base64_text.trim()).expect("base64")).expect("utf-8")
}

fn public_key(base64_text: &str) -> PublicKey {
    PublicKey::decode(&decode(base64_text)).expect("a valid minisign public key")
}

fn signature(base64_text: &str) -> Signature {
    Signature::decode(&decode(base64_text)).expect("a valid minisign signature")
}

fn configured_key() -> String {
    let config: serde_json::Value = serde_json::from_str(CONFIG).expect("valid configuration");
    config["plugins"]["updater"]["pubkey"].as_str().expect("a configured public key").to_string()
}

#[test]
fn the_shipped_public_key_is_valid() {
    public_key(&configured_key());
}

#[test]
fn update_addresses_use_https() {
    let config: serde_json::Value = serde_json::from_str(CONFIG).expect("valid configuration");
    let endpoints = config["plugins"]["updater"]["endpoints"].as_array().expect("endpoints");
    assert!(!endpoints.is_empty());
    assert!(endpoints.iter().all(|url| url.as_str().is_some_and(|url| url.starts_with("https://"))));
}

#[test]
fn a_correctly_signed_update_verifies() {
    let key = public_key(FIXTURE_KEY);
    assert!(key.verify(FIXTURE_PAYLOAD, &signature(FIXTURE_SIGNATURE), true).is_ok());
}

#[test]
fn a_tampered_update_is_rejected() {
    let key = public_key(FIXTURE_KEY);
    let mut tampered = FIXTURE_PAYLOAD.to_vec();
    tampered[0] ^= 1;
    assert!(key.verify(&tampered, &signature(FIXTURE_SIGNATURE), true).is_err());
}

#[test]
fn an_update_signed_by_another_key_is_rejected() {
    let key = public_key(&configured_key());
    assert!(key.verify(FIXTURE_PAYLOAD, &signature(FIXTURE_SIGNATURE), true).is_err());
}

/// Verifies a real build output: set DAYTRACE_VERIFY_ARTIFACT to its path.
#[test]
fn a_built_artifact_verifies_against_the_shipped_key() {
    let Ok(path) = std::env::var("DAYTRACE_VERIFY_ARTIFACT") else {
        eprintln!("skipped: DAYTRACE_VERIFY_ARTIFACT is not set");
        return;
    };
    let artifact = std::fs::read(&path).expect("read artifact");
    let signature_text = std::fs::read_to_string(Path::new(&format!("{path}.sig"))).expect("read signature");
    let key = public_key(&configured_key());
    key.verify(&artifact, &signature(&signature_text), true).expect("the signature must verify");
}
