use crate::git_ops;
use crate::graph;
use crate::types::WorktreeInfo;
use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::str;

#[derive(Debug, Default)]
struct ParsedWorktree {
    seen: bool,
    path: String,
    head: Option<String>,
    branch: Option<String>,
    detached: bool,
    bare: bool,
    locked: bool,
    locked_reason: Option<String>,
    prunable: bool,
    prunable_reason: Option<String>,
}

fn finish_worktree(
    current: &mut ParsedWorktree,
    out: &mut Vec<ParsedWorktree>,
) -> Result<(), String> {
    if !current.seen {
        return Ok(());
    }
    if current.path.is_empty() {
        return Err("git worktree output contained a record without a path".to_string());
    }
    out.push(std::mem::take(current));
    Ok(())
}

/// Parse the stable NUL-delimited worktree porcelain format. Splitting each
/// field at only its first space preserves spaces and newlines inside paths and
/// lock/prune reasons. Invalid UTF-8 is rejected instead of being made lossy:
/// this path may later identify a destructive removal target.
fn parse_worktree_porcelain(raw: &[u8]) -> Result<Vec<ParsedWorktree>, String> {
    let mut out = Vec::new();
    let mut current = ParsedWorktree::default();

    for field in raw.split(|b| *b == 0) {
        if field.is_empty() {
            finish_worktree(&mut current, &mut out)?;
            continue;
        }
        let split = field.iter().position(|b| *b == b' ');
        let (key_raw, value_raw) = match split {
            Some(i) => (&field[..i], &field[i + 1..]),
            None => (field, &[][..]),
        };
        let key = str::from_utf8(key_raw)
            .map_err(|_| "git worktree output contained an invalid UTF-8 field name".to_string())?;
        let value = str::from_utf8(value_raw)
            .map_err(|_| format!("git worktree {key} contained an invalid UTF-8 value"))?;

        // Be tolerant of a missing blank separator while still keeping records
        // isolated if a future Git version starts a new `worktree` field.
        if key == "worktree" && current.seen {
            finish_worktree(&mut current, &mut out)?;
        }
        current.seen = true;
        match key {
            "worktree" => current.path = value.to_string(),
            "HEAD" => current.head = Some(value.to_string()),
            "branch" => current.branch = value.strip_prefix("refs/heads/").map(str::to_string),
            "detached" => current.detached = true,
            "bare" => current.bare = true,
            "locked" => {
                current.locked = true;
                current.locked_reason = (!value.is_empty()).then(|| value.to_string());
            }
            "prunable" => {
                current.prunable = true;
                current.prunable_reason = (!value.is_empty()).then(|| value.to_string());
            }
            _ => {} // forward-compatible: ignore fields added by future Git versions
        }
    }
    finish_worktree(&mut current, &mut out)?;
    Ok(out)
}

fn same_existing_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

struct WorktreeHeadLock {
    path: PathBuf,
    // Keeping the descriptor open makes ownership of the standard Git lock
    // unambiguous until removal finishes.
    _file: fs::File,
}

struct BranchDeleteSafety {
    branch_oid: String,
    // The upstream or current branch used as the `-d` merge baseline. Adding a
    // verify-only entry for it to the prepared transaction holds that ref at
    // the exact OID on which the safety decision was based.
    baseline: Option<(String, String)>,
}

/// A prepared `git update-ref` transaction holds the branch ref lock while the
/// worktree folder is removed. The expected old OID makes a concurrent ref
/// update fail before any filesystem deletion, and dropping an uncommitted
/// transaction aborts it so the branch remains intact.
struct PreparedBranchDelete {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    finished: bool,
}

impl PreparedBranchDelete {
    fn prepare(
        repo: &Path,
        refname: &str,
        expected_oid: &str,
        baseline: Option<(&str, &str)>,
    ) -> Result<Self, String> {
        let mut child = Command::new("git")
            .current_dir(repo)
            .env("LC_ALL", "C")
            .args(["update-ref", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start the branch delete transaction: {e}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Git did not open the branch transaction input.".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Git did not open the branch transaction output.".to_string())?;
        let mut transaction = Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            finished: false,
        };
        transaction.send("start")?;
        transaction.expect_response("start: ok")?;
        transaction.send(&format!("delete {refname} {expected_oid}"))?;
        if let Some((baseline_ref, baseline_oid)) = baseline {
            transaction.send(&format!("verify {baseline_ref} {baseline_oid}"))?;
        }
        transaction.send("prepare")?;
        transaction.expect_response("prepare: ok")?;
        Ok(transaction)
    }

    fn send(&mut self, command: &str) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "The branch transaction is already closed.".to_string())?;
        writeln!(stdin, "{command}")
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("Could not communicate with the branch transaction: {e}"))
    }

    fn expect_response(&mut self, expected: &str) -> Result<(), String> {
        let mut response = String::new();
        let read = self
            .stdout
            .read_line(&mut response)
            .map_err(|e| format!("Could not read the branch transaction response: {e}"))?;
        if read == 0 {
            return Err("Git ended the branch transaction unexpectedly.".to_string());
        }
        if response.trim_end() != expected {
            return Err(format!(
                "Git returned an unexpected branch transaction response: {}",
                response.trim_end()
            ));
        }
        Ok(())
    }

    fn commit(mut self) -> Result<(), String> {
        self.send("commit")?;
        self.expect_response("commit: ok")?;
        self.stdin.take();
        let status = self
            .child
            .wait()
            .map_err(|e| format!("Could not finish the branch transaction: {e}"))?;
        self.finished = true;
        if status.success() {
            Ok(())
        } else {
            Err("Git could not commit the prepared branch deletion.".to_string())
        }
    }
}

