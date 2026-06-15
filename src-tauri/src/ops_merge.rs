use crate::git_ops;
use crate::types::OpOutcome;
use std::path::Path;
use std::process::Command;

/// Run a git command, returning (success, combined-output) instead of erroring on
/// a non-zero exit — so we can tell a *conflict* (expected, exit 1) apart from a
/// hard error (bad ref, exit 128).
fn run_status(cmd: &mut Command) -> Result<(bool, String), String> {
    let out = cmd.output().map_err(|e| format!("Failed to spawn command: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((out.status.success(), format!("{}{}", stdout, stderr).trim().to_string()))
}

/// Paths with unresolved merge conflicts.
pub fn conflicted_files(repo: &Path) -> Result<Vec<String>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["diff", "--name-only", "--diff-filter=U"]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out.lines().map(|s| s.to_string()).filter(|s| !s.is_empty()).collect())
}

/// Worktree-aware existence check for a git control file (e.g. CHERRY_PICK_HEAD).
fn control_file_exists(repo: &Path, name: &str) -> bool {
    if let Ok(out) = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--git-path", name])
        .output()
    {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                let pb = std::path::PathBuf::from(&p);
                return if pb.is_absolute() { pb } else { repo.join(pb) }.exists();
            }
        }
    }
    false
}

/// A cherry-pick/revert sequence is still mid-flight (e.g. stuck on an empty commit).
fn sequencer_in_progress(repo: &Path, sub: &str) -> bool {
    let head = if sub == "revert" { "REVERT_HEAD" } else { "CHERRY_PICK_HEAD" };
    control_file_exists(repo, head) || control_file_exists(repo, "sequencer")
}

/// Interpret a finished op. `sub` is the git subcommand ("merge"|"cherry-pick"|"revert").
/// Returns: clean success; an expected conflict (with files); or — for a cherry-pick/
/// revert that stopped on an EMPTY (already-applied) commit, which otherwise strands
/// the repo mid-sequence — auto-skip that commit and report a clean outcome; else Err.
fn outcome_for(repo: &Path, sub: &str, ok: bool, message: String) -> Result<OpOutcome, String> {
    if ok {
        return Ok(OpOutcome { conflicted: false, files: vec![], message });
    }
    let files = conflicted_files(repo)?;
    if !files.is_empty() {
        return Ok(OpOutcome { conflicted: true, files, message });
    }
    if (sub == "cherry-pick" || sub == "revert") && sequencer_in_progress(repo, sub) {
        // Non-zero, no conflicts, but the sequence is stuck — an empty/no-op commit.
        // Skip it so the user isn't stranded mid-operation.
        let mut skip = Command::new("git");
        skip.current_dir(repo).args([sub, "--skip"]);
        if git_ops::run(&mut skip).is_ok() {
            return Ok(OpOutcome {
                conflicted: false,
                files: vec![],
                message: format!("Skipped an empty commit. {}", message.lines().next().unwrap_or("")),
            });
        }
    }
    // Genuine failure (bad ref, dirty tree, …).
    Err(message)
}

/// Merge `reference` into the current branch.
pub fn merge(repo: &Path, reference: &str, no_ff: bool, squash: bool) -> Result<OpOutcome, String> {
    if no_ff && squash {
        return Err("Choose either a squash merge or a no-ff merge, not both.".to_string());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).env("GIT_TERMINAL_PROMPT", "0").arg("merge");
    if no_ff {
        c.arg("--no-ff");
    }
    if squash {
        c.arg("--squash");
    }
    c.arg("--end-of-options").arg(reference);
    let (ok, msg) = run_status(&mut c)?;
    outcome_for(repo, "merge", ok, msg)
}

/// Cherry-pick one or more commits onto the current branch.
pub fn cherry_pick(repo: &Path, shas: &[String]) -> Result<OpOutcome, String> {
    if shas.is_empty() {
        return Err("No commits to cherry-pick.".to_string());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).arg("cherry-pick").arg("--end-of-options");
    for s in shas {
        c.arg(s);
    }
    let (ok, msg) = run_status(&mut c)?;
    outcome_for(repo, "cherry-pick", ok, msg)
}

/// Revert one or more commits (creating new commits that undo them).
pub fn revert(repo: &Path, shas: &[String]) -> Result<OpOutcome, String> {
    if shas.is_empty() {
        return Err("No commits to revert.".to_string());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).arg("revert").arg("--no-edit").arg("--end-of-options");
    for s in shas {
        c.arg(s);
    }
    let (ok, msg) = run_status(&mut c)?;
    outcome_for(repo, "revert", ok, msg)
}

fn op_subcommand(kind: &str) -> Result<&'static str, String> {
    match kind {
        "merge" => Ok("merge"),
        "cherry-pick" => Ok("cherry-pick"),
        "revert" => Ok("revert"),
        _ => Err(format!("Unknown operation: {}", kind)),
    }
}

