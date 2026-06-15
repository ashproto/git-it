# Phase 3b — Conflict-Resolution UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Surface the already-built merge/cherry-pick/revert backend in the UI — let the user start those ops from context menus, see conflicts in a dedicated panel, resolve each file the right way for its conflict type, then continue/skip/abort.

**Architecture:** Backend gains conflict *classification* (so the UI offers the correct buttons) plus two tree-level resolutions (`git add` keep / `git rm` remove) and an op `--skip`. The store gains a `repoStatus` field refreshed alongside the graph. `gitActions` gains an `OpOutcome`-aware `runOp` plus thin wrappers. A new self-guarding `ConflictView.svelte` renders only while an operation is in progress. Existing context menus get merge/cherry-pick/revert entries.

**Tech Stack:** Rust (git shell-out) + Tauri 2 commands; SvelteKit 5 runes; existing `contextMenu`/`dialogs`/`gitActions` patterns.

**Branch:** `feat/phase3b-conflict-ui` (already created).

## Honor these UI contracts (from the 3a adversarial review)
- **(a)** After any `cherryPick`/`revert`/`opContinue`/`opSkip` that returns Err, re-read `repo_status`; if `operation` is present, the panel must offer Abort/Skip so the user can recover.
- **(b)** `resolve_side` (checkout --ours/--theirs) only handles both-sides-have-content conflicts. Modify/delete needs **Keep file** (`git add`) vs **Remove file** (`git rm`). Classify each file and show the right buttons.
- **(c)** Gate Abort/Continue/Skip on an operation actually being in progress (idle `git merge --abort` errors). The panel only renders when `repoStatus.operation` is set; Continue is disabled while `conflicted > 0`.
- **(d)** Never dump the raw multi-line git `OpOutcome.message` into a status toast — summarize to one line.
- **(e)** `--squash` and `--no-ff` are mutually exclusive (backend `merge()` already errors if both); the merge UI must never send both.

## Security principle (non-negotiable)
Every git shell-out with a user-supplied operand separates options from operands: `--` before paths (`add`/`rm`/`checkout`), `--end-of-options` before refs. `op_skip` takes only a fixed subcommand (no user operand). Follow the existing `ops_merge.rs` style.

## File structure
- Modify `src-tauri/src/types.rs` — add `ConflictKind` + `ConflictEntry`.
- Modify `src-tauri/src/ops_merge.rs` — add `conflict_details`, `resolve_keep`, `resolve_remove`, `skip` + tests.
- Modify `src-tauri/src/commands.rs` — 4 new `#[tauri::command]`s.
- Modify `src-tauri/src/lib.rs` — register the 4 commands.
- Modify `src/lib/types.ts` — TS mirrors.
- Modify `src/lib/api.ts` — 4 new bindings.
- Modify `src/lib/store.svelte.ts` — `repoStatus` state.
- Modify `src/lib/gitActions.ts` — `refreshStatus`, `runOp`, op + resolve wrappers; refresh status inside `reloadGraph`.
- Create `src/lib/components/ConflictView.svelte`.
- Modify `src/routes/+page.svelte` — mount `<ConflictView/>`.
- Modify `src/lib/components/Sidebar.svelte` — merge menu items.
- Modify `src/lib/components/GraphHistory.svelte` — cherry-pick/revert menu items.

---

## Task 1 — Backend: conflict classification + tree resolutions + skip

**Files:**
- Modify: `src-tauri/src/types.rs`
- Modify: `src-tauri/src/ops_merge.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts`, `src/lib/api.ts`

- [ ] **Step 1: Add the conflict types to `types.rs`** (append after `OpOutcome`):

```rust
/// How a conflicted path must be resolved, derived from git's status XY code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// Both sides have content (UU / AA) — resolvable by taking ours or theirs.
    Both,
    /// One side modified, the other deleted/added (UD/DU/AU/UA) — keep or remove.
    ModifyDelete,
    /// Both sides deleted (DD) — only removal finalizes it.
    BothDeleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictEntry {
    pub path: String,
    pub kind: ConflictKind,
}
```

