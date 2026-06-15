use crate::git_ops;
use crate::types::{RemoteInfo, RemoteOutcome};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Validate a remote name: no leading dash, conservative charset. Prevents a name
/// like "-x" or "--upload-pack=…" from being parsed as a git option.
fn valid_remote_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
}

/// List remotes (one entry per name, using the fetch URL).
pub fn remotes(repo: &Path) -> Result<Vec<RemoteInfo>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "-v"]);
    let (out, _) = git_ops::run(&mut c)?;
    let mut seen = std::collections::BTreeMap::new();
    for line in out.lines() {
        // "<name>\t<url> (fetch|push)"
        let mut parts = line.split('\t');
        let name = parts.next().unwrap_or("").trim().to_string();
        let rest = parts.next().unwrap_or("");
        let url = rest.split(" (").next().unwrap_or("").trim().to_string();
        if !name.is_empty() && line.ends_with("(fetch)") {
            seen.insert(name, url);
        }
    }
    Ok(seen.into_iter().map(|(name, url)| RemoteInfo { name, url }).collect())
}

pub fn remote_add(repo: &Path, name: &str, url: &str) -> Result<(), String> {
    if !valid_remote_name(name) {
        return Err(format!("Invalid remote name: {}", name));
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "add", name, url]); // name validated; url is positional-2
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn remote_remove(repo: &Path, name: &str) -> Result<(), String> {
    if !valid_remote_name(name) {
        return Err(format!("Invalid remote name: {}", name));
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "remove", name]);
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn remote_set_url(repo: &Path, name: &str, url: &str) -> Result<(), String> {
    if !valid_remote_name(name) {
        return Err(format!("Invalid remote name: {}", name));
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "set-url", name, url]);
    git_ops::run(&mut c)?;
    Ok(())
}

// ── Streaming pull/push/cancel ───────────────────────────────────────────────

/// Tauri-managed state for the in-flight remote operation child process.
/// Wrapped in Arc<RemoteState> so it can be moved into spawn_blocking.
#[derive(Default)]
pub struct RemoteState {
    pub child: Mutex<Option<Child>>,
}

/// Returns true if the git output looks like an authentication failure.
pub fn looks_like_auth_failure(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("authentication failed")
        || l.contains("could not read username")
        || l.contains("could not read password")
        || l.contains("permission denied")
        || l.contains("terminal prompts disabled")
        || l.contains("invalid username or password")
        || l.contains("fatal: authentication")
}

/// Run a git network op, streaming combined stdout+stderr to `on_line`, registering
/// the child in `state` so `cancel` can kill it. Returns (success, combined_output).
///
/// Concurrency model:
///   - Two reader threads drain stdout and stderr, sending each line over an mpsc channel.
///   - Both tx clones are moved into those threads; when both threads finish the channel closes.
///   - The calling thread drives the channel loop: calls `on_line` + accumulates output.
///   - The Child is stored in state.child AFTER spawning so cancel can reach it.
///   - After the channel closes, we take the child from state.child and call wait().
///     If cancel() already took it, the slot is None → we return (false, "Cancelled.").
///   - We never hold the mutex while blocking on I/O.
pub fn stream(
    mut cmd: Command,
    state: &RemoteState,
    on_line: &dyn Fn(String),
) -> Result<(bool, String), String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("spawn: {}", e))?;

    // Take the piped handles before storing the child.
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    // mpsc channel: both reader threads send lines to the calling thread.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let tx_err = tx.clone(); // second clone for the stderr thread

    // Spawn reader threads. Each thread owns its tx clone; when it exits the clone is dropped.
    let t_out = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
        // tx dropped here → one sender gone
    });
    let t_err = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx_err.send(line);
        }
        // tx_err dropped here → second sender gone
    });

    // Register the child so cancel() can take+kill it.
    // We do this AFTER spawning and taking the pipes (no mutex held during spawn/IO).
    {
        let mut g = state.child.lock().unwrap();
        *g = Some(child);
    }

    // Drain the channel on the calling thread until both reader threads finish
    // (channel closes when all tx clones are dropped — i.e. both threads exit).
    let mut accumulated = String::new();
    for line in rx {
        on_line(line.clone());
        accumulated.push_str(&line);
        accumulated.push('\n');
    }

    // Join reader threads (they are already done since the channel closed).
    let _ = t_out.join();
    let _ = t_err.join();

    // Take the child from the slot and wait for it.
    // If cancel() already took+killed it, the slot is None → report cancellation.
    let status = {
        let mut g = state.child.lock().unwrap();
        match g.take() {
            Some(mut c) => c.wait().map_err(|e| format!("wait: {}", e))?,
            None => return Ok((false, "Cancelled.".to_string())),
        }
    };

    Ok((status.success(), accumulated))
}

