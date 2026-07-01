use crate::git_ops;
use std::path::Path;
use std::process::Command;

/// Switch to a branch, or check out a commit (detached HEAD). Git refuses if the
/// working tree has conflicting local changes; that refusal surfaces as Err.
pub fn checkout(repo: &Path, target: &str) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        // --end-of-options so a target like "-f" can't smuggle in a flag that
        // would discard uncommitted work.
        .args(["checkout", "--end-of-options", target]);
    let (o, e) = git_ops::run(&mut c)?;
    Ok(format!("{}{}", o, e).trim().to_string())
}

/// Create a branch at `start_point` without switching to it.
pub fn create_branch(repo: &Path, name: &str, start_point: &str) -> Result<(), String> {
    let mut c = Command::new("git");
    // `--` so an option-like name (e.g. "-D") is treated as an operand, not a flag.
    c.current_dir(repo).args(["branch", "--", name, start_point]);
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn rename_branch(repo: &Path, old: &str, new: &str) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["branch", "-m", "--", old, new]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Delete a branch. `force` uses -D (drops even unmerged commits); without it,
/// -d refuses to delete a branch whose commits aren't merged into HEAD. When
/// `delete_remote` is set and a remote/branch is given, ALSO delete the upstream
/// branch via `git push <remote> --delete`. The local delete runs first; if it
/// fails the remote delete is skipped. If the local succeeds but the remote delete
/// fails, return an Err describing the partial outcome.
pub fn delete_branch(
    repo: &Path,
    name: &str,
    force: bool,
    delete_remote: bool,
    remote: Option<&str>,
    remote_branch: Option<&str>,
) -> Result<(), String> {
    let flag = if force { "-D" } else { "-d" };
    let mut c = Command::new("git");
    c.current_dir(repo).args(["branch", flag, "--", name]);
    git_ops::run(&mut c)?;

    if delete_remote {
        if let (Some(rem), Some(rb)) = (remote, remote_branch) {
            let mut p = Command::new("git");
            p.current_dir(repo)
                .env("GIT_TERMINAL_PROMPT", "0")
                // Options first, then --end-of-options, so BOTH the remote name and
                // the branch operand are guarded against leading-dash flag injection.
                .args(["push", "--delete", "--end-of-options", rem, rb]);
            git_ops::run(&mut p)
                .map_err(|e| format!("Deleted local branch, but remote delete failed: {}", e))?;
        }
    }
    Ok(())
}

/// Create a tag at `target`. With a message it's an annotated tag; otherwise lightweight.
pub fn create_tag(repo: &Path, name: &str, target: &str, message: Option<&str>) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("tag");
    if let Some(m) = message {
        c.args(["-a", "-m", m]);
    }
    c.args(["--", name, target]);
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn delete_tag(repo: &Path, name: &str) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["tag", "-d", name]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Fast-forward a LOCAL branch to its remote tracking tip WITHOUT checking it out,
/// using `git fetch <remote> <branch>:<branch>`. Git rejects a non-fast-forward
/// update, so a diverged branch fails cleanly and leaves the ref untouched. Git
/// also refuses to update the branch that is currently checked out — callers only
/// offer this for non-current branches.
pub fn fast_forward_branch(repo: &Path, branch: &str, remote: &str) -> Result<String, String> {
    let refspec = format!("{}:{}", branch, branch);
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["fetch", "--end-of-options", remote, &refspec]);
    let (o, e) = git_ops::run(&mut c)?;
    Ok(format!("{}{}", o, e).trim().to_string())
}

