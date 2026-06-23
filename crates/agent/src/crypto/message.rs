//! Phase 2b-S3 message layer: the OUTER envelope wire format (what lives in the
//! relay's `messages.body`) + the high-level seal/open that combine the AAD, inner
//! CBOR, HPKE, and Ed25519 signature into one call, plus `process_inbound` — the
//! authenticated receive path (decode → roster-resolve → verify-sig-first → open →
//! anti-replay) the relay runs for every message. The crypto primitives live in
//! `envelope`; the trust roster in `roster`.
//!
//! The outer CBOR (integer keys, per D-ENVELOPE) need only be MUTUALLY PARSEABLE
//! across Rust/Swift — it is not byte-critical, because every routing field is
//! re-bound into the AAD and covered by the signature; the byte-critical parts are
//! the AAD + enc/ct/sig, all proven by the S1 KAT.

use crate::crypto::envelope::{self, Sealed};
use crate::crypto::replay::ReplayGuard;
use crate::crypto::roster::VerifiedRoster;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use ciborium::value::Value;

const B64URL: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Wire-format version (the outer envelope `ver`, distinct from the AAD version).
pub const VER: u8 = 1;

/// The decoded outer envelope. `enc`/`ct`/`sig` are raw bytes; the routing fields
/// are all bound into the AAD and covered by `sig`.
#[derive(Clone)]
pub struct Wire {
    pub ver: u8,
    pub epoch: u32,
    pub from: String,
    pub to: String,
    pub ts_ms: u64,
    pub nonce: [u8; 16],
    pub enc: Vec<u8>,
    pub ct: Vec<u8>,
    pub sig: Vec<u8>,
    pub msg_id: String,
}

/// A successfully opened message: routing fields + the recovered `(kind, body)`.
pub struct Opened {
    pub from: String,
    pub epoch: u32,
    pub ts_ms: u64,
    pub nonce_b64: String,
    pub kind: String,
    pub body: Vec<u8>,
}

fn ik(n: u8) -> Value {
    Value::Integer(n.into())
}
fn field(m: &[(Value, Value)], k: u8) -> Option<&Value> {
    m.iter().find(|(key, _)| *key == ik(k)).map(|(_, v)| v)
}
fn as_text(v: &Value) -> Result<String> {
    v.as_text().map(String::from).ok_or_else(|| anyhow!("expected text"))
}
fn as_bytes(v: &Value) -> Result<Vec<u8>> {
    v.as_bytes().cloned().ok_or_else(|| anyhow!("expected bytes"))
}
fn as_u64(v: &Value) -> Result<u64> {
    match v {
        // try_from fails closed on a CBOR major-type-1 (negative) integer or one
        // beyond u64::MAX — every numeric envelope field is an unsigned quantity.
        Value::Integer(i) => u64::try_from(*i).map_err(|_| anyhow!("integer out of u64 range")),
        _ => bail!("expected integer"),
    }
}

/// Serialize the outer envelope to the `base64url(no-pad)` CBOR string for
/// `messages.body`. `blobId` (File-Storage spillover) is omitted in S3.
pub fn encode_wire(w: &Wire) -> String {
    let map = Value::Map(vec![
        (ik(0), Value::Integer(w.ver.into())),
        (ik(1), Value::Integer(w.epoch.into())),
        (ik(2), Value::Text(w.from.clone())),
        (ik(3), Value::Text(w.to.clone())),
        (ik(4), Value::Integer(w.ts_ms.into())),
        (ik(5), Value::Bytes(w.nonce.to_vec())),
        (ik(6), Value::Bytes(w.enc.clone())),
        (ik(7), Value::Bytes(w.ct.clone())),
        (ik(8), Value::Bytes(w.sig.clone())),
        (ik(10), Value::Text(w.msg_id.clone())),
    ]);
    let mut buf = Vec::new();
    ciborium::into_writer(&map, &mut buf).expect("cbor encode of an in-memory map cannot fail");
    B64URL.encode(buf)
}