/// Classify the outcome of a finished remote op.
fn outcome(repo: &Path, ok: bool, msg: String) -> RemoteOutcome {
    if ok {
        return RemoteOutcome { ok: true, auth_failed: false, conflicted: false, message: msg };
    }
    if looks_like_auth_failure(&msg) {
        return RemoteOutcome { ok: false, auth_failed: true, conflicted: false, message: msg };
    }
    // A pull that stopped on conflicts leaves unmerged files / an op in progress.
    let conflicted = crate::ops_merge::conflicted_files(repo)
        .map(|f| !f.is_empty())
        .unwrap_or(false);
    RemoteOutcome { ok: false, auth_failed: false, conflicted, message: msg }
}

/// Pull from the configured upstream. Uses `--rebase` or `--no-edit` (merge).
/// `GIT_TERMINAL_PROMPT=0` + `GIT_EDITOR=true` so git never blocks waiting for input.
pub fn pull(
    repo: &Path,
    rebase: bool,
    state: &RemoteState,
    on_line: &dyn Fn(String),
) -> Result<RemoteOutcome, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .arg("pull");
    if rebase {
        c.arg("--rebase");
    } else {
        c.arg("--no-edit");
    }
    let (ok, msg) = stream(c, state, on_line)?;
    Ok(outcome(repo, ok, msg))
}

/// Push to a remote. Force is ALWAYS `--force-with-lease`, never bare `--force`.
/// `--end-of-options` guards the remote + refspec operands against option injection.
pub fn push(
    repo: &Path,
    remote: &str,
    refspec: Option<&str>,
    force_with_lease: bool,
    set_upstream: bool,
    state: &RemoteState,
    on_line: &dyn Fn(String),
) -> Result<RemoteOutcome, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("push");
    if force_with_lease {
        c.arg("--force-with-lease"); // NEVER bare --force
    }
    if set_upstream {
        c.arg("--set-upstream");
    }
    // --end-of-options guards remote + refspec against option injection.
    c.arg("--end-of-options").arg(remote);
    if let Some(rs) = refspec {
        c.arg(rs);
    }
    let (ok, msg) = stream(c, state, on_line)?;
    Ok(outcome(repo, ok, msg))
}

