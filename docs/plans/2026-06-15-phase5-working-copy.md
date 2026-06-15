# Phase 5 — Working Copy + Diff Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add the working copy: a file list of changes (staged/unstaged/untracked/conflicted), whole-file **and hunk-level** stage/unstage/discard, a commit composer, **stash** (push/list/apply/pop/drop), and the line-by-line **diff viewer** (Shiki-highlighted) — surfaced via a synthetic "Uncommitted changes" entry at the top of the history. The diff viewer also fills the existing commit-detail diff placeholder.

**Architecture:** New Rust `ops_worktree.rs`: `working_changes` (file list from `git status --porcelain=v2 -z`), `stage`/`unstage`/`discard`/`clean`, `commit`, `diff` (working/staged/commit → raw unified patch text), `stage_hunk`/`unstage_hunk` (reconstruct a one-hunk patch from git's OWN diff output and pipe to `git apply --cached [--reverse]`), and `stash_*`. The frontend parses the raw patch with a **pure, vitest-tested `parseDiff`**; `DiffView.svelte` renders hunks with **Shiki** syntax highlighting (bundled offline) in unified mode with a split toggle, plus per-hunk stage/unstage. A synthetic "Uncommitted changes" row in `GraphHistory` selects working-copy mode; the bottom panel then shows `WorkingCopyView` (file list + `CommitComposer` + `DiffView`) instead of `CommitDetail`. Discard routes through the Phase 4 confirm-destructive flow (it is the one unrecoverable action — no reflog for uncommitted work).

**Tech Stack:** Rust (git shell-out) + Tauri 2; SvelteKit 5 runes; **Shiki** (`npm i shiki`, bundled by Vite — offline, no CDN); reuse Phase 2–4 patterns.

**Branch:** `feat/phase5-working-copy` (already created).

## SECURITY (every git shell-out with a user operand)
- All path operands separated with `--`: `add -- <paths>`, `restore -- <paths>`, `restore --staged -- <paths>`, `clean -f -- <paths>`, `diff [--cached] -- <path>`, `commit … -- ` (no paths), `apply` reads a patch we built from git's own output.
- Commit message via `-F <tmpfile>` (NEVER `-m <user>` on the command line — avoids any arg/quoting edge; the message is written to a temp file). `--amend`/`--signoff` are fixed flags.
- `git show`/commit-diff ref via `--end-of-options <sha>`. Stash index used only as a fixed `stash@{N}` built from a validated integer.
- Hunk patches are reconstructed from `git diff` output (never user text) and applied via `git apply --cached` reading the patch on **stdin** (no shell). The path in the diff comes from git.

## SAFETY
- **Discard** (and **clean** untracked) are destructive and **not undoable** (no reflog/bundle covers uncommitted/untracked work). Route through `gitActions` confirm-destructive with a blunt consequence ("Permanently discard changes to N file(s) — this cannot be undone."). No bundle (a `--all` bundle wouldn't capture uncommitted work); offer "Stash instead" as the non-destructive alternative in the UI copy.
- Commit/stage/unstage are non-destructive → no confirm.

## File structure
- Create `src-tauri/src/ops_worktree.rs`; modify `types.rs`, `commands.rs`, `lib.rs`.
- Create `src/lib/diff/parse.ts` (+ `parse.test.ts`), `src/lib/diff/highlight.ts` (Shiki), `src/lib/diff/types.ts`.
- Create components: `DiffView.svelte`, `WorkingCopyView.svelte`, `CommitComposer.svelte`, `StashPanel.svelte`.
- Modify `store.svelte.ts`, `gitActions.ts`, `api.ts`, `types.ts`, `GraphHistory.svelte` (synthetic row), `+page.svelte` (bottom-panel switch), `CommitDetail.svelte` (mount DiffView), `Sidebar.svelte` (StashPanel), `package.json` (shiki).

---

## Task 1 — Backend: working file list + stage/unstage/discard/clean + commit

**Files:** create `src-tauri/src/ops_worktree.rs`; modify `types.rs`, `commands.rs`, `lib.rs`, `src/lib/types.ts`, `src/lib/api.ts`.

- [ ] **Step 1: Types (`types.rs`):**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingFile {
    pub path: String,
    pub staged: bool,     // has an index change (X)
    pub unstaged: bool,   // has a worktree change (Y)
    pub untracked: bool,
    pub conflicted: bool,
    pub status: String,   // human label: "modified"|"added"|"deleted"|"renamed"|"untracked"|"conflicted"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    pub index: u32,
    pub message: String,
    pub sha: String,
}
```

- [ ] **Step 2: Create `ops_worktree.rs` (this task's ops) + tests.**

```rust
use crate::git_ops;
use crate::types::WorkingFile;
use std::path::Path;
use std::process::Command;

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
        if let Some(rest) = rec.strip_prefix("1 ").or_else(|| rec.strip_prefix("2 ")) {
            let is_rename = rec.starts_with("2 ");
            // rest = "<XY> <sub> <mH> <mI> <mW> <hH> <hI> [<Xscore>] <path>"
            let xy: Vec<char> = rest.chars().take(2).collect();
            let x = *xy.first().unwrap_or(&'.');
            let y = *xy.get(1).unwrap_or(&'.');
            // path is the last space-separated field of the record
            let path = rest.rsplit(' ').next().unwrap_or("").to_string();
            if is_rename {
                let _ = it.next(); // consume the rename origin path
            }
            files.push(WorkingFile {
                path,
                staged: x != '.',
                unstaged: y != '.',
                untracked: false,
                conflicted: false,
                status: status_label(x, y),
            });
        } else if let Some(rest) = rec.strip_prefix("u ") {
            let path = rest.rsplit(' ').next().unwrap_or("").to_string();
            files.push(WorkingFile { path, staged: false, unstaged: true, untracked: false, conflicted: true, status: "conflicted".into() });
        } else if let Some(path) = rec.strip_prefix("? ") {
            files.push(WorkingFile { path: path.to_string(), staged: false, unstaged: true, untracked: true, conflicted: false, status: "untracked".into() });
        }
    }
    Ok(files)
}