/// Abort an in-progress merge/cherry-pick/revert, restoring the pre-op state.
pub fn abort(repo: &Path, kind: &str) -> Result<(), String> {
    let sub = op_subcommand(kind)?;
    let mut c = Command::new("git");
    c.current_dir(repo).args([sub, "--abort"]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Continue an in-progress op after conflicts are resolved + staged.
pub fn continue_op(repo: &Path, kind: &str) -> Result<OpOutcome, String> {
    let sub = op_subcommand(kind)?;
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_EDITOR", "true") // accept the default message, don't open an editor
        .args([sub, "--continue"]);
    let (ok, msg) = run_status(&mut c)?;
    outcome_for(repo, sub, ok, msg)
}

/// Resolve a conflicted file by taking one side wholesale, then staging it.
/// `ours` = the current branch's version; otherwise the incoming version.
pub fn resolve_side(repo: &Path, path: &str, ours: bool) -> Result<(), String> {
    let flag = if ours { "--ours" } else { "--theirs" };
    let mut co = Command::new("git");
    co.current_dir(repo).args(["checkout", flag, "--", path]);
    git_ops::run(&mut co)?;
    let mut add = Command::new("git");
    add.current_dir(repo).args(["add", "--", path]);
    git_ops::run(&mut add)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("gte-merge-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "Tester"]);
            r
        }

        fn git(&self, args: &[&str]) {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(args)
                .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
                .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
                .output()
                .unwrap();
            assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
        }

        fn write(&self, file: &str, contents: &str) {
            fs::write(self.path.join(file), contents).unwrap();
        }

        fn commit(&self, file: &str, contents: &str, msg: &str) {
            self.write(file, contents);
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", msg]);
        }

        fn rev(&self, refname: &str) -> String {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(["rev-parse", refname])
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }

        fn read(&self, file: &str) -> String {
            fs::read_to_string(self.path.join(file)).unwrap()
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn merge_clean_is_not_conflicted() {
        let r = TempRepo::new();
        r.commit("base.txt", "base", "base");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.commit("feature.txt", "f", "feature work"); // touches a different file
        r.git(&["checkout", "-q", "main"]);
        let out = merge(&r.path, "feature", false, false).unwrap();
        assert!(!out.conflicted);
        assert!(out.files.is_empty());
    }

    #[test]
    fn merge_conflict_reports_files_then_aborts() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.commit("f.txt", "feature-change\n", "feature edit");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "main-change\n", "main edit"); // same file, diverged
        let out = merge(&r.path, "feature", false, false).unwrap();
        assert!(out.conflicted, "diverging edits to the same file must conflict");
        assert_eq!(out.files, vec!["f.txt".to_string()]);
        // Abort restores a clean tree.
        abort(&r.path, "merge").unwrap();
        assert!(conflicted_files(&r.path).unwrap().is_empty());
    }

    #[test]
    fn resolve_ours_then_continue_completes_merge() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.commit("f.txt", "theirs\n", "feature edit");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "ours\n", "main edit");
        let out = merge(&r.path, "feature", false, false).unwrap();
        assert!(out.conflicted);
        resolve_side(&r.path, "f.txt", true).unwrap(); // keep "ours"
        let done = continue_op(&r.path, "merge").unwrap();
        assert!(!done.conflicted, "merge should complete after resolving");
        assert_eq!(r.read("f.txt"), "ours\n");
        // A merge commit now exists (HEAD has two parents).
        let parents = {
            let o = Command::new("git")
                .current_dir(&r.path)
                .args(["rev-list", "--parents", "-n", "1", "HEAD"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).trim().split_whitespace().count()
        };
        assert_eq!(parents, 3, "merge commit = self + two parents");
    }

    #[test]
    fn cherry_pick_applies_commit() {
        let r = TempRepo::new();
        r.commit("base.txt", "base", "base");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.commit("new.txt", "hello", "add new.txt");
        let pick = r.rev("HEAD");
        r.git(&["checkout", "-q", "main"]);
        let out = cherry_pick(&r.path, &[pick]).unwrap();
        assert!(!out.conflicted);
        assert_eq!(r.read("new.txt"), "hello");
    }

    #[test]
    fn revert_undoes_commit() {
        let r = TempRepo::new();
        r.commit("f.txt", "v1\n", "v1");
        r.commit("f.txt", "v2\n", "v2");
        let last = r.rev("HEAD");
        let out = revert(&r.path, &[last]).unwrap();
        assert!(!out.conflicted);
        assert_eq!(r.read("f.txt"), "v1\n", "revert should restore the prior content");
    }

    #[test]
    fn empty_cherry_pick_recovers_not_stuck() {
        let r = TempRepo::new();
        r.commit("f.txt", "1\n", "A");
        r.commit("f.txt", "2\n", "B");
        let b = r.rev("HEAD");
        r.git(&["checkout", "-q", "-b", "feat"]); // feat is at B
        // Cherry-picking B onto feat is a no-op (already applied) → empty.
        let out = cherry_pick(&r.path, &[b]).unwrap();
        assert!(!out.conflicted);
        assert!(
            !sequencer_in_progress(&r.path, "cherry-pick"),
            "an empty cherry-pick must be skipped, not left stranded"
        );
    }

    #[test]
    fn continue_after_empty_resolution_recovers() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.commit("f.txt", "theirs\n", "feat edit");
        let pick = r.rev("HEAD");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "ours\n", "main edit");
        let out = cherry_pick(&r.path, &[pick]).unwrap();
        assert!(out.conflicted);
        resolve_side(&r.path, "f.txt", true).unwrap(); // take ours → net patch becomes empty
        let done = continue_op(&r.path, "cherry-pick").unwrap();
        assert!(!done.conflicted);
        assert!(
            !sequencer_in_progress(&r.path, "cherry-pick"),
            "an empty resolution must not strand the cherry-pick"
        );
        assert_eq!(r.read("f.txt"), "ours\n");
    }

    #[test]
    fn cherry_pick_conflict_reports_files() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.commit("f.txt", "feat\n", "feat edit");
        let pick = r.rev("HEAD");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "main\n", "main edit");
        let out = cherry_pick(&r.path, &[pick]).unwrap();
        assert!(out.conflicted, "diverging cherry-pick must conflict");
        assert_eq!(out.files, vec!["f.txt".to_string()]);
        abort(&r.path, "cherry-pick").unwrap();
        assert!(!sequencer_in_progress(&r.path, "cherry-pick"));
    }
}
