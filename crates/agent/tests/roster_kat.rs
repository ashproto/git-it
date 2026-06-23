//! Phase 2b-S2 roster Known-Answer-Test. `tests/vectors/roster_kat_v1.json` is the
//! canonical cross-language vector: a fixed recovery key, a set of entry cases
//! (canonical bytes + hash + recovery signature), and one valid chain with its
//! expected head epoch + live device set. The iOS side reads a sha256-identical
//! copy and asserts the same things, proving the canonical entry encoding and the
//! chain verifier agree byte-for-byte between Rust and Swift — a roster signed on
//! the phone must verify on the Mac agent and vice versa.
//!
//! Regenerate after an intentional roster-format change:
//!   cargo test -p git-it-agent --test roster_kat -- --ignored regenerate

use git_it_agent::crypto::envelope::ed25519_public;
use git_it_agent::crypto::roster::{verify_chain, Entry, Op, SignedEntry};
use serde_json::Value;
use std::path::PathBuf;

fn vector_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/roster_kat_v1.json")
}
fn load() -> Value {
    let raw = std::fs::read_to_string(vector_path()).expect("roster_kat_v1.json (run --ignored regenerate)");
    serde_json::from_str(&raw).expect("valid JSON")
}
fn hexd(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().expect("hex string")).expect("valid hex")
}
fn arr32(v: &Value) -> [u8; 32] {
    hexd(v).try_into().expect("32 bytes")
}
fn arr64(v: &Value) -> [u8; 64] {
    hexd(v).try_into().expect("64 bytes")
}

fn entry_from(c: &Value) -> Entry {
    Entry {
        op: match c["op"].as_str().unwrap() {
            "add" => Op::Add,
            "revoke" => Op::Revoke,
            x => panic!("bad op {x}"),
        },
        epoch: c["epoch"].as_u64().unwrap() as u32,
        device_id: c["deviceId"].as_str().unwrap().to_string(),
        kind: c["kind"].as_str().unwrap().to_string(),
        x25519_pub: arr32(&c["x25519_pub_hex"]),
        ed25519_pub: arr32(&c["ed25519_pub_hex"]),
        added_at: c["addedAt"].as_u64().unwrap(),
        prev_hash: arr32(&c["prevHash_hex"]),
    }
}

#[test]
fn roster_kat_entry_cases_are_byte_exact() {
    let v = load();
    let recovery_pub = arr32(&v["recovery"]["public_hex"]);
    for c in v["entry_cases"].as_array().unwrap() {
        let entry = entry_from(c);
        let canonical = entry.canonical().unwrap();
        assert_eq!(
            hex::encode(&canonical),
            c["canonical_hex"].as_str().unwrap(),
            "canonical bytes for {}",
            c["deviceId"]
        );
        // entryHash = sha256(canonical) — the chain link.
        let se = SignedEntry::parse(canonical.clone(), arr64(&c["sig_hex"])).unwrap();
        assert_eq!(hex::encode(se.hash()), c["entryHash_hex"].as_str().unwrap(), "entry hash");
        // The recovery signature in the vector verifies.
        assert!(
            git_it_agent::crypto::envelope::ed25519_verify(&recovery_pub, &canonical, &arr64(&c["sig_hex"])),
            "recovery signature must verify for {}",
            c["deviceId"]
        );
        // Round-trips through decode.
        assert_eq!(Entry::decode(&canonical).unwrap(), entry);
    }
}

