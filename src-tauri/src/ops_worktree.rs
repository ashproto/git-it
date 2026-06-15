use crate::git_ops;
use crate::types::{StashEntry, WorkingFile};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// All working-tree changes (staged, unstaged, untracked, conflicted) as a file list.
/// Parses `git status --porcelain=v2 -z`. Records: `1`/`2` (ordinary/rename, "XY" code),
/// `u` (unmerged), `?` (untracked). `-z` → NUL-terminated, verbatim paths.
pub fn working_changes(repo: &Path) -> Result<Vec<WorkingFile>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["status", "--porcelain=v2", "-z"]);
    let (out, _) = git_ops::run(&mut c)?;
    let mut files = Vec::new();
    // Records are NUL-separated; a `2` (rename) record is followed by an extra NUL-
    // separated origin path which we skip.
    let mut it = out.split('\0').peekable();
    while let Some(rec) = it.next() {
        if rec.is_empty() {
            continue;
        }
        if let Some(rest) = rec.strip_prefix("1 ") {
            // rest = "<XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>"
            // 7 fixed fields before path → splitn(8, ' ').nth(7)
            let x = rest.chars().next().unwrap_or('.');
            let y = rest.chars().nth(1).unwrap_or('.');
            let path = rest.splitn(8, ' ').nth(7).unwrap_or("").to_string();
            files.push(WorkingFile {
                path,
                staged: x != '.',
                unstaged: y != '.',
                untracked: false,
                conflicted: false,
                status: status_label(x, y),
            });
        } else if let Some(rest) = rec.strip_prefix("2 ") {
            // rest = "<XY> <sub> <mH> <mI> <mW> <hH> <hI> <Xscore> <path>"
            // 8 fixed fields before path → splitn(9, ' ').nth(8)
            let x = rest.chars().next().unwrap_or('.');
            let y = rest.chars().nth(1).unwrap_or('.');
            let path = rest.splitn(9, ' ').nth(8).unwrap_or("").to_string();
            let _ = it.next(); // consume the NUL-separated rename origin path
            files.push(WorkingFile {
                path,
                staged: x != '.',
                unstaged: y != '.',
                untracked: false,
                conflicted: false,
                status: status_label(x, y),
            });
        } else if let Some(rest) = rec.strip_prefix("u ") {
            // rest = "<XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>"
            // 9 fixed fields before path → splitn(10, ' ').nth(9)
            let path = rest.splitn(10, ' ').nth(9).unwrap_or("").to_string();
            files.push(WorkingFile {
                path,
                staged: false,
                unstaged: true,
                untracked: false,
                conflicted: true,
                status: "conflicted".into(),
            });
        } else if let Some(path) = rec.strip_prefix("? ") {
            files.push(WorkingFile {
                path: path.to_string(),
                staged: false,
                unstaged: true,
                untracked: true,
                conflicted: false,
                status: "untracked".into(),
            });
        }
    }
    Ok(files)
}

pub fn status_label(x: char, y: char) -> String {
    let c = if x != '.' { x } else { y };
    match c {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'T' => "typechange",
        _ => "modified",
    }
    .to_string()
}

/// Stage paths (also stages untracked files = intent to add + content).
pub fn stage(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).arg("add").arg("--");
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Unstage paths (index → HEAD), keeping worktree changes.
pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["restore", "--staged", "--"]);
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Discard tracked-file changes (index + worktree → HEAD). DESTRUCTIVE / not undoable.
pub fn discard(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["restore", "--staged", "--worktree", "--"]);
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Remove untracked files. DESTRUCTIVE / not undoable.
pub fn clean(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["clean", "-f", "--"]);
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Commit the staged changes. Message via a temp file (never the command line).
pub fn commit(repo: &Path, message: &str, signoff: bool) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("gte-commit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("tmp: {}", e))?;
    let mf = dir.join("msg");
    std::fs::write(&mf, message).map_err(|e| format!("msg: {}", e))?;
    let mut c = Command::new("git");
    c.current_dir(repo).arg("commit").arg("-F").arg(&mf);
    if signoff {
        c.arg("--signoff");
    }
    let res = git_ops::run(&mut c);
    let _ = std::fs::remove_dir_all(&dir);
    res?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Task 2: diff + hunk staging + stash
// ---------------------------------------------------------------------------

/// Unified diff text. `staged` → index vs HEAD; else worktree vs index. `path` scopes it.
pub fn diff(repo: &Path, path: Option<&str>, staged: bool) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("diff").arg("--no-color").arg("-U3");
    if staged {
        c.arg("--cached");
    }
    c.arg("--");
    if let Some(p) = path {
        c.arg(p);
    }
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out)
}