fn status_label(x: char, y: char) -> String {
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
    if paths.is_empty() { return Ok(()); }
    let mut c = Command::new("git");
    c.current_dir(repo).arg("add").arg("--");
    for p in paths { c.arg(p); }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Unstage paths (index → HEAD), keeping worktree changes.
pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() { return Ok(()); }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["restore", "--staged", "--"]);
    for p in paths { c.arg(p); }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Discard tracked-file changes (index + worktree → HEAD). DESTRUCTIVE / not undoable.
pub fn discard(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() { return Ok(()); }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["restore", "--staged", "--worktree", "--"]);
    for p in paths { c.arg(p); }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Remove untracked files. DESTRUCTIVE / not undoable.
pub fn clean(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() { return Ok(()); }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["clean", "-f", "--"]);
    for p in paths { c.arg(p); }
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
    if signoff { c.arg("--signoff"); }
    let res = git_ops::run(&mut c);
    let _ = std::fs::remove_dir_all(&dir);
    res?;
    Ok(())
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
            let path = std::env::temp_dir().join(format!("gte-wt-{}-{}", std::process::id(), id));
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
                .env("GIT_AUTHOR_DATE","2020-01-01T00:00:00 +0000").env("GIT_COMMITTER_DATE","2020-01-01T00:00:00 +0000")
                .output().unwrap();
            assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
        }
        fn write(&self, f: &str, s: &str) { fs::write(self.path.join(f), s).unwrap(); }
        fn commit_file(&self, f: &str, s: &str, m: &str) { self.write(f, s); self.git(&["add","."]); self.git(&["commit","-q","-m",m]); }
        fn staged_paths(&self) -> Vec<String> {
            let o = Command::new("git").current_dir(&self.path).args(["diff","--cached","--name-only"]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).lines().map(|s| s.to_string()).collect()
        }
    }
    impl Drop for TempRepo { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); } }

    #[test]
    fn lists_untracked_modified_staged() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n");        // modified (unstaged)
        r.write("b.txt", "new\n");      // untracked
        r.write("c.txt", "c\n"); r.git(&["add","c.txt"]); // staged add
        let f = working_changes(&r.path).unwrap();
        let by = |p: &str| f.iter().find(|x| x.path == p).cloned();
        assert!(by("a.txt").unwrap().unstaged);
        assert!(by("b.txt").unwrap().untracked);
        assert!(by("c.txt").unwrap().staged);
    }

    #[test]
    fn stage_then_unstage_roundtrip() {
        let r = TempRepo::new();
        r.commit_file("a.txt","1\n","init");
        r.write("a.txt","2\n");
        stage(&r.path, &["a.txt".into()]).unwrap();
        assert_eq!(r.staged_paths(), vec!["a.txt".to_string()]);
        unstage(&r.path, &["a.txt".into()]).unwrap();
        assert!(r.staged_paths().is_empty());
    }

    #[test]
    fn discard_reverts_tracked_changes() {
        let r = TempRepo::new();
        r.commit_file("a.txt","orig\n","init");
        r.write("a.txt","changed\n");
        discard(&r.path, &["a.txt".into()]).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("a.txt")).unwrap(), "orig\n");
    }

    #[test]
    fn clean_removes_untracked() {
        let r = TempRepo::new();
        r.commit_file("a.txt","1\n","init");
        r.write("junk.txt","x\n");
        clean(&r.path, &["junk.txt".into()]).unwrap();
        assert!(!r.path.join("junk.txt").exists());
    }

    #[test]
    fn commit_creates_commit_from_staged() {
        let r = TempRepo::new();
        r.commit_file("a.txt","1\n","init");
        r.write("a.txt","2\n"); stage(&r.path,&["a.txt".into()]).unwrap();
        commit(&r.path, "second commit", false).unwrap();
        let o = Command::new("git").current_dir(&r.path).args(["log","-1","--format=%s"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "second commit");
    }

    #[test]
    fn stage_path_cannot_be_option() {
        let r = TempRepo::new();
        r.commit_file("a.txt","1\n","init");
        // A path that looks like an option must be a (non-matching) pathspec, not a flag.
        assert!(stage(&r.path, &["--all".into()]).is_err());
    }
}
```

- [ ] **Step 3: Run tests (expect fail → implement → pass).** From `src-tauri/`: `cargo test` (all green).

- [ ] **Step 4: Commands (`commands.rs`)** (+ `use crate::ops_worktree;`, extend types import with `WorkingFile`):

```rust
#[tauri::command]
pub fn working_changes(repo: String) -> Result<Vec<WorkingFile>, String> {
    ops_worktree::working_changes(&PathBuf::from(repo))
}
#[tauri::command]
pub fn stage(repo: String, paths: Vec<String>) -> Result<(), String> { ops_worktree::stage(&PathBuf::from(repo), &paths) }
#[tauri::command]
pub fn unstage(repo: String, paths: Vec<String>) -> Result<(), String> { ops_worktree::unstage(&PathBuf::from(repo), &paths) }
#[tauri::command]
pub fn discard(repo: String, paths: Vec<String>) -> Result<(), String> { ops_worktree::discard(&PathBuf::from(repo), &paths) }
#[tauri::command]
pub fn clean(repo: String, paths: Vec<String>) -> Result<(), String> { ops_worktree::clean(&PathBuf::from(repo), &paths) }
#[tauri::command]
pub fn commit(repo: String, message: String, signoff: bool) -> Result<(), String> { ops_worktree::commit(&PathBuf::from(repo), &message, signoff) }
```
Register all 6 in `lib.rs` (+ `mod ops_worktree;`).

- [ ] **Step 5: TS mirrors + bindings.** `src/lib/types.ts`:
```ts
export type WorkingFile = { path: string; staged: boolean; unstaged: boolean; untracked: boolean; conflicted: boolean; status: string };
export type StashEntry = { index: number; message: string; sha: string };
```
`src/lib/api.ts` (add `WorkingFile` import):
```ts
  workingChanges: (repo: string) => invoke<WorkingFile[]>("working_changes", { repo }),
  stage: (repo: string, paths: string[]) => invoke<void>("stage", { repo, paths }),
  unstage: (repo: string, paths: string[]) => invoke<void>("unstage", { repo, paths }),
  discard: (repo: string, paths: string[]) => invoke<void>("discard", { repo, paths }),
  clean: (repo: string, paths: string[]) => invoke<void>("clean", { repo, paths }),
  commit: (repo: string, message: string, signoff = false) => invoke<void>("commit", { repo, message, signoff }),
