//! Phase 2b-S3: this agent's long-lived device keypairs — X25519 (the HPKE
//! recipient key the phone seals to) + Ed25519 (the sender-signature / roster
//! identity key). Generated once at pairing and persisted (private halves,
//! base64) under the `keys` object of the 0600 `agent.json`; the public halves
//! are re-derived on load. These keys are pinned into the signed device roster at
//! enrollment, so they MUST stay byte-stable across restarts.

use crate::crypto::envelope;
use anyhow::{anyhow, Context, Result};
use base64::Engine;

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// This device's raw 32-byte key material. Private halves are secret (0600 on
/// disk); public halves are derived, never trusted from disk.
#[derive(Clone)]
pub struct DeviceKeys {
    pub x25519_priv: [u8; 32],
    pub x25519_pub: [u8; 32],
    pub ed25519_seed: [u8; 32],
    pub ed25519_pub: [u8; 32],
}

impl DeviceKeys {
    /// Generate a fresh X25519 + Ed25519 keypair from the OS CSPRNG.
    pub fn generate() -> Result<Self> {
        let (x25519_priv, x25519_pub) = envelope::gen_x25519()?;
        let ed25519_seed = envelope::random_32();
        let ed25519_pub = envelope::ed25519_public(&ed25519_seed);
        Ok(Self { x25519_priv, x25519_pub, ed25519_seed, ed25519_pub })
    }

    /// Rebuild from the two stored private seeds, re-deriving the public halves.
    fn from_privs(x25519_priv: [u8; 32], ed25519_seed: [u8; 32]) -> Result<Self> {
        Ok(Self {
            x25519_pub: envelope::x25519_public(&x25519_priv)?,
            ed25519_pub: envelope::ed25519_public(&ed25519_seed),
            x25519_priv,
            ed25519_seed,
        })
    }

    /// The `keys` object as persisted in `agent.json` (only the private halves;
    /// pubs are re-derived). Versioned so a later format change is detectable.
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "v": 1,
            "x25519Priv": B64.encode(self.x25519_priv),
            "ed25519Seed": B64.encode(self.ed25519_seed),
        })
    }
}

/// Decode a base64 field of `obj` into a fixed 32-byte array.
fn field32(obj: &serde_json::Value, key: &str) -> Result<[u8; 32]> {
    let s = obj.get(key).and_then(|x| x.as_str()).ok_or_else(|| anyhow!("keys.{key} missing"))?;
    let bytes = B64.decode(s).with_context(|| format!("keys.{key} is not base64"))?;
    bytes.as_slice().try_into().map_err(|_| anyhow!("keys.{key} is not 32 bytes"))
}

/// Load this device's keys from `agent.json`, generating + persisting a fresh
/// keypair on first use (or if the stored `keys` object is absent/malformed).
/// Idempotent after the first call: the same keys round-trip every time, so the
/// roster pin stays valid. Persistence reuses the atomic 0600 config writer.
pub fn load_or_create() -> Result<DeviceKeys> {
    let cfg = crate::repos::read_config_value();
    if let Some(keys) = cfg.get("keys") {
        match (field32(keys, "x25519Priv"), field32(keys, "ed25519Seed")) {
            (Ok(x), Ok(seed)) => return DeviceKeys::from_privs(x, seed),
            _ => {
                // Malformed/partial keys object — regenerate rather than run with
                // a half-key. (A pre-enrollment agent has no roster pin to break.)
            }
        }
    }
    let keys = DeviceKeys::generate()?;
    save(&keys)?;
    Ok(keys)
}

/// Load this device's keys if present + well-formed, without creating any.
pub fn load() -> Option<DeviceKeys> {
    let cfg = crate::repos::read_config_value();
    let keys = cfg.get("keys")?;
    DeviceKeys::from_privs(field32(keys, "x25519Priv").ok()?, field32(keys, "ed25519Seed").ok()?).ok()
}

/// Persist the `keys` object into `agent.json`, preserving every sibling field
/// (auth/repos/armed) via a read-modify-write through the atomic 0600 writer.
fn save(keys: &DeviceKeys) -> Result<()> {
    let mut cfg = crate::repos::read_config_value();
    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("keys".into(), keys.to_json());
    }
    crate::repos::write_config_value(&cfg).context("persist device keys")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_home<T>(tag: &str, f: impl FnOnce() -> T) -> T {
        // Crate-wide HOME guard, shared with repos::tests so cross-module HOME
        // mutations can't race under the parallel test runner.
        let _g = crate::repos::TEST_HOME_GUARD.lock().unwrap();
        let home = std::env::temp_dir().join(format!("gitit-keys-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        let out = f();
        let _ = std::fs::remove_dir_all(&home);
        out
    }

    #[test]
    fn generate_produces_consistent_pubs() {
        let k = DeviceKeys::generate().unwrap();
        assert_eq!(k.x25519_pub, envelope::x25519_public(&k.x25519_priv).unwrap());
        assert_eq!(k.ed25519_pub, envelope::ed25519_public(&k.ed25519_seed));
        // Two generations differ (real randomness).
        assert_ne!(k.ed25519_seed, DeviceKeys::generate().unwrap().ed25519_seed);
    }

    #[test]
    fn load_or_create_is_stable_and_preserves_siblings() {
        with_home("stable", || {
            // Seed agent.json with sibling fields that must survive key creation.
            let cfg = crate::repos::read_config_value();
            assert!(cfg.get("keys").is_none());
            crate::repos::add_repo("/repo-a");

            let k1 = load_or_create().unwrap();
            // Second call returns the SAME keys (stable roster pin), not new ones.
            let k2 = load_or_create().unwrap();
            assert_eq!(k1.x25519_priv, k2.x25519_priv);
            assert_eq!(k1.ed25519_seed, k2.ed25519_seed);
            assert_eq!(k1.ed25519_pub, k2.ed25519_pub);

            // `load` sees the same keys; the repos sibling field is intact.
            let loaded = load().unwrap();
            assert_eq!(loaded.x25519_pub, k1.x25519_pub);
            assert_eq!(crate::repos::list_repos().repos.len(), 1);
        });
    }

    #[test]
    fn malformed_keys_object_is_regenerated() {
        with_home("malformed", || {
            // A keys object missing ed25519Seed must NOT half-load; regenerate.
            let mut cfg = crate::repos::read_config_value();
            cfg.as_object_mut().unwrap().insert(
                "keys".into(),
                serde_json::json!({ "v": 1, "x25519Priv": B64.encode([7u8; 32]) }),
            );
            crate::repos::write_config_value(&cfg).unwrap();
            assert!(load().is_none(), "partial keys do not load");

            let k = load_or_create().unwrap();
            // It persisted a full, self-consistent pair.
            let reloaded = load().unwrap();
            assert_eq!(reloaded.ed25519_seed, k.ed25519_seed);
            assert_eq!(reloaded.ed25519_pub, envelope::ed25519_public(&reloaded.ed25519_seed));
        });
    }
}
