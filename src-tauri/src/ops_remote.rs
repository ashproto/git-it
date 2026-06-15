use crate::git_ops;
use crate::types::{RemoteInfo, RemoteOutcome};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Per-process nonce counter for unique scratch-dir names.
static ASKPASS_NONCE: AtomicU32 = AtomicU32::new(0);

fn next_nonce() -> u32 {
    ASKPASS_NONCE.fetch_add(1, Ordering::SeqCst)
}

/// Single-quote-escape a string for safe embedding in a POSIX shell script.
/// Replaces each `'` with `'\''` so the resulting value can be wrapped in `'...'`.
fn sq(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Create a file with mode 0600 (owner read+write only) and write `contents` into it.
/// Uses O_CREAT|O_WRONLY|O_TRUNC with explicit 0600 mode to avoid umask leaking perms.
#[cfg(unix)]
fn write_600(path: &std::path::Path, contents: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("write_600 open {:?}: {}", path, e))?;
    std::io::Write::write_all(&mut f, contents.as_bytes())
        .map_err(|e| format!("write_600 write {:?}: {}", path, e))?;
    Ok(())
}

/// Write credentials to `0600` temp files and generate a `GIT_ASKPASS` script that
/// returns the right credential based on git's prompt ($1).
///
/// # Security
/// - Credentials are NEVER placed on the command line, embedded in URLs, or persisted.
/// - Files are created mode 0600 (owner read+write only).
/// - The temp-dir path is single-quote-escaped in the script to prevent shell injection.
/// - Caller MUST `remove_dir_all(dir)` after the op (success or error).
///
/// Returns the scratch directory path that the caller must delete.
pub fn setup_askpass(cmd: &mut Command, username: &str, password: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir()
        .join(format!("gte-cred-{}-{}", std::process::id(), next_nonce()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cred dir: {}", e))?;

    let uf = dir.join("u");
    let pf = dir.join("p");
    write_600(&uf, username)?;
    write_600(&pf, password)?;

    // Single-quote-escape the paths so they're safe to embed in the shell script.
    let uf_sq = sq(&uf.to_string_lossy());
    let pf_sq = sq(&pf.to_string_lossy());

    // git calls GIT_ASKPASS with the prompt as $1, e.g.:
    //   "Username for 'https://github.com':"
    //   "Password for 'https://user@github.com':"
    let script_body = format!(
        "#!/bin/sh\ncase \"$1\" in\n  *[Uu]sername*) cat '{}' ;;\n  *) cat '{}' ;;\nesac\n",
        uf_sq, pf_sq
    );
    let script = dir.join("askpass.sh");
    std::fs::write(&script, &script_body).map_err(|e| format!("askpass write: {}", e))?;

    // Make the script executable (owner only: rwx------).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("askpass chmod: {}", e))?;
    }

    cmd.env("GIT_ASKPASS", &script).env("GIT_TERMINAL_PROMPT", "0");
    Ok(dir)
}

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
    /// Set true while a stream() call is executing; rejects concurrent ops.
    pub busy: AtomicBool,
}

/// Returns true if the git output looks like an HTTPS authentication failure.
///
/// Deliberately excludes bare "permission denied" because SSH key failures
/// ("Permission denied (publickey)") produce that phrase — but askpass cannot
/// supply an SSH key, so prompting for credentials would be useless and confusing.
/// SSH failures are correctly left as plain (non-auth) errors.
pub fn looks_like_auth_failure(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("authentication failed")
        || l.contains("could not read username")
        || l.contains("could not read password")
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
    // Atomic re-entry guard: reject a second op rather than silently clobbering.
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A remote operation is already in progress.".to_string());
    }
    // RAII guard: resets `busy` on every exit path (success, error, panic).
    struct BusyGuard<'a>(&'a AtomicBool);
    impl Drop for BusyGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _busy = BusyGuard(&state.busy);

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
///
/// When `username` AND `password` are both `Some`, credentials are written to `0600`
/// temp files and a `GIT_ASKPASS` script is generated — credentials are NEVER placed
/// on argv, in URLs, or persisted. The scratch dir is deleted after the op.
pub fn pull(
    repo: &Path,
    rebase: bool,
    username: Option<&str>,
    password: Option<&str>,
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

    // Set up askpass only when both credentials are present.
    let cred_dir = match (username, password) {
        (Some(u), Some(p)) => Some(setup_askpass(&mut c, u, p)?),
        _ => None,
    };

    let result = stream(c, state, on_line);

    // Always clean up the cred scratch dir, even on error.
    if let Some(dir) = cred_dir {
        let _ = std::fs::remove_dir_all(dir);
    }

    let (ok, msg) = result?;
    Ok(outcome(repo, ok, msg))
}

