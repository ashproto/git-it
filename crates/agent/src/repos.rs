//! Repo discovery: read the registered repo paths and summarize each via git-core.
use serde::Serialize;
use std::path::{Path, PathBuf};
use git_core::{graph, path_setup};

#[derive(Serialize)]
pub struct RepoSummary {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ReposResult { pub device: String, pub repos: Vec<RepoSummary> }

pub fn list_repos() -> ReposResult {
    // `repo_status` shells out to git; ensure Homebrew git is on PATH for
    // Finder/launchd-spawned agents (matches `status_json`).
    path_setup::ensure_homebrew_path();
    ReposResult { device: device_name(), repos: registered_paths().iter().map(|p| summarize(p)).collect() }
}

fn config_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/git-it/agent.json")
}

fn registered_paths() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(config_path()) else { return vec![] };
    serde_json::from_str::<serde_json::Value>(&text).ok()
        .and_then(|v| v.get("repos").and_then(|r| r.as_array()).cloned())
        .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn summarize(path: &str) -> RepoSummary {
    let name = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path).to_string();
    match graph::repo_status(Path::new(path)) {
        Ok(s) => RepoSummary {
            name, path: path.to_string(), branch: s.head.branch,
            staged: s.staged, unstaged: s.unstaged, untracked: s.untracked, error: None,
        },
        Err(e) => RepoSummary {
            name, path: path.to_string(), branch: None,
            staged: 0, unstaged: 0, untracked: 0, error: Some(e.to_string()),
        },
    }
}

/// Stable per-machine id = the macOS hardware UUID (IOPlatformUUID).
pub fn device_id() -> String {
    std::process::Command::new("ioreg").args(["-rd1", "-c", "IOPlatformExpertDevice"]).output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().find(|l| l.contains("IOPlatformUUID")).map(String::from))
        .and_then(|l| l.split('"').nth(3).map(String::from))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(device_name)
}

/// Read `"armed"` from the config. If the key is present, honor it. If it's
/// absent but the agent is PAIRED (an `auth` object exists), default to DISARMED
/// — a paired-but-not-explicitly-armed agent must be gated (fail-safe), matching
/// the provisioned-disarmed device row. An unpaired/standalone config (no auth)
/// defaults armed (legacy; the relay refuses to run unpaired anyway).
pub fn armed() -> bool {
    let v = read_config_value();
    if let Some(b) = v.get("armed").and_then(|a| a.as_bool()) {
        return b;
    }
    v.get("auth").is_none()
}

/// Read the config as a JSON **object**. If the file is absent, empty, an
/// array, or otherwise not a JSON object, return an empty object `{}` so the
/// mutations below always have a place to insert/update keys (never a silent
/// no-op against a non-object value).
fn read_config_value() -> serde_json::Value {
    let parsed = std::fs::read_to_string(config_path()).ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    match parsed {
        Some(v) if v.is_object() => v,
        _ => serde_json::json!({}),
    }
}

/// Persist the config atomically: write to a sibling temp file, then rename
/// over the real path (an atomic replace on the same filesystem). This avoids
/// leaving a half-written `agent.json` if the process is killed mid-write.
fn write_config_value(v: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let path = config_path();
    if let Some(dir) = path.parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
            // Owner-only config dir — it holds the durable refresh token.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(v)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    // Create the temp file owner-only (0600) so the refresh token is never
    // world-readable; fsync before the atomic rename so a crash can't leave a
    // truncated single-copy secret.
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, &path) // atomic replace (preserves the 0600 temp mode)
}

/// Path to the agent's env file (`~/.config/git-it/.env`) — a FIXED location, so
/// the refresh-target URLs are never read from a CWD-relative dotfile an attacker
/// could plant. Production launches set these via the launchd plist instead.
pub fn env_path() -> std::path::PathBuf {
    config_path().with_file_name(".env")
}

/// Register a repo path, preserving all other config fields. Dedups so the same
/// path is never added twice.
pub fn add_repo(path: &str) {
    let mut v = read_config_value();
    if let Some(obj) = v.as_object_mut() {
        let arr = obj.entry("repos").or_insert_with(|| serde_json::json!([]));
        if !arr.is_array() { *arr = serde_json::json!([]); }
        if let Some(a) = arr.as_array_mut() {
            if !a.iter().any(|p| p.as_str() == Some(path)) {
                a.push(serde_json::json!(path));
            }
        }
    }
    let _ = write_config_value(&v);
}

