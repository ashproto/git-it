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
    c.current_dir(repo)
        .args(["status", "--porcelain=v2", "-z", "--untracked-files=all"]);
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
            // -uall lists files individually, but guard anyway: never surface a
            // blank-named or directory entry (basename of "dir/" is "").
            if path.is_empty() || path.ends_with('/') {
                continue;
            }
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
/// `context` is the number of unchanged context lines around changes (`-U{context}`).
pub fn diff(repo: &Path, path: Option<&str>, staged: bool, context: u32) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .arg("diff")
        .arg("--no-color")
        // Pin the path prefixes. `diff.mnemonicPrefix` rewrites them per-command (`i/`, `w/`, `c/`)
        // and `diff.noprefix` removes them entirely, and this output is not just displayed — the
        // hunks are fed straight back to `git apply`. Under `noprefix` the patch loses a path
        // component to apply's default `-p1` and lands on the WRONG FILE; anything that reads the
        // `a/`…`b/` convention out of the header is likewise wrong. The user's config governs what
        // they read in a terminal, not what this reconstructs and re-applies.
        .args(["--src-prefix=a/", "--dst-prefix=b/"])
        .arg(format!("-U{}", context));
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
pub fn diff_untracked(repo: &Path, path: &str, context: u32) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["diff", "--no-index", "--no-color"])
        .arg(format!("-U{}", context))
        .args(["--", "/dev/null"])
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
pub fn commit_diff(repo: &Path, sha: &str, path: Option<&str>, context: u32) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .args(["show", "--no-color"])
        .arg(format!("-U{}", context))
        .args(["--first-parent", "--format=", "--end-of-options"]);
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

/// Parse `@@ -A[,B] +C[,D] @@[ heading]` into `(old_start, old_n, new_start, new_n, heading)`.
/// An omitted count means 1 — git writes `@@ -5 +5 @@` for a single-line range. `heading` is
/// everything after the closing `@@` (git's function-context hint), returned verbatim so it can
/// be spliced back on. None if the line is not a hunk header.
fn parse_hunk_header(at_line: &str) -> Option<(u64, u64, u64, u64, &str)> {
    let (ranges, heading) = at_line.strip_prefix("@@ ")?.split_once(" @@")?;
    let (old, new) = ranges.split_once(' ')?;
    let range = |s: &str, sign: char| -> Option<(u64, u64)> {
        let s = s.strip_prefix(sign)?;
        Some(match s.split_once(',') {
            Some((start, count)) => (start.parse().ok()?, count.parse().ok()?),
            None => (s.parse().ok()?, 1),
        })
    };
    let (old_start, old_n) = range(old, '-')?;
    let (new_start, new_n) = range(new, '+')?;
    Some((old_start, old_n, new_start, new_n, heading))
}

/// Format a hunk header, always with explicit counts and always newline-terminated.
fn hunk_header(old_start: u64, old_n: u64, new_start: u64, new_n: u64, heading: &str) -> String {
    let heading = heading.strip_suffix('\n').unwrap_or(heading);
    format!("@@ -{},{} +{},{} @@{}\n", old_start, old_n, new_start, new_n, heading)
}

/// Re-anchor one hunk lifted out of a multi-hunk diff so it applies at the right line ALONE.
///
/// `git apply` positions a hunk by the coordinate of the image it is producing: `new_start`
/// going forward, `old_start` going `--reverse` (reverse swaps the two sides). It then searches
/// for the preimage around that line, which is why an off-by-N coordinate usually goes unnoticed.
/// But at `-U0` a pure insertion (forward) or a pure deletion (reverse) has an EMPTY preimage —
/// there is nothing to search for, so git applies at exactly the line named and reports success
/// from the wrong place.
///
/// An extracted hunk carries both coordinates from the FULL diff, where the side we are not
/// applying against is offset by every line the hunks we did NOT extract added or removed earlier
/// in the file. Only the side facing our target is trustworthy: `old_start` for a forward apply
/// (the target is the diff's old image — the index), `new_start` for a reverse one (the target is
/// its new image — the worktree, or the index when unstaging). Keep that side, derive the other.
///
/// `prefix` is the number of unchanged lines before the hunk. git writes a zero-length range as
/// the line it sits AFTER (`-16,0` = insert after old line 16) and a non-empty range as the first
/// line it covers (`prefix + 1`) — that convention is the whole of the arithmetic below.
fn reanchor(old_start: u64, old_n: u64, new_start: u64, new_n: u64, reverse: bool) -> (u64, u64) {
    if reverse {
        let prefix = if new_n == 0 { new_start } else { new_start.saturating_sub(1) };
        (if old_n == 0 { prefix } else { prefix + 1 }, new_start)
    } else {
        let prefix = if old_n == 0 { old_start } else { old_start.saturating_sub(1) };
        (old_start, if new_n == 0 { prefix } else { prefix + 1 })
    }
}

/// Re-anchor a whole hunk taken verbatim from git's diff (see `reanchor`). The body — and so both
/// line counts — is untouched. A header we cannot parse is handed back unchanged: git wrote it, so
/// git can read it, and refusing to guess beats emitting something we invented.
fn reanchor_hunk(hunk: &str, reverse: bool) -> String {
    let mut parts = hunk.splitn(2, '\n');
    let at_line = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("");
    match parse_hunk_header(at_line) {
        Some((old_start, old_n, new_start, new_n, heading)) => {
            let (o, n) = reanchor(old_start, old_n, new_start, new_n, reverse);
            format!("{}{}", hunk_header(o, old_n, n, new_n, heading), body)
        }
        None => hunk.to_string(),
    }
}

/// Pipe a patch (reconstructed from git's own diff output) to `git apply` via stdin.
/// `cached` selects the target: true → `--cached` (the index, for stage/unstage),
/// false → the WORKING TREE only (for discard). Never uses a temp file or shell.
///
/// `zero_context` must be set when the patch came from a `-U0` diff. Without it git enforces
/// "a hunk with no trailing context must match at the end of the file", which a context-free
/// hunk always trips: reverse applies then fail outright, and a forward insertion silently lands
/// at EOF. `--unidiff-zero` lifts exactly that rule, so it is passed ONLY at `-U0` — at any real
/// context depth those checks are the safety net and stay on.
fn git_apply(repo: &Path, patch: &str, reverse: bool, cached: bool, zero_context: bool) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("apply");
    if cached {
        c.arg("--cached");
    }
    if reverse {
        c.arg("--reverse");
    }
    if zero_context {
        c.arg("--unidiff-zero");
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
/// The patch is reconstructed from a diff fetched at the caller's display `context`,
/// so the supplied `hunk_index` lines up with the hunks the user is actually viewing
/// (the frontend fetches the displayed diff at the same context). `reanchor_hunk` repairs
/// the coordinate the extracted hunk inherited from the hunks left behind, so any context
/// depth works — see `reanchor` for why that is not cosmetic at `-U0`.
/// Hunk indices refer to the CURRENT live diff; after a successful stage/unstage the remaining
/// diff re-indexes, so callers must re-fetch the diff before issuing another hunk op.
pub fn stage_hunk(repo: &Path, path: &str, hunk_index: usize, context: u32) -> Result<(), String> {
    let d = diff(repo, Some(path), false, context)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, reanchor_hunk(h, false)), false, true, context == 0)
}

/// Unstage one hunk (by index) of `path`'s STAGED diff.
/// The patch is reconstructed from a diff fetched at the caller's display `context`,
/// so the supplied `hunk_index` lines up with the hunks the user is actually viewing
/// (the frontend fetches the displayed diff at the same context). `reanchor_hunk` repairs
/// the coordinate the extracted hunk inherited from the hunks left behind, so any context
/// depth works — see `reanchor` for why that is not cosmetic at `-U0`.
/// Hunk indices refer to the CURRENT live diff; after a successful stage/unstage the remaining
/// diff re-indexes, so callers must re-fetch the diff before issuing another hunk op.
pub fn unstage_hunk(repo: &Path, path: &str, hunk_index: usize, context: u32) -> Result<(), String> {
    let d = diff(repo, Some(path), true, context)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, reanchor_hunk(h, true)), true, true, context == 0)
}

// ---------------------------------------------------------------------------
// Line-level (intra-hunk) staging
// ---------------------------------------------------------------------------

