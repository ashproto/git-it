//! Phase 2b-S1 Known-Answer-Test interop harness (the hard CI gate).
//!
//! `tests/vectors/kat_v1.json` is the CANONICAL cross-language vector: fixed
//! keys + inputs + the deterministic expected outputs (Ed25519 signature, AAD
//! bytes, padded inner-plaintext, bucket lengths) + sealed round-trip fixtures.
//! The iOS side (`git-it-ios/Tests/KATInteropTests.swift`) reads a byte-identical
//! copy (enforced by a sha256 checksum gate) and asserts the same four parts.
//! Because both languages agree with the SAME canonical vector, they agree with
//! each other — that is the byte-interop proof.
//!
//! Regenerate the vector after an intentional crypto change:
//!   cargo test -p git-it-agent --test kat_interop -- --ignored regenerate
//! then copy it into the iOS test bundle (see the checksum gate).

use git_it_agent::crypto::envelope as env;
use serde_json::Value;
use std::path::PathBuf;

fn vector_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/kat_v1.json")
}

fn load() -> Value {
    let raw = std::fs::read_to_string(vector_path()).expect("kat_v1.json must exist (run --ignored regenerate)");
    serde_json::from_str(&raw).expect("kat_v1.json must be valid JSON")
}

fn hexd(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().expect("expected a hex string")).expect("valid hex")
}
fn arr32(v: &Value) -> [u8; 32] {
    hexd(v).try_into().expect("expected 32 bytes")
}
fn arr64(v: &Value) -> [u8; 64] {
    hexd(v).try_into().expect("expected 64 bytes")
}
fn arr16(v: &Value) -> [u8; 16] {
    hexd(v).try_into().expect("expected 16 bytes")
}

/// Rebuild `(ver,epoch,from,to,msgId,ts,nonce)` from a JSON aad-case object.
fn aad_from_case(c: &Value) -> Vec<u8> {
    env::build_aad(
        c["ver"].as_u64().unwrap() as u8,
        c["epoch"].as_u64().unwrap() as u32,
        c["from"].as_str().unwrap(),
        c["to"].as_str().unwrap(),
        c["msgId"].as_str().unwrap(),
        c["ts"].as_u64().unwrap(),
        &arr16(&c["nonce_hex"]),
    )
}

// ---------------------------------------------------------------------------
// Part 1 — Ed25519 sign/verify is byte-exact (deterministic RFC 8032).
// ---------------------------------------------------------------------------
#[test]
fn kat_part1_ed25519_signature_is_byte_exact() {
    let v = load();
    let e = &v["ed25519"];
    let seed = arr32(&e["seed_hex"]);
    let msg = hexd(&e["message_hex"]);

    assert_eq!(
        env::ed25519_public(&seed),
        arr32(&e["public_hex"]),
        "derived public key must match the vector"
    );
    let sig = env::ed25519_sign(&seed, &msg);
    assert_eq!(
        hex::encode(sig),
        e["signature_hex"].as_str().unwrap(),
        "signature must be byte-exact with the canonical vector"
    );
    assert!(env::ed25519_verify(&arr32(&e["public_hex"]), &msg, &arr64(&e["signature_hex"])));
    // A flipped bit must fail closed.
    let mut bad = sig;
    bad[0] ^= 0x01;
    assert!(!env::ed25519_verify(&arr32(&e["public_hex"]), &msg, &bad));
}

// ---------------------------------------------------------------------------
// Part 2 — AAD bytes + the padded inner-plaintext are deterministic & canonical.
// ---------------------------------------------------------------------------
#[test]
fn kat_part2_aad_and_inner_plaintext_match_vector() {
    let v = load();
    let aad = aad_from_case(&v["aad_case"]);
    assert_eq!(
        hex::encode(&aad),
        v["aad_case"]["expected_hex"].as_str().unwrap(),
        "build_aad must produce the canonical length-prefixed bytes"
    );

    let rt = &v["round_trip"];
    let inner = env::encode_inner(rt["kind"].as_str().unwrap(), &hexd(&rt["body_hex"]));
    assert_eq!(
        hex::encode(&inner),
        rt["plaintext_inner_hex"].as_str().unwrap(),
        "encode_inner must produce the canonical padded plaintext"
    );
    // The INFO constant is part of the suite binding — pin it too.
    assert_eq!(env::INFO, v["info_utf8"].as_str().unwrap().as_bytes());
}

// ---------------------------------------------------------------------------
// Part 3 — Cross-library round trips. Rust opens the Rust-sealed fixture always;
// when the Swift step has filled `cryptokit_sealed`, Rust opens THAT too (the
// CryptoKit-seal → Rust-open direction — the real interop proof).
// ---------------------------------------------------------------------------
#[test]
fn kat_part3_open_sealed_fixtures_recovers_plaintext() {
    let v = load();
    let rt = &v["round_trip"];
    let recip_priv = arr32(&v["x25519_recipient"]["priv_hex"]);
    let sender_pub = arr32(&v["sender"]["ed25519_public_hex"]);
    let aad = hexd(&rt["aad_hex"]);
    let want_kind = rt["kind"].as_str().unwrap();
    let want_body = hexd(&rt["body_hex"]);

    let open_and_check = |sealed_json: &Value, label: &str| {
        let sealed = env::Sealed {
            enc: hexd(&sealed_json["enc_hex"]),
            ct: hexd(&sealed_json["ct_hex"]),
            sig: hexd(&sealed_json["sig_hex"]),
        };
        let inner = env::open(&recip_priv, &sender_pub, &aad, &sealed)
            .unwrap_or_else(|e| panic!("{label}: open failed: {e}"));
        let (kind, body) = env::decode_inner(&inner).expect("decode inner");
        assert_eq!(kind, want_kind, "{label}: kind mismatch");
        assert_eq!(body, want_body, "{label}: body mismatch");
    };

    open_and_check(&rt["rust_sealed"], "rust_sealed");

    match &rt["cryptokit_sealed"] {
        Value::Null => eprintln!(
            "NOTE: cryptokit_sealed is null — the CryptoKit→Rust direction is \
             pending the Swift KAT step (fill it from KATInteropTests)."
        ),
        ck => open_and_check(ck, "cryptokit_sealed"),
    }
}

