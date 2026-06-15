use crate::git_ops;
use crate::safety;
use crate::types::RewriteResult;
use std::path::Path;
use std::process::Command;

/// Move the current branch to `target`. mode = soft | mixed | hard.
pub fn reset(repo: &Path, target: &str, mode: &str, auto_backup: bool) -> Result<RewriteResult, String> {
    let flag = match mode {
        "soft" => "--soft",
        "mixed" => "--mixed",
        "hard" => "--hard",
        _ => return Err(format!("Unknown reset mode: {}", mode)),
    };
    let bundle = safety::maybe_backup(repo, auto_backup)?;
    let undo = safety::snapshot(repo, "reset")?;
    let mut c = Command::new("git");
    c.current_dir(repo).args(["reset", flag, "--end-of-options", target]);
    git_ops::run(&mut c)?;
    Ok(RewriteResult { undo, bundle })
}

/// Amend HEAD: optional new message, optional reset of author/committer date to now.
/// No file staging (Phase 5). With no message + no dates this just rewrites HEAD.
pub fn amend(
    repo: &Path,
    message: Option<&str>,
    reset_author_date: bool,
    reset_committer_date: bool,
    auto_backup: bool,
) -> Result<RewriteResult, String> {
    let bundle = safety::maybe_backup(repo, auto_backup)?;
    let undo = safety::snapshot(repo, "amend")?;
    let mut c = Command::new("git");
    c.current_dir(repo);
    // Clear any inherited date env vars so --date=now / current time are authoritative.
    if reset_author_date || reset_committer_date {
        c.env_remove("GIT_AUTHOR_DATE");
        c.env_remove("GIT_COMMITTER_DATE");
    }
    c.arg("commit").arg("--amend");
    match message {
        Some(m) => {
            c.arg("-m").arg(m);
        }
        None => {
            c.arg("--no-edit");
        }
    }
    if reset_author_date {
        c.arg("--date=now");
    }
    git_ops::run(&mut c)?;
    Ok(RewriteResult { undo, bundle })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    struct TempRepo { path: PathBuf }
    impl TempRepo {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!("gte-rw-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "Tester"]);
            r
        }
        fn git(&self, args: &[&str]) {
            let o = Command::new("git").current_dir(&self.path).args(args)
                .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
                .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
                .output().unwrap();
            assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
        }
        fn commit(&self, file: &str, contents: &str, msg: &str) {
            fs::write(self.path.join(file), contents).unwrap();
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", msg]);
        }
        fn rev(&self, r: &str) -> String {
            let o = Command::new("git").current_dir(&self.path).args(["rev-parse", r]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        fn subject(&self, r: &str) -> String {
            let o = Command::new("git").current_dir(&self.path).args(["log","-1","--format=%s", r]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
    }
    impl Drop for TempRepo { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); } }

    #[test]
    fn reset_hard_moves_branch_and_undo_restores() {
        let r = TempRepo::new();
        r.commit("f", "1", "A");
        let a = r.rev("HEAD");
        r.commit("f", "2", "B");
        let res = reset(&r.path, &a, "hard", false).unwrap();
        assert_eq!(r.rev("HEAD"), a, "hard reset moved HEAD to A");
        assert_eq!(res.undo.sha, r.rev("HEAD@{1}"), "snapshot captured pre-reset sha");
        // Undo restores B.
        safety::restore(&r.path, &res.undo.sha).unwrap();
        assert_eq!(r.subject("HEAD"), "B");
    }

    #[test]
    fn reset_soft_keeps_tree() {
        let r = TempRepo::new();
        r.commit("f", "1", "A");
        let a = r.rev("HEAD");
        r.commit("f", "2", "B");
        reset(&r.path, &a, "soft", false).unwrap();
        assert_eq!(r.rev("HEAD"), a);
        // working tree still has "2" (soft keeps the index + tree) — content preserved.
        assert_eq!(fs::read_to_string(r.path.join("f")).unwrap(), "2");
    }

    #[test]
    fn amend_changes_message() {
        let r = TempRepo::new();
        r.commit("f", "1", "original");
        let before = r.rev("HEAD");
        amend(&r.path, Some("reworded"), false, false, false).unwrap();
        assert_eq!(r.subject("HEAD"), "reworded");
        assert_ne!(r.rev("HEAD"), before, "amend creates a new commit");
    }

    #[test]
    fn amend_reset_author_date_changes_it() {
        let r = TempRepo::new();
        r.commit("f", "1", "msg");
        let old_ad = {
            let o = Command::new("git").current_dir(&r.path).args(["log","-1","--format=%aI"]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };
        amend(&r.path, None, true, true, false).unwrap();
        let new_ad = {
            let o = Command::new("git").current_dir(&r.path).args(["log","-1","--format=%aI"]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };
        assert_ne!(old_ad, new_ad, "author date should change to now");
    }

    #[test]
    fn reset_rejects_bad_mode() {
        let r = TempRepo::new();
        r.commit("f", "1", "A");
        assert!(reset(&r.path, "HEAD", "nuke", false).is_err());
    }

    #[test]
    fn reset_target_cannot_be_option() {
        // A target that looks like an option must be treated as a (bad) revision, not a flag.
        let r = TempRepo::new();
        r.commit("f", "1", "A");
        assert!(reset(&r.path, "--hard", "soft", false).is_err(),
            "--end-of-options must stop '--hard' from being parsed as a flag");
    }
}