/// Parse the outer envelope string. Fails closed on bad base64/CBOR, a non-map,
/// any missing field, or a nonce that is not 16 bytes.
pub fn decode_wire(s: &str) -> Result<Wire> {
    let buf = B64URL.decode(s.trim()).map_err(|e| anyhow!("envelope not base64url: {e}"))?;
    let v: Value = ciborium::from_reader(buf.as_slice()).map_err(|e| anyhow!("envelope not CBOR: {e}"))?;
    let m = match v {
        Value::Map(m) => m,
        _ => bail!("envelope is not a CBOR map"),
    };
    let req = |k: u8| field(&m, k).ok_or_else(move || anyhow!("envelope missing field {k}"));
    let nonce: [u8; 16] = as_bytes(req(5)?)?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("nonce is not 16 bytes"))?;
    Ok(Wire {
        ver: as_u64(req(0)?)? as u8,
        epoch: u32::try_from(as_u64(req(1)?)?).map_err(|_| anyhow!("epoch out of range"))?,
        from: as_text(req(2)?)?,
        to: as_text(req(3)?)?,
        ts_ms: as_u64(req(4)?)?,
        nonce,
        enc: as_bytes(req(6)?)?,
        ct: as_bytes(req(7)?)?,
        sig: as_bytes(req(8)?)?,
        msg_id: as_text(req(10)?)?,
    })
}

/// Seal an application `(kind, body)` to a recipient: build the AAD, pad + encrypt
/// the inner CBOR, sign, and assemble the wire string. The caller supplies a fresh
/// random `nonce` (see [`random_nonce`]) and the current `epoch` + `ts_ms`.
#[allow(clippy::too_many_arguments)]
pub fn seal_message(
    recipient_x25519_pub: &[u8; 32],
    sender_ed25519_seed: &[u8; 32],
    from: &str,
    to: &str,
    msg_id: &str,
    epoch: u32,
    ts_ms: u64,
    nonce: [u8; 16],
    kind: &str,
    body: &[u8],
) -> Result<String> {
    let aad = envelope::build_aad(VER, epoch, from, to, msg_id, ts_ms, &nonce)?;
    let inner = envelope::encode_inner(kind, body)?;
    let sealed = envelope::seal(recipient_x25519_pub, sender_ed25519_seed, &aad, &inner)?;
    Ok(encode_wire(&Wire {
        ver: VER,
        epoch,
        from: from.to_string(),
        to: to.to_string(),
        ts_ms,
        nonce,
        enc: sealed.enc,
        ct: sealed.ct,
        sig: sealed.sig,
        msg_id: msg_id.to_string(),
    }))
}

/// Open an already-decoded wire addressed to `expect_to`, using OUR x25519 private
/// key and the SENDER's Ed25519 public key. Rebuilds the AAD from the wire fields
/// and delegates to `envelope::open` (verify-sig-FIRST, then AEAD).
pub fn open_wire(
    wire: &Wire,
    recipient_x25519_priv: &[u8; 32],
    sender_ed25519_pub: &[u8; 32],
    expect_to: &str,
) -> Result<Opened> {
    if wire.ver != VER {
        bail!("unsupported envelope version {}", wire.ver);
    }
    if wire.to != expect_to {
        bail!("envelope is not addressed to this device");
    }
    if wire.enc.len() != 32 {
        bail!("encapsulated key is not 32 bytes");
    }
    let aad = envelope::build_aad(wire.ver, wire.epoch, &wire.from, &wire.to, &wire.msg_id, wire.ts_ms, &wire.nonce)?;
    let sealed = Sealed { enc: wire.enc.clone(), ct: wire.ct.clone(), sig: wire.sig.clone() };
    let inner = envelope::open(recipient_x25519_priv, sender_ed25519_pub, &aad, &sealed)?;
    let (kind, body) = envelope::decode_inner(&inner)?;
    Ok(Opened {
        from: wire.from.clone(),
        epoch: wire.epoch,
        ts_ms: wire.ts_ms,
        nonce_b64: B64URL.encode(wire.nonce),
        kind,
        body,
    })
}

