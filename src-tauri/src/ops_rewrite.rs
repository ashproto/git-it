use crate::git_ops;
use crate::ops_merge;
use crate::safety;
use crate::types::{RebaseOutcome, RebaseStep, ReflogEntry, RewriteResult};
use std::fs;
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

/// Rebase the current branch onto `onto` (non-interactive).
pub fn rebase(repo: &Path, onto: &str, auto_backup: bool) -> Result<RebaseOutcome, String> {
    let bundle = safety::maybe_backup(repo, auto_backup)?;
    let undo = safety::snapshot(repo, "rebase")?;
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_EDITOR", "true")
        .arg("rebase")
        .arg("--end-of-options")
        .arg(onto);
    let (ok, msg) = ops_merge::run_status(&mut c)?;
    let outcome = ops_merge::outcome_for(repo, "rebase", ok, msg)?;
    Ok(RebaseOutcome { outcome, undo, bundle })
}

/// The commits an interactive rebase from `base` would edit (base..HEAD), oldest first.
/// Reuses the ReflogEntry shape {sha, short, selector(unused), subject}.
pub fn rebase_todo_preview(repo: &Path, base: &str) -> Result<Vec<ReflogEntry>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args([
        "log", "--reverse", "--format=%H%x1f%h%x1f%s", "--end-of-options",
        &format!("{}..HEAD", base),
    ]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out.lines().filter(|l| !l.is_empty()).map(|l| {
        let mut p = l.splitn(3, '\u{1f}');
        ReflogEntry {
            sha: p.next().unwrap_or("").to_string(),
            short: p.next().unwrap_or("").to_string(),
            selector: String::new(),
            subject: p.next().unwrap_or("").to_string(),
        }
    }).collect())
}

const REBASE_ACTIONS: &[&str] = &["pick", "reword", "edit", "squash", "fixup", "drop"];

/// Interactive rebase driven entirely non-interactively. `base` is the commit BELOW the
/// edited range (git rebase -i <base> edits base..HEAD). `steps` are in final todo order.
/// reword becomes `pick` + a git-rebase run-command line (`x git commit --amend -F <file>`)
/// so messages live in files (no editor, no squash-group alignment problem). squash/fixup
/// keep git's default combined message via GIT_EDITOR=true.
pub fn rebase_interactive(repo: &Path, base: &str, steps: &[RebaseStep], auto_backup: bool) -> Result<RebaseOutcome, String> {
    for s in steps {
        if !REBASE_ACTIONS.contains(&s.action.as_str()) {
            return Err(format!("Invalid rebase action: {}", s.action));
        }
    }
    let bundle = safety::maybe_backup(repo, auto_backup)?;
    let undo = safety::snapshot(repo, "rebase")?;

    // Scratch dir for the todo, the seq-editor script, and reword message files.
    let suffix = undo.sha.get(0..7).unwrap_or("x");
    let dir = std::env::temp_dir().join(format!("gte-rebase-{}-{}", std::process::id(), suffix));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("scratch dir: {}", e))?;

    // Build the todo. reword -> pick + a run-command line that amends from a message file.
    let mut todo = String::new();
    let mut msg_i = 0usize;
    for s in steps {
        match s.action.as_str() {
            "reword" => {
                todo.push_str(&format!("pick {}\n", s.sha));
                let mf = dir.join(format!("msg-{}", msg_i));
                fs::write(&mf, s.message.clone().unwrap_or_default()).map_err(|e| format!("msg file: {}", e))?;
                // `x` is git's documented shorthand for the run-command todo verb.
                todo.push_str(&format!("x git commit --amend -F '{}'\n", mf.display()));
                msg_i += 1;
            }
            other => {
                todo.push_str(&format!("{} {}\n", other, s.sha));
            }
        }
    }
    let todo_path = dir.join("todo");
    fs::write(&todo_path, &todo).map_err(|e| format!("todo: {}", e))?;

    // GIT_SEQUENCE_EDITOR script: copy our todo over the file git passes ($1).
    let seq = dir.join("seq-editor.sh");
    fs::write(&seq, format!("#!/bin/sh\ncp '{}' \"$1\"\n", todo_path.display()))
        .map_err(|e| format!("seq script: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&seq, fs::Permissions::from_mode(0o755)).ok();
    }

    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_SEQUENCE_EDITOR", &seq)
        .env("GIT_EDITOR", "true")
        .arg("rebase")
        .arg("-i")
        .arg("--end-of-options")
        .arg(base);
    let (ok, msg) = ops_merge::run_status(&mut c)?;
    let outcome = ops_merge::outcome_for(repo, "rebase", ok, msg)?;
    let _ = fs::remove_dir_all(&dir); // best-effort cleanup
    Ok(RebaseOutcome { outcome, undo, bundle })
}