/// A commit's diff (vs its first parent), for CommitDetail.
pub fn commit_diff(repo: &Path, sha: &str, path: Option<&str>) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args([
        "show",
        "--no-color",
        "-U3",
        "--first-parent",
        "--format=",
        "--end-of-options",
    ]);
    c.arg(sha).arg("--");
    if let Some(p) = path {
        c.arg(p);
    }
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out)
}

/// Split a `git diff` for a SINGLE file into (header, Vec<hunk_text>). header is everything
/// before the first `@@`; each hunk starts at an `@@` line and runs to the next `@@`/EOF.
pub fn split_hunks(diff: &str) -> (String, Vec<String>) {
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    let mut in_hunks = false;
    for line in diff.split_inclusive('\n') {
        if line.starts_with("@@") {
            in_hunks = true;
            hunks.push(String::new());
        }
        if in_hunks {
            if let Some(last) = hunks.last_mut() {
                last.push_str(line);
            }
        } else {
            header.push_str(line);
        }
    }
    (header, hunks)
}

/// Pipe a patch (reconstructed from git's own diff output) to `git apply --cached [--reverse]`
/// via stdin. Never uses a temp file or shell.
fn git_apply(repo: &Path, patch: &str, reverse: bool) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("apply").arg("--cached");
    if reverse {
        c.arg("--reverse");
    }
    c.arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("spawn apply: {}", e))?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(patch.as_bytes())
        .map_err(|e| format!("write patch: {}", e))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("apply: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

/// Stage one hunk (by index) of `path`'s UNSTAGED diff.
/// Hunk indices refer to the CURRENT live diff; after a successful stage/unstage the remaining
/// diff re-indexes, so callers must re-fetch the diff before issuing another hunk op.
pub fn stage_hunk(repo: &Path, path: &str, hunk_index: usize) -> Result<(), String> {
    let d = diff(repo, Some(path), false)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, h), false)
}

/// Unstage one hunk (by index) of `path`'s STAGED diff.
/// Hunk indices refer to the CURRENT live diff; after a successful stage/unstage the remaining
/// diff re-indexes, so callers must re-fetch the diff before issuing another hunk op.
pub fn unstage_hunk(repo: &Path, path: &str, hunk_index: usize) -> Result<(), String> {
    let d = diff(repo, Some(path), true)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, h), true)
}

/// Stash current changes (staged + unstaged). Message is optional.
pub fn stash_push(repo: &Path, message: Option<&str>) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["stash", "push"]);
    if let Some(m) = message {
        c.arg("-m").arg(m);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// List stash entries.
pub fn stash_list(repo: &Path) -> Result<Vec<StashEntry>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["stash", "list", "--format=%gd%x1f%H%x1f%gs"]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out
        .lines()
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, l)| {
            let mut p = l.splitn(3, '\u{1f}');
            let _selector = p.next().unwrap_or("");
            let sha = p.next().unwrap_or("").to_string();
            let message = p.next().unwrap_or("").to_string();
            StashEntry {
                index: i as u32,
                sha,
                message,
            }
        })
        .collect())
}

/// Build a stash refspec from a validated u32 index.
fn stash_ref(index: u32) -> String {
    format!("stash@{{{}}}", index)
}

/// Apply a stash (keep it in the stash list).
pub fn stash_apply(repo: &Path, index: u32) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["stash", "apply", "--end-of-options", &stash_ref(index)]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Pop a stash (apply + drop).
pub fn stash_pop(repo: &Path, index: u32) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["stash", "pop", "--end-of-options", &stash_ref(index)]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Drop a stash entry without applying it.
pub fn stash_drop(repo: &Path, index: u32) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["stash", "drop", "--end-of-options", &stash_ref(index)]);
    git_ops::run(&mut c)?;
    Ok(())
}

