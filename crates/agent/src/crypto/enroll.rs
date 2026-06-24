//! Phase 2b-S5 agent-side roster trust: TOFU-pin the account recovery public key
//! (which the phone publishes to a Convex `accounts` field) and resolve the
//! fetched roster chain into the live device set. The agent pins the recovery key
//! on first sight (`agent.json recoveryPub`) and thereafter refuses any roster not
//! signed by it (SSH known-hosts model, D-ROSTER-RECOVERY-PUBKEY); a persisted
//! `rosterEpochFloor` advances monotonically to detect rollback. The relay's live
//! receive path (S5) feeds the result to `message::process_inbound`.

use crate::crypto::roster::{self, VerifiedRoster};
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// One roster row as carried by Convex `roster:chain` (base64 STANDARD — the
/// encoding the phone's `Enrollment.publishArgs` emits).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ChainRow {
    #[serde(rename = "entryB64")]
    pub entry_b64: String,
    #[serde(rename = "sigB64")]
    pub sig_b64: String,
}

/// Parse + verify the fetched chain against the pinned recovery key and the
/// persisted epoch floor, returning the live device set at the head. Fails closed
/// on a bad row, a bad signature, a broken link, a rewind/fork, or rollback below
/// the floor (all enforced by `roster::verify_chain`).
pub fn resolve_roster(rows: &[ChainRow], recovery_pub: &[u8; 32], epoch_floor: u32) -> Result<VerifiedRoster> {
    let mut chain = Vec::with_capacity(rows.len());
    for r in rows {
        let canonical = B64.decode(&r.entry_b64).context("roster entry is not base64")?;
        let sig_bytes = B64.decode(&r.sig_b64).context("roster sig is not base64")?;
        let sig: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("roster sig is not 64 bytes"))?;
        chain.push(roster::SignedEntry::parse(canonical, sig)?);
    }
    roster::verify_chain(&chain, recovery_pub, epoch_floor)
}

/// The TOFU-pinned account recovery public key, if one has been pinned.
pub fn pinned_recovery_pub() -> Option<[u8; 32]> {
    let s = crate::repos::read_config_value()
        .get("recoveryPub")?
        .as_str()?
        .to_string();
    B64.decode(s).ok()?.as_slice().try_into().ok()
}

/// TOFU-pin the account recovery public key. First sight wins; a later DIFFERENT
/// key is REFUSED (known-hosts model) — re-pairing is required to reset trust.
/// Re-pinning the same key is a no-op.
pub fn pin_recovery_pub(recovery_pub: &[u8; 32]) -> Result<()> {
    if let Some(existing) = pinned_recovery_pub() {
        if existing == *recovery_pub {
            return Ok(());
        }
        bail!("a different recovery key is already pinned — refusing to overwrite (re-pair to reset trust)");
    }
    let mut cfg = crate::repos::read_config_value();
    if let Some(o) = cfg.as_object_mut() {
        o.insert("recoveryPub".into(), serde_json::json!(B64.encode(recovery_pub)));
    }
    crate::repos::write_config_value(&cfg).context("persist recoveryPub")
}

/// The persisted monotonic roster epoch floor (0 if unset) — rollback detection.
pub fn roster_epoch_floor() -> u32 {
    crate::repos::read_config_value()
        .get("rosterEpochFloor")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

/// Advance the epoch floor to `epoch` if it is higher (never lower — a roster
/// whose head is below the floor is a rollback and is rejected on the next fetch).
pub fn advance_epoch_floor(epoch: u32) -> Result<()> {
    if epoch <= roster_epoch_floor() {
        return Ok(());
    }
    let mut cfg = crate::repos::read_config_value();
    if let Some(o) = cfg.as_object_mut() {
        o.insert("rosterEpochFloor".into(), serde_json::json!(epoch));
    }
    crate::repos::write_config_value(&cfg).context("persist rosterEpochFloor")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::envelope;
    use crate::crypto::roster::{Entry, Op, SignedEntry, GENESIS_PREV};

    fn keypair(tag: u8) -> (Vec<u8>, Vec<u8>) {
        let x = vec![tag; 32];
        let xp = envelope::x25519_public(&x.clone().try_into().unwrap()).unwrap();
        let e = vec![tag.wrapping_add(1); 32];
        let ep = envelope::ed25519_public(&e.clone().try_into().unwrap());
        (xp.to_vec(), ep.to_vec())
    }

    fn rows_for(recovery_seed: &[u8; 32]) -> Vec<ChainRow> {
        // add phone @1, add agent @2.
        let mk = |op, epoch: u32, dev: &str, kind: &str, tag: u8, prev: [u8; 32]| {
            let (xp, ep) = keypair(tag);
            Entry {
                op,
                epoch,
                device_id: dev.to_string(),
                kind: kind.to_string(),
                x25519_pub: xp.try_into().unwrap(),
                ed25519_pub: ep.try_into().unwrap(),
                added_at: epoch as u64,
                prev_hash: prev,
            }
        };
        let phone = SignedEntry::create(recovery_seed, mk(Op::Add, 1, "phone", "phone", 0x10, GENESIS_PREV)).unwrap();
        let agent = SignedEntry::create(recovery_seed, mk(Op::Add, 2, "agent", "mac", 0x20, phone.hash())).unwrap();
        [phone, agent]
            .iter()
            .map(|se| ChainRow { entry_b64: B64.encode(&se.canonical), sig_b64: B64.encode(se.sig) })
            .collect()
    }

    #[test]
    fn resolve_roster_verifies_and_rejects_wrong_key() {
        let recovery_seed = [0x55u8; 32];
        let recovery_pub = envelope::ed25519_public(&recovery_seed);
        let rows = rows_for(&recovery_seed);

        let v = resolve_roster(&rows, &recovery_pub, 0).unwrap();
        assert_eq!(v.head_epoch, 2);
        assert!(v.keys_for("phone").is_some());
        assert_eq!(v.keys_for("agent").unwrap().kind, "mac");

        // A different recovery key does not verify (TOFU pin).
        let other = envelope::ed25519_public(&[0x66u8; 32]);
        assert!(resolve_roster(&rows, &other, 0).is_err());
        // A floor above the head is a rollback → rejected.
        assert!(resolve_roster(&rows, &recovery_pub, 99).is_err());
        // A malformed row fails closed.
        let bad = vec![ChainRow { entry_b64: "!!".into(), sig_b64: "!!".into() }];
        assert!(resolve_roster(&bad, &recovery_pub, 0).is_err());
    }

    #[test]
    fn recovery_pin_is_tofu_and_floor_advances() {
        let _g = crate::repos::TEST_HOME_GUARD.lock().unwrap();
        let home = std::env::temp_dir().join(format!("gitit-enroll-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);

        assert!(pinned_recovery_pub().is_none());
        let a = [0x11u8; 32];
        pin_recovery_pub(&a).unwrap();
        assert_eq!(pinned_recovery_pub(), Some(a));
        pin_recovery_pub(&a).unwrap(); // re-pin same is a no-op
        // A different key is refused (known-hosts).
        assert!(pin_recovery_pub(&[0x22u8; 32]).is_err());
        assert_eq!(pinned_recovery_pub(), Some(a), "the original pin is preserved");

        assert_eq!(roster_epoch_floor(), 0);
        advance_epoch_floor(5).unwrap();
        assert_eq!(roster_epoch_floor(), 5);
        advance_epoch_floor(3).unwrap(); // lower is a no-op
        assert_eq!(roster_epoch_floor(), 5);

        let _ = std::fs::remove_dir_all(&home);
    }
}