```

- [ ] **Step 6: Gates + commit.** `cargo test` + `cargo check` + `npm run check`. Commit: `feat(worktree): file list + stage/unstage/discard/commit backend (Phase 5)` (+ trailer).

---

## Task 2 — Backend: diff + hunk staging + stash

**Files:** modify `ops_worktree.rs`, `types.rs`, `commands.rs`, `lib.rs`, `src/lib/api.ts`, `src/lib/types.ts`.

- [ ] **Step 1: Diff (raw unified patch text).**

```rust
/// Unified diff text. `staged` → index vs HEAD; else worktree vs index. `path` scopes it.
pub fn diff(repo: &Path, path: Option<&str>, staged: bool) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("diff").arg("--no-color").arg("-U3");
    if staged { c.arg("--cached"); }
    c.arg("--");
    if let Some(p) = path { c.arg(p); }
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out)
}

/// A commit's diff (vs its first parent), for CommitDetail. Includes untracked-as-added.
pub fn commit_diff(repo: &Path, sha: &str, path: Option<&str>) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["show", "--no-color", "-U3", "--first-parent", "--format=", "--end-of-options"]);
    c.arg(sha).arg("--");
    if let Some(p) = path { c.arg(p); }
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out)
}
```

- [ ] **Step 2: Hunk staging via reconstructed one-hunk patch piped to `git apply`.**

```rust
use std::io::Write;
use std::process::Stdio;