- [ ] **Step 2: Write failing tests in `ops_merge.rs`** (add inside `mod tests`, after `cherry_pick_conflict_reports_files`). These drive the new functions. Use the existing `TempRepo` helper.

```rust
    #[test]
    fn conflict_details_classifies_both_modified() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.commit("f.txt", "feat\n", "feat edit");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "main\n", "main edit");
        assert!(merge(&r.path, "feat", false, false).unwrap().conflicted);
        let d = conflict_details(&r.path).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].path, "f.txt");
        assert_eq!(d[0].kind, crate::types::ConflictKind::Both);
        abort(&r.path, "merge").unwrap();
    }

    #[test]
    fn conflict_details_classifies_modify_delete() {
        // We modify f.txt; they delete it → UD (modify/delete).
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.git(&["rm", "-q", "f.txt"]);
        r.git(&["commit", "-q", "-m", "delete f"]);
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "main-change\n", "main edit");
        assert!(merge(&r.path, "feat", false, false).unwrap().conflicted);
        let d = conflict_details(&r.path).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].kind, crate::types::ConflictKind::ModifyDelete);
        abort(&r.path, "merge").unwrap();
    }

    #[test]
    fn resolve_keep_then_continue_keeps_file() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.git(&["rm", "-q", "f.txt"]);
        r.git(&["commit", "-q", "-m", "delete f"]);
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "kept\n", "main edit");
        assert!(merge(&r.path, "feat", false, false).unwrap().conflicted);
        resolve_keep(&r.path, "f.txt").unwrap();
        assert!(conflicted_files(&r.path).unwrap().is_empty());
        let done = continue_op(&r.path, "merge").unwrap();
        assert!(!done.conflicted);
        assert_eq!(r.read("f.txt"), "kept\n");
    }

    #[test]
    fn resolve_remove_then_continue_drops_file() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.git(&["rm", "-q", "f.txt"]);
        r.git(&["commit", "-q", "-m", "delete f"]);
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "main-change\n", "main edit");
        assert!(merge(&r.path, "feat", false, false).unwrap().conflicted);
        resolve_remove(&r.path, "f.txt").unwrap();
        assert!(conflicted_files(&r.path).unwrap().is_empty());
        let done = continue_op(&r.path, "merge").unwrap();
        assert!(!done.conflicted);
        assert!(!r.path.join("f.txt").exists(), "file should be gone");
    }

    #[test]
    fn skip_cherry_pick_advances_without_applying() {
        let r = TempRepo::new();
        r.commit("f.txt", "base\n", "base");
        r.git(&["checkout", "-q", "-b", "feat"]);
        r.commit("f.txt", "feat\n", "feat edit");
        let pick = r.rev("HEAD");
        r.git(&["checkout", "-q", "main"]);
        r.commit("f.txt", "main\n", "main edit");
        assert!(cherry_pick(&r.path, &[pick]).unwrap().conflicted);
        let out = skip(&r.path, "cherry-pick").unwrap();
        assert!(!out.conflicted);
        assert!(!sequencer_in_progress(&r.path, "cherry-pick"));
        assert_eq!(r.read("f.txt"), "main\n", "skip discards the picked change");
    }

    #[test]
    fn skip_merge_is_rejected() {
        let r = TempRepo::new();
        r.commit("f.txt", "x\n", "x");
        assert!(skip(&r.path, "merge").is_err(), "merge has no --skip");
    }
```