/// Fetch from a remote (or all remotes when None), pruning deleted remote refs.
pub fn fetch(repo: &Path, remote: Option<&str>) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["fetch", "--prune"]);
    if let Some(r) = remote {
        // --end-of-options so a remote like "--upload-pack=<cmd>" can't inject a flag
        // (that vector is arbitrary command execution).
        c.arg("--end-of-options");
        c.arg(r);
    }
    let (o, e) = git_ops::run(&mut c)?;
    Ok(format!("{}{}", o, e).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_dir(prefix: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("gte-{}-{}-{}", prefix, std::process::id(), id));
        let _ = fs::remove_dir_all(&p);
        p
    }

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let path = unique_dir("ops");
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

        fn commit(&self, file: &str, msg: &str) {
            fs::write(self.path.join(file), msg).unwrap();
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", msg]);
        }

        fn current_branch(&self) -> String {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(["symbolic-ref", "--short", "-q", "HEAD"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }

        fn rev(&self, refname: &str) -> String {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(["rev-parse", refname])
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }

        fn has_ref(&self, refname: &str) -> bool {
            Command::new("git")
                .current_dir(&self.path)
                .args(["rev-parse", "--verify", "-q", refname])
                .output()
                .unwrap()
                .status
                .success()
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn create_branch_and_checkout() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        create_branch(&r.path, "feat", "main").unwrap();
        assert!(r.has_ref("refs/heads/feat"));
        assert_eq!(r.rev("feat"), r.rev("main"));
        checkout(&r.path, "feat").unwrap();
        assert_eq!(r.current_branch(), "feat");
    }

    #[test]
    fn delete_branch_refuses_unmerged_then_forces() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.commit("b.txt", "B"); // feat now has a commit not in main
        r.git(&["checkout", "-q", "main"]);
        assert!(
            delete_branch(&r.path, "feat", false, false, None, None).is_err(),
            "safe delete must refuse an unmerged branch"
        );
        assert!(r.has_ref("refs/heads/feat"));
        delete_branch(&r.path, "feat", true, false, None, None).unwrap();
        assert!(!r.has_ref("refs/heads/feat"));
    }

    #[test]
    fn rename_branch_moves_ref() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        create_branch(&r.path, "feat", "main").unwrap();
        rename_branch(&r.path, "feat", "feature").unwrap();
        assert!(!r.has_ref("refs/heads/feat"));
        assert!(r.has_ref("refs/heads/feature"));
    }

    #[test]
    fn tags_create_lightweight_annotated_and_delete() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        create_tag(&r.path, "v1", "main", None).unwrap();
        assert!(r.has_ref("refs/tags/v1"));
        create_tag(&r.path, "v2", "main", Some("release 2")).unwrap();
        assert!(r.has_ref("refs/tags/v2"));
        delete_tag(&r.path, "v1").unwrap();
        assert!(!r.has_ref("refs/tags/v1"));
    }

    #[test]
    fn checkout_commit_detaches_head() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let first = r.rev("HEAD");
        r.commit("b.txt", "B");
        checkout(&r.path, &first).unwrap();
        assert_eq!(r.current_branch(), "", "checking out a commit should detach HEAD");
        assert_eq!(r.rev("HEAD"), first);
    }

    #[test]
    fn fetch_brings_new_remote_branch() {
        // Bare repo acts as the remote.
        let bare = unique_dir("bare");
        fs::create_dir_all(&bare).unwrap();
        Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .unwrap();

        // A working repo pushes main to the bare remote.
        let work = TempRepo::new();
        work.commit("a.txt", "A");
        work.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        work.git(&["push", "-q", "origin", "main"]);

        // A separate clone starts without the soon-to-be-pushed branch.
        let local = unique_dir("local");
        Command::new("git")
            .args(["clone", "-q", bare.to_str().unwrap(), local.to_str().unwrap()])
            .output()
            .unwrap();

        // The working repo publishes a new branch.
        work.git(&["branch", "feat", "main"]);
        work.git(&["push", "-q", "origin", "feat"]);

        // Fetch should bring the new remote-tracking branch into the clone.
        fetch(&local, None).unwrap();
        let has = Command::new("git")
            .current_dir(&local)
            .args(["rev-parse", "--verify", "-q", "refs/remotes/origin/feat"])
            .output()
            .unwrap()
            .status
            .success();
        assert!(has, "fetch --prune should bring origin/feat");

        let _ = fs::remove_dir_all(&bare);
        let _ = fs::remove_dir_all(&local);
    }

    // --- option-injection regressions (must fail safely) ---

    #[test]
    fn create_branch_rejects_option_like_name() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        r.git(&["checkout", "-q", "-b", "victim"]);
        r.commit("b.txt", "B"); // unmerged work that `git branch -D victim` would destroy
        r.git(&["checkout", "-q", "main"]);
        assert!(
            create_branch(&r.path, "-D", "victim").is_err(),
            "an option-like branch name must not be parsed as a flag"
        );
        assert!(r.has_ref("refs/heads/victim"), "victim branch must survive");
    }

    #[test]
    fn fetch_rejects_option_like_remote() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let marker = unique_dir("pwned");
        let payload = format!("--upload-pack=touch {}", marker.to_str().unwrap());
        let _ = fetch(&r.path, Some(&payload));
        assert!(!marker.exists(), "an option-injected --upload-pack payload must not run");
    }

    #[test]
    fn fast_forward_branch_advances_non_current_branch() {
        // Set up "origin": bare repo with main + c1.
        let bare = unique_dir("ff-bare");
        fs::create_dir_all(&bare).unwrap();
        Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .unwrap();

        // A working repo pushes c1 to origin.
        let upstream = TempRepo::new();
        upstream.commit("a.txt", "c1");
        upstream.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        upstream.git(&["push", "-q", "origin", "main"]);

        // Clone origin to get "clone" with main tracking origin/main.
        let clone_path = unique_dir("ff-clone");
        Command::new("git")
            .args(["clone", "-q", bare.to_str().unwrap(), clone_path.to_str().unwrap()])
            .output()
            .unwrap();

        // Advance origin: add c2 and push.
        upstream.commit("b.txt", "c2");
        upstream.git(&["push", "-q", "origin", "main"]);

        // In clone: switch to a second branch "work" so main is not checked out.
        let clone = TempRepo { path: clone_path.clone() };
        clone.git(&["config", "user.email", "t@example.com"]);
        clone.git(&["config", "user.name", "Tester"]);
        clone.git(&["checkout", "-q", "-b", "work"]);

        // Fetch so origin/main is updated.
        fetch(&clone.path, Some("origin")).unwrap();

        let before = clone.rev("main");
        let origin_main = clone.rev("origin/main");
        assert_ne!(before, origin_main, "main should be behind origin/main before ff");

        // Fast-forward main to origin/main without checking it out.
        let res = fast_forward_branch(&clone.path, "main", "origin");
        assert!(res.is_ok(), "ff failed: {:?}", res);

        let after = clone.rev("main");
        assert_ne!(before, after, "main should have advanced");
        assert_eq!(after, origin_main, "main should now equal origin/main");

        let _ = fs::remove_dir_all(&bare);
        let _ = fs::remove_dir_all(&clone_path);
    }

    #[test]
    fn delete_branch_can_also_delete_the_remote() {
        // origin = bare repo; clone -> work
        let bare = unique_dir("del-remote-bare");
        fs::create_dir_all(&bare).unwrap();
        Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .unwrap();

        let work = TempRepo::new();
        work.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        work.commit("a.txt", "c1");
        work.git(&["push", "-q", "origin", "HEAD:main"]);

        // Create and push feature branch.
        work.git(&["checkout", "-q", "-b", "feature"]);
        work.commit("b.txt", "c2");
        work.git(&["push", "-q", "origin", "feature"]);

        // Go back to main so feature is not checked out.
        work.git(&["checkout", "-q", "main"]);

        // Delete local + remote feature.
        let res = delete_branch(&work.path, "feature", true, true, Some("origin"), Some("feature"));
        assert!(res.is_ok(), "{:?}", res);

        // Local branch must be gone.
        assert!(!work.has_ref("refs/heads/feature"), "local feature must be deleted");

        // Remote (bare) must not have refs/heads/feature.
        let has_remote = Command::new("git")
            .current_dir(&bare)
            .args(["rev-parse", "--verify", "-q", "refs/heads/feature"])
            .output()
            .unwrap()
            .status
            .success();
        assert!(!has_remote, "origin must not have refs/heads/feature after remote delete");

        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn checkout_rejects_option_like_target() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        fs::write(r.path.join("a.txt"), "DIRTY").unwrap();
        let _ = checkout(&r.path, "-f");
        assert_eq!(
            fs::read_to_string(r.path.join("a.txt")).unwrap(),
            "DIRTY",
            "an option-like checkout target must not discard uncommitted work"
        );
    }
}