/// The full authenticated receive path. Resolves the sender's CURRENT keys from
/// the verified roster (rejecting unknown / revoked senders), verifies the
/// signature BEFORE decrypting, then runs anti-replay only AFTER authentication
/// (so a forged message can never pollute the replay set). Returns the opened
/// message + the sender's x25519 public key so the caller can seal the reply.
pub fn process_inbound(
    me: &str,
    my_x25519_priv: &[u8; 32],
    roster: &VerifiedRoster,
    replay: &mut ReplayGuard,
    body: &str,
    relay_msg_id: &str,
    now_ms: u64,
) -> Result<(Opened, [u8; 32])> {
    let wire = decode_wire(body)?;
    // The relay's `msgId` column is an UNAUTHENTICATED hint; it MUST equal the
    // in-envelope (AAD-bound) msgId or the message is rejected.
    if wire.msg_id != relay_msg_id {
        bail!("msgId mismatch between the relay row and the envelope");
    }
    // Liveness: the sender must be a current (non-revoked) device in the roster.
    let sender = roster
        .keys_for(&wire.from)
        .ok_or_else(|| anyhow!("sender {} is not a live device in the roster", wire.from))?;
    let sender_x25519 = sender.x25519_pub;
    // Verify-sig-first + decrypt.
    let opened = open_wire(&wire, my_x25519_priv, &sender.ed25519_pub, me)?;
    // Anti-replay AFTER authentication.
    replay.check_and_record(&opened.from, &opened.nonce_b64, opened.ts_ms, now_ms)?;
    Ok((opened, sender_x25519))
}

