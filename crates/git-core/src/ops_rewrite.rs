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

    // When asked to PRESERVE the committer date, capture it before the amend rewrites it.
    // `git commit --amend` always resets the committer date to now unless we pin it via
    // GIT_COMMITTER_DATE.  The author date is controlled by --date=now / no flag.
    let preserved_committer_date: Option<String> = if !reset_committer_date {
        let mut gc = Command::new("git");
        gc.current_dir(repo).args(["log", "-1", "--format=%cI", "HEAD"]);
        git_ops::run(&mut gc).ok().map(|(out, _)| out.trim().to_string())
    } else {
        None
    };

    let mut c = Command::new("git");
    c.current_dir(repo);

    // Pin the committer date to the pre-amend value when the flag is OFF.
    if let Some(ref cd) = preserved_committer_date {
        c.env("GIT_COMMITTER_DATE", cd);
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

/// Full commit message (%B) for a single commit. Read-only.
pub fn commit_message(repo: &Path, sha: &str) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["log", "-1", "--format=%B", "--end-of-options", sha]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out.trim_end_matches('\n').to_string())
}

/// Number of merge commits in `base..HEAD`. A reword replays that range with
/// `pick`, and git refuses a non-interactive `pick <merge-commit>`, so a reword
/// whose replay range crosses a merge must be rejected up-front (otherwise the
/// rebase fails and strands the repo mid-operation).
pub fn count_merges_in_range(repo: &Path, base: &str) -> Result<usize, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args([
        "rev-list",
        "--merges",
        "--count",
        "--end-of-options",
        &format!("{}..HEAD", base),
    ]);
    let (out, _) = git_ops::run(&mut c)?;
    out.trim()
        .parse::<usize>()
        .map_err(|e| format!("could not parse merge count: {}", e))
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

/// Shell-safe single-quote a path for use in generated sh scripts.
/// Replaces every `'` in the path with `'\''` so the result is safe inside `'…'`.
fn sq(p: &std::path::Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
}

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

    // Scratch dir: stable per-(process, repo) so msg files survive a mid-rebase
    // conflict pause and `--continue` can still find them.  Using a hash of the
    // canonical repo path keeps parallel test repos (each a different path) from
    // colliding even though they share a pid.
    // The leftover from a prior conflicted run is reclaimed here before recreating.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    repo.hash(&mut h);
    let repo_hash = h.finish();
    let dir = std::env::temp_dir().join(format!("gte-rebase-{}-{:x}", std::process::id(), repo_hash));
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
                todo.push_str(&format!("x git commit --amend -F {}\n", sq(&mf)));
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
    fs::write(&seq, format!("#!/bin/sh\ncp {} \"$1\"\n", sq(&todo_path)))
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
    // Only clean up when the rebase is FULLY finished (no .git/rebase-merge or
    // .git/rebase-apply directory). The scratch dir (with its msg-N files) must be
    // preserved across ANY mid-rebase pause — a conflict OR an `edit`/`break` stop —
    // so that a pending `x git commit --amend -F <file>` todo line can still complete
    // after `git rebase --continue`. The dir is reclaimed at the top of the NEXT call
    // to rebase_interactive.
    if !rebase_in_progress(repo) {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(RebaseOutcome { outcome, undo, bundle })
}