impl Drop for PreparedBranchDelete {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let _ = self.send("abort");
        let _ = self.expect_response("abort: ok");
        self.stdin.take();
        let _ = self.child.wait();
    }
}

impl Drop for WorktreeHeadLock {
    fn drop(&mut self) {
        // A successful `git worktree remove` deletes the administrative
        // directory (and therefore this lock) for us. On any earlier failure,
        // release only the lock file we created.
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn git_path_output(repo: &Path, args: &[&str], description: &str) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to resolve {description}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!("Could not resolve {description}.")
        } else {
            stderr.into_owned()
        });
    }
    let stdout = str::from_utf8(&output.stdout)
        .map_err(|_| format!("Git returned an invalid UTF-8 {description}."))?;
    // Remove exactly Git's record terminator, preserving any newline that is
    // actually part of the path.
    let value = stdout.strip_suffix('\n').unwrap_or(stdout);
    let value = value.strip_suffix('\r').unwrap_or(value);
    if value.is_empty() {
        return Err(format!("Git returned an empty {description}."));
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("Git returned a non-absolute {description}."));
    }
    fs::canonicalize(&path).map_err(|e| format!("Could not verify {description}: {e}"))
}

fn acquire_worktree_head_lock(repo: &Path, worktree: &Path) -> Result<WorktreeHeadLock, String> {
    let repo_common = git_path_output(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        "repository Git directory",
    )?;
    let worktree_common = git_path_output(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        "worktree Git directory",
    )?;
    if repo_common != worktree_common {
        return Err(
            "The requested path is no longer a worktree of this repository. Refresh and try again."
                .to_string(),
        );
    }
    let admin_dir = git_path_output(
        worktree,
        &["rev-parse", "--absolute-git-dir"],
        "worktree administrative directory",
    )?;
    let lock_path = admin_dir.join("HEAD.lock");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                "Another Git operation is using this worktree. Wait for it to finish, then try again."
                    .to_string()
            } else {
                format!("Could not lock the worktree for safe removal: {e}")
            }
        })?;
    Ok(WorktreeHeadLock {
        path: lock_path,
        _file: file,
    })
}

fn exact_ref_oid(repo: &Path, refname: &str) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["show-ref", "--verify", "--hash", refname])
        .output()
        .map_err(|e| format!("Failed to inspect branch: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Branch '{}' no longer exists. Refresh and try again.",
            refname.strip_prefix("refs/heads/").unwrap_or(refname)
        ));
    }
    let oid = str::from_utf8(&output.stdout)
        .map_err(|_| "Git returned an invalid branch object ID.".to_string())?
        .trim();
    if oid.is_empty() {
        return Err("Git returned an empty branch object ID.".to_string());
    }
    Ok(oid.to_string())
}

fn branch_upstream(repo: &Path, refname: &str) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["for-each-ref", "--format=%(refname)%00%(upstream)", refname])
        .output()
        .map_err(|e| format!("Failed to inspect branch upstream: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            "Could not inspect the branch upstream.".to_string()
        } else {
            stderr.into_owned()
        });
    }
    let stdout = str::from_utf8(&output.stdout)
        .map_err(|_| "Git returned an invalid UTF-8 branch upstream.".to_string())?;
    for line in stdout.lines() {
        let Some((candidate, upstream)) = line.split_once('\0') else {
            continue;
        };
        if candidate == refname {
            return Ok((!upstream.is_empty()).then(|| upstream.to_string()));
        }
    }
    Err(
        "The branch disappeared while its worktree was being checked. Refresh and try again."
            .to_string(),
    )
}