/// Reject a selection whose change-line ordinals are not one contiguous run.
///
/// Callers select a contiguous RANGE of rows, so the ordinals they produce are always
/// consecutive — context rows carry no ordinal and therefore open no gap. A gapped set
/// means the caller grouped lines that are not adjacent in the hunk. `build_partial_hunk`
/// emits in hunk source order, so such a patch applies cleanly while placing the kept and
/// restored lines in the wrong order — silent corruption with no reflog to recover from.
/// There is no position for the omitted lines that matches the caller's intent, so refusing
/// is the only safe answer.
fn require_contiguous(selected: &[usize]) -> Result<(), String> {
    if selected.is_empty() {
        return Ok(());
    }
    let mut s: Vec<usize> = selected.to_vec();
    s.sort_unstable();
    s.dedup();
    let span = s[s.len() - 1] - s[0] + 1;
    if span != s.len() {
        return Err(format!(
            "refusing a non-contiguous line selection {:?}: those change lines are not adjacent \
             in the hunk, and applying them would reorder the file",
            s
        ));
    }
    Ok(())
}

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
/// The any_real_change guard is unchanged; `\ No newline` handling is described below.
///
/// The `@@` counts are recomputed from the emitted lines and the start lines are re-anchored
/// (see `reanchor`). Dropping lines never moves the side we anchor on: forward, the emitted old
/// side is exactly the hunk's old side (dropped `+` occupy no old line); reverse, the emitted new
/// side is exactly the hunk's new side (dropped `-` occupy no new line).
///
/// `Ok(None)` when the selection keeps no change line (caller supplies its own message).
/// `Err` when the selection cannot be expressed as a patch at all — see the `'\\'` arm.
fn build_partial_hunk(
    hunk: &str,
    selected: &std::collections::HashSet<usize>,
    reverse: bool,
) -> Result<Option<String>, String> {
    let mut lines = hunk.splitn(2, '\n');
    let at_line = lines.next().unwrap_or("");
    let body = lines.next().unwrap_or("");

    let Some((old_start, _, new_start, _, heading)) = parse_hunk_header(at_line) else {
        return Ok(None);
    };

    // Signs of the change lines, indexed by ordinal — the lookahead a `\ No newline`
    // marker needs to tell whether the line it follows is still file-final once the
    // unselected lines are dropped or demoted.
    let signs: Vec<char> = body
        .split_inclusive('\n')
        .filter_map(|l| match l.chars().next() {
            Some(c @ ('+' | '-')) => Some(c),
            _ => None,
        })
        .collect();

    // Does any change line from ordinal `from` onward still occupy `old_side`
    // (or the new side) of the emitted patch? A demoted line becomes context and
    // so occupies both; a dropped one occupies neither.
    let occupies_from = |from: usize, old_side: bool| -> bool {
        signs.iter().enumerate().skip(from).any(|(o, &s)| {
            match (s, selected.contains(&o)) {
                ('+', true) => !old_side,
                ('-', true) => old_side,
                ('+', false) => reverse,
                ('-', false) => !reverse,
                _ => false,
            }
        })
    };

    // What the previous source line was actually emitted as. Paired with the first
    // ordinal that comes after it — the marker arm needs both.
    #[derive(Clone, Copy)]
    enum Emitted {
        Context,
        Del,
        Add,
    }

    let mut old_n: u64 = 0;
    let mut new_n: u64 = 0;
    let mut ord: usize = 0;
    let mut last: Option<(Emitted, usize)> = None;
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
                last = Some((Emitted::Context, ord));
            }
            Some('+') => {
                let this = ord;
                ord += 1;
                if selected.contains(&this) {
                    // Selected addition: keep as `+` in both directions.
                    out.push_str(raw_line);
                    new_n += 1;
                    last = Some((Emitted::Add, ord));
                    any_real_change = true;
                } else if reverse {
                    // Unstage path: this `+` line IS in the staged (new) image,
                    // so the reverse patch must treat it as context.
                    let rest = &raw_line[1..];
                    out.push(' ');
                    out.push_str(rest);
                    old_n += 1;
                    new_n += 1;
                    last = Some((Emitted::Context, ord));
                } else {
                    // Stage path: unselected addition → drop it entirely.
                    last = None;
                }
            }
            Some('-') => {
                let this = ord;
                ord += 1;
                if selected.contains(&this) {
                    // Selected deletion: keep as `-` in both directions.
                    out.push_str(raw_line);
                    old_n += 1;
                    last = Some((Emitted::Del, ord));
                    any_real_change = true;
                } else if reverse {
                    // Unstage path: this `-` line is absent from the staged (new) image,
                    // so drop it entirely (it has no presence in the staged file to anchor on).
                    last = None;
                } else {
                    // Stage path: unselected deletion → demote to context.
                    let rest = &raw_line[1..];
                    out.push(' ');
                    out.push_str(rest);
                    old_n += 1;
                    new_n += 1;
                    last = Some((Emitted::Context, ord));
                }
            }
            Some('\\') => {
                // `\ No newline at end of file` claims the line before it is the LAST line
                // of the image(s) that line belongs to. A partial selection can leave content
                // after it on one of those sides, which makes the claim false — and `git apply`
                // resolves the contradiction by CONCATENATING the two lines onto one, silently
                // and with no reflog on the discard path. So the marker survives only while it
                // is still true.
                match last {
                    Some((Emitted::Del, from)) => {
                        if !occupies_from(from, true) {
                            out.push_str(raw_line);
                        }
                    }
                    Some((Emitted::Add, from)) => {
                        if !occupies_from(from, false) {
                            out.push_str(raw_line);
                        }
                    }
                    Some((Emitted::Context, from)) => {
                        // A context line is one line shared by both images, so it cannot end
                        // one of them and not the other. When the selection asks for exactly
                        // that — staging an addition after a demoted final line, say — no
                        // patch expresses it, and emitting one anyway is how the corruption
                        // happened. Refuse instead; the whole-hunk action still works.
                        let old_more = occupies_from(from, true);
                        let new_more = occupies_from(from, false);
                        if old_more != new_more {
                            return Err("this selection cannot be expressed as a patch: it splits \
                                        a change at a no-newline end of file, where the kept line \
                                        would have to end with a newline on one side and not the \
                                        other. Use the whole-hunk action instead."
                                .to_string());
                        }
                        if !old_more {
                            out.push_str(raw_line);
                        }
                    }
                    None => {}
                }
                // do not change counts or `last`
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
                    last = Some((Emitted::Context, ord));
                }
            }
        }
    }

    if !any_real_change {
        return Ok(None);
    }

    let (o, n) = reanchor(old_start, old_n, new_start, new_n, reverse);
    Ok(Some(format!("{}{}", hunk_header(o, old_n, n, new_n, heading), out)))
}

/// Stage selected lines (change-line ordinals) of one hunk of `path`'s UNSTAGED diff.
/// The diff is fetched at the caller's display `context` so `hunk_index` and the
/// change-line ordinals line up with the diff the user is viewing. `build_partial_hunk`
/// recomputes the `@@` header counts from the emitted lines and re-anchors its start lines
/// (see `reanchor`), so any context depth works.
pub fn stage_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize], context: u32) -> Result<(), String> {
    require_contiguous(selected)?;
    let d = diff(repo, Some(path), false, context)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let partial = build_partial_hunk(h, &set, false)?.ok_or("no lines selected to stage")?;
    git_apply(repo, &format!("{}{}", header, partial), false, true, context == 0)
}

/// Unstage selected lines (change-line ordinals) of one hunk of `path`'s STAGED diff.
/// The diff is fetched at the caller's display `context` so `hunk_index` and the
/// change-line ordinals line up with the diff the user is viewing. `build_partial_hunk`
/// recomputes the `@@` header counts from the emitted lines and re-anchors its start lines
/// (see `reanchor`), so any context depth works.
pub fn unstage_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize], context: u32) -> Result<(), String> {
    require_contiguous(selected)?;
    let d = diff(repo, Some(path), true, context)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let partial = build_partial_hunk(h, &set, true)?.ok_or("no lines selected to unstage")?;
    git_apply(repo, &format!("{}{}", header, partial), true, true, context == 0)
}