- [ ] **Step 3: Run the new tests, expect failures** (functions don't exist yet):

Run from `src-tauri/`: `cargo test conflict_details_classifies_both_modified conflict_details_classifies_modify_delete resolve_keep_then_continue_keeps_file resolve_remove_then_continue_drops_file skip_cherry_pick_advances_without_applying skip_merge_is_rejected`
Expected: compile error / unresolved `conflict_details`, `resolve_keep`, `resolve_remove`, `skip`.

- [ ] **Step 4: Implement the functions in `ops_merge.rs`.** Add `use crate::types::{ConflictEntry, ConflictKind};` to the existing top `use crate::types::OpOutcome;` line (make it `use crate::types::{ConflictEntry, ConflictKind, OpOutcome};`). Then add these functions (place after `resolve_side`):

```rust
/// Map a git porcelain-v2 unmerged XY code to a resolution kind.
fn classify(xy: &str) -> ConflictKind {
    match xy {
        "UU" | "AA" => ConflictKind::Both,
        "DD" => ConflictKind::BothDeleted,
        _ => ConflictKind::ModifyDelete, // UD, DU, AU, UA
    }
}

/// Conflicted paths with their resolution kind. Parses `status --porcelain=v2 -z`
/// unmerged ("u") records: `u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`.
/// `-z` makes records NUL-terminated and paths verbatim (no quoting), so a path
/// with spaces survives the field split.
pub fn conflict_details(repo: &Path) -> Result<Vec<ConflictEntry>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["status", "--porcelain=v2", "-z"]);
    let (out, _) = git_ops::run(&mut c)?;
    let mut entries = Vec::new();
    for rec in out.split('\0') {
        if let Some(rest) = rec.strip_prefix("u ") {
            // rest = "<XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>"
            let mut it = rest.splitn(10, ' ');
            let xy = it.next().unwrap_or("");
            let path = it.nth(8).unwrap_or("");
            if !path.is_empty() {
                entries.push(ConflictEntry { path: path.to_string(), kind: classify(xy) });
            }
        }
    }
    Ok(entries)
}

/// Keep the working-tree version of a modify/delete-conflicted file (stages it).
pub fn resolve_keep(repo: &Path, path: &str) -> Result<(), String> {
    let mut add = Command::new("git");
    add.current_dir(repo).args(["add", "--", path]);
    git_ops::run(&mut add)?;
    Ok(())
}

/// Remove a conflicted file as the resolution (stages the deletion). `-f` because
/// a modify/delete conflict leaves modified content in the worktree that the plain
/// `git rm` up-to-date check would otherwise reject — removal is the chosen intent.
pub fn resolve_remove(repo: &Path, path: &str) -> Result<(), String> {
    let mut rm = Command::new("git");
    rm.current_dir(repo).args(["rm", "-f", "--", path]);
    git_ops::run(&mut rm)?;
    Ok(())
}

/// Skip the current commit of an in-progress cherry-pick/revert sequence. Merge has
/// no `--skip` (resolve or abort instead) — reject it rather than shell out.
pub fn skip(repo: &Path, kind: &str) -> Result<OpOutcome, String> {
    let sub = op_subcommand(kind)?;
    if sub == "merge" {
        return Err("Merge cannot skip — resolve the conflicts or abort.".to_string());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args([sub, "--skip"]);
    let (ok, msg) = run_status(&mut c)?;
    outcome_for(repo, sub, ok, msg)
}
```

> Implementer note: the `splitn(10, ' ')` then `nth(8)` consumes XY (index 0) via the first `next()`, then skips the 8 middle fields (sub, m1, m2, m3, mW, h1, h2, h3) to land on the path remainder. Verify against the test output; if `git rm -f` still fails for `both-deleted` in a future test, fall back to `git rm --ignore-unmatch -f`.

- [ ] **Step 5: Run the new tests, expect PASS:**

Run from `src-tauri/`: `cargo test` (run the whole suite — must stay green).
Expected: all tests pass (existing 8 + 6 new = 14 in this file; full crate green).

- [ ] **Step 6: Add the 4 commands to `commands.rs`** (after `conflicted_files`). Import update: change the `types` import line to include the new types — `use crate::types::{BundleInfo, Commit, ConflictEntry, DateMapping, GraphCommit, OpOutcome, PrerequisiteCheck, Ref, RepoStatus, RewriteOptions, SafetyRef};`

```rust
#[tauri::command]
pub fn conflict_details(repo: String) -> Result<Vec<ConflictEntry>, String> {
    ops_merge::conflict_details(&PathBuf::from(repo))
}

#[tauri::command]
pub fn resolve_keep(repo: String, path: String) -> Result<(), String> {
    ops_merge::resolve_keep(&PathBuf::from(repo), &path)
}

#[tauri::command]
pub fn resolve_remove(repo: String, path: String) -> Result<(), String> {
    ops_merge::resolve_remove(&PathBuf::from(repo), &path)
}

#[tauri::command]
pub fn op_skip(repo: String, kind: String) -> Result<OpOutcome, String> {
    ops_merge::skip(&PathBuf::from(repo), &kind)
}
```

- [ ] **Step 7: Register the 4 commands in `lib.rs`.** Read `src-tauri/src/lib.rs`, find the `tauri::generate_handler![ ... ]` list, and add `commands::conflict_details, commands::resolve_keep, commands::resolve_remove, commands::op_skip,` alongside the existing `commands::resolve_conflict, commands::conflicted_files` entries.

- [ ] **Step 8: Add the TS type mirrors to `src/lib/types.ts`** (append):

```ts
export type ConflictKind = "both" | "modify-delete" | "both-deleted";

export type ConflictEntry = { path: string; kind: ConflictKind };
```

- [ ] **Step 9: Add the 4 bindings to `src/lib/api.ts`.** Add `ConflictEntry` to the type import block, and add after `conflictedFiles`:

```ts
  conflictDetails: (repo: string) =>
    invoke<ConflictEntry[]>("conflict_details", { repo }),
  resolveKeep: (repo: string, path: string) =>
    invoke<void>("resolve_keep", { repo, path }),
  resolveRemove: (repo: string, path: string) =>
    invoke<void>("resolve_remove", { repo, path }),
  opSkip: (repo: string, kind: string) => invoke<OpOutcome>("op_skip", { repo, kind }),
```

- [ ] **Step 10: Verify Rust + TS compile.** Run from `src-tauri/`: `cargo check` (clean). Run from `git-it/`: `npm run check` (svelte-check clean — bindings/types only so far).

- [ ] **Step 11: Commit.**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(ops): classify conflicts + add keep/remove/skip resolutions (Phase 3b)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2 — Store: `repoStatus` state

**Files:**
- Modify: `src/lib/store.svelte.ts`

- [ ] **Step 1: Import the type.** Change the types import line to include `RepoStatus`:

```ts
import type { Commit, GraphCommit, RefEntry, RepoStatus } from "./types";
```

- [ ] **Step 2: Add the state field** (inside `makeState()`, near the other `$state` declarations, e.g. after `let graphLineStyle = ...`):

```ts
  // Working-copy/op status from repo_status; null in browser/sample mode (no op).
  let repoStatus = $state<RepoStatus | null>(null);
```

- [ ] **Step 3: Expose getter + setter** (add to the returned object, e.g. after `setGraphLineStyle`):

```ts
    get repoStatus() {
      return repoStatus;
    },
    setRepoStatus(s: RepoStatus | null) {
      repoStatus = s;
    },
```

- [ ] **Step 4: Verify.** Run from `git-it/`: `npm run check` (clean).

- [ ] **Step 5: Commit.**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(store): add repoStatus state for in-progress operations (Phase 3b)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3 — gitActions: `OpOutcome`-aware runner + wrappers

**Files:**
- Modify: `src/lib/gitActions.ts`

- [ ] **Step 1: Import `OpOutcome` type** at the top:

```ts
import type { OpOutcome } from "./types";
```

- [ ] **Step 2: Add `refreshStatus` and call it from `reloadGraph`.** Replace the current `reloadGraph` body so the Tauri branch also pulls `repo_status`:

```ts
export async function refreshStatus(): Promise<void> {
  if (!isTauri() || !appState.repo) {
    appState.setRepoStatus(null);
    return;
  }
  try {
    appState.setRepoStatus(await api.repoStatus(appState.repo));
  } catch (e) {
    console.warn("[gte] repo status refresh failed", e);
    appState.setRepoStatus(null);
  }
}

export async function reloadGraph(): Promise<void> {
  if (!isTauri()) {
    appState.setGraphCommits(SAMPLE_GRAPH);
    return;
  }
  if (!appState.repo) return;
  const gc = await api.loadGraph(appState.repo, RELOAD_COUNT, 0);
  appState.setGraphCommits(gc);
  await refreshStatus();
}
```

- [ ] **Step 3: Add the one-line error helper + `runOp` + `runResolve`** (after the existing `run` function):

```ts
// Git error/output text is often multiline; toasts get only the first line (contract d).
function firstLine(e: unknown): string {
  return String(e).split("\n")[0];
}

// Run an OpOutcome-returning op. Always refreshes graph+status (even on error) so
// the conflict panel can offer recovery (contract a). Summarizes status (contract d).
async function runOp(label: string, fn: () => Promise<OpOutcome>): Promise<OpOutcome | null> {
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return null;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return null;
  }
  appState.status = `${label}…`;
  try {
    const outcome = await fn();
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] graph refresh after op failed", e);
    }
    appState.status = outcome.conflicted
      ? `${label}: ${outcome.files.length} conflict(s) to resolve.`
      : `${label} — done.`;
    return outcome;
  } catch (e) {
    // Refresh so a half-finished op surfaces in the conflict panel for recovery.
    try {
      await reloadGraph();
    } catch (re) {
      console.warn("[gte] graph refresh after failed op failed", re);
    }
    appState.status = `${label} failed: ${firstLine(e)}`;
    return null;
  }
}

// Resolving a single file changes neither the commit graph nor selection/edits, so
// refresh only status (which drives the conflict list) — never reloadGraph here.
async function runResolve(label: string, fn: () => Promise<unknown>): Promise<boolean> {
  if (!isTauri() || !appState.repo) return false;
  try {
    await fn();
    await refreshStatus();
    return true;
  } catch (e) {
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}
```

- [ ] **Step 4: Add the wrappers to the `gitActions` object** (after `fetch`):

```ts
  merge: (reference: string, opts?: { noFf?: boolean; squash?: boolean }) =>
    runOp(`Merge ${reference}`, () =>
      api.merge(appState.repo, reference, opts?.noFf ?? false, opts?.squash ?? false),
    ),
  cherryPick: (shas: string[], label?: string) =>
    runOp(label ?? "Cherry-pick", () => api.cherryPick(appState.repo, shas)),
  revert: (shas: string[], label?: string) =>
    runOp(label ?? "Revert", () => api.revert(appState.repo, shas)),
  opContinue: (kind: string) =>
    runOp(`Continue ${kind}`, () => api.opContinue(appState.repo, kind)),
  opSkip: (kind: string) => runOp("Skip commit", () => api.opSkip(appState.repo, kind)),
  opAbort: (kind: string) => run(`Abort ${kind}`, () => api.opAbort(appState.repo, kind)),
  resolveConflict: (path: string, ours: boolean) =>
    runResolve(`Resolve ${path}`, () => api.resolveConflict(appState.repo, path, ours)),
  resolveKeep: (path: string) =>
    runResolve(`Keep ${path}`, () => api.resolveKeep(appState.repo, path)),
  resolveRemove: (path: string) =>
    runResolve(`Remove ${path}`, () => api.resolveRemove(appState.repo, path)),
```

> `opAbort` returns void, so it uses the existing `run()` — whose `reloadGraph()` now also calls `refreshStatus()`, clearing `repoStatus.operation` and hiding the panel.

- [ ] **Step 5: Verify.** Run from `git-it/`: `npm run check` (clean) and `npm test` (existing vitest still green — no behavior touched).

- [ ] **Step 6: Commit.**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(ops-ui): OpOutcome-aware action wrappers + status refresh (Phase 3b)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4 — `ConflictView.svelte` + mount

**Files:**
- Create: `src/lib/components/ConflictView.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Create `ConflictView.svelte`** with this exact content:

```svelte
<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { gitActions } from "../gitActions";
  import type { ConflictEntry } from "../types";

  function inTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  const op = $derived(appState.repoStatus?.operation ?? null);
  const conflicted = $derived(appState.repoStatus?.conflicted ?? 0);

  let details = $state<ConflictEntry[]>([]);

  async function refreshDetails() {
    if (!inTauri() || !appState.repo || !op) {
      details = [];
      return;
    }
    try {
      details = await api.conflictDetails(appState.repo);
    } catch {
      details = [];
    }
  }

  // Re-fetch the file list whenever an op is present and the conflict count changes
  // (each resolve decrements it). Reading both signals registers them as deps.
  $effect(() => {
    const o = appState.repoStatus?.operation;
    void appState.repoStatus?.conflicted;
    if (o) refreshDetails();
    else details = [];
  });

  function opLabel(kind: string): string {
    if (kind === "cherry-pick") return "Cherry-pick";
    return kind.charAt(0).toUpperCase() + kind.slice(1);
  }
</script>

{#if op}
  <section class="conflict panel">
    <header class="ch">
      <span class="title">{opLabel(op)} in progress</span>
      <span class="n" class:clear={conflicted === 0}>
        {conflicted === 0 ? "all conflicts resolved" : `${conflicted} conflict(s)`}
      </span>
    </header>

    {#if details.length}
      <ul class="files">
        {#each details as f (f.path)}
          <li>
            <span class="path mono" title={f.path}>{f.path}</span>
            <span class="acts">
              {#if f.kind === "both"}
                <button onclick={() => gitActions.resolveConflict(f.path, true)}>Use ours</button>
                <button onclick={() => gitActions.resolveConflict(f.path, false)}>Use theirs</button>
              {:else if f.kind === "modify-delete"}
                <button onclick={() => gitActions.resolveKeep(f.path)}>Keep file</button>
                <button onclick={() => gitActions.resolveRemove(f.path)}>Remove file</button>
              {:else}
                <button onclick={() => gitActions.resolveRemove(f.path)}>Remove file</button>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {:else if conflicted === 0}
      <p class="done">All conflicts resolved — continue to finish, or abort.</p>
    {/if}

    <footer class="cf">
      <button class="primary" disabled={conflicted > 0} onclick={() => gitActions.opContinue(op)}>
        Continue {opLabel(op).toLowerCase()}
      </button>
      {#if op === "cherry-pick" || op === "revert"}
        <button onclick={() => gitActions.opSkip(op)}>Skip this commit</button>
      {/if}
      <button class="danger" onclick={() => gitActions.opAbort(op)}>Abort</button>
    </footer>
  </section>
{/if}

<style>
  .conflict {
    border: 1px solid var(--err);
    border-radius: 8px;
    padding: 10px 12px;
    background: var(--panel-bg);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ch {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .title {
    font-weight: 600;
    color: var(--err);
  }
  .n {
    font-size: 12px;
    color: var(--text-muted);
  }
  .n.clear {
    color: var(--accent);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 220px;
    overflow: auto;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .files li:hover {
    background: var(--row-hover);
  }
  .path {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--text);
  }
  .acts {
    flex: 0 0 auto;
    display: flex;
    gap: 6px;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .cf {
    display: flex;
    gap: 8px;
    padding-top: 2px;
  }
  button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  button.danger {
    margin-left: auto;
    color: var(--danger);
    border-color: var(--danger);
  }
</style>
```

- [ ] **Step 2: Mount it in `+page.svelte`.** Add the import after the `CommitDetail` import:

```ts
  import ConflictView from "$lib/components/ConflictView.svelte";
```

Then in the `.main-col`, place it between `<GraphHistory />` and `<CommitDetail />`:

```svelte
    <div class="main-col">
      <GraphHistory />
      <ConflictView />
      <CommitDetail />
```

- [ ] **Step 3: Verify.** Run from `git-it/`: `npm run check` (clean). The panel renders nothing in the browser (no `repoStatus.operation`) — that's expected; visual verification is in Task 6.

- [ ] **Step 4: Commit.**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(ops-ui): ConflictView panel for in-progress merge/pick/revert (Phase 3b)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5 — Wire merge / cherry-pick / revert into context menus

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/components/GraphHistory.svelte`

- [ ] **Step 1: Add merge items to the Sidebar ref menu.** In `onRefContext`, after building the kind-specific items and before `contextMenu.openAt(...)`, append merge entries. Plain / no-ff / squash are separate items so they are inherently mutually exclusive (contract e). Don't offer merging the current branch into itself.

```ts
    // Merge this ref into the current branch (contract e: ff/no-ff/squash are
    // distinct items, never combined). Skip when this IS the checked-out branch.
    if (!(kind === "local" && r.isHead)) {
      items.push({ separator: true });
      items.push({ label: `Merge ${r.name} into current`, action: () => gitActions.merge(r.name) });
      items.push({ label: `Merge ${r.name} (no-ff)`, action: () => gitActions.merge(r.name, { noFf: true }) });
      items.push({ label: `Merge ${r.name} (squash)`, action: () => gitActions.merge(r.name, { squash: true }) });
    }
```

- [ ] **Step 2: Add cherry-pick/revert to the GraphHistory row menu.** In `onRowContext`, change the tail of the items array (currently `{ separator: true }, { label: "Copy SHA", ... }`) to insert the two ops:

```ts
      { separator: true },
      {
        label: "Cherry-pick onto current",
        action: () => gitActions.cherryPick([sha], `Cherry-pick ${short}`),
      },
      {
        label: "Revert commit",
        action: () => gitActions.revert([sha], `Revert ${short}`),
      },
      { separator: true },
      { label: "Copy SHA", action: () => navigator.clipboard?.writeText(sha) },
```

- [ ] **Step 3: Verify.** Run from `git-it/`: `npm run check` (clean).

- [ ] **Step 4: Commit.**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(ops-ui): merge/cherry-pick/revert context-menu entries (Phase 3b)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6 — Full verification + preview

**Files:** none (verification only)

- [ ] **Step 1: All gates green.**
  - From `src-tauri/`: `cargo test` (all pass) and `cargo check` (clean).
  - From `git-it/`: `npm run check` (clean) and `npm test` (green).

- [ ] **Step 2: Browser preview — menus.** Start the preview (Claude_Preview, `preview_start` name `vite`, port 1420), resize to ~1200 wide. Right-click a graph row → confirm "Cherry-pick onto current" and "Revert commit" appear. Right-click a sidebar branch (non-current) → confirm the three Merge items appear; right-click the current branch → confirm Merge items are absent. (Clicking them shows the "needs desktop app" status — that's expected in-browser.)

- [ ] **Step 3: Browser preview — ConflictView render.** The panel needs `repoStatus.operation`, absent in-browser. Verify rendering by *temporarily* forcing state: in `+page.svelte`'s `onMount`, after the sample load, add a throwaway block (REVERT before commit):

```ts
    if (!inTauri) {
      appState.setRepoStatus({
        head: { sha: null, branch: "main", detached: false },
        staged: 0, unstaged: 0, untracked: 0, conflicted: 2, operation: "merge",
      });
    }
```

Also temporarily stub `ConflictView`'s `refreshDetails` to set two mock entries (one `both`, one `modify-delete`) so the buttons render. Screenshot light + dark (`preview_resize` / prefers-color-scheme). Confirm: header shows "Merge in progress — 2 conflict(s)", correct buttons per kind, Continue disabled, Abort present. **Revert both temporary edits**, re-run `npm run check`, and confirm the panel disappears.

- [ ] **Step 4: Commit** (only if Step 3 left any intentional, non-throwaway change — normally nothing to commit here).

---

## After all tasks
- Dispatch a final adversarial review (Agent `superpowers:code-reviewer`, opus) over the whole branch diff vs `main`, specifically checking the 5 contracts, the porcelain-v2 parse (paths with spaces, the `nth(8)` offset), `git rm -f` safety, and that no git shell-out lost its `--` / `--end-of-options`.
- Fix findings, re-verify gates.
- `superpowers:finishing-a-development-branch` → merge `feat/phase3b-conflict-ui` to `main` (`--ff-only`), delete the branch.
- Update memory (`git-client-migration.md`) + `docs/RESUME.md` to mark Phase 3b done and point to Phase 4.

## Deferred (not in this slice)
- Multi-commit cherry-pick/revert from a selection (MVP does the right-clicked commit only).
- A diff/preview of each conflicted hunk (Phase 5 diff viewer).
- Rebase conflicts (Phase 4 — same panel should generalize; `op_subcommand`/`detect_operation` already know "rebase").
- Refreshing refs/graph without nuking pending `newDates` (cross-cutting, tracked since the 2b review).
