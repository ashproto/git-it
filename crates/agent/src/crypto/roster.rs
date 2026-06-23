//! Signed device roster — the root of trust for message content (NOT Convex
//! auth). A per-account, append-only, hash-chained list of `add`/`revoke`
//! entries, every entry signed by the account RECOVERY key. Each device fetches
//! the full chain from the relay, verifies it end-to-end against a TOFU-pinned
//! recovery public key, and extracts the set of currently-live peer keys at the
//! head. Convex stores the entries as opaque blobs and never parses or forges
//! them — a malicious relay can at most withhold/reorder, which this verifier
//! detects as a fork/gap/rollback and HARD-FAILS.
//!
//! The canonical entry encoding here is the byte-exact contract the Swift mirror
//! (git-it-ios Sources/Support/Crypto/Roster.swift) must match; the shared
//! roster KAT pins it. Decisions: D-ROSTER, D-ROSTER-RECOVERY-PUBKEY,
//! D-REVOCATION-LIVENESS (see the 2b design spec).

use crate::crypto::envelope::{ed25519_sign, ed25519_verify};
use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Domain separator prepended to the canonical entry bytes — keeps a roster
/// signature from ever being confused with an envelope or any other signature.
pub const ROSTER_CTX: &[u8] = b"gitit-roster-v1\0";

/// Genesis link: the first entry's `prev_hash` is 32 zero bytes.
pub const GENESIS_PREV: [u8; 32] = [0u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Revoke,
}

/// One roster entry. `canonical()` is the exact byte sequence that is signed by
/// the recovery key and SHA-256-chained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub op: Op,
    pub epoch: u32,
    pub device_id: String,
    pub kind: String, // "mac" | "ios" | …
    pub x25519_pub: [u8; 32],
    pub ed25519_pub: [u8; 32],
    pub added_at: u64, // ms
    pub prev_hash: [u8; 32],
}

/// The current (non-revoked) keys for a live device at the roster head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceKeys {
    pub kind: String,
    pub x25519_pub: [u8; 32],
    pub ed25519_pub: [u8; 32],
}

/// The result of verifying a full chain: the head epoch + the live device set.
#[derive(Clone, Debug)]
pub struct VerifiedRoster {
    pub head_epoch: u32,
    /// Live devices at the head, keyed by deviceId. Iteration order is an
    /// implementation detail (a BTreeMap here, a Swift Dictionary in the mirror) —
    /// consumers MUST use keyed lookup ([`VerifiedRoster::keys_for`]) and never
    /// depend on order, so the two languages stay observably identical.
    pub live: BTreeMap<String, DeviceKeys>,
}

impl VerifiedRoster {
    /// The current keys for `device_id`, or None if it is not live (never added,
    /// or revoked and not re-added) at the head — the receive-path liveness check.
    pub fn keys_for(&self, device_id: &str) -> Option<&DeviceKeys> {
        self.live.get(device_id)
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(bytes));
    out
}

impl Entry {
    /// Canonical encoding (length-prefixed RAW bytes — never CBOR/JSON ordering):
    /// `ROSTER_CTX ‖ ver(1) ‖ op(1) ‖ epoch(4 BE) ‖ u16(len deviceId)‖deviceId ‖
    /// u16(len kind)‖kind ‖ x25519(32) ‖ ed25519(32) ‖ addedAt(8 BE) ‖ prevHash(32)`.
    pub fn canonical(&self) -> Result<Vec<u8>> {
        let mut v = Vec::with_capacity(ROSTER_CTX.len() + self.device_id.len() + self.kind.len() + 96);
        v.extend_from_slice(ROSTER_CTX);
        v.push(1); // ver
        v.push(match self.op {
            Op::Add => 0,
            Op::Revoke => 1,
        });
        v.extend_from_slice(&self.epoch.to_be_bytes());
        for (label, s) in [("deviceId", &self.device_id), ("kind", &self.kind)] {
            if s.len() > u16::MAX as usize {
                bail!("roster field {label} is {} bytes — exceeds 65535", s.len());
            }
            v.extend_from_slice(&(s.len() as u16).to_be_bytes());
            v.extend_from_slice(s.as_bytes());
        }
        v.extend_from_slice(&self.x25519_pub);
        v.extend_from_slice(&self.ed25519_pub);
        v.extend_from_slice(&self.added_at.to_be_bytes());
        v.extend_from_slice(&self.prev_hash);
        Ok(v)
    }