// ---------------------------------------------------------------------------
// Part 4 — Padding lands EXACTLY on a bucket at every boundary case.
// ---------------------------------------------------------------------------
#[test]
fn kat_part4_pad_boundaries_hit_exact_buckets() {
    let v = load();
    for case in v["pad_cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let body = hexd(&case["body_hex"]);
        let got = env::encode_inner(kind, &body).len();
        let want = case["expected_len"].as_u64().unwrap() as usize;
        assert_eq!(got, want, "pad case kind={kind} body_len={}", body.len());
        assert!(env::BUCKETS.contains(&got), "len {got} is not a bucket");
    }
}

// ---------------------------------------------------------------------------
// Generator (ignored) — writes the canonical vector. Run after an intentional
// crypto change, then sync the iOS copy + checksum.
// ---------------------------------------------------------------------------
#[test]
#[ignore = "regenerates the canonical KAT vector on demand"]
fn regenerate_kat_vectors() {
    // Fixed, obviously-synthetic inputs (distinct byte patterns).
    let ed_seed: [u8; 32] = std::array::from_fn(|i| i as u8); // 00..1f
    let ed_msg = b"git-it/e2e KAT message v1".to_vec();

    let recip_priv: [u8; 32] = std::array::from_fn(|i| 0x40 + i as u8); // 40..5f
    let recip_pub = env::x25519_public(&recip_priv).unwrap();
    let sender_seed: [u8; 32] = std::array::from_fn(|i| 0x20 + i as u8); // 20..3f
    let sender_pub = env::ed25519_public(&sender_seed);

    let nonce: [u8; 16] = std::array::from_fn(|i| 0x10 + i as u8); // 10..1f
    let (ver, epoch, from, to, msg_id, ts) =
        (1u8, 7u32, "phone-1111", "agent-2222", "msg-00000001", 1_718_000_000_000u64);
    let aad = env::build_aad(ver, epoch, from, to, msg_id, ts, &nonce);

    // Round-trip fixture: seal the known (kind, body) for Swift to open.
    let kind = "setArmed";
    let body = b"{\"armed\":true}".to_vec();
    let inner = env::encode_inner(kind, &body);
    let sealed = env::seal(&recip_pub, &sender_seed, &aad, &inner).expect("seal");

    // Pad boundary cases: body lengths chosen around each bucket edge.
    let pad_lens = [0usize, 200, 230, 240, 250, 1000, 4000, 16000];
    let pad_cases: Vec<Value> = pad_lens
        .iter()
        .map(|&n| {
            let b = vec![0xABu8; n];
            serde_json::json!({
                "kind": "k",
                "body_hex": hex::encode(&b),
                "expected_len": env::encode_inner("k", &b).len(),
            })
        })
        .collect();

    let vector = serde_json::json!({
        "version": 1,
        "info_utf8": String::from_utf8(env::INFO.to_vec()).unwrap(),
        "ed25519": {
            "seed_hex": hex::encode(ed_seed),
            "public_hex": hex::encode(env::ed25519_public(&ed_seed)),
            "message_hex": hex::encode(&ed_msg),
            "signature_hex": hex::encode(env::ed25519_sign(&ed_seed, &ed_msg)),
        },
        "aad_case": {
            "ver": ver, "epoch": epoch, "from": from, "to": to, "msgId": msg_id, "ts": ts,
            "nonce_hex": hex::encode(nonce),
            "expected_hex": hex::encode(&aad),
        },
        "buckets": env::BUCKETS,
        "pad_cases": pad_cases,
        "x25519_recipient": { "priv_hex": hex::encode(recip_priv), "pub_hex": hex::encode(recip_pub) },
        "sender": { "ed25519_seed_hex": hex::encode(sender_seed), "ed25519_public_hex": hex::encode(sender_pub) },
        "round_trip": {
            "aad_hex": hex::encode(&aad),
            "kind": kind,
            "body_hex": hex::encode(&body),
            "plaintext_inner_hex": hex::encode(&inner),
            "rust_sealed": {
                "enc_hex": hex::encode(&sealed.enc),
                "ct_hex": hex::encode(&sealed.ct),
                "sig_hex": hex::encode(&sealed.sig),
            },
            "cryptokit_sealed": Value::Null,
        }
    });

    let path = vector_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let pretty = serde_json::to_string_pretty(&vector).unwrap();
    std::fs::write(&path, pretty + "\n").unwrap();
    eprintln!("wrote {}", path.display());
}