/// Split a `git diff` for a SINGLE file into (header, Vec<hunk_text>). header is everything
/// before the first `@@`; each hunk starts at an `@@` line and runs to the next `@@`/EOF.
fn split_hunks(diff: &str) -> (String, Vec<String>) {
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    let mut in_hunks = false;
    for line in diff.split_inclusive('\n') {
        if line.starts_with("@@") {
            in_hunks = true;
            hunks.push(String::new());
        }
        if in_hunks {
            if let Some(last) = hunks.last_mut() { last.push_str(line); }
        } else {
            header.push_str(line);
        }
    }
    (header, hunks)
}

fn git_apply(repo: &Path, patch: &str, reverse: bool) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("apply").arg("--cached");
    if reverse { c.arg("--reverse"); }
    c.arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("spawn apply: {}", e))?;
    child.stdin.take().unwrap().write_all(patch.as_bytes()).map_err(|e| format!("write patch: {}", e))?;
    let out = child.wait_with_output().map_err(|e| format!("apply: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

/// Stage one hunk (by index) of `path`'s UNSTAGED diff.
pub fn stage_hunk(repo: &Path, path: &str, hunk_index: usize) -> Result<(), String> {
    let d = diff(repo, Some(path), false)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, h), false)
}

/// Unstage one hunk (by index) of `path`'s STAGED diff.
pub fn unstage_hunk(repo: &Path, path: &str, hunk_index: usize) -> Result<(), String> {
    let d = diff(repo, Some(path), true)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, h), true)
}
```

- [ ] **Step 3: Stash.**

```rust
use crate::types::StashEntry;

