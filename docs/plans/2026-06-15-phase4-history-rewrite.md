# Phase 4 — History Rewriting (reset / amend / rebase) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add the destructive history-rewriting operations — reset (soft/mixed/hard), amend (message + date reset), and rebase (onto + full interactive) — each wrapped in a `safety.rs` layer (configurable auto-backup + pre-op snapshot), with a plain-language confirmation, a one-click **Undo**, and a **reflog browser**.

**Architecture:** A new Rust `safety.rs` provides `snapshot` (capture branch+SHA before the op), `restore` (the undo), and `maybe_backup` (reuse `create_bundle`). A new `ops_rewrite.rs` implements reset/amend/rebase/interactive-rebase/reflog, each calling safety first and returning the undo snapshot + optional bundle path. Rebase reuses the Phase 3b `ConflictView` (already shows when `repo_status.operation == "rebase"`); we extend `op_subcommand` so Continue/Skip/Abort drive `git rebase`. Headless interactive rebase uses `GIT_SEQUENCE_EDITOR` (a generated script that copies our todo file in) and implements **reword via a git-rebase run-command todo line (the `x` keyword) that runs `git commit --amend -F <msgfile>`** — messages live in files, never on the command line, so there is no message-editor queue and no squash-group alignment problem. Frontend: a persisted `autoBackupDestructive` setting (default ON), a `runDestructive` gitActions wrapper that always confirms, the `RebaseTodo` editor, `ReflogPanel`, `AmendDialog`, and an `UndoBar`.

**Tech Stack:** Rust (git shell-out) + Tauri 2 commands; SvelteKit 5 runes; reuse Phase 2/3 patterns (`contextMenu`, `dialogs`, `gitActions`, `ConflictView`, Tauri Store settings).

**Branch:** `feat/phase4-history-rewrite` (already created).