/// Kill the in-flight child process (if any). Called by `cancel_remote` command.
pub fn cancel(state: &RemoteState) {
    if let Some(mut child) = state.child.lock().unwrap().take() {
        let _ = child.kill();
    }
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
            let path = std::env::temp_dir()
                .join(format!("gte-remote-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@e.com"]);
            r.git(&["config", "user.name", "T"]);
            r
        }

        /// Create a bare repo at `path` (pre-existing dir OK).
        fn new_bare(path: PathBuf) -> PathBuf {
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let o = Command::new("git")
                .args(["init", "--bare", "-q", "-b", "main"])
                .current_dir(&path)
                .output()
                .unwrap();
            assert!(o.status.success(), "bare init: {}", String::from_utf8_lossy(&o.stderr));
            path
        }

        /// Clone `url` into `dest` and configure user identity.
        fn new_clone(url: &str, dest: PathBuf) -> Self {
            let _ = fs::remove_dir_all(&dest);
            let o = Command::new("git")
                .args(["clone", "-q", url, dest.to_str().unwrap()])
                .output()
                .unwrap();
            assert!(o.status.success(), "clone: {}", String::from_utf8_lossy(&o.stderr));
            let r = TempRepo { path: dest };
            r.git(&["config", "user.email", "t@e.com"]);
            r.git(&["config", "user.name", "T"]);
            r
        }

        fn git(&self, args: &[&str]) {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(args)
                .output()
                .unwrap();
            assert!(
                o.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&o.stderr)
            );
        }

        /// Run git, returning (success, stderr) — used where failure is expected.
        fn git_may_fail(&self, args: &[&str]) -> (bool, String) {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(args)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            (o.status.success(), stderr)
        }

        fn write_file(&self, name: &str, content: &str) {
            fs::write(self.path.join(name), content).unwrap();
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // ── Task 1 tests (unchanged) ─────────────────────────────────────────────

    #[test]
    fn add_list_seturl_remove() {
        let r = TempRepo::new();
        remote_add(&r.path, "origin", "https://example.com/x.git").unwrap();
        let list = remotes(&r.path).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "origin");
        assert_eq!(list[0].url, "https://example.com/x.git");
        remote_set_url(&r.path, "origin", "https://example.com/y.git").unwrap();
        assert_eq!(remotes(&r.path).unwrap()[0].url, "https://example.com/y.git");
        remote_remove(&r.path, "origin").unwrap();
        assert!(remotes(&r.path).unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_remote_name() {
        let r = TempRepo::new();
        assert!(remote_add(&r.path, "-x", "https://e/x.git").is_err());
        assert!(remote_add(&r.path, "--upload-pack=evil", "u").is_err());
    }

    // ── Task 2 tests — local bare remote (offline) ───────────────────────────

    fn noop_line(_: String) {}

    /// Helper: unique temp path for a bare remote.
    fn bare_path(tag: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("gte-bare-{}-{}-{}", std::process::id(), id, tag))
    }

    #[test]
    fn push_then_pull_roundtrip_local_bare() {
        // 1. Create a bare remote.
        let bare = TempRepo::new_bare(bare_path("roundtrip"));

        // 2. Create clone A, make a commit, push it.
        let clone_a_path = bare_path("clone-a-roundtrip");
        let _ = fs::remove_dir_all(&clone_a_path);
        let clone_a = TempRepo::new_clone(bare.to_str().unwrap(), clone_a_path);
        clone_a.write_file("hello.txt", "hello\n");
        clone_a.git(&["add", "hello.txt"]);
        clone_a.git(&["commit", "-m", "initial commit"]);

        let state_a = RemoteState::default();
        let result = push(
            &clone_a.path,
            "origin",
            Some("main"),
            false,        // no force
            true,         // set-upstream (first push)
            &state_a,
            &noop_line,
        )
        .unwrap();
        assert!(result.ok, "push should succeed; msg={}", result.message);
        assert!(!result.auth_failed);
        assert!(!result.conflicted);

        // 3. Clone B pulls and should see the commit.
        let clone_b_path = bare_path("clone-b-roundtrip");
        let _ = fs::remove_dir_all(&clone_b_path);
        let clone_b = TempRepo::new_clone(bare.to_str().unwrap(), clone_b_path);

        // Verify file is present in clone B.
        assert!(clone_b.path.join("hello.txt").exists(), "hello.txt should be present after pull");

        // Also exercise the pull() path with an explicit pull.
        clone_a.write_file("hello.txt", "hello world\n");
        clone_a.git(&["add", "hello.txt"]);
        clone_a.git(&["commit", "-m", "second commit"]);
        let push2 = push(&clone_a.path, "origin", Some("main"), false, false, &state_a, &noop_line).unwrap();
        assert!(push2.ok, "second push; msg={}", push2.message);

        let state_b = RemoteState::default();
        let pull_result = pull(&clone_b.path, false, &state_b, &noop_line).unwrap();
        assert!(pull_result.ok, "pull should succeed; msg={}", pull_result.message);
        let content = fs::read_to_string(clone_b.path.join("hello.txt")).unwrap();
        assert_eq!(content, "hello world\n");

        // Cleanup bare
        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn push_non_fastforward_needs_force_with_lease() {
        // 1. Bare remote + two clones both starting from a common commit.
        let bare = TempRepo::new_bare(bare_path("nff"));

        let clone_a_path = bare_path("clone-a-nff");
        let _ = fs::remove_dir_all(&clone_a_path);
        let clone_a = TempRepo::new_clone(bare.to_str().unwrap(), clone_a_path);
        let clone_b_path = bare_path("clone-b-nff");
        let _ = fs::remove_dir_all(&clone_b_path);
        let clone_b = TempRepo::new_clone(bare.to_str().unwrap(), clone_b_path);

        // 2. Establish a common base commit pushed from clone A.
        clone_a.write_file("f.txt", "base\n");
        clone_a.git(&["add", "f.txt"]);
        clone_a.git(&["commit", "-m", "base commit"]);
        let sa = RemoteState::default();
        let r0 = push(&clone_a.path, "origin", Some("main"), false, true, &sa, &noop_line).unwrap();
        assert!(r0.ok, "base push; {}", r0.message);

        // 3. Clone B fetches the base commit so it has an accurate tracking ref.
        clone_b.git(&["fetch", "origin"]);
        clone_b.git(&["reset", "--hard", "origin/main"]);

        // 4. Clone A diverges from the base and pushes (moves origin/main forward).
        clone_a.write_file("f.txt", "from A\n");
        clone_a.git(&["add", "f.txt"]);
        clone_a.git(&["commit", "-m", "A diverges"]);
        let sa2 = RemoteState::default();
        let r1 = push(&clone_a.path, "origin", Some("main"), false, false, &sa2, &noop_line).unwrap();
        assert!(r1.ok, "A push; {}", r1.message);

        // 5. Clone B also diverges from the base (origin/main has moved past it).
        clone_b.write_file("f.txt", "from B\n");
        clone_b.git(&["add", "f.txt"]);
        clone_b.git(&["commit", "-m", "B diverges"]);

        // Non-ff push without force → must fail (origin/main is ahead of B's tracking ref).
        let sb = RemoteState::default();
        let r2 = push(&clone_b.path, "origin", Some("main"), false, false, &sb, &noop_line).unwrap();
        assert!(!r2.ok, "non-ff push should fail; msg={}", r2.message);

        // 6. With force_with_lease=true it should succeed.
        // Clone B's tracking ref (origin/main) still points to the base commit it fetched —
        // but origin/main now points to A's commit, so --force-with-lease sees a mismatch
        // and fails unless we also fetch first. In real usage the UI would fetch first.
        // To test force-with-lease success, we fetch to update the tracking ref first.
        clone_b.git(&["fetch", "origin"]);
        let sb2 = RemoteState::default();
        let r3 = push(&clone_b.path, "origin", Some("main"), true, false, &sb2, &noop_line).unwrap();
        assert!(r3.ok, "force-with-lease push should succeed after fetch; {}", r3.message);

        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn pull_conflict_sets_conflicted() {
        // 1. Bare remote + two clones, both starting from a common commit.
        let bare = TempRepo::new_bare(bare_path("conflict"));

        let clone_a_path = bare_path("clone-a-conflict");
        let _ = fs::remove_dir_all(&clone_a_path);
        let clone_a = TempRepo::new_clone(bare.to_str().unwrap(), clone_a_path);
        let clone_b_path = bare_path("clone-b-conflict");
        let _ = fs::remove_dir_all(&clone_b_path);
        let clone_b = TempRepo::new_clone(bare.to_str().unwrap(), clone_b_path);

        // 2. Both start from the same file.
        clone_a.write_file("shared.txt", "original\n");
        clone_a.git(&["add", "shared.txt"]);
        clone_a.git(&["commit", "-m", "base"]);
        let sa = RemoteState::default();
        let r = push(&clone_a.path, "origin", Some("main"), false, true, &sa, &noop_line).unwrap();
        assert!(r.ok, "base push; {}", r.message);

        // 3. Clone B fetches the base so it can diverge from it.
        clone_b.git(&["fetch", "origin"]);
        clone_b.git(&["reset", "--hard", "origin/main"]);

        // 4. Clone A edits + pushes a new version.
        clone_a.write_file("shared.txt", "A's line\n");
        clone_a.git(&["add", "shared.txt"]);
        clone_a.git(&["commit", "-m", "A edit"]);
        let sa2 = RemoteState::default();
        let r2 = push(&clone_a.path, "origin", Some("main"), false, false, &sa2, &noop_line).unwrap();
        assert!(r2.ok, "A push; {}", r2.message);

        // 5. Clone B edits the same line + tries to pull (merge) → conflict.
        // Configure pull.rebase=false so git knows how to reconcile divergent branches.
        clone_b.git(&["config", "pull.rebase", "false"]);
        clone_b.write_file("shared.txt", "B's line\n");
        clone_b.git(&["add", "shared.txt"]);
        clone_b.git(&["commit", "-m", "B edit"]);

        let sb = RemoteState::default();
        let pull_result = pull(&clone_b.path, false, &sb, &noop_line).unwrap();
        // Pull stops on conflict → ok=false, conflicted=true.
        assert!(!pull_result.ok, "conflicting pull should not be ok");
        assert!(pull_result.conflicted, "should report conflicted; msg={}", pull_result.message);

        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn looks_like_auth_failure_detects_patterns() {
        assert!(looks_like_auth_failure("Authentication failed for 'https://github.com/'"));
        assert!(looks_like_auth_failure("fatal: could not read Username"));
        assert!(looks_like_auth_failure("fatal: could not read Password for 'https://...':"));
        assert!(looks_like_auth_failure("Permission denied (publickey)"));
        assert!(looks_like_auth_failure("terminal prompts disabled"));
        assert!(looks_like_auth_failure("Invalid username or password"));
        assert!(!looks_like_auth_failure("Everything is fine"));
        assert!(!looks_like_auth_failure("error: failed to push some refs"));
    }

    #[test]
    fn stream_accumulates_output() {
        // Use `git --version` as a simple command that succeeds and produces output.
        let _tmp = TempRepo::new();
        let mut cmd = Command::new("git");
        cmd.arg("--version");

        let state = RemoteState::default();
        let lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let (ok, combined) = stream(cmd, &state, &|l| lines.lock().unwrap().push(l.clone())).unwrap();
        assert!(ok, "git --version should succeed");
        let locked = lines.lock().unwrap();
        // At least one line forwarded and accumulated.
        assert!(!locked.is_empty(), "should have forwarded lines");
        assert!(combined.contains("git version"), "combined: {}", combined);
        assert_eq!(locked.join("\n") + "\n", combined, "forwarded lines should match accumulated output");
    }

    #[test]
    fn cancel_stops_child_and_returns_cancelled() {
        // Start a long-running command, cancel it, check result is "Cancelled."
        // We'll wrap in Arc so we can cancel from a separate thread.
        use std::sync::Arc;
        let state = Arc::new(RemoteState::default());
        let state_clone = Arc::clone(&state);

        // Run stream in a background thread.
        let mut cmd = Command::new("sleep");
        cmd.arg("30");

        let handle = std::thread::spawn(move || {
            stream(cmd, &state_clone, &|_| {})
        });

        // Give the spawn time to register the child.
        std::thread::sleep(std::time::Duration::from_millis(100));
        cancel(&state);

        let result = handle.join().expect("stream thread panicked");
        match result {
            Ok((ok, msg)) => {
                assert!(!ok, "cancelled op should not be ok");
                assert_eq!(msg, "Cancelled.", "message: {}", msg);
            }
            Err(e) => panic!("stream returned error instead of cancelled: {}", e),
        }
    }
}
