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

fn device_name() -> String {
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
