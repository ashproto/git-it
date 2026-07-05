use crate::git_ops;
use crate::types::UndoSnapshot;
use std::path::{Path, PathBuf};
use std::process::Command;

fn rev_parse(repo: &Path, what: &str) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["rev-parse", "--end-of-options", what]);
    let (out, _) = git_ops::run(&mut c)?;
    // git rev-parse --end-of-options echoes "--end-of-options" on its own line before the
    // resolved sha; take the last non-empty, non-flag line to get just the sha.
    let sha = out
        .lines().rfind(|l| !l.trim().is_empty() && !l.trim().starts_with('-'))
        .ok_or_else(|| format!("rev-parse produced no sha for {}", what))?
        .trim()
        .to_string();
    Ok(sha)
}

/// Current branch name, or None if HEAD is detached.
pub fn current_branch(repo: &Path) -> Result<Option<String>, String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .map_err(|e| format!("Failed to spawn git: {}", e))?;
    if out.status.success() {
        Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
    } else {
        Ok(None) // detached HEAD
    }
}

/// Snapshot HEAD before a destructive op, for one-click undo.
pub fn snapshot(repo: &Path, label: &str) -> Result<UndoSnapshot, String> {
    Ok(UndoSnapshot {
        branch: current_branch(repo)?,
        sha: rev_parse(repo, "HEAD")?,
        label: label.to_string(),
    })
}

/// Reverse a destructive op: move the current branch/HEAD back to the snapshot and
/// reset the working tree to it. (Immediate-undo semantics: any post-op changes are
/// discarded; the auto-backup bundle is the deeper net.)
pub fn restore(repo: &Path, sha: &str) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["reset", "--hard", "--end-of-options", sha]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Create a recovery bundle when auto_backup is on; returns its path string for the log.
pub fn maybe_backup(repo: &Path, auto_backup: bool) -> Result<Option<String>, String> {
    if auto_backup {
        let p: PathBuf = git_ops::create_bundle(repo)?;
        Ok(Some(p.to_string_lossy().into_owned()))
    } else {
        Ok(None)
    }
}