/// Mirror `git branch -d`'s safety rule before deleting the worktree folder:
/// the branch must be merged into its configured upstream, or into HEAD when
/// it has no upstream. The returned baseline is verified and held by the same
/// prepared ref transaction that deletes the branch.
fn ensure_branch_safely_deletable(repo: &Path, branch: &str) -> Result<BranchDeleteSafety, String> {
    let refname = format!("refs/heads/{branch}");
    let branch_oid = exact_ref_oid(repo, &refname)?;
    let upstream = branch_upstream(repo, &refname)?;
    let (base_oid, baseline) = match upstream.as_deref() {
        Some(upstream_ref) => {
            let oid = exact_ref_oid(repo, upstream_ref).map_err(|_| {
                format!(
                    "The upstream for branch '{branch}' is unavailable, so safe deletion could not be verified. Fetch or use Force delete."
                )
            })?;
            (oid.clone(), Some((upstream_ref.to_string(), oid)))
        }
        None => {
            let symbolic = Command::new("git")
                .current_dir(repo)
                .args(["symbolic-ref", "--quiet", "HEAD"])
                .output()
                .map_err(|e| format!("Failed to inspect HEAD: {e}"))?;
            if symbolic.status.success() {
                let baseline_ref = str::from_utf8(&symbolic.stdout)
                    .map_err(|_| "Git returned an invalid HEAD reference.".to_string())?
                    .trim();
                if baseline_ref.is_empty() {
                    return Err("Git returned an empty HEAD reference.".to_string());
                }
                let oid = exact_ref_oid(repo, baseline_ref)?;
                (oid.clone(), Some((baseline_ref.to_string(), oid)))
            } else {
                // Detached HEAD is a direct ref, so verify-and-lock HEAD itself.
                let output = Command::new("git")
                    .current_dir(repo)
                    .args(["rev-parse", "--verify", "HEAD^{commit}"])
                    .output()
                    .map_err(|e| format!("Failed to inspect HEAD: {e}"))?;
                if !output.status.success() {
                    return Err(
                        "HEAD is unavailable, so safe branch deletion could not be verified."
                            .to_string(),
                    );
                }
                let oid = str::from_utf8(&output.stdout)
                    .map_err(|_| "Git returned an invalid HEAD object ID.".to_string())?
                    .trim();
                if oid.is_empty() {
                    return Err("Git returned an empty HEAD object ID.".to_string());
                }
                let oid = oid.to_string();
                (oid.clone(), Some(("HEAD".to_string(), oid)))
            }
        }
    };

    let output = Command::new("git")
        .current_dir(repo)
        .args(["merge-base", "--is-ancestor", &branch_oid, &base_oid])
        .output()
        .map_err(|e| format!("Failed to verify whether the branch is merged: {e}"))?;
    if output.status.success() {
        return Ok(BranchDeleteSafety {
            branch_oid,
            baseline,
        });
    }
    if output.status.code() == Some(1) {
        return Err(format!(
            "Branch '{branch}' is not fully merged. Its worktree was not removed. Enable Force delete to discard unmerged commits."
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(if stderr.trim().is_empty() {
        "Could not verify whether the branch is fully merged; its worktree was not removed."
            .to_string()
    } else {
        stderr.into_owned()
    })
}

/// List every worktree registered with this repository. The main worktree is
/// always first in Git's porcelain output. Missing/prunable entries stay
/// visible but have no status, so the UI can explain why they are not removable.
pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeInfo>, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()
        .map_err(|e| format!("Failed to list worktrees: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!(
                "git worktree list failed with exit code {:?}",
                output.status.code()
            )
        } else {
            stderr.into_owned()
        });
    }

    parse_worktree_porcelain(&output.stdout)?
        .into_iter()
        .enumerate()
        .map(|(index, parsed)| {
            let path = PathBuf::from(&parsed.path);
            let is_current = same_existing_path(repo, &path);
            let status = if !parsed.bare && !parsed.prunable && path.exists() {
                graph::repo_status(&path).ok()
            } else {
                None
            };
            Ok(WorktreeInfo {
                path: parsed.path,
                head: parsed.head,
                branch: parsed.branch,
                is_main: index == 0,
                is_current,
                detached: parsed.detached,
                bare: parsed.bare,
                locked: parsed.locked,
                locked_reason: parsed.locked_reason,
                prunable: parsed.prunable,
                prunable_reason: parsed.prunable_reason,
                status,
            })
        })
        .collect()
}

