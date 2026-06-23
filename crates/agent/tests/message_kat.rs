//! Phase 2b-S4 message-level KAT: prove the OUTER envelope wire format round-trips
//! across Rust `message.rs` ↔ Swift `Message.swift` in BOTH directions. The crypto
//! primitives are already proven byte-exact by the S1 KAT; this pins the new risk
//! — that Swift's hand-rolled CBOR and Rust ciborium mutually parse the outer
//! envelope. Fixed keypairs (so either side can open the other's ciphertext);
//! the sealed wire itself is non-deterministic (random nonce + HPKE ephemeral),
//! so we assert OPEN-recovers-plaintext, never wire-byte-equality.
//!
//! `cargo test -p git-it-agent --test message_kat` verifies; the
//! `--ignored regenerate_message_vectors` test (re)writes the canonical vector.

use git_it_agent::crypto::{envelope, message};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn vec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/message_kat_v1.json")
}
fn load() -> Value {
    serde_json::from_str(&fs::read_to_string(vec_path()).expect("read message_kat_v1.json")).unwrap()
}
fn hexd(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}
fn arr32(v: &Value) -> [u8; 32] {
    hexd(v).as_slice().try_into().unwrap()
}

/// Fixed keypairs: any 32 bytes is a valid X25519 private / Ed25519 seed.
fn keys() -> (([u8; 32], [u8; 32], [u8; 32], [u8; 32]), ([u8; 32], [u8; 32], [u8; 32], [u8; 32])) {
    let agent_x_priv = [0x11u8; 32];
    let agent_ed_seed = [0x22u8; 32];
    let phone_x_priv = [0x33u8; 32];
    let phone_ed_seed = [0x44u8; 32];
    let agent = (
        agent_x_priv,
        envelope::x25519_public(&agent_x_priv).unwrap(),
        agent_ed_seed,
        envelope::ed25519_public(&agent_ed_seed),
    );
    let phone = (
        phone_x_priv,
        envelope::x25519_public(&phone_x_priv).unwrap(),
        phone_ed_seed,
        envelope::ed25519_public(&phone_ed_seed),
    );
    (agent, phone)
}

#[test]
#[ignore]
fn regenerate_message_vectors() {
    let (agent, phone) = keys();
    let body = br#"{"armed":true}"#.to_vec();
    // A phone→agent command sealed by Rust (Swift must open it).
    let wire_rust = message::seal_message(
        &agent.1, // agent x25519 pub (recipient)
        &phone.2, // phone ed25519 seed (sender)
        "phone",
        "agent",
        "kat-msg-1",
        1,
        1_700_000_000_000,
        message::random_nonce(),
        "setArmed",
        &body,
    )
    .unwrap();

    let v = serde_json::json!({
        "agent": {
            "x25519_priv_hex": hex::encode(agent.0),
            "x25519_pub_hex": hex::encode(agent.1),
            "ed25519_seed_hex": hex::encode(agent.2),
            "ed25519_pub_hex": hex::encode(agent.3),
        },
        "phone": {
            "x25519_priv_hex": hex::encode(phone.0),
            "x25519_pub_hex": hex::encode(phone.1),
            "ed25519_seed_hex": hex::encode(phone.2),
            "ed25519_pub_hex": hex::encode(phone.3),
        },
        "msg": { "from": "phone", "to": "agent", "msgId": "kat-msg-1", "epoch": 1, "ts": 1_700_000_000_000u64, "kind": "setArmed", "body_hex": hex::encode(&body) },
        "wire_rust_b64": wire_rust,
        // Filled by the Swift KAT (a Swift-sealed wire Rust must open).
        "wire_swift_b64": null,
    });
    fs::write(vec_path(), serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
    eprintln!("wrote {}", vec_path().display());
}

#[test]
fn rust_opens_its_own_wire() {
    let v = load();
    let (agent, phone) = keys();
    // Sanity: the fixed keys in the vector match this build.
    assert_eq!(arr32(&v["agent"]["x25519_pub_hex"]), agent.1);
    assert_eq!(arr32(&v["phone"]["ed25519_pub_hex"]), phone.3);

    let wire = message::decode_wire(v["wire_rust_b64"].as_str().unwrap()).unwrap();
    let opened = message::open_wire(&wire, &agent.0, &phone.3, "agent").unwrap();
    assert_eq!(opened.kind, v["msg"]["kind"].as_str().unwrap());
    assert_eq!(opened.body, hexd(&v["msg"]["body_hex"]));
    assert_eq!(opened.from, "phone");
}

#[test]
fn rust_opens_the_swift_sealed_wire() {
    let v = load();
    let Some(swift) = v["wire_swift_b64"].as_str() else {
        eprintln!("wire_swift_b64 not yet populated by the Swift KAT — skipping the Swift→Rust direction");
        return;
    };
    let (agent, phone) = keys();
    // Swift seals phone→agent with the SAME fixed keys; Rust must parse its CBOR + open.
    let wire = message::decode_wire(swift).unwrap();
    let opened = message::open_wire(&wire, &agent.0, &phone.3, "agent").unwrap();
    assert_eq!(opened.kind, "setArmed");
    assert_eq!(opened.body, hexd(&v["msg"]["body_hex"]));
}