#[test]
fn roster_kat_chain_verifies_to_expected_live_set() {
    let v = load();
    let recovery_pub = arr32(&v["recovery"]["public_hex"]);
    let ch = &v["chain"];
    let chain: Vec<SignedEntry> = ch["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| SignedEntry::parse(hexd(&e["canonical_hex"]), arr64(&e["sig_hex"])).unwrap())
        .collect();
    let floor = ch["epoch_floor"].as_u64().unwrap() as u32;
    let verified = verify_chain(&chain, &recovery_pub, floor).expect("canonical chain must verify");
    assert_eq!(verified.head_epoch, ch["expected_head_epoch"].as_u64().unwrap() as u32);

    let expected: Vec<&Value> = ch["expected_live"].as_array().unwrap().iter().collect();
    assert_eq!(verified.live.len(), expected.len(), "live device count");
    for dev in expected {
        let id = dev["deviceId"].as_str().unwrap();
        let keys = verified.keys_for(id).unwrap_or_else(|| panic!("{id} must be live"));
        assert_eq!(hex::encode(keys.x25519_pub), dev["x25519_pub_hex"].as_str().unwrap());
        assert_eq!(hex::encode(keys.ed25519_pub), dev["ed25519_pub_hex"].as_str().unwrap());
        assert_eq!(keys.kind, dev["kind"].as_str().unwrap());
    }
}

#[test]
#[ignore = "regenerates the canonical roster KAT vector on demand"]
fn regenerate_roster_vectors() {
    let recovery_seed: [u8; 32] = std::array::from_fn(|i| 0x55 ^ i as u8);
    let recovery_pub = ed25519_public(&recovery_seed);

    let mk = |op: Op, epoch: u32, device: &str, kind: &str, tag: u8, prev: [u8; 32]| -> Entry {
        Entry {
            op,
            epoch,
            device_id: device.to_string(),
            kind: kind.to_string(),
            x25519_pub: std::array::from_fn(|i| tag.wrapping_add(i as u8)),
            ed25519_pub: std::array::from_fn(|i| tag.wrapping_mul(2).wrapping_add(i as u8)),
            added_at: 1_718_000_000_000 + epoch as u64,
            prev_hash: prev,
        }
    };
    let sign = |e: Entry| SignedEntry::create(&recovery_seed, e).unwrap();
    let case_json = |se: &SignedEntry| {
        serde_json::json!({
            "op": match se.entry.op { Op::Add => "add", Op::Revoke => "revoke" },
            "epoch": se.entry.epoch,
            "deviceId": se.entry.device_id,
            "kind": se.entry.kind,
            "x25519_pub_hex": hex::encode(se.entry.x25519_pub),
            "ed25519_pub_hex": hex::encode(se.entry.ed25519_pub),
            "addedAt": se.entry.added_at,
            "prevHash_hex": hex::encode(se.entry.prev_hash),
            "canonical_hex": hex::encode(&se.canonical),
            "entryHash_hex": hex::encode(se.hash()),
            "sig_hex": hex::encode(se.sig),
        })
    };

    // Standalone entry cases: genesis add, a non-genesis add with a different kind
    // length, and a revoke — exercise op + the u16 string-length prefixes.
    let g = git_it_agent::crypto::roster::GENESIS_PREV;
    let c0 = sign(mk(Op::Add, 1, "phone-aaaa", "ios", 0x10, g));
    let c1 = sign(mk(Op::Revoke, 9, "mac-longer-device-id", "macos", 0x20, c0.hash()));
    // Edge-case entry cases that pin the u16 length-prefix branches cross-language:
    // empty deviceId + kind (0x0000), and a multi-byte-UTF-8 deviceId/kind whose
    // CBOR-free length prefix is the BYTE count (not the character count).
    let c2 = sign(mk(Op::Add, 3, "", "", 0x30, c1.hash()));
    let c3 = sign(mk(Op::Add, 7, "📱-dev", "iòs", 0x40, c2.hash()));
    let entry_cases = vec![case_json(&c0), case_json(&c1), case_json(&c2), case_json(&c3)];

    // A valid chain that exercises revoke + RE-ADD: add phone, add mac, revoke mac,
    // add ipad, re-add mac with NEW keys → live {phone, ipad, mac(new)}.
    let mut prev = g;
    let mut entries = Vec::new();
    let mut live_keys: std::collections::BTreeMap<&str, (u8, &str)> = Default::default();
    for (op, epoch, device, kind, tag) in [
        (Op::Add, 1u32, "phone", "ios", 0x11u8),
        (Op::Add, 2, "mac", "macos", 0x22),
        (Op::Revoke, 3, "mac", "macos", 0x22),
        (Op::Add, 4, "ipad", "ipados", 0x44),
        (Op::Add, 5, "mac", "macos", 0x55),
    ] {
        let se = sign(mk(op, epoch, device, kind, tag, prev));
        prev = se.hash();
        match op {
            Op::Add => {
                live_keys.insert(device, (tag, kind));
            }
            Op::Revoke => {
                live_keys.remove(device);
            }
        }
        entries.push(serde_json::json!({
            "canonical_hex": hex::encode(&se.canonical),
            "sig_hex": hex::encode(se.sig),
        }));
    }
    let expected_live: Vec<Value> = live_keys
        .iter()
        .map(|(device, (tag, kind))| {
            let x: [u8; 32] = std::array::from_fn(|i| tag.wrapping_add(i as u8));
            let e: [u8; 32] = std::array::from_fn(|i| tag.wrapping_mul(2).wrapping_add(i as u8));
            serde_json::json!({
                "deviceId": device,
                "kind": kind,
                "x25519_pub_hex": hex::encode(x),
                "ed25519_pub_hex": hex::encode(e),
            })
        })
        .collect();

    let vector = serde_json::json!({
        "version": 1,
        "roster_ctx_utf8": String::from_utf8(git_it_agent::crypto::roster::ROSTER_CTX.to_vec()).unwrap(),
        "recovery": { "seed_hex": hex::encode(recovery_seed), "public_hex": hex::encode(recovery_pub) },
        "entry_cases": entry_cases,
        "chain": {
            "epoch_floor": 0,
            "entries": entries,
            "expected_head_epoch": 5,
            "expected_live": expected_live,
        }
    });

    let path = vector_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_string_pretty(&vector).unwrap() + "\n").unwrap();
    eprintln!("wrote {}", path.display());
}