fn remove_linked_worktree_for_branch(
    repo: &Path,
    branch: &str,
    requested_path: &str,
    force: bool,
) -> Result<String, String> {
    let initial_matches: Vec<WorktreeInfo> = list_worktrees(repo)?
        .into_iter()
        .filter(|w| w.branch.as_deref() == Some(branch))
        .collect();
    if initial_matches.len() > 1 {
        return Err(format!(
            "Branch '{branch}' is associated with multiple worktrees; refusing an ambiguous removal."
        ));
    }
    let initial = initial_matches.into_iter().next().ok_or_else(|| {
        format!(
            "The requested worktree is no longer associated with branch '{branch}'. Refresh and try again."
        )
    })?;
    if !same_existing_path(Path::new(&initial.path), Path::new(requested_path)) {
        return Err(format!(
            "The requested worktree is no longer associated with branch '{branch}'. Refresh and try again."
        ));
    }
    if initial.is_current {
        return Err(
            "Cannot remove the current worktree. Open this repository from another worktree first."
                .to_string(),
        );
    }
    if initial.is_main {
        return Err("Cannot remove the repository's main worktree.".to_string());
    }

    // HEAD.lock is Git's own exclusion mechanism. Freeze the target before the
    // authoritative second listing so a concurrent checkout, commit, reset, or
    // merge cannot repurpose it during removal. The prepared transaction below
    // separately verifies and locks the merge-safety baseline.
    let _target_head_lock = acquire_worktree_head_lock(repo, Path::new(&initial.path))?;
    let branch_worktrees: Vec<WorktreeInfo> = list_worktrees(repo)?
        .into_iter()
        .filter(|w| w.branch.as_deref() == Some(branch))
        .collect();
    if branch_worktrees.len() > 1 {
        return Err(format!(
            "Branch '{branch}' is associated with multiple worktrees; refusing an ambiguous removal."
        ));
    }
    let worktree = branch_worktrees.into_iter().next().ok_or_else(|| {
        format!(
            "The requested worktree is no longer associated with branch '{branch}'. Refresh and try again."
        )
    })?;
    if !same_existing_path(Path::new(&worktree.path), Path::new(requested_path)) {
        return Err(format!(
            "The requested worktree is no longer associated with branch '{branch}'. Refresh and try again."
        ));
    }
    if worktree.is_current {
        return Err(
            "Cannot remove the current worktree. Open this repository from another worktree first."
                .to_string(),
        );
    }
    if worktree.is_main {
        return Err("Cannot remove the repository's main worktree.".to_string());
    }
    if worktree.bare || worktree.detached {
        return Err(
            "The selected worktree no longer has the requested branch checked out.".to_string(),
        );
    }
    if worktree.locked {
        let detail = worktree
            .locked_reason
            .as_deref()
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        return Err(format!(
            "The worktree is locked{detail}. Unlock it before removing it."
        ));
    }
    if worktree.prunable {
        return Err(
            "The worktree is stale or missing. Prune its Git metadata before deleting the branch."
                .to_string(),
        );
    }
    let status = worktree.status.as_ref().ok_or_else(|| {
        "Could not verify that the worktree is clean, so it was not removed.".to_string()
    })?;
    if status.staged > 0
        || status.unstaged > 0
        || status.untracked > 0
        || status.conflicted > 0
        || status.operation.is_some()
    {
        return Err(
            "The worktree has uncommitted changes or an operation in progress. Commit, stash, or discard them before removing it."
                .to_string(),
        );
    }

    let authoritative = PathBuf::from(&worktree.path);
    if !authoritative.is_absolute() {
        return Err(
            "Git returned a non-absolute worktree path; refusing to remove it.".to_string(),
        );
    }
    let refname = format!("refs/heads/{branch}");
    let safety = if force {
        BranchDeleteSafety {
            branch_oid: exact_ref_oid(repo, &refname)?,
            baseline: None,
        }
    } else {
        ensure_branch_safely_deletable(repo, branch)?
    };
    // `prepare` atomically verifies and locks both the exact branch tip and the
    // merge-safety baseline before any filesystem deletion.
    let branch_delete = PreparedBranchDelete::prepare(
        repo,
        &refname,
        &safety.branch_oid,
        safety
            .baseline
            .as_ref()
            .map(|(refname, oid)| (refname.as_str(), oid.as_str())),
    )?;
    let mut remove = Command::new("git");
    // Deliberately no --force: Git gets the final race-safe say and refuses if
    // files become dirty after the status snapshot above.
    remove
        .current_dir(repo)
        .args(["worktree", "remove", "--"])
        .arg(&authoritative);
    git_ops::run(&mut remove)?;
    branch_delete.commit().map_err(|error| {
        format!(
            "Removed worktree at '{}', but the prepared branch delete failed: {error}",
            worktree.path
        )
    })?;
    // `update-ref` removes the ref and reflog; mirror `git branch -d`'s config
    // cleanup. A missing section is normal for branches without local config.
    let section = format!("branch.{branch}");
    let _ = Command::new("git")
        .current_dir(repo)
        .args(["config", "--remove-section", &section])
        .output();
    Ok(worktree.path)
}

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
/// -d semantics require the branch to be merged into its upstream or HEAD. For
/// a linked worktree, a prepared ref transaction holds the exact branch and
/// safety-baseline OIDs while the clean worktree is removed. When
/// `delete_remote` is set, the local delete still runs first; a remote failure
/// is returned as an explicit partial outcome.
pub fn delete_branch(
    repo: &Path,
    name: &str,
    force: bool,
    delete_remote: bool,
    remote: Option<&str>,
    remote_branch: Option<&str>,
    worktree_path: Option<&str>,
) -> Result<(), String> {
    if let Some(path) = worktree_path {
        remove_linked_worktree_for_branch(repo, name, path, force)?;
    } else {
        let flag = if force { "-D" } else { "-d" };
        let mut c = Command::new("git");
        c.current_dir(repo).args(["branch", flag, "--", name]);
        git_ops::run(&mut c)?;
    }

    if delete_remote {
        if let (Some(rem), Some(rb)) = (remote, remote_branch) {
            delete_remote_branch(repo, rem, rb)
                .map_err(|e| format!("Deleted local branch, but remote delete failed: {}", e))?;
        }
    }
    Ok(())
}