    /// Parse a canonical entry, rejecting truncated/over-long/trailing-byte input.
    pub fn decode(b: &[u8]) -> Result<Entry> {
        let mut p = 0usize;
        let take = |p: &mut usize, n: usize| -> Result<&[u8]> {
            if *p + n > b.len() {
                bail!("roster entry truncated (need {n} at {p}, len {})", b.len());
            }
            let s = &b[*p..*p + n];
            *p += n;
            Ok(s)
        };
        if take(&mut p, ROSTER_CTX.len())? != ROSTER_CTX {
            bail!("bad roster context");
        }
        let ver = take(&mut p, 1)?[0];
        if ver != 1 {
            bail!("unsupported roster entry version {ver}");
        }
        let op = match take(&mut p, 1)?[0] {
            0 => Op::Add,
            1 => Op::Revoke,
            x => bail!("bad roster op {x}"),
        };
        let epoch = u32::from_be_bytes(take(&mut p, 4)?.try_into().unwrap());
        let read_str = |p: &mut usize| -> Result<String> {
            let len = u16::from_be_bytes(take(p, 2)?.try_into().unwrap()) as usize;
            Ok(String::from_utf8(take(p, len)?.to_vec())?)
        };
        let device_id = read_str(&mut p)?;
        let kind = read_str(&mut p)?;
        let x25519_pub: [u8; 32] = take(&mut p, 32)?.try_into().unwrap();
        let ed25519_pub: [u8; 32] = take(&mut p, 32)?.try_into().unwrap();
        let added_at = u64::from_be_bytes(take(&mut p, 8)?.try_into().unwrap());
        let prev_hash: [u8; 32] = take(&mut p, 32)?.try_into().unwrap();
        if p != b.len() {
            bail!("trailing bytes after roster entry");
        }
        Ok(Entry {
            op,
            epoch,
            device_id,
            kind,
            x25519_pub,
            ed25519_pub,
            added_at,
            prev_hash,
        })
    }
}

/// A roster entry as carried by the relay: the EXACT canonical bytes that were
/// signed (verified/hashed as-received, never re-encoded) + the recovery sig.
#[derive(Clone, Debug)]
pub struct SignedEntry {
    pub canonical: Vec<u8>,
    pub sig: [u8; 64],
    pub entry: Entry,
}

impl SignedEntry {
    /// Build + sign an entry with the account recovery seed (phone-side; tests).
    pub fn create(recovery_seed: &[u8; 32], entry: Entry) -> Result<Self> {
        let canonical = entry.canonical()?;
        let sig = ed25519_sign(recovery_seed, &canonical);
        Ok(Self { canonical, sig, entry })
    }

    /// Reconstruct from relay-carried bytes (canonical ‖ sig), parsing the view.
    pub fn parse(canonical: Vec<u8>, sig: [u8; 64]) -> Result<Self> {
        let entry = Entry::decode(&canonical)?;
        Ok(Self { canonical, sig, entry })
    }

    /// SHA-256 of the canonical bytes — this entry's link, the next entry's
    /// `prev_hash`.
    pub fn hash(&self) -> [u8; 32] {
        sha256(&self.canonical)
    }
}