/// Build the file header for a discard patch: git's own header, minus the parts that describe
/// the FILE rather than its contents and would otherwise be applied as unasked-for side effects.
///
/// - `old mode` / `new mode`. `git apply` honours a mode pair, so `chmod +x` plus an edited line
///   — one diff, one header — meant "Discard 1 line" also took the executable bit off. Not in the
///   confirmation, not in the line count, not undoable. Always dropped here. Stage and unstage
///   keep the mode deliberately: there it belongs to the same index entry the caller is moving,
///   and the result is recoverable either way.
///
/// - `new file mode` + `--- /dev/null`. True of the whole diff of an intent-to-add path
///   (`git add -N`), which `working_changes` reports as tracked-and-unstaged so the UI offers
///   line-level discard on it. But a partial selection keeps the unselected additions as CONTEXT,
///   which gives the patch an old side the header denies, and git refuses the lot with
///   "new file X depends on old contents". Rewritten to an ordinary content header exactly when
///   the emitted hunk has an old side, so a selection covering every addition still deletes the
///   file the way a whole-hunk discard does.
///
/// The deleted-file mirror (`+++ /dev/null`) needs no such repair: reverse-apply DROPS unselected
/// `-` lines rather than demoting them, so the new side stays empty and the header stays true.
fn discard_header(header: &str, hunk: &str) -> String {
    let keeps_old_side = parse_hunk_header(hunk.lines().next().unwrap_or(""))
        .is_some_and(|(_, old_n, _, _, _)| old_n > 0);
    // Derive the `---` side from the `+++` one rather than re-deriving the path: swapping the
    // leading `b/` leaves git's quoting of exotic paths intact (the quote precedes the prefix,
    // so `"b/od\td"` becomes `"a/od\td"`).
    //
    // Only ever swap a LEADING `b/`. `diff()` pins the prefixes, but if that ever stops being
    // true a blind `replace` would hit the first `b/` inside the pathname instead — turning
    // `w/lib/util.js` into `w/lia/util.js`, which git reads as a rename and applies to a file the
    // user never touched. Verified: it empties the real file and rewrites the innocent one, and
    // returns success. When the prefix is absent we emit nothing and git refuses the patch, which
    // is the only acceptable default on an apply with no reflog behind it.
    let old_side = header
        .lines()
        .find_map(|l| l.strip_prefix("+++ "))
        .and_then(|p| {
            let (quote, rest) = match p.strip_prefix('"') {
                Some(rest) => ("\"", rest),
                None => ("", p),
            };
            rest.strip_prefix("b/")
                .map(|tail| format!("--- {quote}a/{tail}\n"))
        });
    // Repair the new-file header only when there is a real path to repair it WITH. Otherwise
    // leave every line of it alone: an untouched header is the behaviour that shipped before
    // this repair existed, and git refuses it. Half-repairing — dropping `new file mode` while
    // keeping `--- /dev/null` — is a shape nothing has verified.
    let normalize_new_file = keeps_old_side && old_side.is_some();
    header
        .split_inclusive('\n')
        .filter_map(|line| {
            let mode_pair = line.starts_with("old mode ") || line.starts_with("new mode ");
            let new_file = normalize_new_file && line.starts_with("new file mode ");
            if mode_pair || new_file {
                None
            } else if normalize_new_file && line.starts_with("--- /dev/null") {
                old_side.clone()
            } else {
                Some(line.to_string())
            }
        })
        .collect()
}

/// Refuse a discard whose `hunk_index` / ordinals were picked against a different diff.
///
/// `expected_diff` is the exact `diff()` text the caller displayed. The confirmation dialog in
/// front of a discard has no timeout, so an external edit — another editor, another Git It window,
/// a build step — can re-split the file between the click and the confirm. Nothing about a plain
/// integer index says which hunk it meant, so a re-fetched diff would happily reverse-apply hunk N
/// of a file the user never saw, and unlike stage/unstage there is no index or reflog to undo it.
///
/// The comparison is deliberately whole-file rather than per-hunk. Not every edit moves the hunk
/// the user picked — one confined to a later hunk does not — but telling those apart means trusting
/// the same index arithmetic that is in question, and refusing costs one retry while guessing wrong
/// costs work that cannot be recovered. This is additive — `require_contiguous` remains the
/// independent defence against a gapped selection reordering the file.
fn require_unchanged_diff(live: &str, expected: &str) -> Result<(), String> {
    if live == expected {
        return Ok(());
    }
    Err("the file changed since the diff you were shown — nothing was discarded. \
         Check the updated diff and try again."
        .to_string())
}

/// Discard one hunk (by index) of `path`'s UNSTAGED diff: reverse-apply it to the
/// WORKING TREE only (no `--cached`). Because the unstaged diff's old side is the
/// INDEX, the lines revert to their staged state — staged changes to the same file
/// are untouched. DESTRUCTIVE / not undoable.
///
/// `expected_diff` is the diff the caller showed the user; the op is refused if the live diff
/// no longer matches it (see `require_unchanged_diff`). Hunk indices refer to that diff; after a
/// successful op the remaining diff re-indexes, so callers must re-fetch before issuing another.
pub fn discard_hunk(
    repo: &Path,
    path: &str,
    hunk_index: usize,
    expected_diff: &str,
    context: u32,
) -> Result<(), String> {
    let d = diff(repo, Some(path), false, context)?;
    require_unchanged_diff(&d, expected_diff)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let reanchored = reanchor_hunk(h, true);
    git_apply(repo, &format!("{}{}", discard_header(&header, &reanchored), reanchored), true, false, context == 0)
}