/// Unregister a repo path, preserving all other config fields.
pub fn remove_repo(path: &str) {
    let mut v = read_config_value();
    if let Some(a) = v.as_object_mut()
        .and_then(|o| o.get_mut("repos"))
        .and_then(|r| r.as_array_mut())
    {
        a.retain(|p| p.as_str() != Some(path));
    }
    let _ = write_config_value(&v);
}

/// Set the `"armed"` flag, preserving the `"repos"` list and all other fields.
/// Returns the write result so callers can surface a failed persist instead of
/// reporting a possibly-false success on this safety-critical path.
pub fn set_armed(armed: bool) -> std::io::Result<()> {
    let mut v = read_config_value();
    if let Some(o) = v.as_object_mut() {
        o.insert("armed".into(), serde_json::json!(armed));
    }
    write_config_value(&v)
}

// --- Pairing credentials (Phase 2a auth) ---
// Stored under an `"auth"` object in agent.json: { refreshToken, account }. The
// refresh token is the durable secret (rotated on every /auth/refresh); the
// session JWT is never persisted (always re-minted from the refresh token).

/// The persisted refresh token, or None if this agent isn't paired yet.
pub fn load_refresh_token() -> Option<String> {
    read_config_value()
        .get("auth")?
        .get("refreshToken")?
        .as_str()
        .map(String::from)
}