/// Verify a full roster chain against a TOFU-pinned recovery public key and a
/// persisted epoch floor, returning the live device set at the head. HARD-FAILS
/// on any bad signature, broken prevHash link (fork/gap), non-increasing epoch
/// (rewind/fork), or a head epoch below the pinned floor (rollback).
///
/// Liveness uses replay-to-head semantics: `add` inserts/updates a device's
/// keys, `revoke` removes it, a later `add` re-enrolls it. A peer is live iff it
/// is present in the returned set at the head (D-REVOCATION-LIVENESS).
pub fn verify_chain(
    chain: &[SignedEntry],
    recovery_pub: &[u8; 32],
    epoch_floor: u32,
) -> Result<VerifiedRoster> {
    if chain.is_empty() {
        bail!("empty roster chain");
    }
    let mut expected_prev = GENESIS_PREV;
    let mut last_epoch: Option<u32> = None;
    let mut live: BTreeMap<String, DeviceKeys> = BTreeMap::new();

    for (i, se) in chain.iter().enumerate() {
        // The signed bytes must parse to the view we apply (defends against a
        // canonical/entry mismatch in a hand-built SignedEntry).
        if Entry::decode(&se.canonical)? != se.entry {
            bail!("entry {i}: parsed view does not match its canonical bytes");
        }
        if !ed25519_verify(recovery_pub, &se.canonical, &se.sig) {
            bail!("entry {i}: recovery signature invalid (not signed by the pinned key)");
        }
        if se.entry.prev_hash != expected_prev {
            bail!("entry {i}: prevHash does not chain — fork or dropped entry");
        }
        if let Some(le) = last_epoch {
            if se.entry.epoch <= le {
                bail!("entry {i}: epoch {} not strictly greater than {le} — rewind/fork", se.entry.epoch);
            }
        }
        last_epoch = Some(se.entry.epoch);
        expected_prev = se.hash();

        match se.entry.op {
            Op::Add => {
                live.insert(
                    se.entry.device_id.clone(),
                    DeviceKeys {
                        kind: se.entry.kind.clone(),
                        x25519_pub: se.entry.x25519_pub,
                        ed25519_pub: se.entry.ed25519_pub,
                    },
                );
            }
            Op::Revoke => {
                live.remove(&se.entry.device_id);
            }
        }
    }

    let head_epoch = last_epoch.expect("non-empty chain has a last epoch");
    if head_epoch < epoch_floor {
        bail!("roster head epoch {head_epoch} < pinned floor {epoch_floor} — rollback");
    }
    Ok(VerifiedRoster { head_epoch, live })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::envelope::ed25519_public;

    const RECOVERY_SEED: [u8; 32] = [7u8; 32];

    fn entry(op: Op, epoch: u32, device: &str, prev: [u8; 32], tag: u8) -> Entry {
        Entry {
            op,
            epoch,
            device_id: device.to_string(),
            kind: "ios".to_string(),
            x25519_pub: [tag; 32],
            ed25519_pub: [tag.wrapping_add(1); 32],
            added_at: 1_718_000_000_000 + epoch as u64,
            prev_hash: prev,
        }
    }

    /// Build a valid signed chain from a list of (op, epoch, device, tag),
    /// chaining prevHash automatically.
    fn signed_chain(steps: &[(Op, u32, &str, u8)]) -> Vec<SignedEntry> {
        let mut prev = GENESIS_PREV;
        let mut out = Vec::new();
        for &(op, epoch, device, tag) in steps {
            let se = SignedEntry::create(&RECOVERY_SEED, entry(op, epoch, device, prev, tag)).unwrap();
            prev = sha256(&se.canonical);
            out.push(se);
        }
        out
    }

    #[test]
    fn canonical_roundtrips() {
        let e = entry(Op::Add, 5, "phone-1", [9u8; 32], 0x33);
        let bytes = e.canonical().unwrap();
        assert_eq!(Entry::decode(&bytes).unwrap(), e);
        // ROSTER_CTX prefix present.
        assert!(bytes.starts_with(ROSTER_CTX));
    }

    #[test]
    fn valid_chain_yields_live_set() {
        let chain = signed_chain(&[
            (Op::Add, 1, "phone", 0x11),
            (Op::Add, 2, "mac", 0x22),
        ]);
        let pub_k = ed25519_public(&RECOVERY_SEED);
        let r = verify_chain(&chain, &pub_k, 0).unwrap();
        assert_eq!(r.head_epoch, 2);
        assert_eq!(r.live.len(), 2);
        assert_eq!(r.keys_for("mac").unwrap().x25519_pub, [0x22; 32]);
        assert!(r.keys_for("ghost").is_none());
    }

    #[test]
    fn revoke_removes_device_then_readd_restores() {
        let chain = signed_chain(&[
            (Op::Add, 1, "phone", 0x11),
            (Op::Add, 2, "mac", 0x22),
            (Op::Revoke, 3, "mac", 0x22),
        ]);
        let pub_k = ed25519_public(&RECOVERY_SEED);
        let r = verify_chain(&chain, &pub_k, 0).unwrap();
        assert!(r.keys_for("mac").is_none(), "revoked device must not be live");
        assert!(r.keys_for("phone").is_some());

        // Re-add at a later epoch restores liveness with (possibly new) keys.
        let mut prev = sha256(&chain.last().unwrap().canonical);
        let readd = SignedEntry::create(&RECOVERY_SEED, entry(Op::Add, 4, "mac", prev, 0x44)).unwrap();
        prev = sha256(&readd.canonical);
        let _ = prev;
        let mut full = chain;
        full.push(readd);
        let r2 = verify_chain(&full, &pub_k, 0).unwrap();
        assert_eq!(r2.keys_for("mac").unwrap().x25519_pub, [0x44; 32]);
    }

    #[test]
    fn wrong_recovery_key_is_rejected() {
        let chain = signed_chain(&[(Op::Add, 1, "phone", 0x11)]);
        let other = ed25519_public(&[8u8; 32]);
        assert!(verify_chain(&chain, &other, 0).is_err(), "TOFU: a different signer must fail");
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let mut chain = signed_chain(&[(Op::Add, 1, "phone", 0x11)]);
        chain[0].sig[0] ^= 0xFF;
        let pub_k = ed25519_public(&RECOVERY_SEED);
        assert!(verify_chain(&chain, &pub_k, 0).is_err());
    }

    #[test]
    fn broken_prevhash_link_is_rejected() {
        let mut chain = signed_chain(&[
            (Op::Add, 1, "phone", 0x11),
            (Op::Add, 2, "mac", 0x22),
        ]);
        // Corrupt the 2nd entry's prevHash + re-sign so only the LINK is broken.
        chain[1].entry.prev_hash = [0xEE; 32];
        chain[1].canonical = chain[1].entry.canonical().unwrap();
        chain[1].sig = ed25519_sign(&RECOVERY_SEED, &chain[1].canonical);
        let pub_k = ed25519_public(&RECOVERY_SEED);
        assert!(verify_chain(&chain, &pub_k, 0).is_err(), "fork/gap must hard-fail");
    }

    #[test]
    fn non_increasing_epoch_is_rejected() {
        // Two entries with the same epoch, correctly chained + signed.
        let mut prev = GENESIS_PREV;
        let e0 = SignedEntry::create(&RECOVERY_SEED, entry(Op::Add, 5, "a", prev, 0x11)).unwrap();
        prev = sha256(&e0.canonical);
        let e1 = SignedEntry::create(&RECOVERY_SEED, entry(Op::Add, 5, "b", prev, 0x22)).unwrap();
        let pub_k = ed25519_public(&RECOVERY_SEED);
        assert!(verify_chain(&[e0, e1], &pub_k, 0).is_err(), "rewind/equal epoch must fail");
    }

    #[test]
    fn head_below_epoch_floor_is_rollback() {
        let chain = signed_chain(&[(Op::Add, 3, "phone", 0x11)]);
        let pub_k = ed25519_public(&RECOVERY_SEED);
        assert!(verify_chain(&chain, &pub_k, 10).is_err(), "head epoch 3 < floor 10 is a rollback");
        // At/above the floor it verifies.
        assert!(verify_chain(&chain, &pub_k, 3).is_ok());
    }

    #[test]
    fn empty_chain_is_rejected() {
        let pub_k = ed25519_public(&RECOVERY_SEED);
        assert!(verify_chain(&[], &pub_k, 0).is_err());
    }

    #[test]
    fn canonical_mismatch_view_is_rejected() {
        // A hand-built SignedEntry whose parsed view disagrees with its bytes.
        let good = SignedEntry::create(&RECOVERY_SEED, entry(Op::Add, 1, "phone", GENESIS_PREV, 0x11)).unwrap();
        let mut tampered = good.clone();
        tampered.entry.device_id = "evil".to_string(); // bytes still say "phone"
        let pub_k = ed25519_public(&RECOVERY_SEED);
        assert!(verify_chain(&[tampered], &pub_k, 0).is_err());
    }
}