/// True while an interactive/standard rebase is mid-flight (conflict pause OR an
/// `edit`/`break` stop). git keeps `.git/rebase-merge` (interactive) or
/// `.git/rebase-apply` (am-based) until the rebase finishes or is aborted.
pub fn rebase_in_progress(repo: &Path) -> bool {
    repo.join(".git").join("rebase-merge").exists()
        || repo.join(".git").join("rebase-apply").exists()
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
    fn commit_message_returns_full_subject_and_body() {
        let r = TempRepo::new();
        // Two -m flags → git joins them with a blank line into one %B message
        // (subject on line 1, blank line, body on line 3).
        fs::write(r.path.join("f"), "1").unwrap();
        r.git(&["add", "."]);
        r.git(&["commit", "-q", "-m", "the subject", "-m", "the body line"]);
        let msg = commit_message(&r.path, "HEAD").unwrap();
        assert_eq!(msg, "the subject\n\nthe body line",
            "full message must include subject + blank line + body, not just the subject");
        // Sanity: the subject helper only sees the first line.
        assert_eq!(r.subject("HEAD"), "the subject");
    }

    #[test]
    fn count_merges_in_range_detects_a_merge() {
        let r = TempRepo::new();
        r.commit("f", "1", "A");
        let base = r.rev("HEAD");
        // Side branch B, then a --no-ff merge into main so base..HEAD has a merge.
        r.git(&["checkout", "-q", "-b", "side"]);
        r.commit("g", "1", "B");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f", "2", "C");
        r.git(&["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
        assert_eq!(count_merges_in_range(&r.path, &base).unwrap(), 1, "merge in base..HEAD");
        // A linear range from the merge tip onward has zero merges.
        let tip = r.rev("HEAD");
        r.commit("f", "3", "D");
        assert_eq!(count_merges_in_range(&r.path, &tip).unwrap(), 0, "linear range");
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

    // Fix 2: prove that reword message files survive a mid-rebase conflict so that
    // after `git rebase --continue` the reworded subject actually lands.
    //
    // Scenario (all commits touch file "f" except the last which adds file "g"):
    //   base  → A (f="a\n")  → B (f="b\n")  → C_orig (g="g\n")   [on main/HEAD]
    //
    // Interactive rebase of base..HEAD reorders so B replays before A:
    //   pick B, pick A, reword C_orig → "NEW C"
    //
    // Replaying B then A on the same file "f" guarantees a conflict (A was the
    // parent of B in the original history, so applying B first then A creates a
    // textual conflict on "f").  C touches only "g" so the reword step is safe.
    //
    // The test resolves the conflict in a bounded loop using continue_op, then
    // asserts HEAD subject == "NEW C".
    #[test]
    fn interactive_reword_after_conflict_survives_continue() {
        let r = TempRepo::new();
        // base commit — anchor point, no content we'll conflict on
        r.commit("base", "0\n", "base");
        let base = r.rev("HEAD");

        // A: sets f to "a\n"
        r.commit("f", "a\n", "A");
        let sha_a = r.rev("HEAD");

        // B: sets f to "b\n" (diverges from A's value)
        r.commit("f", "b\n", "B");
        let sha_b = r.rev("HEAD");

        // C: touches a different file so the reword step is conflict-free
        r.commit("g", "g\n", "C_orig");
        let sha_c = r.rev("HEAD");

        // Reorder: pick B first, then A — this conflicts on "f" because B's parent
        // recorded "a\n" but now its parent is base which has no "f" at all, so
        // applying B (patch: "" → "b\n") then A (patch: "a\n" → "b\n"… actually
        // A's patch is base→"a\n", B's patch is A→"b\n"; replaying B before A means
        // B's patch applies cleanly (base has no f, adds f="b\n"), but then A's patch
        // tries to set f="a\n" where B already wrote "b\n" → conflict on "f").
        let steps = vec![
            RebaseStep { action: "pick".into(),   sha: sha_b.clone(), message: None },
            RebaseStep { action: "pick".into(),   sha: sha_a.clone(), message: None },
            RebaseStep { action: "reword".into(), sha: sha_c.clone(), message: Some("NEW C".into()) },
        ];

        let out = rebase_interactive(&r.path, &base, &steps, false).unwrap();
        assert!(out.outcome.conflicted, "reordering B before A must conflict on file 'f'");

        // Resolve in a bounded loop: write a definitive "resolved\n" and continue.
        let mut final_outcome = out.outcome;
        for _ in 0..5 {
            if !final_outcome.conflicted { break; }
            // Write resolved content over the conflicted file.
            fs::write(r.path.join("f"), "resolved\n").unwrap();
            // Stage the resolution.
            let mut add = Command::new("git");
            add.current_dir(&r.path).args(["add", "--", "f"]);
            add.output().unwrap();
            // Continue the rebase.
            final_outcome = crate::ops_merge::continue_op(&r.path, "rebase").unwrap();
        }

        assert!(!final_outcome.conflicted, "rebase should complete after resolving the conflict");
        assert_eq!(r.subject("HEAD"), "NEW C",
            "reword message must survive the conflict pause (Fix 1: conditional cleanup)");
    }

    #[test]
    fn interactive_edit_pauses_and_keeps_rebase_in_progress() {
        // base → b → c on main; mark b as `edit` so the rebase stops at it.
        // The rebase should pause cleanly (not conflicted) but leave the rebase
        // in progress (.git/rebase-merge must still exist).
        let r = TempRepo::new();
        r.commit("a", "1", "A");
        let base = r.rev("HEAD");
        r.commit("b", "2", "B");
        let b = r.rev("HEAD");
        r.commit("c", "3", "C");
        let c_sha = r.rev("HEAD");
        let steps = vec![
            RebaseStep { action: "edit".into(), sha: b.clone(), message: None },
            RebaseStep { action: "pick".into(), sha: c_sha.clone(), message: None },
        ];
        let out = rebase_interactive(&r.path, &base, &steps, false).unwrap();
        assert!(!out.outcome.conflicted, "an edit stop is a clean pause, not a conflict");
        assert!(rebase_in_progress(&r.path), "rebase must still be in progress (paused at edit)");
        // clean up the paused rebase so the temp repo isn't left mid-rebase
        std::process::Command::new("git")
            .current_dir(&r.path)
            .args(["rebase", "--abort"])
            .status()
            .unwrap();
        assert!(!rebase_in_progress(&r.path), "abort ends the rebase");
    }

    // Fix 4: prove that amend(reset_author_date=false, reset_committer_date=false)
    // leaves both dates identical to the pre-amend values.
    #[test]
    fn amend_preserves_dates_when_flags_off() {
        let r = TempRepo::new();
        // commit() sets both dates to 2020-01-01T00:00:00+00:00 via env.
        r.commit("f", "1", "original");

        let get_date = |fmt: &str| -> String {
            let o = Command::new("git")
                .current_dir(&r.path)
                .args(["log", "-1", &format!("--format={}", fmt)])
                .output().unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };

        let before_ad = get_date("%aI");
        let before_cd = get_date("%cI");

        // Amend with both reset flags OFF — message changes, dates must not.
        amend(&r.path, Some("reworded"), false, false, false).unwrap();

        let after_ad = get_date("%aI");
        let after_cd = get_date("%cI");

        assert_eq!(before_ad, after_ad, "author date must be preserved when reset_author_date=false");
        assert_eq!(before_cd, after_cd, "committer date must be preserved when reset_committer_date=false (Fix 3)");
    }
}
