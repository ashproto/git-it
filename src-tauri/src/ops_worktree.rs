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

/// Diff for an UNTRACKED file. An untracked file has no index entry, so a plain
/// `git diff` shows nothing — compare it against /dev/null with `--no-index` so the
/// new file's full contents render as additions. `--no-index` exits 1 whenever the
/// inputs differ, so this CANNOT reuse `git_ops::run()` (which maps any non-zero status
/// to an error): capture stdout and accept exit code 0 or 1; anything else (e.g. 128)
/// is a real failure. Note exit 1 also covers an access error (e.g. the file vanished
/// between `status` and here) — that yields empty stdout, which we treat as "no diff"
/// (the now-stale row disappears on the next refresh). `--` keeps the path from being
/// read as an option.
pub fn diff_untracked(repo: &Path, path: &str) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["diff", "--no-index", "--no-color", "-U3", "--", "/dev/null"])
        .arg(path);
    let output = c
        .output()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;
    match output.status.code() {
        Some(0) | Some(1) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        other => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(if stderr.trim().is_empty() {
                format!("git diff --no-index failed (exit {:?})", other)
            } else {
                stderr.into_owned()
            })
        }
    }
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

// ---------------------------------------------------------------------------
// Line-level (intra-hunk) staging
// ---------------------------------------------------------------------------

/// Build a partial single-hunk patch keeping only the selected change lines.
/// `selected` holds ORDINALS over the hunk's change lines (the +/- lines, counted
/// in order starting at 0; context and `\ No newline` lines are NOT counted).
///
/// When `reverse` is false (staging / `git apply --cached`):
///   - Unselected `+` lines are dropped.
///   - Unselected `-` lines become context (present in working tree, not yet staged).
///
/// When `reverse` is true (unstaging / `git apply --cached --reverse`):
///   - The patch's NEW side must match the staged file.
///   - Unselected `+` lines stay as context (they ARE in the staged file).
///   - Unselected `-` lines are dropped (they are absent from the staged file).
///
/// Selected `+`/`-` lines: kept as-is in both directions (mark real change).
/// Context, `\ No newline` handling, header recompute, and the any_real_change
/// guard are unchanged.
/// Returns None when the selection keeps no change line (caller should no-op).
fn build_partial_hunk(hunk: &str, selected: &std::collections::HashSet<usize>, reverse: bool) -> Option<String> {
    let mut lines = hunk.splitn(2, '\n');
    let at_line = lines.next().unwrap_or("");
    let body = lines.next().unwrap_or("");

    // Parse @@ -A[,B] +C[,D] @@ [heading]
    // We only need A (old_start). Be tolerant of missing counts.
    let old_start: u64 = {
        // Find "-A" after "@@"
        let after_at = at_line.trim_start_matches('@').trim_start_matches(' ');
        // e.g. "-12,6 +12,7 @@ heading" or "-5 +5 @@"
        let old_part = after_at.trim_start_matches('-');
        let end = old_part.find(|c: char| c == ',' || c == ' ').unwrap_or(old_part.len());
        old_part[..end].parse().unwrap_or(1)
    };

    let mut old_n: u64 = 0;
    let mut new_n: u64 = 0;
    let mut ord: usize = 0;
    let mut last_emitted = false;
    let mut any_real_change = false;
    let mut out = String::new();

    for raw_line in body.split_inclusive('\n') {
        // strip_inclusive('\n') preserves the newline; handle lines that may or
        // may not end with \n (last line of hunk).
        let first = raw_line.chars().next();
        match first {
            Some(' ') => {
                out.push_str(raw_line);
                old_n += 1;
                new_n += 1;
                last_emitted = true;
            }
            Some('+') => {
                let this = ord;
                ord += 1;
                if selected.contains(&this) {
                    // Selected addition: keep as `+` in both directions.
                    out.push_str(raw_line);
                    new_n += 1;
                    last_emitted = true;
                    any_real_change = true;
                } else if reverse {
                    // Unstage path: this `+` line IS in the staged (new) image,
                    // so the reverse patch must treat it as context.
                    let rest = &raw_line[1..];
                    out.push(' ');
                    out.push_str(rest);
                    old_n += 1;
                    new_n += 1;
                    last_emitted = true;
                } else {
                    // Stage path: unselected addition → drop it entirely.
                    last_emitted = false;
                }
            }
            Some('-') => {
                let this = ord;
                ord += 1;
                if selected.contains(&this) {
                    // Selected deletion: keep as `-` in both directions.
                    out.push_str(raw_line);
                    old_n += 1;
                    last_emitted = true;
                    any_real_change = true;
                } else if reverse {
                    // Unstage path: this `-` line is absent from the staged (new) image,
                    // so drop it entirely (it has no presence in the staged file to anchor on).
                    last_emitted = false;
                } else {
                    // Stage path: unselected deletion → demote to context.
                    let rest = &raw_line[1..];
                    out.push(' ');
                    out.push_str(rest);
                    old_n += 1;
                    new_n += 1;
                    last_emitted = true;
                }
            }
            Some('\\') => {
                // "\ No newline at end of file" — keep only if last line was emitted
                if last_emitted {
                    out.push_str(raw_line);
                }
                // do not change counts or last_emitted
            }
            None | Some(_) => {
                // bare empty line or other: treat as context
                if !raw_line.trim().is_empty() || raw_line.contains('\n') {
                    let content = if raw_line.trim().is_empty() {
                        // empty context line: emit as " \n" or " " without trailing \n
                        if raw_line.ends_with('\n') { " \n".to_string() } else { " ".to_string() }
                    } else {
                        format!(" {}", raw_line)
                    };
                    out.push_str(&content);
                    old_n += 1;
                    new_n += 1;
                    last_emitted = true;
                }
            }
        }
    }

    if !any_real_change {
        return None;
    }

    let header = format!("@@ -{},{} +{},{} @@\n", old_start, old_n, old_start, new_n);
    Some(format!("{}{}", header, out))
}

