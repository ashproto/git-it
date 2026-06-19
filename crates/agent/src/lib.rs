//! Headless agent core: drives git-core on this machine. Transport-agnostic —
//! Phase 1a exposes a single `status_json` entry the CLI calls.

use git_core::{graph, path_setup};
use std::path::Path;

/// Return the repo's status as a JSON string, or an error message.
pub fn status_json(repo: &str) -> Result<String, String> {
    path_setup::ensure_homebrew_path();
    // `graph::repo_status` takes `&Path` (not `String`), so borrow the arg.
    let status = graph::repo_status(Path::new(repo)).map_err(|e| e.to_string())?;
    serde_json::to_string(&status).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init_temp_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gitit-agent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(&dir).output().unwrap();
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("a.txt"), "hi").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "first"]);
        dir
    }

    #[test]
    fn status_json_returns_branch_for_a_real_repo() {
        let dir = init_temp_repo();
        let out = status_json(dir.to_str().unwrap()).expect("status_json should succeed");
        // RepoStatus nests a HeadInfo { branch: Option<String> }; on a freshly
        // committed repo on `main` the serialized JSON includes the "branch" key.
        assert!(out.contains("\"branch\""), "status JSON should include a branch field: {out}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
