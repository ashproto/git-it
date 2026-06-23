//! Phase 2b-S3 anti-replay: a PERSISTED, time-indexed per-sender seen-(nonce,ts)
//! set over a ±5-minute freshness window (D-REPLAY). A captured ciphertext can't
//! be re-delivered: its `(from, nonce)` is remembered until its timestamp ages
//! out of the window, and a message whose timestamp is outside the window is
//! rejected outright. Eviction drops ONLY already-expired entries, so a flood of
//! fresh nonces can never push a still-valid nonce out of the set. Persisted to a
//! dedicated 0600 `replay.json` (reloaded on boot) so a restart can't replay.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Accept timestamps within ±5 minutes of now; remember a nonce for that long.
pub const WINDOW_MS: u64 = 300_000;

#[derive(serde::Serialize, serde::Deserialize)]
struct ReplayRow {
    from: String,
    nonce: String,
    ts: u64,
}

pub struct ReplayGuard {
    seen: HashMap<(String, String), u64>,
    path: PathBuf,
}

impl ReplayGuard {
    /// Load the persisted set from `path`, dropping entries already expired vs
    /// `now_ms` (they can never be validly replayed again).
    pub fn load(path: PathBuf, now_ms: u64) -> Self {
        let mut seen = HashMap::new();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(rows) = serde_json::from_str::<Vec<ReplayRow>>(&text) {
                for r in rows {
                    if now_ms.saturating_sub(r.ts) <= WINDOW_MS {
                        seen.insert((r.from, r.nonce), r.ts);
                    }
                }
            }
        }
        Self { seen, path }
    }

    /// Open the default store (`~/.config/git-it/replay.json`).
    pub fn open(now_ms: u64) -> Self {
        Self::load(crate::repos::replay_path(), now_ms)
    }

    /// Accept a message exactly once within the freshness window. Errors on a
    /// stale/future timestamp (|now−ts| > window) or a duplicate `(from, nonce)`.
    /// On accept it records the nonce and persists. Expired entries are evicted
    /// first; the just-checked message is in-window (guarded above) so eviction
    /// never targets a still-valid nonce.
    pub fn check_and_record(
        &mut self,
        from: &str,
        nonce_b64: &str,
        ts_ms: u64,
        now_ms: u64,
    ) -> Result<()> {
        if now_ms.abs_diff(ts_ms) > WINDOW_MS {
            return Err(anyhow!("message timestamp outside the ±5min window"));
        }
        self.seen.retain(|_, ts| now_ms.saturating_sub(*ts) <= WINDOW_MS);
        let key = (from.to_string(), nonce_b64.to_string());
        if self.seen.contains_key(&key) {
            return Err(anyhow!("replayed message (nonce already seen for this sender)"));
        }
        self.seen.insert(key, ts_ms);
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        let rows: Vec<ReplayRow> = self
            .seen
            .iter()
            .map(|((from, nonce), ts)| ReplayRow { from: from.clone(), nonce: nonce.clone(), ts: *ts })
            .collect();
        let v = serde_json::to_value(rows)?;
        crate::repos::write_json_atomic(&self.path, &v).map_err(|e| anyhow!("persist replay set: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("gitit-replay-{tag}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn accepts_once_then_rejects_replay() {
        let mut g = ReplayGuard::load(tmp("once"), 1_000_000);
        assert!(g.check_and_record("phone", "nonceA", 1_000_000, 1_000_000).is_ok());
        // Same (from, nonce) again → replay.
        assert!(g.check_and_record("phone", "nonceA", 1_000_000, 1_000_050).is_err());
        // A different nonce from the same sender is fine.
        assert!(g.check_and_record("phone", "nonceB", 1_000_050, 1_000_050).is_ok());
        // The SAME nonce from a different sender is a distinct key → accepted.
        assert!(g.check_and_record("ipad", "nonceA", 1_000_050, 1_000_050).is_ok());
    }

    #[test]
    fn rejects_stale_and_future_timestamps() {
        let mut g = ReplayGuard::load(tmp("skew"), 2_000_000);
        // 6 minutes in the past.
        assert!(g.check_and_record("phone", "n1", 2_000_000 - 360_000, 2_000_000).is_err());
        // 6 minutes in the future.
        assert!(g.check_and_record("phone", "n2", 2_000_000 + 360_000, 2_000_000).is_err());
        // Exactly at the window edge is accepted.
        assert!(g.check_and_record("phone", "n3", 2_000_000 - WINDOW_MS, 2_000_000).is_ok());
    }

    #[test]
    fn a_flood_of_fresh_nonces_does_not_evict_a_valid_one() {
        let now = 5_000_000;
        let mut g = ReplayGuard::load(tmp("flood"), now);
        assert!(g.check_and_record("phone", "keep", now, now).is_ok());
        for i in 0..500 {
            assert!(g.check_and_record("phone", &format!("flood-{i}"), now, now).is_ok());
        }
        // "keep" is still in-window so it was never evicted → replay rejected.
        assert!(g.check_and_record("phone", "keep", now, now).is_err());
    }

    #[test]
    fn persists_across_reload_so_a_restart_cannot_replay() {
        let path = tmp("persist");
        let now = 9_000_000;
        {
            let mut g = ReplayGuard::load(path.clone(), now);
            g.check_and_record("phone", "boot-nonce", now, now).unwrap();
        }
        // A fresh guard (agent restart) reloads the set and still rejects the replay.
        let mut g2 = ReplayGuard::load(path.clone(), now + 1000);
        assert!(g2.check_and_record("phone", "boot-nonce", now, now + 1000).is_err());

        // After the window passes, the reloaded set drops the expired entry — and
        // the stale timestamp would be rejected on its own anyway.
        let mut g3 = ReplayGuard::load(path, now + WINDOW_MS + 1);
        assert!(
            g3.check_and_record("phone", "boot-nonce", now, now + WINDOW_MS + 1).is_err(),
            "expired timestamp is rejected by the window check"
        );
    }
}
