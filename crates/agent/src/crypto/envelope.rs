//! HPKE base-mode seal/open + Ed25519 sender signatures + the AAD/sig-preimage
//! byte layout + inner-CBOR padding. This is the byte-exact contract the Swift
//! side must mirror (git-it-ios Sources/Support/Crypto/Envelope.swift); the
//! shared KAT (tests/kat_interop.rs ↔ Tests/KATInteropTests.swift over the same
//! tests/vectors/kat_v1.json) proves both halves agree on every byte.
//!
//! Suite: HPKE RFC 9180 base mode, DHKEM(X25519, HKDF-SHA256) + HKDF-SHA256 +
//! ChaCha20-Poly1305 — the exact match for CryptoKit's
//! `HPKE.Ciphersuite.Curve25519_SHA256_ChachaPoly`. Encrypt-then-sign on seal;
//! verify-sig-FIRST-fail-closed on open.

use anyhow::{anyhow, bail, Context, Result};
use ciborium::value::Value;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hpke::{
    aead::ChaCha20Poly1305, kdf::HkdfSha256, kem::X25519HkdfSha256, single_shot_open,
    single_shot_seal, Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable,
};
use rand_core::{OsRng, TryRngCore};

type Kem = X25519HkdfSha256;

/// rand_core 0.9's `OsRng` is *fallible* (`TryRngCore`), but hpke wants an
/// infallible `RngCore + CryptoRng`. `unwrap_err()` adapts it via `UnwrapErr`,
/// which panics only if the OS RNG itself fails — which doesn't happen on
/// macOS/iOS in practice. A fresh adapter per call is correct (no RNG state).
fn os_rng() -> rand_core::UnwrapErr<OsRng> {
    OsRng.unwrap_err()
}

/// HPKE `info` — fixed and byte-identical across Rust and Swift. Binds the suite
/// into the key schedule so a suite confusion produces a decrypt failure.
pub const INFO: &[u8] = b"gitit/e2e/v1 DHKEM(X25519,SHA256)/HKDF-SHA256/ChaCha20Poly1305";

/// Domain separator prepended to the Ed25519 signature pre-image (D-AAD-INFO).
const SIG_CTX: &[u8] = b"gitit-sig-v1\0";

/// Plaintext padding buckets (bytes). The plaintext is padded INSIDE the HPKE
/// seal so the AEAD tag never leaks the true message size — the relay sees only
/// which of 5 buckets. >64KB spills to Convex File Storage (handled by callers).
pub const BUCKETS: [usize; 5] = [256, 1024, 4096, 16384, 65536];

/// The smallest bucket that fits `len`, or `None` if it exceeds the largest
/// (caller must take the File-Storage path).
pub fn bucket(len: usize) -> Option<usize> {
    BUCKETS.iter().copied().find(|&b| len <= b)
}

/// The additional-authenticated-data: length-prefixed RAW bytes (never CBOR/JSON
/// ordering). Binds the envelope to its routing so a captured ciphertext can't be
/// replayed under a different (from,to,msgId,ts) without breaking the AEAD tag.
///
/// Layout: `ver(1) ‖ epoch(4 BE) ‖ u16BE(len from)‖from ‖ u16BE(len to)‖to ‖
/// u16BE(len msgId)‖msgId ‖ ts(8 BE) ‖ nonce(16)`.
pub fn build_aad(
    ver: u8,
    epoch: u32,
    from: &str,
    to: &str,
    msg_id: &str,
    ts: u64,
    nonce: &[u8; 16],
) -> Result<Vec<u8>> {
    let mut a = Vec::new();
    a.push(ver);
    a.extend_from_slice(&epoch.to_be_bytes());
    for (label, s) in [("from", from), ("to", to), ("msgId", msg_id)] {
        // u16 length prefix: these are device-id/message UUIDs in practice. Guard
        // the cast so an over-long field fails closed instead of silently
        // truncating (Rust) — Swift's UInt16(...) would trap — which would forge a
        // length/value mismatch in the aad.
        if s.len() > u16::MAX as usize {
            bail!("aad field {label} is {} bytes — exceeds the 65535-byte limit", s.len());
        }
        a.extend_from_slice(&(s.len() as u16).to_be_bytes());
        a.extend_from_slice(s.as_bytes());
    }
    a.extend_from_slice(&ts.to_be_bytes());
    a.extend_from_slice(nonce);
    Ok(a)
}