/// Current wall-clock time in milliseconds (for `ts_ms` + the replay window).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A fresh random 16-byte message nonce from the OS CSPRNG.
pub fn random_nonce() -> [u8; 16] {
    let r = envelope::random_32();
    let mut n = [0u8; 16];
    n.copy_from_slice(&r[..16]);
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::DeviceKeys;
    use crate::crypto::roster::{DeviceKeys as RosterKeys, VerifiedRoster};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("gitit-msg-{tag}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    // A roster with one live device ("phone") holding the given keys.
    fn roster_with(device: &str, k: &DeviceKeys) -> VerifiedRoster {
        let mut live = BTreeMap::new();
        live.insert(
            device.to_string(),
            RosterKeys { kind: "phone".into(), x25519_pub: k.x25519_pub, ed25519_pub: k.ed25519_pub },
        );
        VerifiedRoster { head_epoch: 1, live }
    }

    // Seal a phone→agent command.
    fn seal(phone: &DeviceKeys, agent: &DeviceKeys, msg_id: &str, kind: &str, body: &[u8], ts: u64) -> String {
        seal_message(&agent.x25519_pub, &phone.ed25519_seed, "phone", "agent", msg_id, 1, ts, random_nonce(), kind, body)
            .unwrap()
    }

    #[test]
    fn wire_round_trips() {
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        let body = seal(&phone, &agent, "m1", "setArmed", br#"{"armed":true}"#, 1000);
        let w = decode_wire(&body).unwrap();
        assert_eq!(w.from, "phone");
        assert_eq!(w.to, "agent");
        assert_eq!(w.msg_id, "m1");
        assert_eq!(w.enc.len(), 32);
        assert_eq!(w.sig.len(), 64);
    }

    #[test]
    fn valid_message_round_trips_through_process_inbound() {
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        let roster = roster_with("phone", &phone);
        let mut replay = ReplayGuard::load(tmp("ok"), 1000);
        let body = seal(&phone, &agent, "m1", "setArmed", br#"{"armed":true}"#, 1000);
        let (opened, sender_x) = process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &body, "m1", 1000).unwrap();
        assert_eq!(opened.kind, "setArmed");
        assert_eq!(opened.body, br#"{"armed":true}"#);
        assert_eq!(sender_x, phone.x25519_pub, "returns the sender x25519 for the reply");
    }

    #[test]
    fn rejects_unknown_or_revoked_sender() {
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        // An empty roster models both "never enrolled" and "revoked" (absent from live).
        let roster = VerifiedRoster { head_epoch: 1, live: BTreeMap::new() };
        let mut replay = ReplayGuard::load(tmp("unk"), 1000);
        let body = seal(&phone, &agent, "m1", "setArmed", b"{}", 1000);
        assert!(process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &body, "m1", 1000).is_err());
    }

    #[test]
    fn rejects_replay() {
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        let roster = roster_with("phone", &phone);
        let mut replay = ReplayGuard::load(tmp("replay"), 1000);
        let body = seal(&phone, &agent, "m1", "listRepos", b"{}", 1000);
        assert!(process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &body, "m1", 1000).is_ok());
        // Re-delivering the SAME envelope is rejected (same from+nonce).
        assert!(process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &body, "m1", 1001).is_err());
    }

    #[test]
    fn rejects_tampered_signature_and_ciphertext() {
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        let roster = roster_with("phone", &phone);
        let body = seal(&phone, &agent, "m1", "listRepos", b"{}", 1000);

        // Flip a byte of the signature → verify-first rejects.
        let mut w = decode_wire(&body).unwrap();
        w.sig[0] ^= 0xff;
        let bad_sig = encode_wire(&w);
        let mut r1 = ReplayGuard::load(tmp("sig"), 1000);
        assert!(process_inbound("agent", &agent.x25519_priv, &roster, &mut r1, &bad_sig, "m1", 1000).is_err());

        // Flip a byte of the ciphertext → the sig is over (aad,enc,ct), so verify
        // already fails; the AEAD would too.
        let mut w2 = decode_wire(&body).unwrap();
        w2.ct[0] ^= 0xff;
        let bad_ct = encode_wire(&w2);
        let mut r2 = ReplayGuard::load(tmp("ct"), 1000);
        assert!(process_inbound("agent", &agent.x25519_priv, &roster, &mut r2, &bad_ct, "m1", 1000).is_err());
    }

    #[test]
    fn rejects_msgid_mismatch_and_wrong_recipient() {
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        let roster = roster_with("phone", &phone);
        let mut replay = ReplayGuard::load(tmp("mm"), 1000);
        let body = seal(&phone, &agent, "m1", "listRepos", b"{}", 1000);
        // Relay column says "m2" but the envelope binds "m1".
        assert!(process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &body, "m2", 1000).is_err());
        // A different recipient device id is refused.
        assert!(process_inbound("other", &agent.x25519_priv, &roster, &mut replay, &body, "m1", 1000).is_err());
    }

    #[test]
    fn reply_seals_back_to_the_sender() {
        // The agent opens a command, then seals a reply to the returned sender key,
        // which the phone can open — the full bidirectional path.
        let phone = DeviceKeys::generate().unwrap();
        let agent = DeviceKeys::generate().unwrap();
        let roster = roster_with("phone", &phone);
        let mut replay = ReplayGuard::load(tmp("reply"), 1000);
        let body = seal(&phone, &agent, "m1", "listRepos", b"{}", 1000);
        let (_opened, sender_x) = process_inbound("agent", &agent.x25519_priv, &roster, &mut replay, &body, "m1", 1000).unwrap();

        let reply = seal_message(&sender_x, &agent.ed25519_seed, "agent", "phone", "m1-r", 1, 1001, random_nonce(), "reposResult", b"[]").unwrap();
        let rw = decode_wire(&reply).unwrap();
        let opened = open_wire(&rw, &phone.x25519_priv, &agent.ed25519_pub, "phone").unwrap();
        assert_eq!(opened.kind, "reposResult");
        assert_eq!(opened.body, b"[]");
    }
}