/// Delete a branch on a remote via `git push <remote> --delete <branch>`. On success git
/// also removes the local `refs/remotes/<remote>/<branch>` tracking ref, so callers don't
/// need a separate prune. GIT_TERMINAL_PROMPT=0 so a missing credential fails instead of
/// hanging.
///
/// Self-heal: if the remote branch is ALREADY gone (deleted elsewhere — e.g. a merged PR
/// whose branch was auto-deleted), `git push --delete` fails with "remote ref does not
/// exist" and leaves the stale local tracking ref behind, so the branch keeps showing in
/// the UI and every delete retry re-fails. Treat that specific case as success and prune
/// the stale tracking ref locally — the user's intent (make this branch go away) is met.
pub fn delete_remote_branch(repo: &Path, remote: &str, branch: &str) -> Result<(), String> {
    let mut p = Command::new("git");
    p.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        // Pin the C locale so the "already gone" diagnostic we match below is git's stable
        // English text, not a translation from the user's LANG/LC_* — the app shells out to
        // whatever git is on their PATH, which may ship localized messages.
        .env("LC_ALL", "C")
        // Options first, then --end-of-options, so BOTH the remote name and the branch
        // operand are guarded against leading-dash flag injection.
        .args(["push", "--delete", "--end-of-options", remote, branch]);
    match git_ops::run(&mut p) {
        Ok(_) => Ok(()),
        // Only the "already gone" case is safe to swallow; every other failure (auth,
        // network, protected branch) must still surface — the remote ref may live on.
        // The message is reliably English thanks to the LC_ALL=C pin above.
        Err(e) if e.contains("remote ref does not exist") => {
            prune_remote_tracking_ref(repo, remote, branch)
        }
        Err(e) => Err(e),
    }
}