/// The Ed25519 signature pre-image: `SIG_CTX ‖ aad ‖ enc ‖ ct_or_blobhash`.
/// For an inline ciphertext pass the AEAD output as `ct_or_blobhash`; for a
/// File-Storage blob pass `sha256(blob)` instead (binds the off-row payload).
pub fn sig_preimage(aad: &[u8], enc: &[u8], ct_or_blobhash: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(SIG_CTX.len() + aad.len() + enc.len() + ct_or_blobhash.len());
    p.extend_from_slice(SIG_CTX);
    p.extend_from_slice(aad);
    p.extend_from_slice(enc);
    p.extend_from_slice(ct_or_blobhash);
    p
}

/// The sealed parts of one message. `enc` is the 32-byte HPKE encapsulated key,
/// `ct` the AEAD ciphertext‖tag, `sig` the 64-byte Ed25519 signature. These map
/// to outer-CBOR fields 6/7/8 (the outer envelope assembly lives with the wiring
/// slices; S1 proves the crypto core).
#[derive(Debug, Clone)]
pub struct Sealed {
    pub enc: Vec<u8>,
    pub ct: Vec<u8>,
    pub sig: Vec<u8>,
}

fn to32(bytes: &[u8]) -> Result<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes, got {}", bytes.len()))
}

/// Seal `inner_plaintext` to the recipient's X25519 public key (HPKE base mode),
/// then sign `sig_preimage(aad, enc, ct)` with the sender's Ed25519 key.
/// A fresh ephemeral key is drawn per call (== CryptoKit's per-message `Sender`).
pub fn seal(
    recipient_x25519_pub: &[u8],
    sender_ed25519_seed: &[u8; 32],
    aad: &[u8],
    inner_plaintext: &[u8],
) -> Result<Sealed> {
    let pk_recip = <Kem as KemTrait>::PublicKey::from_bytes(recipient_x25519_pub)
        .map_err(|e| anyhow!("bad recipient public key: {e}"))?;
    let (encapped, ct) = single_shot_seal::<ChaCha20Poly1305, HkdfSha256, Kem, _>(
        &OpModeS::Base,
        &pk_recip,
        INFO,
        inner_plaintext,
        aad,
        &mut os_rng(),
    )
    .map_err(|e| anyhow!("hpke seal: {e}"))?;
    let enc = encapped.to_bytes().to_vec();

    let sk = SigningKey::from_bytes(sender_ed25519_seed);
    let sig = sk.sign(&sig_preimage(aad, &enc, &ct)).to_bytes().to_vec();

    Ok(Sealed { enc, ct, sig })
}