/// The account this agent is paired to (empty string if unknown).
pub fn load_account() -> String {
    read_config_value()
        .get("auth")
        .and_then(|a| a.get("account"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

/// The device id captured at pairing (stable across later ioreg flakiness).
pub fn load_device_id() -> Option<String> {
    read_config_value()
        .get("auth")?
        .get("deviceId")?
        .as_str()
        .map(String::from)
}

/// At pairing: persist creds (refresh token + account + the pairing-time device
/// id) AND the local disarmed flag in ONE atomic write — so there is never a
/// window where auth exists without the disarm gate engaged.
pub fn complete_pairing(refresh_token: &str, account: &str, device_id: &str) -> std::io::Result<()> {
    let mut v = read_config_value();
    if let Some(o) = v.as_object_mut() {
        o.insert(
            "auth".into(),
            serde_json::json!({ "refreshToken": refresh_token, "account": account, "deviceId": device_id }),
        );
        o.insert("armed".into(), serde_json::json!(false));
    }
    write_config_value(&v)
}

/// On rotation: update ONLY the refresh token, preserving account/deviceId/armed.
/// Errors if there is no `auth` object (not paired) so a lost rotation fails loud.
pub fn update_refresh_token(refresh_token: &str) -> std::io::Result<()> {
    let mut v = read_config_value();
    if let Some(auth) = v.get_mut("auth").and_then(|a| a.as_object_mut()) {
        auth.insert("refreshToken".into(), serde_json::json!(refresh_token));
        return write_config_value(&v);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no auth object to update — agent is not paired",
    ))
}

pub fn device_name() -> String {
    // macOS friendly name ("Ash's MacBook Pro"); fall back to hostname.
    std::process::Command::new("scutil").args(["--get", "ComputerName"]).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::process::Command::new("hostname").output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Mac".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The config helpers resolve `$HOME` at call time, so the tests that point
    // HOME at a temp dir must not run concurrently with each other. A single
    // combined test holding this guard keeps the env mutation serialized; the
    // guard also protects against any future HOME-touching tests in this file.
    static HOME_GUARD: Mutex<()> = Mutex::new(());

    fn read_raw() -> serde_json::Value {
        let text = std::fs::read_to_string(config_path()).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn config_mutations_preserve_fields_and_dedup_and_write_atomically() {
        let _guard = HOME_GUARD.lock().unwrap();
        let home = std::env::temp_dir().join(format!("gitit-repos-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        // Serialized by HOME_GUARD so this env mutation can't race other tests.
        std::env::set_var("HOME", &home);

        // Seed a config carrying an unrelated field plus armed=false, so we can
        // confirm mutations never clobber sibling keys.
        let cfg = config_path();
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, r#"{"armed": false, "note": "keep me", "repos": ["/a"]}"#).unwrap();

        // add_repo preserves `armed`, `note`, and the existing entry.
        add_repo("/b");
        let v = read_raw();
        assert_eq!(v["armed"], serde_json::json!(false));
        assert_eq!(v["note"], serde_json::json!("keep me"));
        assert_eq!(v["repos"], serde_json::json!(["/a", "/b"]));

        // add_repo dedups — adding /b again is a no-op.
        add_repo("/b");
        assert_eq!(read_raw()["repos"], serde_json::json!(["/a", "/b"]));

        // set_armed flips the flag WITHOUT clobbering `repos` or `note`.
        set_armed(true).unwrap();
        let v = read_raw();
        assert_eq!(v["armed"], serde_json::json!(true));
        assert_eq!(v["repos"], serde_json::json!(["/a", "/b"]));
        assert_eq!(v["note"], serde_json::json!("keep me"));
        assert!(armed());

        // remove_repo drops one entry, preserves the rest + `armed` + `note`.
        remove_repo("/a");
        let v = read_raw();
        assert_eq!(v["repos"], serde_json::json!(["/b"]));
        assert_eq!(v["armed"], serde_json::json!(true));
        assert_eq!(v["note"], serde_json::json!("keep me"));

        // No leftover temp file after an atomic write.
        assert!(!cfg.with_extension("json.tmp").exists());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn mutations_on_a_non_object_config_recover_to_an_object() {
        let _guard = HOME_GUARD.lock().unwrap();
        let home = std::env::temp_dir().join(format!("gitit-repos-test2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        // Serialized by HOME_GUARD so this env mutation can't race other tests.
        std::env::set_var("HOME", &home);

        // A garbage/array config must not make the mutation silently no-op.
        let cfg = config_path();
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, r#"["/legacy"]"#).unwrap();

        add_repo("/fresh");
        let v = read_raw();
        assert!(v.is_object(), "config recovered to an object: {v}");
        assert_eq!(v["repos"], serde_json::json!(["/fresh"]));

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn auth_creds_roundtrip_and_preserve_repos_armed() {
        let _guard = HOME_GUARD.lock().unwrap();
        let home = std::env::temp_dir().join(format!("gitit-repos-test3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);

        let cfg = config_path();
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, r#"{"repos": ["/a"]}"#).unwrap();

        // Unpaired agent: no refresh token, and armed() defaults TRUE (no auth).
        assert_eq!(load_refresh_token(), None);
        assert!(armed());

        // complete_pairing writes creds + deviceId + armed:false in one go,
        // preserving repos.
        complete_pairing("rt1", "acct_x", "dev-123").unwrap();
        assert_eq!(load_refresh_token().as_deref(), Some("rt1"));
        assert_eq!(load_account(), "acct_x");
        assert_eq!(load_device_id().as_deref(), Some("dev-123"));
        assert_eq!(read_raw()["repos"], serde_json::json!(["/a"]));
        assert!(!armed(), "freshly paired agent is disarmed");

        // Rotation updates only the refresh token; account/deviceId/repos intact.
        update_refresh_token("rt2").unwrap();
        assert_eq!(load_refresh_token().as_deref(), Some("rt2"));
        assert_eq!(load_account(), "acct_x");
        assert_eq!(load_device_id().as_deref(), Some("dev-123"));
        assert_eq!(read_raw()["repos"], serde_json::json!(["/a"]));

        // Arming preserves the auth object.
        set_armed(true).unwrap();
        assert_eq!(load_refresh_token().as_deref(), Some("rt2"));
        assert!(armed());

        // Paired but armed key removed -> fail-safe DISARMED (not the old true).
        std::fs::write(&cfg, r#"{"auth": {"refreshToken": "rt2", "account": "acct_x"}}"#).unwrap();
        assert!(!armed(), "paired-without-armed-key defaults disarmed");

        // The persisted file is owner-only (0600) on unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            complete_pairing("rt3", "acct_x", "dev-123").unwrap();
            let mode = std::fs::metadata(&cfg).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "agent.json must be owner-only");
        }

        let _ = std::fs::remove_dir_all(&home);
    }
}
