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

/// Read `"armed"` from the config; default true if absent/unparsable.
pub fn armed() -> bool {
    std::fs::read_to_string(config_path()).ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("armed").and_then(|a| a.as_bool()))
        .unwrap_or(true)
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
    let path = config_path();
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(v)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path) // atomic replace
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
}