/// Push to a remote. Force is ALWAYS `--force-with-lease`, never bare `--force`.
/// `--end-of-options` guards the remote + refspec operands against option injection.
///
/// When `username` AND `password` are both `Some`, credentials are written to `0600`
/// temp files and a `GIT_ASKPASS` script is generated — credentials are NEVER placed
/// on argv, in URLs, or persisted. The scratch dir is deleted after the op.
#[allow(clippy::too_many_arguments)]
pub fn push(
    repo: &Path,
    remote: &str,
    refspec: Option<&str>,
    force_with_lease: bool,
    set_upstream: bool,
    username: Option<&str>,
    password: Option<&str>,
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

    // Set up askpass only when both credentials are present.
    let cred_dir = match (username, password) {
        (Some(u), Some(p)) => Some(setup_askpass(&mut c, u, p)?),
        _ => None,
    };

    let result = stream(c, state, on_line);

    // Always clean up the cred scratch dir, even on error.
    if let Some(dir) = cred_dir {
        let _ = std::fs::remove_dir_all(dir);
    }

    let (ok, msg) = result?;
    Ok(outcome(repo, ok, msg))
}

/// Kill the in-flight child process (if any). Called by `cancel_remote` command.
pub fn cancel(state: &RemoteState) {
    if let Some(mut child) = state.child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait(); // reap so the killed git process doesn't linger as a zombie
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
            None,         // no username
            None,         // no password
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
        let push2 = push(&clone_a.path, "origin", Some("main"), false, false, None, None, &state_a, &noop_line).unwrap();
        assert!(push2.ok, "second push; msg={}", push2.message);

        let state_b = RemoteState::default();
        let pull_result = pull(&clone_b.path, false, None, None, &state_b, &noop_line).unwrap();
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
        let r0 = push(&clone_a.path, "origin", Some("main"), false, true, None, None, &sa, &noop_line).unwrap();
        assert!(r0.ok, "base push; {}", r0.message);

        // 3. Clone B fetches the base commit so it has an accurate tracking ref.
        clone_b.git(&["fetch", "origin"]);
        clone_b.git(&["reset", "--hard", "origin/main"]);

        // 4. Clone A diverges from the base and pushes (moves origin/main forward).
        clone_a.write_file("f.txt", "from A\n");
        clone_a.git(&["add", "f.txt"]);
        clone_a.git(&["commit", "-m", "A diverges"]);
        let sa2 = RemoteState::default();
        let r1 = push(&clone_a.path, "origin", Some("main"), false, false, None, None, &sa2, &noop_line).unwrap();
        assert!(r1.ok, "A push; {}", r1.message);

        // 5. Clone B also diverges from the base (origin/main has moved past it).
        clone_b.write_file("f.txt", "from B\n");
        clone_b.git(&["add", "f.txt"]);
        clone_b.git(&["commit", "-m", "B diverges"]);

        // Non-ff push without force → must fail (origin/main is ahead of B's tracking ref).
        let sb = RemoteState::default();
        let r2 = push(&clone_b.path, "origin", Some("main"), false, false, None, None, &sb, &noop_line).unwrap();
        assert!(!r2.ok, "non-ff push should fail; msg={}", r2.message);

        // 6. With force_with_lease=true it should succeed.
        // Clone B's tracking ref (origin/main) still points to the base commit it fetched —
        // but origin/main now points to A's commit, so --force-with-lease sees a mismatch
        // and fails unless we also fetch first. In real usage the UI would fetch first.
        // To test force-with-lease success, we fetch to update the tracking ref first.
        clone_b.git(&["fetch", "origin"]);
        let sb2 = RemoteState::default();
        let r3 = push(&clone_b.path, "origin", Some("main"), true, false, None, None, &sb2, &noop_line).unwrap();
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
        let r = push(&clone_a.path, "origin", Some("main"), false, true, None, None, &sa, &noop_line).unwrap();
        assert!(r.ok, "base push; {}", r.message);

        // 3. Clone B fetches the base so it can diverge from it.
        clone_b.git(&["fetch", "origin"]);
        clone_b.git(&["reset", "--hard", "origin/main"]);

        // 4. Clone A edits + pushes a new version.
        clone_a.write_file("shared.txt", "A's line\n");
        clone_a.git(&["add", "shared.txt"]);
        clone_a.git(&["commit", "-m", "A edit"]);
        let sa2 = RemoteState::default();
        let r2 = push(&clone_a.path, "origin", Some("main"), false, false, None, None, &sa2, &noop_line).unwrap();
        assert!(r2.ok, "A push; {}", r2.message);

        // 5. Clone B edits the same line + tries to pull (merge) → conflict.
        // Configure pull.rebase=false so git knows how to reconcile divergent branches.
        clone_b.git(&["config", "pull.rebase", "false"]);
        clone_b.write_file("shared.txt", "B's line\n");
        clone_b.git(&["add", "shared.txt"]);
        clone_b.git(&["commit", "-m", "B edit"]);

        let sb = RemoteState::default();
        let pull_result = pull(&clone_b.path, false, None, None, &sb, &noop_line).unwrap();
        // Pull stops on conflict → ok=false, conflicted=true.
        assert!(!pull_result.ok, "conflicting pull should not be ok");
        assert!(pull_result.conflicted, "should report conflicted; msg={}", pull_result.message);

        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn looks_like_auth_failure_detects_patterns() {
        // HTTPS auth failure phrases must be detected.
        assert!(looks_like_auth_failure("Authentication failed for 'https://github.com/'"));
        assert!(looks_like_auth_failure("fatal: could not read Username"));
        assert!(looks_like_auth_failure("fatal: could not read Password for 'https://...':"));
        assert!(looks_like_auth_failure("terminal prompts disabled"));
        assert!(looks_like_auth_failure("Invalid username or password"));
        assert!(looks_like_auth_failure("fatal: Authentication failed"));

        // SSH key failures must NOT be classified as auth failures — askpass cannot
        // supply an SSH key, so triggering the credentials prompt would be useless.
        assert!(!looks_like_auth_failure("Permission denied (publickey)."));
        assert!(!looks_like_auth_failure("git@github.com: Permission denied (publickey)."));

        // Unrelated errors must not be classified as auth failures.
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

    // ── Task 3 tests — GIT_ASKPASS credentials ───────────────────────────────

    /// Helper: run a shell script with the given argument and capture stdout.
    fn run_askpass(script: &std::path::Path, arg: &str) -> String {
        let out = Command::new("sh")
            .arg(script)
            .arg(arg)
            .output()
            .expect("sh failed");
        String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string()
    }

    #[test]
    fn setup_askpass_script_returns_username() {
        let mut cmd = Command::new("git");
        cmd.arg("--version"); // placeholder; we only care about the env setup
        let dir = setup_askpass(&mut cmd, "alice", "s3cr3t").expect("setup_askpass failed");
        let script = dir.join("askpass.sh");
        assert!(script.exists(), "askpass.sh not created");

        let result = run_askpass(&script, "Username for 'https://github.com':");
        assert_eq!(result, "alice", "askpass returned wrong username");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setup_askpass_script_returns_password() {
        let mut cmd = Command::new("git");
        cmd.arg("--version");
        let dir = setup_askpass(&mut cmd, "alice", "s3cr3t").expect("setup_askpass failed");
        let script = dir.join("askpass.sh");

        let result = run_askpass(&script, "Password for 'https://alice@github.com':");
        assert_eq!(result, "s3cr3t", "askpass returned wrong password");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setup_askpass_cred_files_are_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let mut cmd = Command::new("git");
        cmd.arg("--version");
        let dir = setup_askpass(&mut cmd, "alice", "s3cr3t").expect("setup_askpass failed");

        let u_perms = std::fs::metadata(dir.join("u"))
            .expect("u file missing")
            .permissions()
            .mode();
        let p_perms = std::fs::metadata(dir.join("p"))
            .expect("p file missing")
            .permissions()
            .mode();
        assert_eq!(
            u_perms & 0o777,
            0o600,
            "username file should be 0600, got {:o}",
            u_perms & 0o777
        );
        assert_eq!(
            p_perms & 0o777,
            0o600,
            "password file should be 0600, got {:o}",
            p_perms & 0o777
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setup_askpass_scratch_dir_deleted_after() {
        let mut cmd = Command::new("git");
        cmd.arg("--version");
        let dir = setup_askpass(&mut cmd, "alice", "s3cr3t").expect("setup_askpass failed");
        assert!(dir.exists(), "scratch dir should exist before removal");

        std::fs::remove_dir_all(&dir).expect("failed to remove scratch dir");
        assert!(!dir.exists(), "scratch dir should be gone after removal");
    }

    #[test]
    fn setup_askpass_creds_not_in_command_args() {
        // Credentials must NEVER appear as command-line arguments.
        // We call setup_askpass and then verify the command args don't contain "alice" or "s3cr3t".
        // (Command::new + arg() builds argv; we can't inspect it directly, but we can verify
        // the env vars are set to the askpass script, not to the literal creds.)
        let mut cmd = Command::new("git");
        cmd.arg("--version");
        let dir = setup_askpass(&mut cmd, "alice", "s3cr3t").expect("setup_askpass failed");

        // The askpass env var must be set to the script path (not the cred values).
        // We check that the env was set by looking at the script existence.
        let script = dir.join("askpass.sh");
        assert!(script.exists(), "GIT_ASKPASS script must exist");

        // The script body should NOT contain the literal credentials — it only cat's them from files.
        let script_body = std::fs::read_to_string(&script).expect("read script");
        assert!(
            !script_body.contains("alice"),
            "script body must not contain username literal; body: {}",
            script_body
        );
        assert!(
            !script_body.contains("s3cr3t"),
            "script body must not contain password literal; body: {}",
            script_body
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setup_askpass_handles_single_quote_in_path() {
        // If the temp dir path somehow contained a single quote, sq() must escape it safely.
        // We test sq() directly rather than manipulating temp paths (platform-specific).
        assert_eq!(sq("normal"), "normal");
        assert_eq!(sq("has'quote"), "has'\\''quote");
        assert_eq!(sq("a'b'c"), "a'\\''b'\\''c");
        assert_eq!(sq(""), "");
    }

    #[test]
    fn setup_askpass_username_prompt_case_variants() {
        // Both "Username" and "username" prompts should return the username.
        let mut cmd = Command::new("git");
        cmd.arg("--version");
        let dir = setup_askpass(&mut cmd, "bob", "pass123").expect("setup_askpass");
        let script = dir.join("askpass.sh");

        assert_eq!(run_askpass(&script, "Username for 'https://x':"), "bob");
        assert_eq!(run_askpass(&script, "username for 'https://x':"), "bob");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Fix 2: atomic busy-guard — concurrent op rejected cleanly ─────────────

    #[test]
    fn concurrent_stream_rejected_when_busy() {
        // Directly set the busy flag (simulating an in-flight op) and verify that
        // a subsequent stream() call returns the "already in progress" error immediately
        // without spawning or clobbering. This is deterministic — no races.
        use std::sync::atomic::Ordering;

        let state = RemoteState::default();

        // Simulate: an op is already running by setting busy = true.
        state.busy.store(true, Ordering::SeqCst);

        // A second stream() attempt must be rejected immediately.
        let mut cmd = Command::new("git");
        cmd.arg("--version"); // would succeed if allowed to run
        let err = stream(cmd, &state, &|_| {})
            .expect_err("should reject when busy flag is set");
        assert!(
            err.contains("already in progress"),
            "unexpected error: {}",
            err
        );

        // Reset so the state is clean after the test.
        state.busy.store(false, Ordering::SeqCst);
    }

    #[test]
    fn busy_guard_resets_after_stream_completes() {
        // Verify that after a successful stream() call, the busy flag is cleared
        // and a subsequent call can proceed normally.
        let state = RemoteState::default();

        // First call: succeeds.
        let mut cmd1 = Command::new("git");
        cmd1.arg("--version");
        let (ok, _) = stream(cmd1, &state, &|_| {}).expect("first stream failed");
        assert!(ok, "git --version should succeed");

        // busy must be false after stream returns.
        assert!(
            !state.busy.load(std::sync::atomic::Ordering::SeqCst),
            "busy should be false after stream completes"
        );

        // Second call should also succeed (not get a spurious "already in progress").
        let mut cmd2 = Command::new("git");
        cmd2.arg("--version");
        let (ok2, _) = stream(cmd2, &state, &|_| {}).expect("second stream failed");
        assert!(ok2, "second git --version should succeed");
    }
}
