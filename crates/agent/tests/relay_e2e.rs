//! Phase 2b-S5 relay integration test: drives the SAME crypto stack the live
//! `relay.rs` uses (roster resolve → `process_inbound` → seal reply), acting as the
//! phone test-client. The phone builds a recovery-key-signed roster enrolling
//! itself + the agent; the agent verifies it, opens a sealed command, and seals a
//! reply the phone re-opens. Then the adversarial cases: a duplicate is replay-
//! rejected, and a sender the roster has REVOKED is rejected at the head. This is
//! the in-process proof of the receive/reply path; the live Convex round-trip is a
//! manual simulator+agent step (the transport is thin glue over these calls).

use base64::Engine;
use git_it_agent::crypto::enroll::{resolve_roster, ChainRow};
use git_it_agent::crypto::envelope;
use git_it_agent::crypto::keys::DeviceKeys;
use git_it_agent::crypto::message::{decode_wire, open_wire, process_inbound, random_nonce, seal_message};
use git_it_agent::crypto::replay::ReplayGuard;
use git_it_agent::crypto::roster::{Entry, Op, SignedEntry, GENESIS_PREV};

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn tmp(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("gitit-relay-e2e-{tag}-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// A recovery-key-signed `add`/`revoke` entry for `dev`/`kind` carrying `k`'s pubs.
fn entry(recovery_seed: &[u8; 32], op: Op, epoch: u32, dev: &str, kind: &str, k: &DeviceKeys, prev: [u8; 32]) -> SignedEntry {
    SignedEntry::create(
        recovery_seed,
        Entry {
            op,
            epoch,
            device_id: dev.to_string(),
            kind: kind.to_string(),
            x25519_pub: k.x25519_pub,
            ed25519_pub: k.ed25519_pub,
            added_at: epoch as u64,
            prev_hash: prev,
        },
    )
    .unwrap()
}

/// Encode a signed entry the way Convex `roster:chain` would carry it (base64 STD).
fn row(se: &SignedEntry) -> ChainRow {
    ChainRow { entry_b64: B64.encode(&se.canonical), sig_b64: B64.encode(se.sig) }
}

#[test]
fn full_encrypted_round_trip_then_replay_and_revoke_are_rejected() {
    // --- the phone provisions identities + the recovery (roster-signing) key ---
    let recovery_seed = [0x5au8; 32];
    let recovery_pub = envelope::ed25519_public(&recovery_seed);
    let phone = DeviceKeys::generate().unwrap();
    let agent = DeviceKeys::generate().unwrap();

    // --- the phone builds + signs the roster: add phone @1, add agent @2 ---
    let e_phone = entry(&recovery_seed, Op::Add, 1, "phone", "phone", &phone, GENESIS_PREV);
    let e_agent = entry(&recovery_seed, Op::Add, 2, "agent", "mac", &agent, e_phone.hash());
    let chain = vec![row(&e_phone), row(&e_agent)];

    // --- the agent verifies the roster (TOFU recovery pub) → live device set ---
    let roster = resolve_roster(&chain, &recovery_pub, 0).expect("roster verifies");
    assert_eq!(roster.head_epoch, 2);
    assert_eq!(roster.keys_for("agent").unwrap().x25519_pub, agent.x25519_pub);
    assert_eq!(roster.keys_for("phone").unwrap().x25519_pub, phone.x25519_pub);

    // --- phone → agent: seal a setArmed command to the agent's x25519 key ---
    let cmd_body = br#"{"armed":true}"#;
    let ts = 1_700_000_000_000u64;
    let wire = seal_message(
        &agent.x25519_pub, &phone.ed25519_seed, "phone", "agent", "m1",
        roster.head_epoch, ts, random_nonce(), "setArmed", cmd_body,
    )
    .unwrap();

    // --- the agent runs the live receive path (decode → liveness → verify → replay) ---
    let mut replay = ReplayGuard::load(tmp("ok"), ts);
    let (opened, sender_x25519) =
        process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &wire, "m1", ts).unwrap();
    assert_eq!(opened.kind, "setArmed");
    assert_eq!(opened.body, cmd_body);
    assert_eq!(opened.from, "phone");
    assert_eq!(sender_x25519, phone.x25519_pub, "process_inbound returns the sender x25519 for the reply");

    // --- agent → phone: seal the reply back to the returned sender key ---
    let reply_body = br#"{"armed":true}"#;
    let reply_wire = seal_message(
        &sender_x25519, &agent.ed25519_seed, "agent", "phone", "m1-r",
        roster.head_epoch, ts + 1, random_nonce(), "armedResult", reply_body,
    )
    .unwrap();
    // The phone opens it using the AGENT's ed25519 pub from its OWN verified roster.
    let agent_ed = roster.keys_for("agent").unwrap().ed25519_pub;
    let opened_reply = open_wire(&decode_wire(&reply_wire).unwrap(), &phone.x25519_priv, &agent_ed, "phone").unwrap();
    assert_eq!(opened_reply.kind, "armedResult");
    assert_eq!(opened_reply.body, reply_body);

    // --- replay: re-delivering the SAME command is rejected (persisted nonce set) ---
    assert!(
        process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &wire, "m1", ts + 2).is_err(),
        "a captured ciphertext can't be replayed"
    );

    // --- revoke: a roster that revokes the phone at the head rejects its messages ---
    let e_revoke = entry(&recovery_seed, Op::Revoke, 3, "phone", "phone", &phone, e_agent.hash());
    let revoked_chain = vec![row(&e_phone), row(&e_agent), row(&e_revoke)];
    let revoked_roster = resolve_roster(&revoked_chain, &recovery_pub, 0).expect("revoke chain verifies");
    assert!(revoked_roster.keys_for("phone").is_none(), "phone is no longer live at the head");
    let after_revoke = seal_message(
        &agent.x25519_pub, &phone.ed25519_seed, "phone", "agent", "m2",
        revoked_roster.head_epoch, ts + 10, random_nonce(), "listRepos", b"{}",
    )
    .unwrap();
    let mut replay2 = ReplayGuard::load(tmp("revoke"), ts + 10);
    assert!(
        process_inbound("agent", &agent.x25519_priv, &revoked_roster, &mut replay2, &after_revoke, "m2", ts + 10).is_err(),
        "a revoked sender is rejected at the current head"
    );
}

#[test]
fn an_unpublished_recovery_key_or_wrong_signer_fails_closed() {
    // Resolving against the WRONG recovery key fails (TOFU pin protects the agent).
    let recovery_seed = [0x11u8; 32];
    let recovery_pub = envelope::ed25519_public(&recovery_seed);
    let phone = DeviceKeys::generate().unwrap();
    let e_phone = entry(&recovery_seed, Op::Add, 1, "phone", "phone", &phone, GENESIS_PREV);
    let chain = vec![row(&e_phone)];

    assert!(resolve_roster(&chain, &recovery_pub, 0).is_ok(), "the correct signer verifies");
    let wrong = envelope::ed25519_public(&[0x22u8; 32]);
    assert!(resolve_roster(&chain, &wrong, 0).is_err(), "a different recovery signer is refused");
    // An empty chain (agent paired but phone hasn't enrolled it) is refused too.
    assert!(resolve_roster(&[], &recovery_pub, 0).is_err(), "an unpublished roster serves no commands");
}