// ---------------------------------------------------------------------------

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
                std::env::temp_dir().join(format!("gte-wt-{}-{}", std::process::id(), id));
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
            assert!(
                o.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&o.stderr)
            );
        }
        fn write(&self, f: &str, s: &str) {
            fs::write(self.path.join(f), s).unwrap();
        }
        fn commit_file(&self, f: &str, s: &str, m: &str) {
            self.write(f, s);
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", m]);
        }
        fn staged_paths(&self) -> Vec<String> {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(["diff", "--cached", "--name-only"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        }
    }
    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn lists_untracked_modified_staged() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n"); // modified (unstaged)
        r.write("b.txt", "new\n"); // untracked
        r.write("c.txt", "c\n");
        r.git(&["add", "c.txt"]); // staged add
        let f = working_changes(&r.path).unwrap();
        let by = |p: &str| f.iter().find(|x| x.path == p).cloned();
        assert!(by("a.txt").unwrap().unstaged);
        assert!(by("b.txt").unwrap().untracked);
        assert!(by("c.txt").unwrap().staged);
    }

    #[test]
    fn stage_then_unstage_roundtrip() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n");
        stage(&r.path, &["a.txt".into()]).unwrap();
        assert_eq!(r.staged_paths(), vec!["a.txt".to_string()]);
        unstage(&r.path, &["a.txt".into()]).unwrap();
        assert!(r.staged_paths().is_empty());
    }

    #[test]
    fn discard_reverts_tracked_changes() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "orig\n", "init");
        r.write("a.txt", "changed\n");
        discard(&r.path, &["a.txt".into()]).unwrap();
        assert_eq!(
            fs::read_to_string(r.path.join("a.txt")).unwrap(),
            "orig\n"
        );
    }

    #[test]
    fn clean_removes_untracked() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("junk.txt", "x\n");
        clean(&r.path, &["junk.txt".into()]).unwrap();
        assert!(!r.path.join("junk.txt").exists());
    }

    #[test]
    fn commit_creates_commit_from_staged() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n");
        stage(&r.path, &["a.txt".into()]).unwrap();
        commit(&r.path, "second commit", false).unwrap();
        let o = Command::new("git")
            .current_dir(&r.path)
            .args(["log", "-1", "--format=%s"])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&o.stdout).trim(),
            "second commit"
        );
    }

    #[test]
    fn stage_path_cannot_be_option() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        // A path that looks like an option must be a (non-matching) pathspec, not a flag.
        assert!(stage(&r.path, &["--all".into()]).is_err());
    }

    #[test]
    fn diff_shows_change() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n2\n3\n", "init");
        r.write("a.txt", "1\nCHANGED\n3\n");
        let d = diff(&r.path, Some("a.txt"), false).unwrap();
        assert!(d.contains("+CHANGED"));
        assert!(d.contains("-2"));
    }

    #[test]
    fn stage_hunk_stages_only_that_hunk() {
        let r = TempRepo::new();
        // two well-separated change regions → two hunks
        r.commit_file("a.txt", "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n", "init");
        r.write("a.txt", "A\nb\nc\nd\ne\nf\ng\nh\ni\nJ\n"); // change line 1 and line 10
        let d = diff(&r.path, Some("a.txt"), false).unwrap();
        let (_h, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 2, "two separated edits = two hunks");
        stage_hunk(&r.path, "a.txt", 0).unwrap();
        let staged = diff(&r.path, Some("a.txt"), true).unwrap();
        assert!(staged.contains("+A"));
        assert!(!staged.contains("+J"), "only hunk 0 should be staged");
    }

    #[test]
    fn stash_push_list_pop() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n");
        stash_push(&r.path, Some("wip")).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("a.txt")).unwrap(), "1\n", "stash reverts worktree");
        let list = stash_list(&r.path).unwrap();
        assert_eq!(list.len(), 1);
        stash_pop(&r.path, 0).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("a.txt")).unwrap(), "2\n", "pop restores");
    }

    #[test]
    fn working_changes_handles_space_in_path() {
        let r = TempRepo::new();
        // Commit a file with a space in its name so it has an index entry.
        r.commit_file("my file.txt", "original\n", "init");

        // Modify the tracked spaced file (produces a `1` record with unstaged change).
        r.write("my file.txt", "modified\n");

        // Stage a new spaced file (produces a `1` record with staged add).
        r.write("staged file.txt", "staged\n");
        r.git(&["add", "staged file.txt"]);

        // Add an untracked file with a space (produces a `?` record).
        r.write("new file.txt", "untracked\n");

        let files = working_changes(&r.path).unwrap();
        let by = |p: &str| files.iter().find(|x| x.path == p).cloned();

        // Modified tracked file: path must be the full "my file.txt", not "file.txt".
        let modified = by("my file.txt").expect("'my file.txt' missing from working_changes");
        assert!(modified.unstaged, "'my file.txt' should be unstaged");
        assert!(!modified.untracked);

        // Staged new file: path must be the full "staged file.txt".
        let staged = by("staged file.txt").expect("'staged file.txt' missing from working_changes");
        assert!(staged.staged, "'staged file.txt' should be staged");

        // Untracked file: path must be the full "new file.txt".
        let untracked = by("new file.txt").expect("'new file.txt' missing from working_changes");
        assert!(untracked.untracked, "'new file.txt' should be untracked");
    }
}