/// Verify the sender's Ed25519 signature FIRST (fail closed — no decryption is
/// attempted on a bad signature), then HPKE-open. Returns the inner plaintext
/// (still padded — use [`decode_inner`] to recover `(kind, body)`).
pub fn open(
    recipient_x25519_priv: &[u8; 32],
    sender_ed25519_pub: &[u8; 32],
    aad: &[u8],
    sealed: &Sealed,
) -> Result<Vec<u8>> {
    // --- 1. Sender authentication BEFORE any AEAD work. ---
    let vk = VerifyingKey::from_bytes(sender_ed25519_pub)
        .map_err(|e| anyhow!("bad sender public key: {e}"))?;
    let sig_bytes: [u8; 64] = sealed
        .sig
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("signature must be 64 bytes, got {}", sealed.sig.len()))?;
    let sig = Signature::from_bytes(&sig_bytes);
    vk.verify_strict(&sig_preimage(aad, &sealed.enc, &sealed.ct), &sig)
        .map_err(|_| anyhow!("sender signature invalid — refusing to decrypt"))?;

    // --- 2. HPKE open. ---
    let sk_recip = <Kem as KemTrait>::PrivateKey::from_bytes(recipient_x25519_priv)
        .map_err(|e| anyhow!("bad recipient private key: {e}"))?;
    let encapped = <Kem as KemTrait>::EncappedKey::from_bytes(&sealed.enc)
        .map_err(|e| anyhow!("bad encapsulated key: {e}"))?;
    single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(
        &OpModeR::Base,
        &sk_recip,
        &encapped,
        INFO,
        &sealed.ct,
        aad,
    )
    .map_err(|e| anyhow!("hpke open: {e}"))
}

/// Generate a fresh X25519 keypair as raw 32-byte `(private, public)`.
pub fn gen_x25519() -> Result<([u8; 32], [u8; 32])> {
    let (sk, pk) = <Kem as KemTrait>::gen_keypair(&mut os_rng());
    Ok((to32(&sk.to_bytes())?, to32(&pk.to_bytes())?))
}

/// Derive the X25519 public key for a raw 32-byte private key.
pub fn x25519_public(priv_key: &[u8; 32]) -> Result<[u8; 32]> {
    let sk = <Kem as KemTrait>::PrivateKey::from_bytes(priv_key)
        .map_err(|e| anyhow!("bad x25519 private key: {e}"))?;
    let pk = <Kem as KemTrait>::sk_to_pk(&sk);
    to32(&pk.to_bytes())
}