## SECURITY (non-negotiable — this is the destructive phase)
Every git shell-out with a user operand separates options from operands:
- `reset`, `rebase`: `--end-of-options <ref>`.
- `commit --amend -F <file>` / `-m <msg>`: the value is a separate arg (an option-like message can't become a flag after `-m`/`-F`).
- Reflog reset reuses `reset` (so it inherits `--end-of-options`).
- Interactive-rebase **actions are validated against a fixed whitelist** (pick/reword/edit/squash/fixup/drop) in Rust; SHAs come from our own commit list; reword messages are written to files and consumed via `-F` (never interpolated into a shell command). The generated run-command (`x`) todo line references only our controlled, single-quoted temp file path.
- `op_subcommand` stays a whitelist; adding `"rebase"` keeps abort/continue/skip from ever taking a user operand.

## SAFETY MODEL (spec decision #10 + §11)
Every destructive op (`reset`, `amend`, `rebase`, reflog-reset) flows through `gitActions.runDestructive`:
1. **Confirm** with a plain-language consequence ("Reset `main` back 3 commits? N uncommitted change(s) will be lost.") via a new `dialogs.confirmDestructive`, which includes an inline **"Create backup bundle"** checkbox defaulting to `appState.autoBackupDestructive`.
2. Backend, if backup requested, runs `create_bundle` first, then captures the pre-op `UndoSnapshot` (branch + HEAD sha).
3. Run the op. On success, the frontend stores `appState.lastUndo = result.undo` → the `UndoBar` shows "Undo <label>".
4. **Undo** = `restore` (reset the branch back to the snapshot sha). The bundle + reflog remain as deeper nets; the `ReflogPanel` lists `git reflog` with "Reset here".

## File structure
- Create `src-tauri/src/safety.rs`; `src-tauri/src/ops_rewrite.rs`.
- Modify `src-tauri/src/types.rs` (UndoSnapshot, RewriteResult, RebaseOutcome, ReflogEntry, RebaseStep), `src-tauri/src/ops_merge.rs` (op_subcommand accepts "rebase"), `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`.
- Modify `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/store.svelte.ts`, `src/lib/gitActions.ts`, `src/lib/dialogs.svelte.ts`, `src/lib/components/Modal.svelte`, `src/lib/components/DateFormatMenu.svelte` (settings toggle), `src/lib/components/GraphHistory.svelte` (menus), `src/routes/+page.svelte` (mount UndoBar/ReflogPanel/RebaseTodo/AmendDialog).
- Create `src/lib/components/AmendDialog.svelte`, `UndoBar.svelte`, `ReflogPanel.svelte`, `RebaseTodo.svelte`.

---

## Task 1 — Backend: `safety.rs` + reset + amend

**Files:** Create `src-tauri/src/safety.rs`, `src-tauri/src/ops_rewrite.rs`; modify `types.rs`, `commands.rs`, `lib.rs`, `src/lib/types.ts`, `src/lib/api.ts`.

- [ ] **Step 1: Add types to `types.rs`** (append):

```rust
/// Captured before a destructive op so it can be one-click undone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoSnapshot {
    pub branch: Option<String>, // current branch, or None if detached HEAD
    pub sha: String,            // HEAD sha before the op
    pub label: String,          // "reset" | "amend" | "rebase" | "reflog reset"
}

/// Result of a non-conflicting destructive op.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteResult {
    pub undo: UndoSnapshot,
    pub bundle: Option<String>, // recovery bundle path, if one was made
}

/// Result of a rebase (which may stop on conflicts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseOutcome {
    pub outcome: OpOutcome, // reuse Phase 3a OpOutcome (conflicted/files/message)
    pub undo: UndoSnapshot,
    pub bundle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflogEntry {
    pub sha: String,
    pub short: String,
    pub selector: String, // e.g. "HEAD@{2}"
    pub subject: String,  // e.g. "reset: moving to HEAD~1"
}

/// One line of an interactive-rebase plan from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseStep {
    pub action: String,          // pick|reword|edit|squash|fixup|drop (validated in Rust)
    pub sha: String,
    pub message: Option<String>, // reword: the new message (else ignored)
}
```

- [ ] **Step 2: Create `safety.rs`:**

```rust
use crate::git_ops;
use crate::types::UndoSnapshot;
use std::path::{Path, PathBuf};
use std::process::Command;

fn rev_parse(repo: &Path, what: &str) -> Result<String, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["rev-parse", "--end-of-options", what]);
    let (out, _) = git_ops::run(&mut c)?;
    Ok(out.trim().to_string())
}

/// Current branch name, or None if HEAD is detached.
pub fn current_branch(repo: &Path) -> Result<Option<String>, String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .map_err(|e| format!("Failed to spawn git: {}", e))?;
    if out.status.success() {
        Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
    } else {
        Ok(None) // detached HEAD
    }
}

/// Snapshot HEAD before a destructive op, for one-click undo.
pub fn snapshot(repo: &Path, label: &str) -> Result<UndoSnapshot, String> {
    Ok(UndoSnapshot {
        branch: current_branch(repo)?,
        sha: rev_parse(repo, "HEAD")?,
        label: label.to_string(),
    })
}

/// Reverse a destructive op: move the current branch/HEAD back to the snapshot and
/// reset the working tree to it. (Immediate-undo semantics: any post-op changes are
/// discarded; the auto-backup bundle is the deeper net.)
pub fn restore(repo: &Path, sha: &str) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["reset", "--hard", "--end-of-options", sha]);
    git_ops::run(&mut c)?;
    Ok(())
}

/// Create a recovery bundle when auto_backup is on; returns its path string for the log.
pub fn maybe_backup(repo: &Path, auto_backup: bool) -> Result<Option<String>, String> {
    if auto_backup {
        let p: PathBuf = git_ops::create_bundle(repo)?;
        Ok(Some(p.to_string_lossy().into_owned()))
    } else {
        Ok(None)
    }
}
```

- [ ] **Step 3: Create `ops_rewrite.rs` with reset + amend + tests.** (Rebase/reflog land in Task 2; add them to this same file there.)

```rust
use crate::git_ops;
use crate::safety;
use crate::types::RewriteResult;
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
    let mut c = Command::new("git");
    c.current_dir(repo);
    if reset_committer_date {
        c.env("GIT_COMMITTER_DATE", "now");
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
}
```

- [ ] **Step 4: Register module + commands.** In `lib.rs` add `mod safety;` and `mod ops_rewrite;` next to the other `mod` lines, and add the new commands (below) to `tauri::generate_handler!`. In `commands.rs` add (and extend the `types` import with `RewriteResult`; add `use crate::ops_rewrite;`):

```rust
#[tauri::command]
pub fn reset(repo: String, target: String, mode: String, auto_backup: bool) -> Result<RewriteResult, String> {
    ops_rewrite::reset(&PathBuf::from(repo), &target, &mode, auto_backup)
}

#[tauri::command]
pub fn amend(
    repo: String,
    message: Option<String>,
    reset_author_date: bool,
    reset_committer_date: bool,
    auto_backup: bool,
) -> Result<RewriteResult, String> {
    ops_rewrite::amend(&PathBuf::from(repo), message.as_deref(), reset_author_date, reset_committer_date, auto_backup)
}

#[tauri::command]
pub fn undo_op(repo: String, sha: String) -> Result<(), String> {
    crate::safety::restore(&PathBuf::from(repo), &sha)
}
```

- [ ] **Step 5: TS mirrors (`src/lib/types.ts`):**

```ts
export type UndoSnapshot = { branch: string | null; sha: string; label: string };
export type RewriteResult = { undo: UndoSnapshot; bundle: string | null };
export type RebaseOutcome = { outcome: OpOutcome; undo: UndoSnapshot; bundle: string | null };
export type ReflogEntry = { sha: string; short: string; selector: string; subject: string };
export type RebaseStep = { action: string; sha: string; message?: string | null };
```

- [ ] **Step 6: api.ts bindings** (add `RewriteResult` to imports; add):

```ts
  reset: (repo: string, target: string, mode: "soft" | "mixed" | "hard", autoBackup: boolean) =>
    invoke<RewriteResult>("reset", { repo, target, mode, autoBackup }),
  amend: (repo: string, message: string | null, resetAuthorDate: boolean, resetCommitterDate: boolean, autoBackup: boolean) =>
    invoke<RewriteResult>("amend", { repo, message, resetAuthorDate, resetCommitterDate, autoBackup }),
  undoOp: (repo: string, sha: string) => invoke<void>("undo_op", { repo, sha }),
```

- [ ] **Step 7: Gates + commit.** From `src-tauri/`: `cargo test` (all pass incl. the new tests) + `cargo check`. From repo root: `npm run check`. Commit:
```
feat(rewrite): safety layer + reset/amend backend (Phase 4)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

---

## Task 2 — Backend: rebase (onto + interactive) + reflog + rebase-aware op handling

**Files:** modify `ops_rewrite.rs`, `ops_merge.rs`, `commands.rs`, `lib.rs`, `src/lib/api.ts`.

- [ ] **Step 1: Teach `ops_merge` about rebase.** In `op_subcommand` add the arm `"rebase" => Ok("rebase"),`. In `outcome_for`, leave the empty-skip branch as-is (it's gated to cherry-pick/revert and simply won't trigger for "rebase"); add a one-line comment that rebase relies only on the conflict/err paths. This makes `abort`/`continue_op`/`skip` work for `kind == "rebase"`, so the Phase 3b ConflictView drives rebase.

- [ ] **Step 2: Add rebase + interactive + reflog to `ops_rewrite.rs`.** Add `use crate::ops_merge;`, `use crate::types::{RebaseOutcome, RebaseStep, ReflogEntry};`, and `use std::fs;`.

```rust
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

    // Scratch dir for the todo, the seq-editor script, and reword message files.
    let suffix = undo.sha.get(0..7).unwrap_or("x");
    let dir = std::env::temp_dir().join(format!("gte-rebase-{}-{}", std::process::id(), suffix));
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
                todo.push_str(&format!("x git commit --amend -F '{}'\n", mf.display()));
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
    fs::write(&seq, format!("#!/bin/sh\ncp '{}' \"$1\"\n", todo_path.display()))
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
    let _ = fs::remove_dir_all(&dir); // best-effort cleanup
    Ok(RebaseOutcome { outcome, undo, bundle })
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
```

- [ ] **Step 3: Tests** (add to `ops_rewrite.rs` `mod tests`):

```rust
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
```

- [ ] **Step 4: Commands + registration.** In `commands.rs` (extend `types` import with `RebaseOutcome, RebaseStep, ReflogEntry`):

```rust
#[tauri::command]
pub fn rebase(repo: String, onto: String, auto_backup: bool) -> Result<RebaseOutcome, String> {
    ops_rewrite::rebase(&PathBuf::from(repo), &onto, auto_backup)
}

#[tauri::command]
pub fn rebase_todo_preview(repo: String, base: String) -> Result<Vec<ReflogEntry>, String> {
    ops_rewrite::rebase_todo_preview(&PathBuf::from(repo), &base)
}

#[tauri::command]
pub fn rebase_interactive(repo: String, base: String, steps: Vec<RebaseStep>, auto_backup: bool) -> Result<RebaseOutcome, String> {
    ops_rewrite::rebase_interactive(&PathBuf::from(repo), &base, &steps, auto_backup)
}

#[tauri::command]
pub fn reflog(repo: String, limit: u32) -> Result<Vec<ReflogEntry>, String> {
    ops_rewrite::reflog(&PathBuf::from(repo), limit)
}
```
Register all four in `lib.rs`.

- [ ] **Step 5: api.ts bindings** (add `RebaseOutcome, RebaseStep, ReflogEntry` imports):

```ts
  rebase: (repo: string, onto: string, autoBackup: boolean) =>
    invoke<RebaseOutcome>("rebase", { repo, onto, autoBackup }),
  rebaseTodoPreview: (repo: string, base: string) =>
    invoke<ReflogEntry[]>("rebase_todo_preview", { repo, base }),
  rebaseInteractive: (repo: string, base: string, steps: RebaseStep[], autoBackup: boolean) =>
    invoke<RebaseOutcome>("rebase_interactive", { repo, base, steps, autoBackup }),
  reflog: (repo: string, limit = 50) => invoke<ReflogEntry[]>("reflog", { repo, limit }),
```

- [ ] **Step 6: Gates + commit.** `cargo test` + `cargo check` + `npm run check`. Commit:
```
feat(rewrite): rebase (onto + interactive) + reflog backend (Phase 4)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

---

## Task 3 — Store setting + undo state + `gitActions` destructive layer + confirm dialog

**Files:** modify `store.svelte.ts`, `gitActions.ts`, `dialogs.svelte.ts`, `Modal.svelte`.

- [ ] **Step 1: Persisted `autoBackupDestructive` setting in `store.svelte.ts`** — follow the exact `graphLineStyle` pattern (sync load + async hydrate + write-through). Keys: localStorage `gte.safety.autoBackup.v1`, Store key `safetyAutoBackup`, default `true`. Add `loadSyncAutoBackup()`, the `$state`, a `safetyAutoBackupTouched` guard, the hydrate block, `persistAutoBackup()`, and expose `get autoBackupDestructive()` + `setAutoBackupDestructive(v)`.

- [ ] **Step 2: Undo state in `store.svelte.ts`** — add `let lastUndo = $state<UndoSnapshot | null>(null);` (import `UndoSnapshot`), expose `get lastUndo()` + `setLastUndo(u: UndoSnapshot | null)`. Do NOT clear it in `setGraphCommits` (a destructive op reloads the graph and we want the Undo to persist after); clear it only when consumed (undo) or replaced by the next op.

- [ ] **Step 3: `dialogs.confirmDestructive` in `dialogs.svelte.ts`** — add a third dialog kind:

```ts
  | {
      kind: "destructive";
      title: string;
      consequence: string;
      confirmLabel: string;
      backup: boolean; // current checkbox state
      resolve: (v: { confirmed: boolean; backup: boolean }) => void;
    }
```
Add `confirmDestructive(opts: { title: string; consequence: string; confirmLabel?: string; backupDefault: boolean }): Promise<{ confirmed: boolean; backup: boolean }>` that `settlePending()`s and sets state (`backup = backupDefault`). Add `setDestructiveBackup(v: boolean)` (mutates the checkbox) and `resolveDestructive(confirmed: boolean)` (resolves `{ confirmed, backup: state.backup }`, then clears to `{kind:"none"}`). Update `settlePending` to resolve a pending destructive as `{ confirmed: false, backup: false }`.

- [ ] **Step 4: Render the destructive dialog in `Modal.svelte`** — add a branch for `kind === "destructive"`: title, the `consequence` text, a checkbox (checked = `state.backup`, onchange → `dialogs.setDestructiveBackup`), a Cancel (`resolveDestructive(false)`) and a danger-styled confirm (`resolveDestructive(true)`). Enter = confirm, Escape = cancel (match existing handlers).

- [ ] **Step 5: `gitActions` destructive wrappers.** Add to `gitActions.ts` (extend the type import with `UndoSnapshot, RewriteResult, RebaseOutcome, RebaseStep`; `import { dialogs } from "./dialogs.svelte";`):

```ts
// Confirm (with consequence + backup choice) → run → store undo → refresh. Returns ok.
async function runDestructive(
  label: string,
  consequence: string,
  fn: (backup: boolean) => Promise<RewriteResult>,
): Promise<boolean> {
  if (!isTauri()) { appState.status = "That action needs the desktop app (not the browser preview)."; return false; }
  if (!appState.repo) { appState.status = "Open a repository first."; return false; }
  const { confirmed, backup } = await dialogs.confirmDestructive({
    title: label, consequence, confirmLabel: label, backupDefault: appState.autoBackupDestructive,
  });
  if (!confirmed) return false;
  try {
    appState.status = `${label}…`;
    const res = await fn(backup);
    appState.setLastUndo(res.undo);
    if (res.bundle) appState.appendLog(`[backup] ${res.bundle}`);
    try { await reloadGraph(); } catch (e) { console.warn("[gte] refresh failed", e); }
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    try { await reloadGraph(); } catch {}
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}

// Rebase variant: same confirm+undo, but the result is a RebaseOutcome that may conflict.
async function runDestructiveRebase(
  label: string,
  consequence: string,
  fn: (backup: boolean) => Promise<RebaseOutcome>,
): Promise<boolean> {
  if (!isTauri()) { appState.status = "That action needs the desktop app (not the browser preview)."; return false; }
  if (!appState.repo) { appState.status = "Open a repository first."; return false; }
  const { confirmed, backup } = await dialogs.confirmDestructive({
    title: label, consequence, confirmLabel: label, backupDefault: appState.autoBackupDestructive,
  });
  if (!confirmed) return false;
  try {
    appState.status = `${label}…`;
    const res = await fn(backup);
    appState.setLastUndo(res.undo);
    if (res.bundle) appState.appendLog(`[backup] ${res.bundle}`);
    try { await reloadGraph(); } catch (e) { console.warn("[gte] refresh failed", e); }
    appState.status = res.outcome.conflicted
      ? `${label}: ${res.outcome.files.length} conflict(s) to resolve.`  // ConflictView appears (operation=rebase)
      : `${label} — done.`;
    return true;
  } catch (e) {
    try { await reloadGraph(); } catch {}
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}
```
Public actions:
```ts
  reset: (target: string, mode: "soft" | "mixed" | "hard", consequence: string) =>
    runDestructive(`Reset (${mode})`, consequence, (backup) => api.reset(appState.repo, target, mode, backup)),
  amend: (message: string | null, resetAuthorDate: boolean, resetCommitterDate: boolean) =>
    runDestructive("Amend commit", "Rewrites the latest commit (its hash changes).",
      (backup) => api.amend(appState.repo, message, resetAuthorDate, resetCommitterDate, backup)),
  rebaseOnto: (onto: string, consequence: string) =>
    runDestructiveRebase(`Rebase onto ${onto}`, consequence, (backup) => api.rebase(appState.repo, onto, backup)),
  rebaseInteractive: (base: string, steps: RebaseStep[], consequence: string) =>
    runDestructiveRebase("Interactive rebase", consequence, (backup) => api.rebaseInteractive(appState.repo, base, steps, backup)),
  undo: () => {
    const u = appState.lastUndo;
    if (!u) return Promise.resolve(false);
    return run(`Undo ${u.label}`, () => api.undoOp(appState.repo, u.sha)).then((ok) => {
      if (ok) appState.setLastUndo(null);
      return ok;
    });
  },
```

- [ ] **Step 6: Gates + commit.** `npm run check` + `npm test`. Commit:
```
feat(rewrite-ui): safety setting, undo state, destructive action layer + confirm dialog (Phase 4)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

---

## Task 4 — `AmendDialog` + `UndoBar` + settings toggle + reset/amend menu wiring

**Files:** create `AmendDialog.svelte`, `UndoBar.svelte`, `amendDialog.svelte.ts`; modify `DateFormatMenu.svelte`, `GraphHistory.svelte`, `+page.svelte`.

- [ ] **Step 1: `UndoBar.svelte`** — when `appState.lastUndo` is set, render a slim accent bar: "Undo <label>" with an **Undo** button (`gitActions.undo()`) and a dismiss "×" (`appState.setLastUndo(null)`). Mount in `+page.svelte` `.main-col` just above `GraphHistory`.

- [ ] **Step 2: `amendDialog.svelte.ts` + `AmendDialog.svelte`** — a tiny store `{ open, sha, subject, openWith(sha, subject), close() }`. The component, when open, shows: a textarea prefilled with `subject`, two checkboxes "Reset author date to now" / "Reset committer date to now", and Amend/Cancel. Amend → `gitActions.amend(message, resetAuthorDate, resetCommitterDate)` then `close()`. Style consistent with `Modal.svelte`/`ConflictView.svelte`. Mount in `+page.svelte`.

- [ ] **Step 3: Settings toggle** — add a divider + "Create backup before destructive ops" checkbox to the gear popover in `DateFormatMenu.svelte`, bound to `appState.autoBackupDestructive` via `setAutoBackupDestructive` (controlled `checked` + `onchange`, like the existing options).

- [ ] **Step 4: Reset/amend context menu in `GraphHistory.svelte`** `onRowContext`. Add a `currentBranchName()` helper (`appState.refsByKind.local.find((r) => r.isHead)?.name ?? "HEAD"`) and a `doReset(sha, mode)` that builds a consequence string — count commits from the clicked row to HEAD using `appState.graphCommits` index difference, e.g. ``Move ${branch} back ${n} commit(s)${mode === "hard" ? "; uncommitted changes will be lost" : ""}.`` — then calls `gitActions.reset(sha, mode, consequence)`. Insert after the cherry-pick/revert block:
```ts
      { separator: true },
      { label: `Reset ${currentBranchName()} here (mixed)`, action: () => doReset(sha, "mixed") },
      { label: `Reset ${currentBranchName()} here (soft)`, action: () => doReset(sha, "soft") },
      { label: `Reset ${currentBranchName()} here (hard)`, danger: true, action: () => doReset(sha, "hard") },
```
And an Amend item shown only when the row is HEAD (`commit.refs.some((r) => r.is_head)`): `{ label: "Amend this commit…", action: () => amendDialog.openWith(sha, commit.subject) }`.

- [ ] **Step 5: Mount** `UndoBar` + `AmendDialog`; gates `npm run check` + `npm test`. Commit:
```
feat(rewrite-ui): amend dialog, undo bar, safety toggle, reset/amend menus (Phase 4)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

---

## Task 5 — `RebaseTodo` interactive-rebase editor + rebase menu wiring

**Files:** create `RebaseTodo.svelte`, `rebaseEditor.svelte.ts`; modify `GraphHistory.svelte`, `+page.svelte`.

- [ ] **Step 1: `rebaseEditor.svelte.ts`** — `{ open, base, openWith(base), close() }`.

- [ ] **Step 2: `RebaseTodo.svelte`** — when open, fetch `api.rebaseTodoPreview(repo, base)` (oldest-first). Render a reorderable list; each row: up/down buttons (reorder — no external dnd lib, offline app), an action `<select>` (pick/reword/edit/squash/fixup/drop), and — when `action === "reword"` — a text input prefilled with the subject. Footer: **Start rebase** (build `RebaseStep[]` in listed order; require ≥1 non-drop step; call `gitActions.rebaseInteractive(base, steps, consequence)` then `close()`) and **Cancel**. Consequence: ``Rewrites ${steps.length} commit(s) on ${branch}; hashes change.``. Style like `ConflictView.svelte`. Mount in `+page.svelte`.

- [ ] **Step 3: Rebase menu items in `GraphHistory.svelte`** `onRowContext`:
```ts
      { label: `Rebase ${currentBranchName()} onto here`, action: () => gitActions.rebaseOnto(sha, `Replay ${currentBranchName()}'s commits onto ${sha.slice(0,9)}.`) },
      { label: "Interactive rebase from here…", action: () => rebaseEditor.openWith(sha) },
```

- [ ] **Step 4: Mount `RebaseTodo`**; gates `npm run check` + `npm test`. Commit:
```
feat(rewrite-ui): interactive rebase todo editor + rebase menus (Phase 4)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

---

## Task 6 — `ReflogPanel` (reflog browser) + wiring

**Files:** create `ReflogPanel.svelte`; modify `Sidebar.svelte`.

- [ ] **Step 1: `ReflogPanel.svelte`** — a `CollapsiblePanel` "History (reflog)". On expand and whenever `appState.lastUndo` changes (key a refresh `$effect` off it), call `api.reflog(repo, 50)` and list entries (`selector` · `short` · `subject`). Each row: a "Reset here" button → `gitActions.reset(entry.sha, "mixed", `Reset ${currentBranchName()} to ${entry.short} (${entry.subject}).`)` and a small "hard" secondary button → same with `"hard"`. Browser/sample mode: show a "desktop app only" note (guard on `isTauri()`).

- [ ] **Step 2: Mount** under `BackupsPanel` in `Sidebar.svelte` (both are recovery tools). Gates `npm run check` + `npm test`. Commit:
```
feat(rewrite-ui): reflog browser panel (Phase 4)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

---

## Task 7 — Full verification + preview + review + merge

- [ ] **Step 1: Gates.** From `src-tauri/`: `cargo test` + `cargo check`. From root: `npm run check` + `npm test`. All green.
- [ ] **Step 2: Preview (logic + render).** Start preview (`vite`, port 1420). Verify: the new row menu items render (reset ×3, rebase onto, interactive rebase; amend on the HEAD row); the safety toggle appears in the gear popover; the confirm-destructive dialog renders with its consequence text + backup checkbox; the `RebaseTodo` editor opens and lists commits (seed via sample/temporary force if needed); `UndoBar` renders when `lastUndo` is set (force temporarily, then revert); `ReflogPanel` shows its desktop-only note. Screenshot light + dark. Revert any temporary seeding; re-run `npm run check`.
- [ ] **Step 3: Final adversarial review** (Agent `superpowers:code-reviewer`, opus) over `git diff <merge-base>..HEAD`, focused on: every destructive shell-out keeps `--end-of-options`/`-F`/whitelist (no option/shell injection — especially the interactive run-command (`x`) todo line, the `RebaseStep.action` whitelist, and the generated seq-editor script using only controlled paths); the undo snapshot is captured BEFORE the op and `restore` is correct; `confirmDestructive` can't be bypassed (every destructive action goes through `runDestructive`/`runDestructiveRebase`); rebase conflicts surface via the existing ConflictView (op_subcommand handles "rebase"); the safety setting + per-op backup checkbox behave; no raw multi-line git text in toasts; temp-dir cleanup happens. Fix findings, re-verify gates.
- [ ] **Step 4: Merge.** `superpowers:finishing-a-development-branch` → ff-merge to `main`, delete branch. Update memory (`git-client-migration.md`, `MEMORY.md`) + `docs/RESUME.md` (Phase 4 done; next = Phase 5 working-copy + diff viewer).

## Deferred (not this phase)
- `edit` action mid-rebase has no dedicated flow: it pauses, the ConflictView shows "rebase in progress" with Continue, and the user amends HEAD via the Amend menu then Continues. (Works; just not bespoke.)
- Custom squash messages (squash uses git's default combined message).
- Drag-and-drop reorder (up/down buttons instead — no external dep).
- Staging/working-copy + diff viewer → Phase 5.

## Patterns to REUSE
- `gitActions` guard/refresh/`firstLine`; `runOp`/`reloadGraph`/`refreshStatus` (Phase 3b).
- `ConflictView` already keys off `repo_status.operation` — rebase conflicts appear for free once `op_subcommand` knows "rebase".
- `contextMenu` + `dialogs` + `Modal`; the Tauri Store settings pattern (`graphLineStyle`); `create_bundle`/`list_bundles`/`BackupsPanel` for the recovery net.
- `git_ops::run` (strict) / `ops_merge::run_status` + `outcome_for` (conflict-vs-error).