/// Remove a stale `refs/remotes/<remote>/<branch>` tracking ref (what `fetch --prune`
/// would do). No-op if the ref is already absent. The refname is built with a fixed
/// `refs/remotes/` prefix so it can never be flag-like, and `show-ref --verify` confirms
/// it resolves to exactly that ref before we delete it.
fn prune_remote_tracking_ref(repo: &Path, remote: &str, branch: &str) -> Result<(), String> {
    let refname = format!("refs/remotes/{}/{}", remote, branch);
    let mut check = Command::new("git");
    check
        .current_dir(repo)
        .args(["show-ref", "--verify", "--quiet", &refname]);
    if git_ops::run(&mut check).is_err() {
        return Ok(()); // already absent → nothing to prune
    }
    let mut del = Command::new("git");
    del.current_dir(repo).args(["update-ref", "-d", &refname]);
    git_ops::run(&mut del).map(|_| ())
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

/// Fast-forward a LOCAL branch to its upstream tip WITHOUT checking it out.
///
/// Uses a FULLY-QUALIFIED refspec `refs/heads/<remote_branch>:refs/heads/<local_branch>`
/// so a tag (or any other ref) sharing the branch's name can't shadow the destination:
/// an unqualified `next:next` lets git resolve the ambiguous `next` to a same-named tag
/// and reject the update as non-fast-forward. Git refuses a genuine non-fast-forward, so
/// a diverged branch fails cleanly and the ref is left untouched. Callers only offer this
/// for non-current branches (git also refuses to fetch into the checked-out branch).
pub fn fast_forward_branch(
    repo: &Path,
    local_branch: &str,
    remote: &str,
    remote_branch: &str,
) -> Result<String, String> {
    if local_branch.is_empty() || local_branch.starts_with('-') {
        return Err(format!("Invalid branch: {}", local_branch));
    }
    if remote_branch.is_empty() || remote_branch.starts_with('-') {
        return Err(format!("Invalid remote branch: {}", remote_branch));
    }
    let refspec = format!("refs/heads/{}:refs/heads/{}", remote_branch, local_branch);
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        // Pin the C locale so the "non-fast-forward" / "[rejected]" diagnostic matched
        // below is git's stable English text, not a translation from the user's LANG/LC_*.
        .env("LC_ALL", "C")
        .args(["fetch", "--end-of-options", remote, &refspec]);
    match git_ops::run(&mut c) {
        Ok((o, e)) => Ok(format!("{}{}", o, e).trim().to_string()),
        Err(msg) => {
            if msg.contains("non-fast-forward") || msg.contains("[rejected]") {
                Err(format!(
                    "Can't fast-forward {}: it has diverged from {}/{}.",
                    local_branch, remote, remote_branch
                ))
            } else {
                Err(msg)
            }
        }
    }
}

/// Subjects of the commits on HEAD that aren't on `base` (newest first), for
/// prefilling a PR title/body. Callers treat failure as best-effort (an
/// unknown base just means no prefill), but a leading-dash base is rejected
/// before shelling out so it can never read as a flag.
pub fn branch_subjects(repo: &Path, base: &str, limit: u32) -> Result<Vec<String>, String> {
    if base.is_empty() || base.starts_with('-') {
        return Err(format!("Invalid base branch: {}", base));
    }
    let range = format!("{}..HEAD", base);
    let max_count = format!("--max-count={}", limit);
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["log", "--format=%s", &max_count, "--end-of-options", &range]);
    let (o, _) = git_ops::run(&mut c)?;
    Ok(o.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
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
            delete_branch(&r.path, "feat", false, false, None, None, None).is_err(),
            "safe delete must refuse an unmerged branch"
        );
        assert!(r.has_ref("refs/heads/feat"));
        delete_branch(&r.path, "feat", true, false, None, None, None).unwrap();
        assert!(!r.has_ref("refs/heads/feat"));
    }

    #[test]
    fn parses_worktree_porcelain_with_flags_reasons_and_spaces() {
        let raw = b"worktree /tmp/main repo\0HEAD aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\0branch refs/heads/main\0\0worktree /tmp/linked repo\0HEAD bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\0branch refs/heads/feature/worktree\0locked in use by another process\0prunable gitdir file points to non-existent location\0\0worktree /tmp/detached\0HEAD cccccccccccccccccccccccccccccccccccccccc\0detached\0\0";

        let parsed = parse_worktree_porcelain(raw).unwrap();

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].path, "/tmp/main repo");
        assert_eq!(parsed[0].branch.as_deref(), Some("main"));
        assert_eq!(parsed[1].path, "/tmp/linked repo");
        assert_eq!(parsed[1].branch.as_deref(), Some("feature/worktree"));
        assert!(parsed[1].locked);
        assert_eq!(
            parsed[1].locked_reason.as_deref(),
            Some("in use by another process")
        );
        assert!(parsed[1].prunable);
        assert_eq!(
            parsed[1].prunable_reason.as_deref(),
            Some("gitdir file points to non-existent location")
        );
        assert!(parsed[2].detached);
        assert_eq!(parsed[2].branch, None);
    }

    #[test]
    fn list_worktrees_reports_main_linked_branch_and_dirty_status() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let wt = r.path.join("linked worktree");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        fs::write(wt.join("dirty.txt"), "dirty").unwrap();

        let worktrees = list_worktrees(&r.path).unwrap();

        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_main);
        assert!(worktrees[0].is_current);
        let linked = worktrees
            .iter()
            .find(|w| w.branch.as_deref() == Some("feature"))
            .unwrap();
        assert!(same_existing_path(Path::new(&linked.path), &wt));
        assert!(!linked.is_main);
        assert!(!linked.is_current);
        assert_eq!(linked.status.as_ref().map(|s| s.untracked), Some(1));
    }

    #[test]
    fn delete_branch_can_remove_its_clean_linked_worktree_first() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let wt = r.path.join("linked");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        fs::write(wt.join("feature.txt"), "published feature").unwrap();
        r.git(&["-C", wt.to_str().unwrap(), "add", "feature.txt"]);
        r.git(&[
            "-C",
            wt.to_str().unwrap(),
            "commit",
            "-q",
            "-m",
            "published feature",
        ]);
        r.git(&[
            "update-ref",
            "refs/remotes/origin/feature",
            "refs/heads/feature",
        ]);
        r.git(&["remote", "add", "origin", "unused-test-remote"]);
        r.git(&["config", "branch.feature.remote", "origin"]);
        r.git(&[
            "config",
            "branch.feature.merge",
            "refs/heads/feature",
        ]);
        r.git(&["config", "branch.feature.description", "temporary branch"]);

        delete_branch(
            &r.path,
            "feature",
            false,
            false,
            None,
            None,
            Some(wt.to_str().unwrap()),
        )
        .unwrap();

        assert!(!wt.exists());
        assert!(!r.has_ref("refs/heads/feature"));
        let config = Command::new("git")
            .current_dir(&r.path)
            .args(["config", "--get", "branch.feature.description"])
            .output()
            .unwrap();
        assert!(!config.status.success(), "deleted branch config must be removed");
    }

    #[test]
    fn prepared_branch_delete_blocks_concurrent_ref_movement_and_aborts_cleanly() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        r.git(&["branch", "feature", "main"]);
        r.commit("b.txt", "B");
        let safety = ensure_branch_safely_deletable(&r.path, "feature").unwrap();
        let expected = safety.branch_oid.clone();
        let replacement = r.rev("main");

        let transaction = PreparedBranchDelete::prepare(
            &r.path,
            "refs/heads/feature",
            &expected,
            safety
                .baseline
                .as_ref()
                .map(|(refname, oid)| (refname.as_str(), oid.as_str())),
        )
        .unwrap();
        let attempted_move = Command::new("git")
            .current_dir(&r.path)
            .args([
                "update-ref",
                "refs/heads/feature",
                &replacement,
                &expected,
            ])
            .output()
            .unwrap();

        assert!(!attempted_move.status.success(), "prepared ref must stay locked");
        let attempted_baseline_rewind = Command::new("git")
            .current_dir(&r.path)
            .args(["update-ref", "refs/heads/main", &expected, &replacement])
            .output()
            .unwrap();
        assert!(
            !attempted_baseline_rewind.status.success(),
            "prepared merge baseline must stay locked"
        );
        assert_eq!(r.rev("refs/heads/feature"), expected);
        assert_eq!(r.rev("refs/heads/main"), replacement);
        drop(transaction);
        assert_eq!(r.rev("refs/heads/feature"), expected);
        assert!(!r.path.join(".git/refs/heads/feature.lock").exists());
    }

    #[test]
    fn safe_delete_preserves_unmerged_linked_worktree_and_ignored_files() {
        let r = TempRepo::new();
        r.commit(".gitignore", "ignored.log\n");
        let wt = r.path.join("unmerged-linked");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        fs::write(wt.join("feature.txt"), "unmerged commit").unwrap();
        r.git(&["-C", wt.to_str().unwrap(), "add", "feature.txt"]);
        r.git(&[
            "-C",
            wt.to_str().unwrap(),
            "commit",
            "-q",
            "-m",
            "feature commit",
        ]);
        fs::write(wt.join("ignored.log"), "must survive failed deletion").unwrap();

        let err = delete_branch(
            &r.path,
            "feature",
            false,
            false,
            None,
            None,
            Some(wt.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(err.contains("not fully merged"), "unexpected error: {err}");
        assert!(wt.join("ignored.log").exists());
        assert!(r.has_ref("refs/heads/feature"));
        let admin_dir = git_path_output(
            &wt,
            &["rev-parse", "--absolute-git-dir"],
            "test worktree administrative directory",
        )
        .unwrap();
        assert!(!admin_dir.join("HEAD.lock").exists());
    }

    #[test]
    fn delete_branch_refuses_a_worktree_with_an_active_git_head_lock() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let wt = r.path.join("busy-linked");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        let admin_dir = git_path_output(
            &wt,
            &["rev-parse", "--absolute-git-dir"],
            "test worktree administrative directory",
        )
        .unwrap();
        fs::write(admin_dir.join("HEAD.lock"), "another git process").unwrap();

        let err = delete_branch(
            &r.path,
            "feature",
            true,
            false,
            None,
            None,
            Some(wt.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(
            err.contains("Another Git operation"),
            "unexpected error: {err}"
        );
        assert!(wt.exists());
        assert!(r.has_ref("refs/heads/feature"));
    }

    #[test]
    fn delete_branch_refuses_dirty_linked_worktree_without_mutation() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let wt = r.path.join("linked");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        fs::write(wt.join("untracked.txt"), "keep me").unwrap();

        let err = delete_branch(
            &r.path,
            "feature",
            true,
            false,
            None,
            None,
            Some(wt.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(
            err.contains("uncommitted changes"),
            "unexpected error: {err}"
        );
        assert!(wt.join("untracked.txt").exists());
        assert!(r.has_ref("refs/heads/feature"));
    }

    #[test]
    fn delete_branch_refuses_locked_linked_worktree_without_mutation() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let wt = r.path.join("linked");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        r.git(&[
            "worktree",
            "lock",
            "--reason",
            "owned by another task",
            wt.to_str().unwrap(),
        ]);

        let err = delete_branch(
            &r.path,
            "feature",
            true,
            false,
            None,
            None,
            Some(wt.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(err.contains("locked"), "unexpected error: {err}");
        assert!(
            err.contains("owned by another task"),
            "unexpected error: {err}"
        );
        assert!(wt.exists());
        assert!(r.has_ref("refs/heads/feature"));
    }

    #[test]
    fn delete_branch_revalidates_worktree_path_and_branch() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        let wt = r.path.join("linked");
        r.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
            "main",
        ]);
        let wrong = r.path.join("not-the-linked-worktree");

        let err = delete_branch(
            &r.path,
            "feature",
            true,
            false,
            None,
            None,
            Some(wrong.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(
            err.contains("no longer associated"),
            "unexpected error: {err}"
        );
        assert!(wt.exists());
        assert!(r.has_ref("refs/heads/feature"));
    }

    #[test]
    fn delete_branch_never_removes_the_current_main_worktree() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");

        let err = delete_branch(
            &r.path,
            "main",
            true,
            false,
            None,
            None,
            Some(r.path.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(err.contains("current worktree"), "unexpected error: {err}");
        assert!(r.path.exists());
        assert!(r.has_ref("refs/heads/main"));
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
        let res = fast_forward_branch(&clone.path, "main", "origin", "main");
        assert!(res.is_ok(), "ff failed: {:?}", res);

        let after = clone.rev("main");
        assert_ne!(before, after, "main should have advanced");
        assert_eq!(after, origin_main, "main should now equal origin/main");

        let _ = fs::remove_dir_all(&bare);
        let _ = fs::remove_dir_all(&clone_path);
    }

    #[test]
    fn fast_forward_branch_ignores_same_named_tag() {
        // origin = bare repo with main + feature @ c1.
        let bare = unique_dir("ff-tag-bare");
        fs::create_dir_all(&bare).unwrap();
        Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .unwrap();

        let upstream = TempRepo::new();
        upstream.commit("a.txt", "c1");
        upstream.git(&["branch", "feature"]);
        upstream.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        upstream.git(&["push", "-q", "origin", "main", "feature"]);

        // Clone; create a LOCAL feature branch tracking origin/feature, stay on main.
        let clone_path = unique_dir("ff-tag-clone");
        Command::new("git")
            .args(["clone", "-q", bare.to_str().unwrap(), clone_path.to_str().unwrap()])
            .output()
            .unwrap();
        let clone = TempRepo { path: clone_path.clone() };
        clone.git(&["config", "user.email", "t@example.com"]);
        clone.git(&["config", "user.name", "Tester"]);
        clone.git(&["branch", "--track", "feature", "origin/feature"]);

        // Create a TAG also named `feature` at the old tip → ref ambiguity.
        let old = clone.rev("refs/heads/feature");
        clone.git(&["tag", "feature", &old]);

        // Advance origin/feature by c2.
        upstream.git(&["checkout", "-q", "feature"]);
        upstream.commit("b.txt", "c2");
        upstream.git(&["push", "-q", "origin", "feature"]);
        upstream.git(&["checkout", "-q", "main"]);

        fetch(&clone.path, Some("origin")).unwrap();
        let target = clone.rev("refs/remotes/origin/feature");

        let res = fast_forward_branch(&clone.path, "feature", "origin", "feature");
        assert!(res.is_ok(), "ff failed (tag ambiguity not handled): {:?}", res);
        assert_eq!(clone.rev("refs/heads/feature"), target, "branch must advance to origin/feature");
        assert_eq!(clone.rev("refs/tags/feature"), old, "tag must be left untouched");

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
        let res = delete_branch(&work.path, "feature", true, true, Some("origin"), Some("feature"), None);
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
    fn delete_remote_branch_removes_upstream() {
        // origin = bare repo with main + feature.
        let bare = unique_dir("delrb-bare");
        fs::create_dir_all(&bare).unwrap();
        Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .unwrap();

        let upstream = TempRepo::new();
        upstream.commit("a.txt", "c1");
        upstream.git(&["branch", "feature"]);
        upstream.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        upstream.git(&["push", "-q", "origin", "main", "feature"]);

        let clone_path = unique_dir("delrb-clone");
        Command::new("git")
            .args(["clone", "-q", bare.to_str().unwrap(), clone_path.to_str().unwrap()])
            .output()
            .unwrap();
        let clone = TempRepo { path: clone_path.clone() };

        assert!(
            Command::new("git")
                .current_dir(&clone.path)
                .args(["show-ref", "--verify", "--quiet", "refs/remotes/origin/feature"])
                .status()
                .unwrap()
                .success(),
            "origin/feature should exist before delete"
        );

        delete_remote_branch(&clone.path, "origin", "feature").unwrap();

        assert!(
            !Command::new("git")
                .args(["--git-dir", bare.to_str().unwrap(), "show-ref", "--verify", "--quiet", "refs/heads/feature"])
                .status()
                .unwrap()
                .success(),
            "feature should be deleted on the remote"
        );
        assert!(
            !Command::new("git")
                .current_dir(&clone.path)
                .args(["show-ref", "--verify", "--quiet", "refs/remotes/origin/feature"])
                .status()
                .unwrap()
                .success(),
            "origin/feature tracking ref should be gone after delete"
        );

        let _ = fs::remove_dir_all(&bare);
        let _ = fs::remove_dir_all(&clone_path);
    }

    #[test]
    fn delete_remote_branch_self_heals_when_remote_already_gone() {
        // A merged PR whose branch was auto-deleted on the remote leaves a STALE local
        // remote-tracking ref (only `fetch --prune` clears it). "Delete remote branch"
        // then runs `git push --delete`, which fails with "remote ref does not exist" —
        // so the phantom never went away and every retry re-failed (user-reported).
        // delete_remote_branch must treat that as success and prune the stale ref locally.
        let bare = unique_dir("selfheal-bare");
        fs::create_dir_all(&bare).unwrap();
        Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .unwrap();

        let upstream = TempRepo::new();
        upstream.commit("a.txt", "c1");
        upstream.git(&["branch", "feature"]);
        upstream.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
        upstream.git(&["push", "-q", "origin", "main", "feature"]);

        let clone_path = unique_dir("selfheal-clone");
        Command::new("git")
            .args(["clone", "-q", bare.to_str().unwrap(), clone_path.to_str().unwrap()])
            .output()
            .unwrap();
        let clone = TempRepo { path: clone_path.clone() };

        // Simulate the remote-side delete WITHOUT pruning the clone: drop feature on the
        // bare remote directly. The clone still shows refs/remotes/origin/feature (stale).
        Command::new("git")
            .args(["--git-dir", bare.to_str().unwrap(), "update-ref", "-d", "refs/heads/feature"])
            .output()
            .unwrap();
        assert!(
            clone.has_ref("refs/remotes/origin/feature"),
            "precondition: clone still has the stale tracking ref"
        );

        // The natural user action (delete the phantom) must now SUCCEED, not error.
        let res = delete_remote_branch(&clone.path, "origin", "feature");
        assert!(res.is_ok(), "self-heal expected Ok, got {:?}", res);
        assert!(
            !clone.has_ref("refs/remotes/origin/feature"),
            "stale tracking ref must be pruned after self-heal"
        );

        let _ = fs::remove_dir_all(&bare);
        let _ = fs::remove_dir_all(&clone_path);
    }

    #[test]
    fn branch_subjects_lists_newest_first_and_respects_limit() {
        let r = TempRepo::new();
        r.commit("a.txt", "base commit");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.commit("b.txt", "first change");
        r.commit("c.txt", "second change");
        r.commit("d.txt", "third change");

        let all = branch_subjects(&r.path, "main", 50).unwrap();
        assert_eq!(all, vec!["third change", "second change", "first change"]);

        let limited = branch_subjects(&r.path, "main", 2).unwrap();
        assert_eq!(limited, vec!["third change", "second change"]);
    }

    #[test]
    fn branch_subjects_rejects_option_like_base() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        assert!(
            branch_subjects(&r.path, "--all", 10).is_err(),
            "an option-like base must be rejected before shelling out"
        );
    }

    #[test]
    fn branch_subjects_errors_on_unknown_base() {
        let r = TempRepo::new();
        r.commit("a.txt", "A");
        assert!(branch_subjects(&r.path, "no-such-branch", 10).is_err());
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