/// Discard selected change-line ordinals of one hunk of `path`'s UNSTAGED diff.
/// `build_partial_hunk(.., reverse: true)` emits a patch whose NEW side matches the
/// working file (unselected `+` become context because they ARE present there;
/// unselected `-` are dropped because they are not) — exactly what a worktree
/// reverse-apply needs. DESTRUCTIVE / not undoable.
///
/// `expected_diff` is the diff the caller showed the user; the op is refused if the live diff
/// no longer matches it (see `require_unchanged_diff`).
pub fn discard_lines(
    repo: &Path,
    path: &str,
    hunk_index: usize,
    selected: &[usize],
    expected_diff: &str,
    context: u32,
) -> Result<(), String> {
    require_contiguous(selected)?;
    let d = diff(repo, Some(path), false, context)?;
    require_unchanged_diff(&d, expected_diff)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let partial = build_partial_hunk(h, &set, true)?.ok_or("no lines selected to discard")?;
    git_apply(repo, &format!("{}{}", discard_header(&header, &partial), partial), true, false, context == 0)
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
    fn lists_files_inside_a_new_untracked_directory_individually() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init"); // need a HEAD so status works
        // create a brand-new directory (never been committed) with two files
        fs::create_dir(r.path.join("newdir")).unwrap();
        r.write("newdir/a.txt", "hello\n");
        r.write("newdir/b.txt", "world\n");
        let files = working_changes(&r.path).unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert!(
            paths.contains(&"newdir/a.txt"),
            "newdir/a.txt should be listed individually; got {:?}",
            paths
        );
        assert!(
            paths.contains(&"newdir/b.txt"),
            "newdir/b.txt should be listed individually; got {:?}",
            paths
        );
        assert!(
            !paths.iter().any(|p| p.is_empty() || p.ends_with('/')),
            "no blank or trailing-slash (collapsed dir) entries: {:?}",
            paths
        );
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
        let d = diff(&r.path, Some("a.txt"), false, 3).unwrap();
        assert!(d.contains("+CHANGED"));
        assert!(d.contains("-2"));
    }

    #[test]
    fn diff_untracked_shows_new_file_contents() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("new.txt", "hello\nworld\n"); // untracked — no index entry
        // Plain diff sees nothing for an untracked file…
        assert!(diff(&r.path, Some("new.txt"), false, 3).unwrap().is_empty());
        // …but diff_untracked renders its full contents as additions (exit 1 → Ok).
        let d = diff_untracked(&r.path, "new.txt", 3).unwrap();
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
        let d = diff(&r.path, Some("a.txt"), false, 3).unwrap();
        let (_h, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 2, "two separated edits = two hunks");
        stage_hunk(&r.path, "a.txt", 0, 3).unwrap();
        let staged = diff(&r.path, Some("a.txt"), true, 3).unwrap();
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

    const BASE_20: &str =
        "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\n";
    const TWO_ADD_HUNKS: &str =
        "a\nb\nB2\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nQ2\nq\nr\ns\nt\n";
    const FIRST_ADD_ONLY: &str =
        "a\nb\nB2\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\n";
    const SECOND_ADD_ONLY: &str =
        "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nQ2\nq\nr\ns\nt\n";
    const THREE_LINE_SHIFT_HUNKS: &str =
        "a\nb\nB2a\nB2b\nB2c\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nQ2\nq\nr\ns\nt\n";
    const THREE_LINE_SHIFT_FIRST_ONLY: &str =
        "a\nb\nB2a\nB2b\nB2c\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\n";
    // A 3-line insertion early on, then a PURE DELETION later: the reverse-apply twin of the
    // pure-insertion case above. Restoring `q` has an empty preimage, so nothing anchors the
    // patch except the header coordinate, and the earlier hunk shifts it by 3.
    const SHIFTED_DELETE_HUNKS: &str =
        "a\nb\nB2a\nB2b\nB2c\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nr\ns\nt\n";
    const SHIFTED_DELETE_RESTORED: &str =
        "a\nb\nB2a\nB2b\nB2c\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\n";
    const MIXED_SECOND_HUNK: &str =
        "a\nb\nB2\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nQ2\nr\ns\nt\n";
    const MIXED_SECOND_ONLY: &str =
        "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nQ2\nr\ns\nt\n";
    const MIXED_FIRST_ONLY: &str =
        "a\nb\nB2\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\n";
    const MIXED_SELECTED_DELETION_STAGED: &str =
        "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nr\ns\nt\n";
    const MIXED_SELECTED_DELETION_RESTORED: &str =
        "a\nb\nB2\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nQ2\nr\ns\nt\n";

    fn read_index_file(r: &TempRepo, path: &str) -> String {
        let output = Command::new("git")
            .current_dir(&r.path)
            .arg("show")
            .arg(format!(":{}", path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git show index file: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    /// Discard against the diff as it stands right now — the freshness snapshot the UI
    /// captures when the user clicks. Tests that exercise the stale-snapshot refusal pass
    /// their own `expected_diff` instead.
    fn discard_hunk_now(r: &TempRepo, path: &str, hunk_index: usize, context: u32) -> Result<(), String> {
        let live = diff(&r.path, Some(path), false, context)?;
        discard_hunk(&r.path, path, hunk_index, &live, context)
    }

    fn discard_lines_now(
        r: &TempRepo,
        path: &str,
        hunk_index: usize,
        selected: &[usize],
        context: u32,
    ) -> Result<(), String> {
        let live = diff(&r.path, Some(path), false, context)?;
        discard_lines(&r.path, path, hunk_index, selected, &live, context)
    }

    fn repo_with_second_hunk(
        edited: &str,
        staged: bool,
        context: u32,
        second_header_prefix: &str,
    ) -> TempRepo {
        let r = TempRepo::new();
        r.commit_file("matrix.txt", BASE_20, "init");
        r.write("matrix.txt", edited);
        if staged {
            r.git(&["add", "matrix.txt"]);
        }
        let d = diff(&r.path, Some("matrix.txt"), staged, context).unwrap();
        let (_header, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 2, "expected two hunks, diff was:\n{}", d);
        assert!(
            hunks[1].starts_with(second_header_prefix),
            "second hunk must have the expected divergent starts, diff was:\n{}",
            d
        );
        r
    }

    fn assert_operation_content(
        r: &TempRepo,
        result: Result<(), String>,
        expected_index: &str,
        expected_worktree: &str,
    ) {
        let actual_index = read_index_file(r, "matrix.txt");
        let actual_worktree = fs::read_to_string(r.path.join("matrix.txt")).unwrap();
        assert!(
            result.is_ok()
                && actual_index == expected_index
                && actual_worktree == expected_worktree,
            "result: {:?}\nindex expected:\n{}index actual:\n{}worktree expected:\n{}worktree actual:\n{}",
            result,
            expected_index,
            actual_index,
            expected_worktree,
            actual_worktree
        );
    }

    fn assert_stage_hunk_case(
        edited: &str,
        context: u32,
        second_header_prefix: &str,
        expected_index: &str,
    ) {
        let r = repo_with_second_hunk(edited, false, context, second_header_prefix);
        let result = stage_hunk(&r.path, "matrix.txt", 1, context);
        assert_operation_content(&r, result, expected_index, edited);
    }

    fn assert_unstage_hunk_case(
        edited: &str,
        context: u32,
        second_header_prefix: &str,
        expected_index: &str,
    ) {
        let r = repo_with_second_hunk(edited, true, context, second_header_prefix);
        let result = unstage_hunk(&r.path, "matrix.txt", 1, context);
        assert_operation_content(&r, result, expected_index, edited);
    }

    fn assert_stage_lines_case(
        edited: &str,
        context: u32,
        second_header_prefix: &str,
        selected: &[usize],
        expected_index: &str,
    ) {
        let r = repo_with_second_hunk(edited, false, context, second_header_prefix);
        let result = stage_lines(&r.path, "matrix.txt", 1, selected, context);
        assert_operation_content(&r, result, expected_index, edited);
    }

    fn assert_unstage_lines_case(
        edited: &str,
        context: u32,
        second_header_prefix: &str,
        selected: &[usize],
        expected_index: &str,
    ) {
        let r = repo_with_second_hunk(edited, true, context, second_header_prefix);
        let result = unstage_lines(&r.path, "matrix.txt", 1, selected, context);
        assert_operation_content(&r, result, expected_index, edited);
    }

    fn assert_discard_hunk_case(
        edited: &str,
        context: u32,
        second_header_prefix: &str,
        expected_worktree: &str,
    ) {
        let r = repo_with_second_hunk(edited, false, context, second_header_prefix);
        let result = discard_hunk_now(&r, "matrix.txt", 1, context);
        assert_operation_content(&r, result, BASE_20, expected_worktree);
    }

    fn assert_discard_lines_case(
        edited: &str,
        context: u32,
        second_header_prefix: &str,
        selected: &[usize],
        expected_worktree: &str,
    ) {
        let r = repo_with_second_hunk(edited, false, context, second_header_prefix);
        let result = discard_lines_now(&r, "matrix.txt", 1, selected, context);
        assert_operation_content(&r, result, BASE_20, expected_worktree);
    }

    /// Two added lines; select only ordinal 0. The second add is dropped; new_n reduced by 1.
    #[test]
    fn partial_hunk_two_adds_select_first() {
        let hunk = "@@ -10,3 +10,5 @@\n context\n+add0\n+add1\n context2\n";
        let result = build_partial_hunk(hunk, &set(&[0]), false).unwrap().expect("should produce patch");
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
        let result = build_partial_hunk(hunk, &set(&[0]), false).unwrap().expect("should produce patch");
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
        let result = build_partial_hunk(hunk, &set(&[1]), false).unwrap().expect("should produce patch");
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

        // Select only 1 (the add), so `-old` is demoted to context. This USED to emit a
        // patch keeping the marker after that context line, which asserted `old` was
        // file-final while `+new` followed it. `git apply --cached` accepts that and
        // resolves the contradiction by concatenating: the index became "oldnew", not
        // "old\nnew" (verified against real git). One context line cannot end the old
        // image and not the new one, so no patch expresses this selection — refuse it.
        let err = build_partial_hunk(hunk, &set(&[1]), false).unwrap_err();
        assert!(err.contains("no-newline"), "should name the cause, got: {err}");

        // Now select only 0 (the remove); `+new` is dropped. The old side genuinely ends
        // at `old` with no newline and nothing follows it, so the marker still holds.
        let result_rm_only = build_partial_hunk(hunk, &set(&[0]), false).unwrap().expect("patch");
        // Marker after the kept `-old`, but not a second one after the dropped `+new`.
        let count = result_rm_only.matches("\\ No newline").count();
        assert_eq!(count, 1, "only one no-newline marker (after kept line), got: {}", result_rm_only);
    }

    /// Empty selection → None.
    #[test]
    fn partial_hunk_empty_selection_is_none() {
        let hunk = "@@ -1,2 +1,3 @@\n ctx\n+add\n ctx2\n";
        assert!(build_partial_hunk(hunk, &set(&[]), false).unwrap().is_none());
    }

    /// Count-omitted header `@@ -5 +5 @@` parses old_start as 5.
    #[test]
    fn partial_hunk_count_omitted_header() {
        let hunk = "@@ -5 +5 @@\n+newline\n";
        let result = build_partial_hunk(hunk, &set(&[0]), false).unwrap().expect("patch");
        assert!(result.starts_with("@@ -5,"), "old_start=5: {}", result);
    }

    /// Context-depth independence: a hunk with 6 leading + 6 trailing context lines
    /// around two changes (a `-` and a `+`). build_partial_hunk must select the right
    /// change-line ordinals regardless of how deep the surrounding context is, and the
    /// recomputed header counts must match the emitted body. This proves partial staging
    /// works at any display context (not just -U3), which is the basis for ITEM 6's fix.
    #[test]
    fn partial_hunk_independent_of_context_depth() {
        // 6 context lines, then `-removed` (ord 0) and `+added` (ord 1), then 6 context.
        // Header counts here are illustrative; the builder recomputes them from the body.
        let hunk = "@@ -10,14 +10,14 @@\n c1\n c2\n c3\n c4\n c5\n c6\n-removed\n+added\n c7\n c8\n c9\n c10\n c11\n c12\n";

        // Select only the `+added` (ordinal 1): the unselected `-removed` is demoted to context.
        let result = build_partial_hunk(hunk, &set(&[1]), false).unwrap().expect("should produce patch");
        assert!(result.contains("+added"), "selected add kept");
        assert!(result.contains(" removed"), "unselected minus demoted to context");
        assert!(!result.contains("-removed"), "unselected minus is not a removal");

        // Verify the recomputed header counts match the emitted body lines.
        let mut hl = result.splitn(2, '\n');
        let header = hl.next().unwrap();
        let body = hl.next().unwrap();
        // old_n = lines that exist on the old side (' ' or '-');
        // new_n = lines that exist on the new side (' ' or '+').
        let mut old_n = 0u64;
        let mut new_n = 0u64;
        for line in body.split_inclusive('\n') {
            match line.chars().next() {
                Some(' ') => { old_n += 1; new_n += 1; }
                Some('+') => { new_n += 1; }
                Some('-') => { old_n += 1; }
                _ => {}
            }
        }
        // Body: 12 demoted/real context lines + 1 kept add → old_n=13, new_n=14.
        assert_eq!(old_n, 13, "old count from body");
        assert_eq!(new_n, 14, "new count from body");
        let expected = format!("@@ -10,{} +10,{} @@", old_n, new_n);
        assert_eq!(header, expected, "header counts must match emitted body: {}", result);

        // Select only the `-removed` (ordinal 0): the unselected `+added` is dropped.
        let rm_only = build_partial_hunk(hunk, &set(&[0]), false).unwrap().expect("should produce patch");
        assert!(rm_only.contains("-removed"), "selected minus kept as removal");
        assert!(!rm_only.contains("+added"), "unselected add dropped");
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
        let d = diff(&r.path, Some("f.txt"), false, 3).unwrap();
        let (_h, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 1, "expect 1 hunk");
        // stage only ordinal 1 (line2)
        stage_lines(&r.path, "f.txt", 0, &[1], 3).unwrap();
        // staged diff should contain only +line2
        let staged = diff(&r.path, Some("f.txt"), true, 3).unwrap();
        assert!(staged.contains("+line2"), "line2 should be staged");
        assert!(!staged.contains("+line1"), "line1 should not be staged");
        assert!(!staged.contains("+line3"), "line3 should not be staged");
        // unstaged diff should still show line1 and line3
        let unstaged = diff(&r.path, Some("f.txt"), false, 3).unwrap();
        assert!(unstaged.contains("+line1"), "line1 still unstaged");
        assert!(unstaged.contains("+line3"), "line3 still unstaged");
    }

    // ── context / multi-hunk operation matrix ───────────────────────────────

    #[test]
    fn context_three_second_hunk_stage_hunk_updates_index_at_exact_position() {
        assert_stage_hunk_case(TWO_ADD_HUNKS, 3, "@@ -14,6 +15,7 @@", SECOND_ADD_ONLY);
    }

    #[test]
    fn context_three_second_hunk_unstage_hunk_updates_index_at_exact_position() {
        assert_unstage_hunk_case(TWO_ADD_HUNKS, 3, "@@ -14,6 +15,7 @@", FIRST_ADD_ONLY);
    }

    #[test]
    fn context_three_second_hunk_stage_lines_updates_index_at_exact_position() {
        assert_stage_lines_case(TWO_ADD_HUNKS, 3, "@@ -14,6 +15,7 @@", &[0], SECOND_ADD_ONLY);
    }

    #[test]
    fn context_three_second_hunk_unstage_lines_updates_index_at_exact_position() {
        assert_unstage_lines_case(TWO_ADD_HUNKS, 3, "@@ -14,6 +15,7 @@", &[0], FIRST_ADD_ONLY);
    }

    #[test]
    fn context_three_second_hunk_discard_hunk_updates_worktree_at_exact_position() {
        assert_discard_hunk_case(TWO_ADD_HUNKS, 3, "@@ -14,6 +15,7 @@", FIRST_ADD_ONLY);
    }

    #[test]
    fn context_three_second_hunk_discard_lines_updates_worktree_at_exact_position() {
        assert_discard_lines_case(TWO_ADD_HUNKS, 3, "@@ -14,6 +15,7 @@", &[0], FIRST_ADD_ONLY);
    }

    #[test]
    fn context_zero_second_hunk_stage_hunk_updates_index_at_exact_position() {
        assert_stage_hunk_case(TWO_ADD_HUNKS, 0, "@@ -16,0 +18 @@", SECOND_ADD_ONLY);
    }

    #[test]
    fn context_zero_second_hunk_unstage_hunk_updates_index_at_exact_position() {
        assert_unstage_hunk_case(TWO_ADD_HUNKS, 0, "@@ -16,0 +18 @@", FIRST_ADD_ONLY);
    }

    #[test]
    fn context_zero_second_hunk_stage_lines_updates_index_at_exact_position() {
        assert_stage_lines_case(TWO_ADD_HUNKS, 0, "@@ -16,0 +18 @@", &[0], SECOND_ADD_ONLY);
    }

    #[test]
    fn context_zero_second_hunk_unstage_lines_updates_index_at_exact_position() {
        assert_unstage_lines_case(TWO_ADD_HUNKS, 0, "@@ -16,0 +18 @@", &[0], FIRST_ADD_ONLY);
    }

    #[test]
    fn context_zero_second_hunk_discard_hunk_updates_worktree_at_exact_position() {
        assert_discard_hunk_case(TWO_ADD_HUNKS, 0, "@@ -16,0 +18 @@", FIRST_ADD_ONLY);
    }

    #[test]
    fn context_zero_second_hunk_discard_lines_updates_worktree_at_exact_position() {
        assert_discard_lines_case(TWO_ADD_HUNKS, 0, "@@ -16,0 +18 @@", &[0], FIRST_ADD_ONLY);
    }

    #[test]
    fn context_zero_mixed_second_hunk_stage_hunk_updates_index_exactly() {
        assert_stage_hunk_case(MIXED_SECOND_HUNK, 0, "@@ -17 +18 @@", MIXED_SECOND_ONLY);
    }

    #[test]
    fn context_zero_mixed_second_hunk_unstage_hunk_updates_index_exactly() {
        assert_unstage_hunk_case(MIXED_SECOND_HUNK, 0, "@@ -17 +18 @@", MIXED_FIRST_ONLY);
    }

    #[test]
    fn context_zero_mixed_second_hunk_stage_lines_stages_only_the_deletion() {
        assert_stage_lines_case(
            MIXED_SECOND_HUNK,
            0,
            "@@ -17 +18 @@",
            &[0],
            MIXED_SELECTED_DELETION_STAGED,
        );
    }

    #[test]
    fn context_zero_mixed_second_hunk_unstage_lines_unstages_only_the_deletion() {
        assert_unstage_lines_case(
            MIXED_SECOND_HUNK,
            0,
            "@@ -17 +18 @@",
            &[0],
            MIXED_SELECTED_DELETION_RESTORED,
        );
    }

    #[test]
    fn context_zero_mixed_second_hunk_discard_hunk_updates_worktree_exactly() {
        assert_discard_hunk_case(MIXED_SECOND_HUNK, 0, "@@ -17 +18 @@", MIXED_FIRST_ONLY);
    }

    #[test]
    fn context_zero_mixed_second_hunk_discard_lines_restores_only_the_deletion() {
        assert_discard_lines_case(
            MIXED_SECOND_HUNK,
            0,
            "@@ -17 +18 @@",
            &[0],
            MIXED_SELECTED_DELETION_RESTORED,
        );
    }

    // Reverse-direction twin of the ctx-0 pure-insertion cases: the hunk that RESTORES `q` has
    // an empty preimage, so `git apply --reverse` places it purely by the header coordinate, and
    // the extracted hunk inherits an old-side start that the 3-line earlier insertion has moved.
    // Without `reanchor` the line comes back three rows too high — Ok(()), wrong file.
    #[test]
    fn context_zero_shifted_deletion_discard_hunk_restores_at_exact_position() {
        assert_discard_hunk_case(SHIFTED_DELETE_HUNKS, 0, "@@ -17 +19,0 @@", SHIFTED_DELETE_RESTORED);
    }

    #[test]
    fn context_zero_shifted_deletion_discard_lines_restores_at_exact_position() {
        assert_discard_lines_case(
            SHIFTED_DELETE_HUNKS,
            0,
            "@@ -17 +19,0 @@",
            &[0],
            SHIFTED_DELETE_RESTORED,
        );
    }

    #[test]
    fn context_zero_shifted_deletion_unstage_hunk_restores_at_exact_position() {
        assert_unstage_hunk_case(SHIFTED_DELETE_HUNKS, 0, "@@ -17 +19,0 @@", SHIFTED_DELETE_RESTORED);
    }

    #[test]
    fn context_one_three_line_shift_stage_hunk_updates_index_at_exact_position() {
        assert_stage_hunk_case(
            THREE_LINE_SHIFT_HUNKS,
            1,
            "@@ -16,2 +19,3 @@",
            SECOND_ADD_ONLY,
        );
    }

    #[test]
    fn context_one_three_line_shift_unstage_hunk_updates_index_at_exact_position() {
        assert_unstage_hunk_case(
            THREE_LINE_SHIFT_HUNKS,
            1,
            "@@ -16,2 +19,3 @@",
            THREE_LINE_SHIFT_FIRST_ONLY,
        );
    }

    #[test]
    fn context_one_three_line_shift_stage_lines_updates_index_at_exact_position() {
        assert_stage_lines_case(
            THREE_LINE_SHIFT_HUNKS,
            1,
            "@@ -16,2 +19,3 @@",
            &[0],
            SECOND_ADD_ONLY,
        );
    }

    #[test]
    fn context_one_three_line_shift_unstage_lines_updates_index_at_exact_position() {
        assert_unstage_lines_case(
            THREE_LINE_SHIFT_HUNKS,
            1,
            "@@ -16,2 +19,3 @@",
            &[0],
            THREE_LINE_SHIFT_FIRST_ONLY,
        );
    }

    #[test]
    fn context_one_three_line_shift_discard_hunk_updates_worktree_at_exact_position() {
        assert_discard_hunk_case(
            THREE_LINE_SHIFT_HUNKS,
            1,
            "@@ -16,2 +19,3 @@",
            THREE_LINE_SHIFT_FIRST_ONLY,
        );
    }

    #[test]
    fn context_one_three_line_shift_discard_lines_updates_worktree_at_exact_position() {
        assert_discard_lines_case(
            THREE_LINE_SHIFT_HUNKS,
            1,
            "@@ -16,2 +19,3 @@",
            &[0],
            THREE_LINE_SHIFT_FIRST_ONLY,
        );
    }

    // ── discard (worktree reverse-apply) ─────────────────────────────────────

    #[test]
    fn discard_hunk_reverts_only_that_hunk() {
        let r = TempRepo::new();
        // 20 lines so two separated edits land in two hunks at -U3.
        let base: String = (1..=20).map(|i| format!("line{}\n", i)).collect();
        r.commit_file("f.txt", &base, "init");

        let mut edited: Vec<String> = (1..=20).map(|i| format!("line{}\n", i)).collect();
        edited[1] = "CHANGED2\n".to_string();
        edited[17] = "CHANGED18\n".to_string();
        r.write("f.txt", &edited.concat());

        let d = diff(&r.path, Some("f.txt"), false, 3).unwrap();
        let (_h, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 2, "expected two hunks, diff was:\n{}", d);

        discard_hunk_now(&r, "f.txt", 0, 3).unwrap();

        let now = fs::read_to_string(r.path.join("f.txt")).unwrap();
        assert!(now.contains("line2\n"), "hunk 0 should be reverted");
        assert!(!now.contains("CHANGED2"), "hunk 0's change should be gone");
        assert!(now.contains("CHANGED18"), "hunk 1 must be untouched");
    }

    #[test]
    fn discard_lines_reverts_only_selected() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "base\n", "init");
        r.write("f.txt", "base\nline1\nline2\nline3\n");
        // ordinals 0,1,2 == +line1,+line2,+line3 — discard only line2.
        discard_lines_now(&r, "f.txt", 0, &[1], 3).unwrap();
        assert_eq!(
            fs::read_to_string(r.path.join("f.txt")).unwrap(),
            "base\nline1\nline3\n",
            "only the selected line should be reverted"
        );
    }

    /// The load-bearing one: discarding UNSTAGED lines reverts them to the INDEX
    /// state, not to HEAD, so staged work on the same file survives. This is what
    /// makes Discard safe to sit beside Stage.
    #[test]
    fn discard_lines_leaves_staged_changes_intact() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "base\n", "init");
        r.write("f.txt", "base\nkeep\ndrop\n");

        // Stage only `keep` (ordinal 0); `drop` stays unstaged.
        stage_lines(&r.path, "f.txt", 0, &[0], 3).unwrap();
        let staged = diff(&r.path, Some("f.txt"), true, 3).unwrap();
        assert!(staged.contains("+keep"), "precondition: keep must be staged");

        // The unstaged diff now holds exactly one change line (`+drop`) at ordinal 0.
        discard_lines_now(&r, "f.txt", 0, &[0], 3).unwrap();

        assert_eq!(
            fs::read_to_string(r.path.join("f.txt")).unwrap(),
            "base\nkeep\n",
            "drop reverted to the INDEX state, not HEAD"
        );
        let still = diff(&r.path, Some("f.txt"), true, 3).unwrap();
        assert!(still.contains("+keep"), "staged work must survive the discard");
    }

    #[test]
    fn rejects_a_non_contiguous_selection_instead_of_reordering() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "ctx1\nctx2\naaa\nbbb\nccc\nctx3\nctx4\n", "init");
        r.write("f.txt", "ctx1\nctx2\nXXX\nYYY\nZZZ\nctx3\nctx4\n");

        // Ordinals: -aaa=0 -bbb=1 -ccc=2 +XXX=3 +YYY=4 +ZZZ=5.
        // [2,5] is the shape split view's paired-row selection used to emit.
        let err = discard_lines_now(&r, "f.txt", 0, &[2, 5], 3).unwrap_err();
        assert!(err.contains("non-contiguous"), "unexpected error: {}", err);

        // The refusal must be total — the worktree is untouched.
        assert_eq!(
            fs::read_to_string(r.path.join("f.txt")).unwrap(),
            "ctx1\nctx2\nXXX\nYYY\nZZZ\nctx3\nctx4\n"
        );

        let err = stage_lines(&r.path, "f.txt", 0, &[0, 3], 3).unwrap_err();
        assert!(err.contains("non-contiguous"), "unexpected error: {}", err);
        assert!(r.staged_paths().is_empty(), "nothing may reach the index");
    }

    #[test]
    fn contiguous_selections_are_still_accepted() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "base\n", "init");
        r.write("f.txt", "base\nl1\nl2\nl3\n");
        // Out of order and with a duplicate — still one contiguous run once normalized.
        discard_lines_now(&r, "f.txt", 0, &[2, 1, 1], 3).unwrap();
        assert_eq!(
            fs::read_to_string(r.path.join("f.txt")).unwrap(),
            "base\nl1\n"
        );
    }

    // ── stale diff snapshot (discard TOCTOU) ─────────────────────────────────

    /// Set up the exact race the confirmation dialog opens: the user is shown a two-hunk diff and
    /// picks hunk 1 (`Q2`); while the modal sits there something else edits the file, adding a
    /// change EARLIER in it. Hunk 1 is still in range — it now names the new edit instead.
    /// Returns (repo, the diff the user saw, the file as the user last saw it).
    fn repo_with_diff_changed_under_the_dialog() -> (TempRepo, String, String) {
        // 30 lines so three edits ~9 apart stay three separate hunks at -U3.
        let base: String = (1..=30).map(|i| format!("line{}\n", i)).collect();
        let seen_state = base
            .replace("line3\n", "B2\nline3\n")
            .replace("line21\n", "Q2\nline21\n");

        let r = TempRepo::new();
        r.commit_file("f.txt", &base, "init");
        r.write("f.txt", &seen_state);
        let shown = diff(&r.path, Some("f.txt"), false, 3).unwrap();
        assert_eq!(split_hunks(&shown).1.len(), 2, "user saw two hunks:\n{}", shown);

        // …meanwhile, an external edit lands between `B2` and `Q2`.
        r.write("f.txt", &seen_state.replace("line12\n", "H2\nline12\n"));
        let now = diff(&r.path, Some("f.txt"), false, 3).unwrap();
        assert_eq!(
            split_hunks(&now).1.len(),
            3,
            "index 1 must now name a DIFFERENT hunk:\n{}",
            now
        );
        (r, shown, seen_state)
    }

    /// Stale index 1 would reverse-apply the `H2` hunk — an edit the user never saw, with no
    /// reflog to get it back. The snapshot check must refuse and leave the file alone.
    #[test]
    fn discard_hunk_refuses_a_diff_that_changed_under_the_dialog() {
        let (r, shown, _) = repo_with_diff_changed_under_the_dialog();
        let before = read_file(&r, "f.txt");

        let err = discard_hunk(&r.path, "f.txt", 1, &shown, 3).unwrap_err();
        assert!(err.contains("nothing was discarded"), "unexpected error: {}", err);
        assert_eq!(
            read_file(&r, "f.txt"),
            before,
            "a refused discard must not touch the worktree"
        );
    }

    #[test]
    fn discard_lines_refuses_a_diff_that_changed_under_the_dialog() {
        let (r, shown, _) = repo_with_diff_changed_under_the_dialog();
        let before = read_file(&r, "f.txt");

        let err = discard_lines(&r.path, "f.txt", 1, &[0], &shown, 3).unwrap_err();
        assert!(err.contains("nothing was discarded"), "unexpected error: {}", err);
        assert_eq!(
            read_file(&r, "f.txt"),
            before,
            "a refused discard must not touch the worktree"
        );
    }

    /// The refusal has to be a retry, not a dead end: re-reading the diff and discarding against
    /// THAT succeeds, and hits the hunk the fresh diff actually names.
    #[test]
    fn discard_succeeds_once_the_snapshot_is_refreshed() {
        let (r, shown, seen_state) = repo_with_diff_changed_under_the_dialog();
        assert!(discard_hunk(&r.path, "f.txt", 1, &shown, 3).is_err());

        discard_hunk_now(&r, "f.txt", 1, 3).unwrap();
        assert_eq!(
            read_file(&r, "f.txt"),
            seen_state,
            "hunk 1 of the FRESH diff is the H2 insertion"
        );
    }

    /// The `-` path: discard must put a line BACK, not just remove one. The discard tests
    /// that came before this one all used a pure-addition hunk, so this branch of
    /// `build_partial_hunk` was only ever exercised against the INDEX by `unstage_lines`,
    /// never against the working tree.
    #[test]
    fn discard_lines_restores_a_deleted_line() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "a\nb\nc\n", "init");
        r.write("f.txt", "a\nc\n");
        discard_lines_now(&r, "f.txt", 0, &[0], 3).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("f.txt")).unwrap(), "a\nb\nc\n");
    }

    /// A mixed hunk where only the deletion is discarded: `b` comes back and the
    /// addition `B2` stays. Asserts exact file content so an off-by-one in the
    /// context anchoring is caught rather than passing a substring check.
    #[test]
    fn discard_lines_mixed_hunk_restores_only_the_deletion() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "a\nb\nc\n", "init");
        r.write("f.txt", "a\nB2\nc\n");
        // Ordinals: -b=0, +B2=1. Discard only the deletion.
        discard_lines_now(&r, "f.txt", 0, &[0], 3).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("f.txt")).unwrap(), "a\nb\nB2\nc\n");
    }

    // ---------------------------------------------------------------------
    // Partial selections at a no-newline end of file
    //
    // When the last line of a file with no trailing newline changes, git emits a
    // `-`/`+` pair where BOTH sides carry `\ No newline at end of file`. Selecting
    // one half of that pair asks for a file where the restored line is no longer
    // final — so it must GAIN a trailing newline that the marker denies it.
    //
    // The two directions are not symmetric. Reversing (discard/unstage) keeps the
    // other half as context on the side that still ends there, so dropping the
    // stale marker expresses it exactly. Going forward (stage) would need the
    // demoted line to be newline-terminated on the new side and not on the old —
    // which a single context line cannot say — so that one is refused.
    // ---------------------------------------------------------------------

    fn index_content(r: &TempRepo, f: &str) -> String {
        let o = Command::new("git")
            .current_dir(&r.path)
            .args(["cat-file", "-p", &format!(":{}", f)])
            .output()
            .unwrap();
        assert!(o.status.success(), "git cat-file failed");
        String::from_utf8_lossy(&o.stdout).to_string()
    }

    /// Restoring the deletion must leave `t` and `T2` on SEPARATE lines. Before the
    /// fix this returned Ok and silently produced "a\ntT2" — the two lines merged —
    /// on the discard path, which has no reflog to recover from.
    #[test]
    fn discard_lines_at_no_newline_eof_keeps_lines_separate() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "a\nt", "init");
        r.write("f.txt", "a\nT2");
        // Ordinals: -t=0, +T2=1. Restore the deletion, keep the addition.
        discard_lines_now(&r, "f.txt", 0, &[0], 3).unwrap();
        assert_eq!(read_file(&r, "f.txt"), "a\nt\nT2");
    }

    /// The index twin of the case above: same shape, same merge, same fix.
    #[test]
    fn unstage_lines_at_no_newline_eof_keeps_lines_separate() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "a\nt", "init");
        r.write("f.txt", "a\nT2");
        r.git(&["add", "f.txt"]);
        // Ordinals in the STAGED diff: -t=0, +T2=1. Unstage the deletion.
        unstage_lines(&r.path, "f.txt", 0, &[0], 3).unwrap();
        assert_eq!(index_content(&r, "f.txt"), "a\nt\nT2");
    }

    /// Staging only the addition cannot be expressed: `t` would have to be
    /// newline-terminated in the index and not in HEAD, and one context line cannot
    /// carry both. Refuse rather than emit a patch that applies cleanly and corrupts.
    #[test]
    fn stage_lines_at_no_newline_eof_refuses_rather_than_corrupting() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "a\nt", "init");
        r.write("f.txt", "a\nT2");
        // Ordinals: -t=0, +T2=1. Stage only the addition.
        let err = stage_lines(&r.path, "f.txt", 0, &[1], 3).unwrap_err();
        assert!(
            err.contains("no-newline"),
            "error should name the cause, got: {err}"
        );
        // Nothing staged, and the working tree is untouched.
        assert_eq!(index_content(&r, "f.txt"), "a\nt");
        assert_eq!(read_file(&r, "f.txt"), "a\nT2");
    }

    /// The escape hatch the refusal leaves open: whole-hunk ops replay git's own
    /// hunk verbatim, so they are unaffected by any of this.
    #[test]
    fn whole_hunk_ops_at_no_newline_eof_are_unaffected() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "a\nt", "init");
        r.write("f.txt", "a\nT2");
        stage_hunk(&r.path, "f.txt", 0, 3).unwrap();
        assert_eq!(index_content(&r, "f.txt"), "a\nT2");

        let r2 = TempRepo::new();
        r2.commit_file("f.txt", "a\nt", "init");
        r2.write("f.txt", "a\nT2");
        discard_hunk_now(&r2, "f.txt", 0, 3).unwrap();
        assert_eq!(read_file(&r2, "f.txt"), "a\nt");
    }

    /// `chmod +x` plus an edit makes git put `old mode`/`new mode` in the FILE header, and the
    /// whole header is what gets reverse-applied. Discarding text would then also revert the
    /// executable bit — a change the confirmation never mentions and the line count never counts.
    #[test]
    fn discard_lines_leaves_a_mode_change_alone() {
        use std::os::unix::fs::PermissionsExt;
        let r = TempRepo::new();
        r.commit_file("f.sh", "a\nb\nc\n", "init");
        r.write("f.sh", "a\nB2\nc\n");
        fs::set_permissions(r.path.join("f.sh"), fs::Permissions::from_mode(0o755)).unwrap();

        // Ordinals: -b=0, +B2=1. Discard only the deletion.
        discard_lines_now(&r, "f.sh", 0, &[0], 3).unwrap();

        assert_eq!(read_file(&r, "f.sh"), "a\nb\nB2\nc\n");
        let mode = fs::metadata(r.path.join("f.sh")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "the chmod +x must survive a line discard");
    }

    /// `git add -N` makes a path tracked-but-unstaged, so the UI offers line-level discard on it.
    /// Its diff header says `new file mode` / `--- /dev/null`, and a partial selection keeps the
    /// unselected additions as CONTEXT — giving the patch a non-empty old side the header denies.
    /// Git refused the whole thing with "new file n.txt depends on old contents".
    #[test]
    fn discard_lines_on_an_intent_to_add_file_keeps_the_unselected_lines() {
        let r = TempRepo::new();
        r.commit_file("base.txt", "base\n", "init");
        r.write("n.txt", "a\nb\nc\n");
        r.git(&["add", "-N", "--", "n.txt"]);

        // Ordinals: +a=0, +b=1, +c=2. Discard only the first added line.
        discard_lines_now(&r, "n.txt", 0, &[0], 3).unwrap();

        assert_eq!(read_file(&r, "n.txt"), "b\nc\n");
    }

    /// `diff.mnemonicPrefix` renames the path prefixes per-command (`i/`, `w/`), and `diff.noprefix`
    /// drops them. This output is not merely displayed — it is fed back to `git apply` — so an
    /// unpinned prefix aims the patch at the wrong path. Caught by review: rewriting the `---` side
    /// by replacing the first `b/` anywhere turned `w/lib/util.js` into `w/lia/util.js`, which git
    /// reads as a rename; it emptied the file the user was editing, rewrote an untouched committed
    /// one, and returned Ok. `diff()` now pins `--src-prefix`/`--dst-prefix`.
    #[test]
    fn discard_lines_is_unaffected_by_diff_prefix_config() {
        for (key, value) in [("diff.mnemonicPrefix", "true"), ("diff.noprefix", "true")] {
            let r = TempRepo::new();
            r.git(&["config", key, value]);
            // A committed neighbour whose name is one byte from the `b/`-mangled form of `lib/…`.
            r.git(&["config", "user.email", "t@example.com"]);
            r.write("lia.txt", "a\nb\nc\n");
            r.git(&["add", "--", "lia.txt"]);
            r.git(&["commit", "-qm", "seed"]);
            fs::create_dir_all(r.path.join("lib")).unwrap();
            r.write("lib/util.js", "a\nb\nc\n");
            r.git(&["add", "-N", "--", "lib/util.js"]);

            discard_lines_now(&r, "lib/util.js", 0, &[0], 3)
                .unwrap_or_else(|e| panic!("{key}={value}: discard failed: {e}"));

            assert_eq!(read_file(&r, "lib/util.js"), "b\nc\n", "{key}={value}: wrong file content");
            assert_eq!(read_file(&r, "lia.txt"), "a\nb\nc\n", "{key}={value}: neighbour was touched");
        }
    }

    /// The other side of that repair: when the selection covers every addition the patch has no
    /// old side at all, so the `new file` header is still true and must be left alone — the file
    /// goes away, exactly as a whole-hunk discard would do.
    #[test]
    fn discard_lines_on_an_intent_to_add_file_selecting_all_removes_the_file() {
        let r = TempRepo::new();
        r.commit_file("base.txt", "base\n", "init");
        r.write("n.txt", "a\nb\nc\n");
        r.git(&["add", "-N", "--", "n.txt"]);

        discard_lines_now(&r, "n.txt", 0, &[0, 1, 2], 3).unwrap();

        assert!(!r.path.join("n.txt").exists(), "every added line discarded — file should be gone");
    }

    /// Same leak one function over: `discard_hunk` reverse-applies the same header.
    #[test]
    fn discard_hunk_leaves_a_mode_change_alone() {
        use std::os::unix::fs::PermissionsExt;
        let r = TempRepo::new();
        r.commit_file("f.sh", "a\nb\nc\n", "init");
        r.write("f.sh", "a\nB2\nc\n");
        fs::set_permissions(r.path.join("f.sh"), fs::Permissions::from_mode(0o755)).unwrap();

        discard_hunk_now(&r, "f.sh", 0, 3).unwrap();

        assert_eq!(read_file(&r, "f.sh"), "a\nb\nc\n");
        let mode = fs::metadata(r.path.join("f.sh")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "the chmod +x must survive a hunk discard");
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
        let result = build_partial_hunk(hunk, &set(&[1]), true).unwrap().expect("should produce patch");
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
        let staged = diff(&r.path, Some("g.txt"), true, 3).unwrap();
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
        unstage_lines(&r.path, "g.txt", 0, &[1], 3).unwrap();

        let staged = diff(&r.path, Some("g.txt"), true, 3).unwrap();
        assert!(staged.contains("+lineA"), "lineA should remain staged");
        assert!(!staged.contains("+lineB"), "lineB should be unstaged now");

        let unstaged = diff(&r.path, Some("g.txt"), false, 3).unwrap();
        assert!(unstaged.contains("+lineB"), "lineB should appear in unstaged diff");
        assert!(!unstaged.contains("+lineA"), "lineA should not appear in unstaged diff");
    }

    /// Unstage ordinal [0] (lineA) only → lineA moves to unstaged, lineB stays staged.
    #[test]
    fn unstage_lines_unstages_first_of_two_additions() {
        let r = repo_with_two_staged_additions();
        unstage_lines(&r.path, "g.txt", 0, &[0], 3).unwrap();

        let staged = diff(&r.path, Some("g.txt"), true, 3).unwrap();
        assert!(!staged.contains("+lineA"), "lineA should be unstaged now");
        assert!(staged.contains("+lineB"), "lineB should remain staged");

        let unstaged = diff(&r.path, Some("g.txt"), false, 3).unwrap();
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
        let staged = diff(&r.path, Some("h.txt"), true, 3).unwrap();
        assert!(staged.contains("-lineX"), "setup: lineX deletion staged");
        assert!(staged.contains("-lineY"), "setup: lineY deletion staged");

        // Unstage only the deletion of lineX (ordinal 0)
        unstage_lines(&r.path, "h.txt", 0, &[0], 3).unwrap();

        let staged_after = diff(&r.path, Some("h.txt"), true, 3).unwrap();
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
        stage_lines(&r.path, "rt.txt", 0, &[0], 3).unwrap();
        let staged = diff(&r.path, Some("rt.txt"), true, 3).unwrap();
        assert!(staged.contains("+roundtrip"), "after stage_lines: roundtrip should be staged");

        // Now unstage ordinal 0 → should return to fully unstaged
        unstage_lines(&r.path, "rt.txt", 0, &[0], 3).unwrap();
        let staged_after = diff(&r.path, Some("rt.txt"), true, 3).unwrap();
        assert!(!staged_after.contains("+roundtrip"), "after unstage_lines: nothing staged");

        let unstaged = diff(&r.path, Some("rt.txt"), false, 3).unwrap();
        assert!(unstaged.contains("+roundtrip"), "roundtrip line back in unstaged diff");
    }
}