pub fn stash_push(repo: &Path, message: Option<&str>) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["stash", "push"]);
    if let Some(m) = message { c.arg("-m").arg(m); }
    git_ops::run(&mut c)?;
    Ok(())
}
pub fn stash_list(repo: &Path) -> Result<Vec<StashEntry>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["stash", "list", "--format=%gd%x1f%H%x1f%gs"]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out.lines().filter(|l| !l.is_empty()).enumerate().map(|(i, l)| {
        let mut p = l.splitn(3, '\u{1f}');
        let _selector = p.next().unwrap_or("");
        let sha = p.next().unwrap_or("").to_string();
        let message = p.next().unwrap_or("").to_string();
        StashEntry { index: i as u32, sha, message }
    }).collect())
}
fn stash_ref(index: u32) -> String { format!("stash@{{{}}}", index) }
pub fn stash_apply(repo: &Path, index: u32) -> Result<(), String> {
    let mut c = Command::new("git"); c.current_dir(repo).args(["stash", "apply", "--end-of-options", &stash_ref(index)]);
    git_ops::run(&mut c)?; Ok(())
}
pub fn stash_pop(repo: &Path, index: u32) -> Result<(), String> {
    let mut c = Command::new("git"); c.current_dir(repo).args(["stash", "pop", "--end-of-options", &stash_ref(index)]);
    git_ops::run(&mut c)?; Ok(())
}
pub fn stash_drop(repo: &Path, index: u32) -> Result<(), String> {
    let mut c = Command::new("git"); c.current_dir(repo).args(["stash", "drop", "--end-of-options", &stash_ref(index)]);
    git_ops::run(&mut c)?; Ok(())
}
```

- [ ] **Step 4: Tests** (add to `ops_worktree.rs` tests): a diff contains the changed line; `stage_hunk` stages a file with multiple hunks one-at-a-time (verify only the chosen hunk is in `git diff --cached`); `unstage_hunk` reverses it; `stash_push` then `stash_list` shows one entry then `stash_pop` restores the change. Use the `TempRepo` helper (extend with a `read`/`diff_cached` helper as needed).

```rust
    #[test]
    fn diff_shows_change() {
        let r = TempRepo::new();
        r.commit_file("a.txt","1\n2\n3\n","init");
        r.write("a.txt","1\nCHANGED\n3\n");
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
        r.commit_file("a.txt","1\n","init");
        r.write("a.txt","2\n");
        stash_push(&r.path, Some("wip")).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("a.txt")).unwrap(), "1\n", "stash reverts worktree");
        let list = stash_list(&r.path).unwrap();
        assert_eq!(list.len(), 1);
        stash_pop(&r.path, 0).unwrap();
        assert_eq!(fs::read_to_string(r.path.join("a.txt")).unwrap(), "2\n", "pop restores");
    }
```

- [ ] **Step 5: Commands + registration + bindings.** Add commands `diff`, `commit_diff`, `stage_hunk`, `unstage_hunk`, `stash_push`, `stash_list`, `stash_apply`, `stash_pop`, `stash_drop` (extend types import with `StashEntry`); register in `lib.rs`. `api.ts` bindings:
```ts
  diff: (repo: string, path: string | null, staged: boolean) => invoke<string>("diff", { repo, path, staged }),
  commitDiff: (repo: string, sha: string, path: string | null) => invoke<string>("commit_diff", { repo, sha, path }),
  stageHunk: (repo: string, path: string, hunkIndex: number) => invoke<void>("stage_hunk", { repo, path, hunkIndex }),
  unstageHunk: (repo: string, path: string, hunkIndex: number) => invoke<void>("unstage_hunk", { repo, path, hunkIndex }),
  stashPush: (repo: string, message: string | null) => invoke<void>("stash_push", { repo, message }),
  stashList: (repo: string) => invoke<StashEntry[]>("stash_list", { repo }),
  stashApply: (repo: string, index: number) => invoke<void>("stash_apply", { repo, index }),
  stashPop: (repo: string, index: number) => invoke<void>("stash_pop", { repo, index }),
  stashDrop: (repo: string, index: number) => invoke<void>("stash_drop", { repo, index }),