/// `git reflog` for HEAD, most-recent first.
pub fn reflog(repo: &Path, limit: u32) -> Result<Vec<ReflogEntry>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args([
        "reflog",
        &format!("--max-count={}", limit),
        "--format=%H%x1f%h%x1f%gd%x1f%gs",
    ]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out.lines().filter(|l| !l.is_empty()).map(|l| {
        let mut p = l.splitn(4, '\u{1f}');
        ReflogEntry {
            sha: p.next().unwrap_or("").to_string(),
            short: p.next().unwrap_or("").to_string(),
            selector: p.next().unwrap_or("").to_string(),
            subject: p.next().unwrap_or("").to_string(),
        }
    }).collect())
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

    #[test]
    fn rebase_onto_replays_commits() {
        let r = TempRepo::new();
        r.commit("base", "0", "base");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.commit("feat", "1", "feature work");
        r.git(&["checkout", "-q", "main"]);
        r.commit("main", "2", "main work");
        r.git(&["checkout", "-q", "feature"]);
        let out = rebase(&r.path, "main", false).unwrap();
        assert!(!out.outcome.conflicted);
        assert!(r.path.join("main").exists(), "feature now sits on top of main");
    }

    #[test]
    fn rebase_conflict_then_abort_recovers() {
        let r = TempRepo::new();
        r.commit("f", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.commit("f", "feature\n", "feature edit");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f", "main\n", "main edit");
        r.git(&["checkout", "-q", "feature"]);
        let out = rebase(&r.path, "main", false).unwrap();
        assert!(out.outcome.conflicted, "diverging edits must conflict");
        crate::ops_merge::abort(&r.path, "rebase").unwrap(); // op_subcommand now knows rebase
    }

    #[test]
    fn interactive_drop_removes_commit() {
        let r = TempRepo::new();
        r.commit("a", "1", "A");
        let base = r.rev("HEAD");
        r.commit("b", "2", "B");
        let b = r.rev("HEAD");
        r.commit("c", "3", "C");
        let c_sha = r.rev("HEAD");
        let steps = vec![
            RebaseStep { action: "drop".into(), sha: b.clone(), message: None },
            RebaseStep { action: "pick".into(), sha: c_sha.clone(), message: None },
        ];
        let out = rebase_interactive(&r.path, &base, &steps, false).unwrap();
        assert!(!out.outcome.conflicted);
        assert!(!r.path.join("b").exists(), "dropped commit's file should be gone");
        assert!(r.path.join("c").exists());
    }

    #[test]
    fn interactive_reword_changes_message() {
        let r = TempRepo::new();
        r.commit("a", "1", "A");
        let base = r.rev("HEAD");
        r.commit("b", "2", "old B message");
        let b = r.rev("HEAD");
        let steps = vec![
            RebaseStep { action: "reword".into(), sha: b.clone(), message: Some("new B message".into()) },
        ];
        let out = rebase_interactive(&r.path, &base, &steps, false).unwrap();
        assert!(!out.outcome.conflicted);
        assert_eq!(r.subject("HEAD"), "new B message");
    }

    #[test]
    fn interactive_rejects_bad_action() {
        let r = TempRepo::new();
        r.commit("a", "1", "A");
        let base = r.rev("HEAD");
        r.commit("b", "2", "B");
        let b = r.rev("HEAD");
        let steps = vec![ RebaseStep { action: "bogus".into(), sha: b, message: None } ];
        assert!(rebase_interactive(&r.path, &base, &steps, false).is_err(),
            "non-whitelisted actions must be rejected before any git invocation");
    }

    #[test]
    fn reflog_lists_entries() {
        let r = TempRepo::new();
        r.commit("a", "1", "A");
        r.commit("a", "2", "B");
        let log = reflog(&r.path, 10).unwrap();
        assert!(log.len() >= 2);
        assert!(log[0].selector.starts_with("HEAD@{"));
    }
}