/// Stage selected lines (change-line ordinals) of one hunk of `path`'s UNSTAGED diff.
pub fn stage_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize]) -> Result<(), String> {
    let d = diff(repo, Some(path), false)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let partial = build_partial_hunk(h, &set, false).ok_or("no lines selected to stage")?;
    git_apply(repo, &format!("{}{}", header, partial), false)
}

/// Unstage selected lines (change-line ordinals) of one hunk of `path`'s STAGED diff.
pub fn unstage_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize]) -> Result<(), String> {
    let d = diff(repo, Some(path), true)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let partial = build_partial_hunk(h, &set, true).ok_or("no lines selected to unstage")?;
    git_apply(repo, &format!("{}{}", header, partial), true)
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
    fn diff_untracked_shows_new_file_contents() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("new.txt", "hello\nworld\n"); // untracked — no index entry
        // Plain diff sees nothing for an untracked file…
        assert!(diff(&r.path, Some("new.txt"), false).unwrap().is_empty());
        // …but diff_untracked renders its full contents as additions (exit 1 → Ok).
        let d = diff_untracked(&r.path, "new.txt").unwrap();
        assert!(d.contains("+hello"), "untracked contents shown: {}", d);
        assert!(d.contains("+world"), "untracked contents shown: {}", d);
        assert!(d.contains("new.txt"), "file path present: {}", d);
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

    // ── build_partial_hunk unit tests ────────────────────────────────────────

    fn set(v: &[usize]) -> std::collections::HashSet<usize> {
        v.iter().copied().collect()
    }

    /// Two added lines; select only ordinal 0. The second add is dropped; new_n reduced by 1.
    #[test]
    fn partial_hunk_two_adds_select_first() {
        let hunk = "@@ -10,3 +10,5 @@\n context\n+add0\n+add1\n context2\n";
        let result = build_partial_hunk(hunk, &set(&[0]), false).expect("should produce patch");
        assert!(result.contains("+add0"), "selected add kept");
        assert!(!result.contains("+add1"), "unselected add dropped");
        // old_n: 2 context lines = 2; new_n: 2 context + 1 kept add = 3
        assert!(result.starts_with("@@ -10,2 +10,3 @@\n"), "header: {}", result);
    }

    /// A `-` line that IS selected stays as removal; one that is NOT selected becomes context.
    #[test]
    fn partial_hunk_minus_kept_vs_demoted() {
        // hunk with two `-` lines (ordinals 0,1); select only 0
        let hunk = "@@ -5,4 +5,2 @@\n ctx\n-keep\n-demote\n ctx2\n";
        let result = build_partial_hunk(hunk, &set(&[0]), false).expect("should produce patch");
        // "keep" stays as `-keep`
        assert!(result.contains("-keep"), "kept minus preserved");
        // "demote" becomes ` demote` (context)
        assert!(result.contains(" demote"), "unselected minus demoted to context");
        assert!(!result.contains("-demote"), "unselected minus not a removal");
    }

    /// Mixed ctx,-,+,ctx; select only the `+` (the `-` becomes context).
    /// old_n = 3 (ctx + demoted_minus + ctx), new_n = 4 (ctx + demoted_as_ctx + kept_add + ctx).
    #[test]
    fn partial_hunk_mixed_select_plus_only() {
        let hunk = "@@ -12,4 +12,4 @@\n ctx1\n-removed\n+added\n ctx2\n";
        // ordinal 0 = `-removed`, ordinal 1 = `+added`; select only 1
        let result = build_partial_hunk(hunk, &set(&[1]), false).expect("should produce patch");
        assert!(result.starts_with("@@ -12,3 +12,4 @@\n"), "header: {}", &result);
        assert!(result.contains(" removed"), "demoted to context");
        assert!(!result.contains("-removed"), "not a removal");
        assert!(result.contains("+added"), "add kept");
    }

    /// `\ No newline` kept when its line was emitted, dropped when its `+` was dropped.
    #[test]
    fn partial_hunk_no_newline_marker() {
        let hunk = "@@ -1,1 +1,1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n";
        // ordinal 0 = `-old`, ordinal 1 = `+new`
        // select only 1 (the add); `-old` becomes context
        let result_add_only = build_partial_hunk(hunk, &set(&[1]), false).expect("patch");
        // The no-newline after `-old` becomes context so last_emitted=true → marker kept
        // The no-newline after `+new` which is kept → also kept
        assert!(result_add_only.contains("\\ No newline"), "marker kept after emitted lines");

        // Now select only 0 (the remove); `+new` is dropped
        let result_rm_only = build_partial_hunk(hunk, &set(&[0]), false).expect("patch");
        // The no-newline after `-old` (which is kept): last_emitted=true → kept
        // The no-newline after `+new` (which is dropped): last_emitted=false → dropped
        // We expect marker after the kept `-old`, but not a second one after dropped `+new`
        let count = result_rm_only.matches("\\ No newline").count();
        assert_eq!(count, 1, "only one no-newline marker (after kept line), got: {}", result_rm_only);
    }

    /// Empty selection → None.
    #[test]
    fn partial_hunk_empty_selection_is_none() {
        let hunk = "@@ -1,2 +1,3 @@\n ctx\n+add\n ctx2\n";
        assert!(build_partial_hunk(hunk, &set(&[]), false).is_none());
    }

    /// Count-omitted header `@@ -5 +5 @@` parses old_start as 5.
    #[test]
    fn partial_hunk_count_omitted_header() {
        let hunk = "@@ -5 +5 @@\n+newline\n";
        let result = build_partial_hunk(hunk, &set(&[0]), false).expect("patch");
        assert!(result.starts_with("@@ -5,"), "old_start=5: {}", result);
    }

    // ── stage_lines integration test ─────────────────────────────────────────

    #[test]
    fn stage_lines_stages_only_selected_lines() {
        let r = TempRepo::new();
        // file with multiple added lines in one hunk
        r.commit_file("f.txt", "base\n", "init");
        // overwrite with base + 3 new lines (all in one hunk since they're adjacent)
        r.write("f.txt", "base\nline1\nline2\nline3\n");
        // get the live diff and confirm 1 hunk with 3 change lines (ordinals 0,1,2)
        let d = diff(&r.path, Some("f.txt"), false).unwrap();
        let (_h, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 1, "expect 1 hunk");
        // stage only ordinal 1 (line2)
        stage_lines(&r.path, "f.txt", 0, &[1]).unwrap();
        // staged diff should contain only +line2
        let staged = diff(&r.path, Some("f.txt"), true).unwrap();
        assert!(staged.contains("+line2"), "line2 should be staged");
        assert!(!staged.contains("+line1"), "line1 should not be staged");
        assert!(!staged.contains("+line3"), "line3 should not be staged");
        // unstaged diff should still show line1 and line3
        let unstaged = diff(&r.path, Some("f.txt"), false).unwrap();
        assert!(unstaged.contains("+line1"), "line1 still unstaged");
        assert!(unstaged.contains("+line3"), "line3 still unstaged");
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

    // ── build_partial_hunk REVERSE unit test ─────────────────────────────────

    /// Reverse transform for a mixed ctx,-,+,ctx hunk selecting only the `+`:
    /// - unselected `-` must be DROPPED (not present in the staged/new image).
    /// - unselected `+` would become context, but here the `+` is selected so kept.
    /// old_n = 2 (ctx + ctx), new_n = 3 (ctx + kept_add + ctx).
    /// Also validates the `@@ -N,old_n +N,new_n @@` counts.
    #[test]
    fn partial_hunk_reverse_mixed_select_plus_drops_minus() {
        // ordinal 0 = `-removed`, ordinal 1 = `+added`; select only 1 (the add)
        let hunk = "@@ -12,4 +12,4 @@\n ctx1\n-removed\n+added\n ctx2\n";
        let result = build_partial_hunk(hunk, &set(&[1]), true).expect("should produce patch");
        // `-removed` must be dropped entirely in reverse mode (absent from staged image)
        assert!(!result.contains("-removed"), "unselected minus dropped in reverse");
        assert!(!result.contains(" removed"), "demoted context must NOT appear in reverse");
        // `+added` is selected → kept as addition
        assert!(result.contains("+added"), "selected add kept");
        // ctx1 and ctx2 still present
        assert!(result.contains(" ctx1"), "ctx1 kept");
        assert!(result.contains(" ctx2"), "ctx2 kept");
        // old_n = 2 (ctx1 + ctx2), new_n = 3 (ctx1 + kept_add + ctx2)
        assert!(result.starts_with("@@ -12,2 +12,3 @@\n"), "header: {}", result);
    }

    // ── unstage_lines integration tests ──────────────────────────────────────

    /// Helper: read a file's content from a TempRepo.
    fn read_file(r: &TempRepo, f: &str) -> String {
        fs::read_to_string(r.path.join(f)).unwrap()
    }

    /// Set up: commit base, stage two additions via stage_lines, verify staged state.
    /// Returns TempRepo already in the right state.
    fn repo_with_two_staged_additions() -> TempRepo {
        let r = TempRepo::new();
        r.commit_file("g.txt", "base\n", "init");
        r.write("g.txt", "base\nlineA\nlineB\n");
        // Stage both lines first (full stage)
        stage(&r.path, &["g.txt".into()]).unwrap();
        // Verify staged
        let staged = diff(&r.path, Some("g.txt"), true).unwrap();
        assert!(staged.contains("+lineA"), "setup: lineA staged");
        assert!(staged.contains("+lineB"), "setup: lineB staged");
        r
    }

    /// Unstage ordinal [1] (lineB) only → lineB moves to unstaged, lineA stays staged.
    #[test]
    fn unstage_lines_unstages_last_of_two_additions() {
        let r = repo_with_two_staged_additions();
        // After full stage, worktree == index so unstaged diff is empty.
        // unstage_lines partial-unstages ordinal 1 (lineB) from the staged diff.
        unstage_lines(&r.path, "g.txt", 0, &[1]).unwrap();

        let staged = diff(&r.path, Some("g.txt"), true).unwrap();
        assert!(staged.contains("+lineA"), "lineA should remain staged");
        assert!(!staged.contains("+lineB"), "lineB should be unstaged now");

        let unstaged = diff(&r.path, Some("g.txt"), false).unwrap();
        assert!(unstaged.contains("+lineB"), "lineB should appear in unstaged diff");
        assert!(!unstaged.contains("+lineA"), "lineA should not appear in unstaged diff");
    }

    /// Unstage ordinal [0] (lineA) only → lineA moves to unstaged, lineB stays staged.
    #[test]
    fn unstage_lines_unstages_first_of_two_additions() {
        let r = repo_with_two_staged_additions();
        unstage_lines(&r.path, "g.txt", 0, &[0]).unwrap();

        let staged = diff(&r.path, Some("g.txt"), true).unwrap();
        assert!(!staged.contains("+lineA"), "lineA should be unstaged now");
        assert!(staged.contains("+lineB"), "lineB should remain staged");

        let unstaged = diff(&r.path, Some("g.txt"), false).unwrap();
        assert!(unstaged.contains("+lineA"), "lineA should appear in unstaged diff");
        assert!(!unstaged.contains("+lineB"), "lineB should not appear in unstaged diff");
    }

    /// Staged deletions: commit file with two lines, stage their removal, partially unstage.
    /// Unstage ordinal [0] (the deletion of lineX) only → lineX deletion reverts, lineY stays staged.
    #[test]
    fn unstage_lines_partial_unstage_of_deletions() {
        let r = TempRepo::new();
        r.commit_file("h.txt", "lineX\nlineY\n", "init");
        // Delete both lines
        r.write("h.txt", "");
        stage(&r.path, &["h.txt".into()]).unwrap();
        // Both deletions are staged
        let staged = diff(&r.path, Some("h.txt"), true).unwrap();
        assert!(staged.contains("-lineX"), "setup: lineX deletion staged");
        assert!(staged.contains("-lineY"), "setup: lineY deletion staged");

        // Unstage only the deletion of lineX (ordinal 0)
        unstage_lines(&r.path, "h.txt", 0, &[0]).unwrap();

        let staged_after = diff(&r.path, Some("h.txt"), true).unwrap();
        assert!(!staged_after.contains("-lineX"), "lineX deletion should be unstaged");
        assert!(staged_after.contains("-lineY"), "lineY deletion should remain staged");
    }

    /// Round-trip: stage selected lines, then unstage the same ordinals → file fully unstaged.
    #[test]
    fn unstage_lines_roundtrip_stage_then_unstage() {
        let r = TempRepo::new();
        r.commit_file("rt.txt", "base\n", "init");
        r.write("rt.txt", "base\nroundtrip\n");

        // Stage ordinal 0 (the single addition)
        stage_lines(&r.path, "rt.txt", 0, &[0]).unwrap();
        let staged = diff(&r.path, Some("rt.txt"), true).unwrap();
        assert!(staged.contains("+roundtrip"), "after stage_lines: roundtrip should be staged");

        // Now unstage ordinal 0 → should return to fully unstaged
        unstage_lines(&r.path, "rt.txt", 0, &[0]).unwrap();
        let staged_after = diff(&r.path, Some("rt.txt"), true).unwrap();
        assert!(!staged_after.contains("+roundtrip"), "after unstage_lines: nothing staged");

        let unstaged = diff(&r.path, Some("rt.txt"), false).unwrap();
        assert!(unstaged.contains("+roundtrip"), "roundtrip line back in unstaged diff");
    }
}