```

- [ ] **Step 6: Gates + commit.** `cargo test` + `cargo check` + `npm run check`. Commit: `feat(worktree): diff + hunk staging + stash backend (Phase 5)`.

---

## Task 3 — Frontend: diff parser (TDD) + Shiki highlighter + store/gitActions

**Files:** create `src/lib/diff/types.ts`, `src/lib/diff/parse.ts`, `src/lib/diff/parse.test.ts`, `src/lib/diff/highlight.ts`; modify `store.svelte.ts`, `gitActions.ts`, `package.json`.

- [ ] **Step 1: `diff/types.ts`:**
```ts
export type DiffLineKind = "context" | "add" | "del";
export type DiffLine = { kind: DiffLineKind; text: string; oldNo: number | null; newNo: number | null };
export type DiffHunk = { header: string; lines: DiffLine[] };
export type DiffFile = { oldPath: string; newPath: string; language: string; binary: boolean; hunks: DiffHunk[] };
export type ParsedDiff = { files: DiffFile[] };
```

- [ ] **Step 2: Write `parse.test.ts` FIRST (TDD)** — cover: a single-file modify with one hunk (correct kinds + line numbers), a multi-hunk file, an added file, a deleted file, a binary file (`Binary files … differ` → `binary:true`, no hunks), a rename header, and language detection from extension. Run `npm test` → fail.

- [ ] **Step 3: Implement `parse.ts`** — `parseDiff(patch: string): ParsedDiff`. Split on `diff --git` blocks; per file read `---`/`+++` paths (`/dev/null` → add/delete), detect `Binary files`; parse `@@ -a,b +c,d @@` hunk headers, tracking old/new line numbers; classify each body line by leading char (` `→context, `+`→add, `-`→del), ignoring `\ No newline at end of file`. Derive `language` from the new (or old) path extension via a small ext→lang map (ts, tsx, js, jsx, rs, py, json, html, css, svelte, md, sh, yml/yaml, toml, go, c, h, cpp, java, rb, php, sql, …; unknown → "text"). Re-run `npm test` → pass.

- [ ] **Step 4: Add Shiki + `highlight.ts`.** `npm i shiki`. Create a memoized highlighter:
```ts
import { createHighlighter, type Highlighter } from "shiki";
const LANGS = ["typescript","tsx","javascript","jsx","rust","python","json","html","css","svelte","markdown","bash","yaml","toml","go","c","cpp","java","ruby","php","sql","diff","text"];
let hp: Promise<Highlighter> | null = null;
export function getHighlighter(): Promise<Highlighter> {
  if (!hp) hp = createHighlighter({ themes: ["github-light","github-dark"], langs: LANGS });
  return hp;
}
// Highlight a block of code (already split by line) to per-line token arrays for one theme.
// Used by DiffView: it reconstructs each hunk's "before"/"after" text, highlights each,
// and renders tokens with +/- backgrounds. (Per-hunk highlighting preserves syntax context.)
```
Note: Shiki's grammars/themes are bundled by Vite (dynamic imports become local chunks) — **offline, no CDN**. Keep `LANGS` curated to bound bundle size; unknown languages fall back to `text`.

- [ ] **Step 5: Store state.** Add to `store.svelte.ts`: `workingChanges: WorkingFile[]` (+ setter), `selectedFile: string | null` (the file being diffed; + setter), `workingCopySelected: boolean` (the synthetic-row mode; + setter — set false in `setCurrent` when a real commit is focused), and a persisted `diffSplit: boolean` setting (mirror `graphLineStyle` pattern; default false = unified). Clear `workingChanges`/`selectedFile`/`workingCopySelected` on repo change (extend the repo setter from Phase 4).

- [ ] **Step 6: gitActions wrappers.** Add: `refreshWorkingChanges()` (Tauri-only: `appState.setWorkingChanges(await api.workingChanges(repo))`; call it inside `reloadGraph` after `refreshStatus`), `stage(paths)`, `unstage(paths)`, `stageHunk(path,i)`, `unstageHunk(path,i)`, `commit(message)`, `stashPush/Apply/Pop/Drop`, and `discard(paths)`/`clean(paths)` via `runDestructive`-style confirm (consequence: "Permanently discard changes to N file(s) — cannot be undone. (Stash instead to keep them.)", NO backup bundle — pass a non-backup variant). Staging/commit/stash are non-destructive: a plain guarded runner that refreshes working changes + graph after. Status uses `firstLine`.

- [ ] **Step 7: Gates + commit.** `npm run check` + `npm test` (parser tests green). Commit: `feat(diff): unified-diff parser (TDD) + Shiki highlighter + worktree action layer (Phase 5)`.

---

## Task 4 — `DiffView.svelte` (Shiki-highlighted, unified + split, per-hunk staging)

**Files:** create `src/lib/components/DiffView.svelte`.

- [ ] **Step 1: Build `DiffView.svelte`.** Props: `patch: string` (raw), `language?` (override), `staged?: boolean`, `onStageHunk?`/`onUnstageHunk?` (callbacks, optional — present for working-copy diffs, absent for commit diffs). Behavior:
  - `parseDiff(patch)` → files → hunks.
  - For each hunk: reconstruct the "before" text (context+del lines, in order) and "after" text (context+add lines), call the Shiki highlighter for the file `language` and the active theme (light/dark via `prefers-color-scheme` or the app's mode), get per-line tokens. Render **unified** by default: context (after-tokens), del (before-tokens, red bg), add (after-tokens, green bg), with old/new line-number gutters. **Split** mode (when `appState.diffSplit`): two columns (old | new) aligned per hunk.
  - A per-hunk header row showing the `@@` header; when `onStageHunk`/`onUnstageHunk` is provided, a "Stage hunk"/"Unstage hunk" button calling it with the hunk index.
  - A unified/split toggle (binds `appState.diffSplit`); handle binary files ("Binary file — no preview") and empty patch ("No changes").
  - Highlighting is async: render plain text first, then upgrade when the highlighter resolves (await `getHighlighter()` in an `$effect`/`$derived.by` with a loading fallback). Never block render on Shiki.
  - Style with existing tokens; monospace; subtle add/del backgrounds that work in light + dark.

- [ ] **Step 2: Gates + commit.** `npm run check` + `npm test`. Commit: `feat(diff): DiffView component — Shiki highlight, unified/split, per-hunk staging (Phase 5)`.

---

## Task 5 — Working-copy UI: file list + commit composer + synthetic graph row

**Files:** create `WorkingCopyView.svelte`, `CommitComposer.svelte`; modify `GraphHistory.svelte`, `+page.svelte`, `store.svelte.ts` (if needed).

- [ ] **Step 1: `CommitComposer.svelte`** — a message `<textarea>` + "Commit" button (disabled when message empty or no staged files) + an optional "Sign off" checkbox. On commit → `gitActions.commit(message)` then clear the textarea. Show staged-count ("N staged"). 

- [ ] **Step 2: `WorkingCopyView.svelte`** — three sections (Staged / Unstaged / Untracked) from `appState.workingChanges`, each file a row with the path, a status glyph, and actions: Unstaged/Untracked rows → "Stage" (+ "Discard"/"Remove" danger); Staged rows → "Unstage". A "Stage all"/"Unstage all" header action per section. Clicking a file → `appState.setSelectedFile(path)` and shows its `DiffView` (unstaged or staged depending on section) with the per-hunk stage/unstage callbacks wired to `gitActions.stageHunk`/`unstageHunk`. Mount `CommitComposer` at the bottom. Discard/Remove route through `gitActions.discard`/`clean` (confirm-destructive).

- [ ] **Step 3: Synthetic "Uncommitted changes" row in `GraphHistory.svelte`.** When `appState.repoStatus` shows any change (`staged+unstaged+untracked+conflicted > 0`) OR `workingChanges.length > 0`, render a special first row ABOVE the commit rows: a distinct dot + "Uncommitted changes (N)". Clicking it → `appState.setWorkingCopySelected(true)` (and clear `currentSha`). A normal commit row click sets `workingCopySelected=false` (via `setCurrent`). Style it distinctly (e.g., dashed/“working” dot). Do NOT feed it through the lane engine — it's a presentational row.

- [ ] **Step 4: Bottom-panel switch in `+page.svelte`.** Replace the single `<CommitDetail />` with: `{#if appState.workingCopySelected}<WorkingCopyView />{:else}<CommitDetail />{/if}`.

- [ ] **Step 5: Gates + commit.** `npm run check` + `npm test`. Commit: `feat(worktree-ui): working-copy view, commit composer, uncommitted-changes row (Phase 5)`.

---

## Task 6 — Stash panel + wire DiffView into CommitDetail

**Files:** create `StashPanel.svelte`; modify `Sidebar.svelte`, `CommitDetail.svelte`.

- [ ] **Step 1: `StashPanel.svelte`** — a `CollapsiblePanel` "Stashes" listing `api.stashList(repo)` (refresh on `repoStatus`/working-change changes). A "Stash changes…" button (prompts for a message → `gitActions.stashPush`). Each entry: message + actions Apply / Pop / Drop (Drop is danger → confirm). Mount under `ReflogPanel` in `Sidebar.svelte`. Desktop-only note in browser.

- [ ] **Step 2: Wire `DiffView` into `CommitDetail.svelte`.** Replace the "File changes and the line-by-line diff arrive with the working-copy phase." placeholder: when a commit is selected, fetch `api.commitDiff(repo, sha, null)` (guard Tauri) and render it via `<DiffView patch={...} />` (no stage callbacks — read-only). Show a changed-files summary header (derive file list from the parsed diff). Keep metadata above.

- [ ] **Step 3: Gates + commit.** `npm run check` + `npm test`. Commit: `feat(worktree-ui): stash panel + commit diff in CommitDetail (Phase 5)`.

---

## Task 7 — Verification + preview + review + merge

- [ ] **Step 1: Gates.** From `src-tauri/`: `cargo test` + `cargo check`. From root: `npm run check` + `npm test` (incl. parser tests). All green. Also `npm run build` (confirm Shiki bundles for production/offline).
- [ ] **Step 2: Preview.** Start preview. Verify (seed via temporary mock where Tauri-guarded, then revert): the "Uncommitted changes" row appears when working changes exist and switches the bottom panel to `WorkingCopyView`; `DiffView` renders a sample patch WITH Shiki highlighting (light + dark), unified + split toggle; per-hunk stage buttons present; CommitComposer disabled/enabled logic; StashPanel renders; CommitDetail shows a commit's diff. Screenshot light + dark. Check console clean. Revert all temporary seeding; re-run `npm run check`.
- [ ] **Step 3: Final adversarial review** (Agent `superpowers:code-reviewer`, opus) over `git diff <merge-base>..HEAD`. Focus: every worktree shell-out keeps `--`/`--end-of-options`/`-F`-file (no option/shell injection — esp. `commit` message via file, `stage/discard/clean` paths, `stash` index built from a validated int); **hunk patch reconstruction is from git's own diff (never user text) and applied via stdin** (no shell, no temp-file race); discard/clean confirm + are clearly marked unrecoverable; the diff parser handles adds/deletes/binary/renames/no-newline without crashing (fuzz a malformed patch); Shiki failures degrade to plain text (never block render) and are bundled (offline); the synthetic row doesn't corrupt the lane engine; working changes refresh after each op. Fix findings, re-verify gates.
- [ ] **Step 4: Merge.** `superpowers:finishing-a-development-branch` → ff-merge to `main`, delete branch. Update memory + `docs/RESUME.md` (Phase 5 done; next = Phase 6 remote: pull/push).

## Deferred (not this phase)
- Line-level staging (only whole-file + hunk this phase).
- The deferred `--squash` merge commit-UI (now possible with a commit composer — can fold into a follow-up).
- Interactive-rebase `edit` pause-to-amend bespoke flow (still composes via ConflictView + amend).
- Diff search / word-level intra-line diff.

## Patterns to REUSE
- `gitActions` guard/refresh/`firstLine`; `runDestructive` (Phase 4) for discard/clean; `reloadGraph`/`refreshStatus` (+ new `refreshWorkingChanges`).
- `CollapsiblePanel`; the Tauri Store settings pattern (`graphLineStyle`) for `diffSplit`; `contextMenu`/`dialogs`/`Modal`.
- `git_ops::run` (strict). New `git apply` via stdin pipe (only non-`run` shell-out — keep it tight).