/// Derive the Ed25519 public key for a raw 32-byte signing seed.
pub fn ed25519_public(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

/// Sign `msg` with a raw 32-byte Ed25519 seed (RFC 8032 deterministic, so the
/// signature is byte-exact and the KAT pins agreement with CryptoKit). Reused by
/// the signed device roster (S2) and the sender signature path.
pub fn ed25519_sign(seed: &[u8; 32], msg: &[u8]) -> [u8; 64] {
    SigningKey::from_bytes(seed).sign(msg).to_bytes()
}

/// Verify a raw 64-byte Ed25519 signature over `msg` against a raw 32-byte
/// public key. Returns false on any malformed key/signature (fail closed).
pub fn ed25519_verify(public: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
    let Ok(vk) = VerifyingKey::from_bytes(public) else {
        return false;
    };
    vk.verify_strict(msg, &Signature::from_bytes(sig)).is_ok()
}

// ---------------------------------------------------------------------------
// Inner plaintext CBOR: `{0: kind (text), 1: body (bytes), 2: pad (bytes)}`.
// The real application `kind` + body live INSIDE the ciphertext, so the relay
// never sees the semantic op. Padding is an explicit field (never trailing
// zeros) so the doc stays valid CBOR at every bucket size.
// ---------------------------------------------------------------------------

/// Push a minimal CBOR length header for major type `major` (0x60 text / 0x40
/// bytes) and `len`. Used for `kind`/`body`, whose exact bytes we don't pad.
fn push_len_header(buf: &mut Vec<u8>, major: u8, len: usize) {
    if len <= 23 {
        buf.push(major | len as u8);
    } else if len <= 0xff {
        buf.push(major | 24);
        buf.push(len as u8);
    } else if len <= 0xffff {
        buf.push(major | 25);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        buf.push(major | 26);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    }
}

/// The CBOR map `{0: kind, 1: body, 2: ...}` up to (but not including) the pad
/// value. The pad is appended by [`encode_inner`] with a forced 3-byte header.
fn inner_prefix(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(kind.len() + body.len() + 8);
    v.push(0xA3); // map, 3 pairs
    v.push(0x00); // key 0
    push_len_header(&mut v, 0x60, kind.len()); // text(kind)
    v.extend_from_slice(kind.as_bytes());
    v.push(0x01); // key 1
    push_len_header(&mut v, 0x40, body.len()); // bytes(body)
    v.extend_from_slice(body);
    v.push(0x02); // key 2 — pad value follows
    v
}

/// Encode `(kind, body)` to the padded inner-CBOR plaintext, sized EXACTLY to
/// the smallest bucket that fits. The pad byte string always uses the 0x59
/// (2-byte-length) header — a fixed 3-byte overhead — so for any plaintext that
/// fits the 64KB bucket the total length is a continuous function of pad_len and
/// lands precisely on the bucket. A plaintext exceeding 64KB has no bucket and is
/// REJECTED (fail closed): the caller must route it to Convex File Storage rather
/// than send an unbucketed, size-leaking plaintext.
pub fn encode_inner(kind: &str, body: &[u8]) -> Result<Vec<u8>> {
    let prefix = inner_prefix(kind, body);
    let base = prefix.len() + 3; // + forced pad header (0x59 hi lo) at pad_len 0
    let Some(target) = bucket(base) else {
        bail!("inner plaintext is {base} bytes — exceeds the 64KB inline max; route to File Storage");
    };
    let pad_len = target - base; // fits u16: every bucket ≤ 65536 and base ≥ 13
    let mut out = prefix;
    out.push(0x59);
    out.extend_from_slice(&(pad_len as u16).to_be_bytes());
    out.resize(out.len() + pad_len, 0u8);
    Ok(out)
}

/// Recover `(kind, body)` from an inner-CBOR plaintext, ignoring the pad.
pub fn decode_inner(plaintext: &[u8]) -> Result<(String, Vec<u8>)> {
    let v: Value = ciborium::from_reader(plaintext).context("inner CBOR decode")?;
    let Value::Map(entries) = v else {
        bail!("inner plaintext is not a CBOR map");
    };
    let mut kind = None;
    let mut body = None;
    for (k, val) in entries {
        if k == Value::Integer(0u8.into()) {
            if let Value::Text(s) = val {
                kind = Some(s);
            }
        } else if k == Value::Integer(1u8.into()) {
            if let Value::Bytes(b) = val {
                body = Some(b);
            }
        }
    }
    Ok((
        kind.ok_or_else(|| anyhow!("inner CBOR missing kind (key 0)"))?,
        body.ok_or_else(|| anyhow!("inner CBOR missing body (key 1)"))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_nonce() -> [u8; 16] {
        let mut n = [0u8; 16];
        for (i, b) in n.iter_mut().enumerate() {
            *b = i as u8;
        }
        n
    }

    #[test]
    fn aad_layout_is_deterministic_and_length_prefixed() {
        let nonce = fixed_nonce();
        let aad = build_aad(1, 7, "ab", "cde", "f", 0x0102, &nonce).unwrap();
        // ver(1) + epoch(4) + [2+2] + [2+3] + [2+1] + ts(8) + nonce(16)
        assert_eq!(aad.len(), 1 + 4 + (2 + 2) + (2 + 3) + (2 + 1) + 8 + 16);
        assert_eq!(aad[0], 1);
        assert_eq!(&aad[1..5], &7u32.to_be_bytes());
        // first length prefix = len("ab") = 2
        assert_eq!(&aad[5..7], &2u16.to_be_bytes());
        assert_eq!(&aad[7..9], b"ab");
        // recomputing yields identical bytes (no map ordering involved)
        assert_eq!(aad, build_aad(1, 7, "ab", "cde", "f", 0x0102, &nonce).unwrap());
    }

    #[test]
    fn buckets_are_smallest_fit() {
        assert_eq!(bucket(0), Some(256));
        assert_eq!(bucket(256), Some(256));
        assert_eq!(bucket(257), Some(1024));
        assert_eq!(bucket(1024), Some(1024));
        assert_eq!(bucket(1025), Some(4096));
        assert_eq!(bucket(65536), Some(65536));
        assert_eq!(bucket(65537), None);
    }

    #[test]
    fn inner_roundtrip_and_padding_hits_a_bucket() {
        for (kind, body) in [
            ("status", &b""[..]),
            ("setArmed", &b"{\"armed\":true}"[..]),
            ("addRepo", &vec![0xABu8; 900][..]),
        ] {
            let pt = encode_inner(kind, body).unwrap();
            assert!(
                BUCKETS.contains(&pt.len()),
                "padded len {} for kind {kind} is not a bucket",
                pt.len()
            );
            let (k, b) = decode_inner(&pt).unwrap();
            assert_eq!(k, kind);
            assert_eq!(b, body);
        }
    }

    #[test]
    fn seal_open_roundtrip_recovers_plaintext() {
        let (recip_priv, recip_pub) = gen_x25519().unwrap();
        let sender_seed = [9u8; 32];
        let sender_pub = ed25519_public(&sender_seed);
        let aad = build_aad(1, 1, "phone", "agent", "msg-1", 1234, &fixed_nonce()).unwrap();
        let pt = encode_inner("setArmed", b"{\"armed\":false}").unwrap();

        let sealed = seal(&recip_pub, &sender_seed, &aad, &pt).unwrap();
        assert_eq!(sealed.enc.len(), 32);
        assert_eq!(sealed.sig.len(), 64);

        let opened = open(&recip_priv, &sender_pub, &aad, &sealed).unwrap();
        let (kind, body) = decode_inner(&opened).unwrap();
        assert_eq!(kind, "setArmed");
        assert_eq!(body, b"{\"armed\":false}");
    }

    #[test]
    fn open_fails_closed_on_tampered_signature() {
        let (recip_priv, recip_pub) = gen_x25519().unwrap();
        let sender_seed = [3u8; 32];
        let sender_pub = ed25519_public(&sender_seed);
        let aad = build_aad(1, 1, "phone", "agent", "msg-2", 9, &fixed_nonce()).unwrap();
        let mut sealed = seal(&recip_pub, &sender_seed, &aad, b"hello").unwrap();
        sealed.sig[0] ^= 0xFF;
        assert!(open(&recip_priv, &sender_pub, &aad, &sealed).is_err());
    }

    #[test]
    fn open_fails_closed_on_wrong_aad() {
        let (recip_priv, recip_pub) = gen_x25519().unwrap();
        let sender_seed = [4u8; 32];
        let sender_pub = ed25519_public(&sender_seed);
        let aad = build_aad(1, 1, "phone", "agent", "msg-3", 9, &fixed_nonce()).unwrap();
        let pt = encode_inner("status", b"").unwrap();
        let sealed = seal(&recip_pub, &sender_seed, &aad, &pt).unwrap();
        // Different recipient in the aad ⇒ signature is over the original aad, so
        // verify-first already rejects (and the AEAD would too).
        let evil = build_aad(1, 1, "phone", "EVIL", "msg-3", 9, &fixed_nonce()).unwrap();
        assert!(open(&recip_priv, &sender_pub, &evil, &sealed).is_err());
    }

    #[test]
    fn x25519_public_matches_generated() {
        let (priv_k, pub_k) = gen_x25519().unwrap();
        assert_eq!(x25519_public(&priv_k).unwrap(), pub_k);
    }

    #[test]
    fn encode_inner_rejects_oversized_plaintext() {
        // A body past the 64KB bucket has no bucket — fail closed rather than emit
        // an unbucketed (size-leaking) plaintext.
        assert!(encode_inner("k", &vec![0u8; 70_000]).is_err());
    }

    #[test]
    fn build_aad_rejects_overlong_field() {
        let huge = "x".repeat(70_000);
        assert!(build_aad(1, 1, &huge, "to", "id", 0, &fixed_nonce()).is_err());
    }
}
