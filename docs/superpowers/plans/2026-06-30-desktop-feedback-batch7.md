# Desktop feedback batch 7 — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship five real-world desktop "Git It" fixes/UX items: the blank unstaged new-file bug, branch
double-click checkout + fast-forward-to-origin, branch-delete remote/force options, a tasteful loading
indicator, and the PR detail timeline with inline review comments + CI history.

**Architecture:** Tauri 2 + SvelteKit 5 (Svelte 5 runes). Backend = `crates/git-core` (Tauri-free git
shell-outs + `gh` wrapper) exposed via `src-tauri/src/commands.rs` Tauri commands, called from
`src/lib/api.ts` → `src/lib/gitActions.ts` → components. Each task is an independent slice.

**Tech stack:** Rust (git CLI + gh CLI), TypeScript, Svelte 5, Vitest, cargo test.

**Branch:** `batch7-desktop-feedback` (off `main` @ `6a3c240`).

**Per-task gates (run before each commit):**
- `cd git-it && npm run check` → svelte-check 0 errors / 0 warnings
- `cd git-it && npm test` → vitest green
- `cd git-it && cargo test` → all green (workspace)
- `cd git-it && cargo build -p git-it` → Finished

**Security note (existing convention):** every git shell-out that takes a user operand must use `--` or
`--end-of-options` so an operand like `-D` or `--upload-pack=<cmd>` can't smuggle in a flag. Follow it.

---

## Task 1: Fix the blank unstaged new-file bug (`-uall`)

**Files:**
- Modify: `crates/git-core/src/ops_worktree.rs:10-75` (the `working_changes` fn) + its `#[cfg(test)]` module.

- [ ] **Step 1: Write the failing test**

In the `mod tests` block of `crates/git-core/src/ops_worktree.rs`, add (the existing tests use a helper
`TestRepo` with `.write(path, contents)` and `.path`; mirror `lists_untracked_modified_staged` at ~line
606):

```rust
    #[test]
    fn lists_files_inside_a_new_untracked_directory_individually() {
        let r = TestRepo::new();
        r.write("README.md", "base\n");
        r.git(&["add", "."]);
        r.git(&["commit", "-m", "init"]);
        // A brand-new directory with files. Default `-unormal` collapses this to
        // "newdir/"; we want each file listed.
        r.write("newdir/a.txt", "a\n");
        r.write("newdir/b.txt", "b\n");
        let files = working_changes(&r.path).unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"newdir/a.txt"), "got {:?}", paths);
        assert!(paths.contains(&"newdir/b.txt"), "got {:?}", paths);
        // No collapsed-directory / blank entries.
        assert!(!paths.iter().any(|p| p.is_empty() || p.ends_with('/')), "got {:?}", paths);
    }
```

If `TestRepo`'s exact helper names differ, read the existing tests in this file and match them (e.g. some
use a free `unique_dir(...)` + `git_init`; use whatever the sibling tests use).

- [ ] **Step 2: Run the test — verify it FAILS**

Run: `cd git-it && cargo test -p git-core working_changes`
Expected: the new test fails — `paths` contains `"newdir/"` (and lacks the two files).

- [ ] **Step 3: Implement the fix**

In `working_changes`, change the status invocation (currently `ops_worktree.rs:12`):

```rust
    c.current_dir(repo)
        .args(["status", "--porcelain=v2", "-z", "--untracked-files=all"]);
```

And make the untracked arm defensive (currently `ops_worktree.rs:63-71`) — skip empty/trailing-slash paths
so a collapsed entry can never render blank:

```rust
        } else if let Some(path) = rec.strip_prefix("? ") {
            // `-uall` lists files individually, but guard anyway: never surface a
            // blank-named directory entry (basename of "dir/" is "").
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
```

- [ ] **Step 4: Run the test — verify it PASSES**

Run: `cd git-it && cargo test -p git-core working_changes`
Expected: PASS. Also run `cd git-it && cargo test` (whole workspace) — all green.

- [ ] **Step 5: Commit**

```bash
cd git-it && git add crates/git-core/src/ops_worktree.rs
git commit -m "fix(worktree): list new-directory files individually (-uall) so unstaged new files aren't blank

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Branch double-click checkout + fast-forward to origin

**Files:**
- Modify: `crates/git-core/src/ops.rs` (add `fast_forward_branch` after `fetch`, ~line 77) + tests.
- Modify: `src-tauri/src/commands.rs` (add `fast_forward_branch` command; find the `fetch` command and add beside it).
- Modify: `src/lib/api.ts:70` (add `fastForwardBranch`).
- Modify: `src/lib/gitActions.ts:556` (add `fastForwardBranch` action).
- Modify: `src/lib/components/RefTree.svelte` (add `onCheckout` prop + `ondblclick`).
- Modify: `src/lib/components/Sidebar.svelte` (pass `onCheckout`; add FF menu item).

- [ ] **Step 1: Write the failing Rust test for the FF op**

In `crates/git-core/src/ops.rs` `mod tests`, add (use the file's existing repo helpers — it has
`unique_dir` + git plumbing; create a clone so there's an upstream):

```rust
    #[test]
    fn fast_forward_branch_advances_non_current_branch() {
        // origin repo with main + one commit
        let origin = unique_dir("ff-origin");
        fs::create_dir_all(&origin).unwrap();
        run_git(&origin, &["init", "-q", "-b", "main"]);
        run_git(&origin, &["config", "user.email", "t@t"]);
        run_git(&origin, &["config", "user.name", "t"]);
        fs::write(origin.join("f"), "1").unwrap();
        run_git(&origin, &["add", "."]);
        run_git(&origin, &["commit", "-qm", "c1"]);
        // clone it
        let clone = unique_dir("ff-clone");
        run_git(clone.parent().unwrap(), &["clone", "-q", origin.to_str().unwrap(), clone.to_str().unwrap()]);
        // advance origin/main by one commit
        fs::write(origin.join("f"), "2").unwrap();
        run_git(&origin, &["commit", "-qam", "c2"]);
        // in the clone, create a second branch tracking origin/main, switch away to it,
        // fetch, and FF main (the non-current branch) up to origin/main.
        run_git(&clone, &["fetch", "-q", "origin"]);
        run_git(&clone, &["checkout", "-qb", "work"]);
        let before = rev_parse(&clone, "main");
        let res = fast_forward_branch(&clone, "main", "origin");
        assert!(res.is_ok(), "ff failed: {:?}", res);
        let after = rev_parse(&clone, "main");
        assert_ne!(before, after, "main should have advanced");
        assert_eq!(after, rev_parse(&clone, "origin/main"));
    }
```

Use whatever git-run + rev-parse helpers the file already defines; if there's no `run_git`/`rev_parse`,
add tiny local helpers in the test module that shell out with `std::process::Command` and return trimmed
stdout. Read the existing `ops.rs` tests first and reuse their style.

- [ ] **Step 2: Run — verify it FAILS** (`fast_forward_branch` not defined)

Run: `cd git-it && cargo test -p git-core fast_forward_branch`
Expected: compile error / fail — function doesn't exist.

- [ ] **Step 3: Implement `fast_forward_branch` in `ops.rs`** (after `fetch`, ~line 77)

```rust
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
        // --end-of-options so a remote/branch starting with '-' can't inject a flag.
        .args(["fetch", "--end-of-options", remote, &refspec]);
    let (o, e) = git_ops::run(&mut c)?;
    Ok(format!("{}{}", o, e).trim().to_string())
}
```

- [ ] **Step 4: Run — verify it PASSES**

Run: `cd git-it && cargo test -p git-core fast_forward_branch` → PASS. Then `cargo test` whole workspace.

- [ ] **Step 5: Add the Tauri command** in `src-tauri/src/commands.rs` (next to the `fetch` command)

```rust
#[tauri::command]
pub fn fast_forward_branch(repo: String, branch: String, remote: String) -> Result<String, String> {
    ops::fast_forward_branch(&PathBuf::from(repo), &branch, &remote)
}
```

Then register it in the `generate_handler!` list in `src-tauri/src/lib.rs` (add `commands::fast_forward_branch`
beside `commands::fetch` — search lib.rs for `fetch` to find the list).

- [ ] **Step 6: Add the api binding** in `src/lib/api.ts` (after `fetch`, line 71)

```ts
  fastForwardBranch: (repo: string, branch: string, remote: string) =>
    invoke<string>("fast_forward_branch", { repo, branch, remote }),
```

- [ ] **Step 7: Add the gitActions action** in `src/lib/gitActions.ts` (after `deleteBranch`, line 551)

```ts
  fastForwardBranch: (branch: string, remote: string) =>
    run(`Fast-forward ${branch} → ${remote}/${branch}`, () =>
      api.fastForwardBranch(appState.repo, branch, remote),
    ),
```

- [ ] **Step 8: Wire double-click checkout in `RefTree.svelte`**

Add `onCheckout` to the props block (after `colorOf`):

```ts
    onCheckout,
    ...
    onCheckout: (ref: RefEntry, kind: "local" | "remote" | "tag") => void;
```

On the `.ref` button (line 50-59), add a dblclick handler beside `onclick`:

```svelte
        onclick={() => onJump(node.ref.sha)}
        ondblclick={() => onCheckout(node.ref, kind)}
```

- [ ] **Step 9: Wire `onCheckout` + the FF menu item in `Sidebar.svelte`**

Add a checkout dispatcher near `jumpTo` (handles local/remote/tag):

```ts
  function onRefCheckout(r: RefEntry, kind: "local" | "remote" | "tag") {
    if (kind === "local") {
      gitActions.checkout(r.name);
    } else if (kind === "remote") {
      // origin/feature → local "feature" tracking branch (create if missing, else switch).
      const short = r.name.replace(/^[^/]+\//, "");
      const exists = appState.refsByKind.local.some((b) => b.name === short);
      if (exists) gitActions.checkout(short);
      else gitActions.createBranch(short, r.name).then((ok) => { if (ok) gitActions.checkout(short); });
    }
    // tag → no-op (double-click only jumps).
  }
```

Pass it to all three `<RefTree …>` usages (lines 181/187/193): add `onCheckout={onRefCheckout}`.

In `onRefContext`, for a **local non-current** branch, insert a Fast-forward item after the "Delete
branch" item's separator block (before the colour separator at line 118). Resolve the upstream remote from
`appState.refsDetailed`:

```ts
    if (kind === "local" && !r.isHead) {
      const detail = appState.refsDetailed.find((d) => d.kind === "local" && d.name === r.name);
      const upstream = detail?.upstream ?? null; // e.g. "origin/main"
      if (upstream) {
        const remote = upstream.slice(0, upstream.indexOf("/"));
        items.push({
          label: `Fast-forward to ${remote}`,
          action: () => gitActions.fastForwardBranch(r.name, remote),
        });
      }
    }
```

(Confirm `appState.refsDetailed` is the exposed name — grep store.svelte.ts for `refsDetailed`; the
exploration confirmed `Ref.upstream` exists and `refsDetailed` holds `Ref[]`.)

- [ ] **Step 10: Gates**

Run: `cd git-it && npm run check && npm test && cargo test && cargo build -p git-it` → all green.

- [ ] **Step 11: Commit**

```bash
cd git-it && git add -A
git commit -m "feat(branches): double-click to checkout + right-click fast-forward to origin

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Branch delete — remote + force options

**Files:**
- Modify: `crates/git-core/src/ops.rs:36-42` (`delete_branch` new signature) + test.
- Modify: `src-tauri/src/commands.rs` (`delete_branch` command signature).
- Modify: `src/lib/api.ts:65` (`deleteBranch` signature).
- Modify: `src/lib/gitActions.ts:550` (`deleteBranch` signature).
- Modify: `src/lib/dialogs.svelte.ts` (new `branchDelete` dialog kind).
- Modify: `src/lib/components/Modal.svelte` (render the new kind).
- Modify: `src/lib/components/Sidebar.svelte:48-66` (rewrite `confirmDeleteBranch`).

- [ ] **Step 1: Write the failing Rust test (remote delete)**

In `ops.rs` `mod tests`, add a test that deletes a branch locally AND on a (bare) remote:

```rust
    #[test]
    fn delete_branch_can_also_delete_the_remote() {
        let origin = unique_dir("del-origin");
        fs::create_dir_all(&origin).unwrap();
        run_git(&origin, &["init", "-q", "--bare"]);
        let work = unique_dir("del-work");
        run_git(work.parent().unwrap(), &["clone", "-q", origin.to_str().unwrap(), work.to_str().unwrap()]);
        run_git(&work, &["config", "user.email", "t@t"]);
        run_git(&work, &["config", "user.name", "t"]);
        fs::write(work.join("f"), "1").unwrap();
        run_git(&work, &["add", "."]);
        run_git(&work, &["commit", "-qm", "c1"]);
        run_git(&work, &["push", "-q", "origin", "HEAD:main"]);
        run_git(&work, &["branch", "feature"]);
        run_git(&work, &["push", "-q", "origin", "feature"]);
        run_git(&work, &["checkout", "-q", "main"]); // can't be on feature when deleting it
        // delete local feature (force, since not merged) AND remote origin/feature
        let res = delete_branch(&work, "feature", true, true, Some("origin"), Some("feature"));
        assert!(res.is_ok(), "{:?}", res);
        // remote ref is gone
        let remote_refs = run_git(&origin, &["for-each-ref", "--format=%(refname)"]);
        assert!(!remote_refs.contains("refs/heads/feature"), "remote feature should be deleted: {}", remote_refs);
    }
```

- [ ] **Step 2: Run — verify it FAILS** (arity mismatch). `cd git-it && cargo test -p git-core delete_branch`

- [ ] **Step 3: Implement the new `delete_branch`** in `ops.rs` (replace lines 36-42)

```rust
/// Delete a branch. `force` uses -D (drops even unmerged commits); without it, -d
/// refuses to delete a branch whose commits aren't merged into HEAD. When
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
                .args(["push", rem, "--delete", "--", rb]);
            git_ops::run(&mut p)
                .map_err(|e| format!("Deleted local branch, but remote delete failed: {}", e))?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run — verify it PASSES.** `cd git-it && cargo test -p git-core delete_branch`, then `cargo test`.

- [ ] **Step 5: Update the Tauri command** in `src-tauri/src/commands.rs` (the `delete_branch` command)

```rust
#[tauri::command]
pub fn delete_branch(
    repo: String,
    name: String,
    force: bool,
    delete_remote: Option<bool>,
    remote: Option<String>,
    remote_branch: Option<String>,
) -> Result<(), String> {
    ops::delete_branch(
        &PathBuf::from(repo),
        &name,
        force,
        delete_remote.unwrap_or(false),
        remote.as_deref(),
        remote_branch.as_deref(),
    )
}
```

- [ ] **Step 6: Update the api binding** in `src/lib/api.ts:65`

```ts
  deleteBranch: (
    repo: string,
    name: string,
    force: boolean,
    deleteRemote?: boolean,
    remote?: string,
    remoteBranch?: string,
  ) =>
    invoke<void>("delete_branch", {
      repo, name, force,
      deleteRemote: deleteRemote ?? false,
      remote: remote ?? null,
      remoteBranch: remoteBranch ?? null,
    }),
```

- [ ] **Step 7: Update the gitActions action** in `src/lib/gitActions.ts:550`

```ts
  deleteBranch: (
    name: string,
    force: boolean,
    deleteRemote = false,
    remote?: string,
    remoteBranch?: string,
  ) =>
    run(`Delete branch ${name}`, () =>
      api.deleteBranch(appState.repo, name, force, deleteRemote, remote, remoteBranch),
    ),
```

- [ ] **Step 8: Add the `branchDelete` dialog kind** in `src/lib/dialogs.svelte.ts`

Add to the `DialogState` union:

```ts
  | {
      kind: "branchDelete";
      title: string;
      branch: string;
      upstream: string | null; // e.g. "origin/feature", or null when no upstream
      force: boolean;
      deleteRemote: boolean;
      resolve: (v: { confirmed: boolean; force: boolean; deleteRemote: boolean }) => void;
    }
```

In `settlePending`, add:

```ts
    else if (state.kind === "branchDelete") state.resolve({ confirmed: false, force: false, deleteRemote: false });
```

Add the opener + setters + resolver to the returned object:

```ts
    confirmBranchDelete(opts: { branch: string; upstream: string | null }): Promise<{ confirmed: boolean; force: boolean; deleteRemote: boolean }> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "branchDelete",
          title: "Delete branch",
          branch: opts.branch,
          upstream: opts.upstream,
          force: false,
          deleteRemote: false,
          resolve,
        };
      });
    },
    setBranchDeleteForce(v: boolean) {
      if (state.kind === "branchDelete") state = { ...state, force: v };
    },
    setBranchDeleteRemote(v: boolean) {
      if (state.kind === "branchDelete") state = { ...state, deleteRemote: v };
    },
    resolveBranchDelete(confirmed: boolean) {
      if (state.kind === "branchDelete") {
        const { force, deleteRemote } = state;
        state.resolve({ confirmed, force, deleteRemote });
        state = { kind: "none" };
      }
    },
```

- [ ] **Step 9: Render the kind in `Modal.svelte`**

Read `Modal.svelte` and follow the existing `destructive` branch (which renders a checkbox bound via
`dialogs.setDestructiveBackup` + confirm/cancel via `dialogs.resolveDestructive`). Add a `branchDelete`
branch with the title, the message `Delete branch "{branch}"?`, and two checkboxes:
- "Force delete (discard unmerged commits)" → `onchange` calls `dialogs.setBranchDeleteForce(checked)`,
  bound to `state.force`.
- Only when `state.upstream` is non-null: `Also delete {state.upstream}` → `dialogs.setBranchDeleteRemote`,
  bound to `state.deleteRemote`.
- A danger "Delete" button → `dialogs.resolveBranchDelete(true)`; Cancel → `dialogs.resolveBranchDelete(false)`.
Match the existing destructive dialog's markup/classes for visual consistency.

- [ ] **Step 10: Rewrite `confirmDeleteBranch` in `Sidebar.svelte:48-66`**

```ts
  async function confirmDeleteBranch(name: string) {
    const detail = appState.refsDetailed.find((d) => d.kind === "local" && d.name === name);
    const upstream = detail?.upstream ?? null; // "origin/feature" | null
    const res = await dialogs.confirmBranchDelete({ branch: name, upstream });
    if (!res.confirmed) return;
    let remote: string | undefined;
    let remoteBranch: string | undefined;
    if (res.deleteRemote && upstream) {
      const slash = upstream.indexOf("/");
      remote = upstream.slice(0, slash);
      remoteBranch = upstream.slice(slash + 1);
    }
    await gitActions.deleteBranch(name, res.force, res.deleteRemote && !!upstream, remote, remoteBranch);
  }
```

- [ ] **Step 11: Gates** — `cd git-it && npm run check && npm test && cargo test && cargo build -p git-it`.

- [ ] **Step 12: Commit**

```bash
cd git-it && git add -A
git commit -m "feat(branches): delete dialog with force + also-delete-remote toggles

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Loading indicator — top sweep + graph skeleton

**Files:**
- Create: `src/lib/components/LoadingBar.svelte`.
- Create: `src/lib/components/GraphSkeleton.svelte`.
- Modify: `src/lib/store.svelte.ts` (add a `navBusy` signal alongside `repoLoading`, ~line 1061/1546).
- Modify: `src/lib/gitActions.ts` (set/clear `navBusy` in `run()` for nav ops; ensure cleared on all paths).
- Modify: `src/routes/+page.svelte` (mount `<LoadingBar/>`; render `<GraphSkeleton/>` in the graph area while loading).

- [ ] **Step 1: Add a `navBusy` signal to the store**

In `store.svelte.ts`, next to `repoLoading` (declared ~line 1061, getter ~1546), add a parallel boolean
with a getter/setter, mirroring `repoLoading` exactly:

```ts
  let navBusy = $state<boolean>(false);
  // …in the returned object, beside repoLoading:
  get navBusy() { return navBusy; },
  setNavBusy(v: boolean) { navBusy = v; },
```

- [ ] **Step 2: Drive `navBusy` from the `run()` wrapper** in `gitActions.ts:285`

Wrap the body so navBusy is true during the op + graph refresh, always cleared:

```ts
async function run(label: string, fn: () => Promise<unknown>): Promise<boolean> {
  if (!isTauri()) { appState.status = "That action needs the desktop app (not the browser preview)."; return false; }
  if (!appState.repo) { appState.status = "Open a repository first."; return false; }
  appState.setNavBusy(true);
  try {
    appState.status = `${label}…`;
    await fn();
    try { await reloadGraph(); } catch (e) { console.warn("[gte] graph refresh after op failed", e); }
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  } finally {
    appState.setNavBusy(false);
  }
}
```

- [ ] **Step 3: Create `LoadingBar.svelte`**

A thin indeterminate accent sweep, in-flow at the very top of the app shell. Visible when
`appState.repoLoading || appState.navBusy`. Respects reduced motion.

```svelte
<script lang="ts">
  import { appState } from "../store.svelte";
  const busy = $derived(appState.repoLoading || appState.navBusy);
</script>

{#if busy}
  <div class="loadbar" role="progressbar" aria-busy="true" aria-label="Loading">
    <div class="seg"></div>
  </div>
{/if}

<style>
  .loadbar {
    position: relative;
    height: 3px;
    width: 100%;
    overflow: hidden;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .seg {
    position: absolute;
    top: 0;
    height: 100%;
    width: 35%;
    border-radius: 999px;
    background: var(--accent);
    animation: gi-sweep 1.1s ease-in-out infinite;
  }
  @keyframes gi-sweep {
    0% { left: -35%; }
    100% { left: 100%; }
  }
  @media (prefers-reduced-motion: reduce) {
    .seg { animation: none; left: 0; width: 100%; opacity: 0.4; }
  }
</style>
```

- [ ] **Step 4: Create `GraphSkeleton.svelte`**

Shaped commit-graph rows with the existing accent-glow sweep (mirror `github/Skeleton.svelte`'s
`gh-sweep`). ~8 rows; each row = dot + lane bar + subject bar + meta bar; a moving accent glow overlay.
Respect reduced motion (static, no sweep).

```svelte
<script lang="ts">
  const rows = Array.from({ length: 8 }, (_, i) => i);
  const widths = [54, 42, 62, 48, 58, 38, 66, 50];
</script>

<div class="gs" aria-hidden="true">
  {#each rows as i}
    <div class="row">
      <span class="dot" style={`margin-left:${(i % 3) * 14}px`}></span>
      <span class="bar" style={`width:${widths[i]}%`}></span>
      <span class="meta"></span>
      <span class="glow" style={`animation-delay:${i * 60}ms`}></span>
    </div>
  {/each}
</div>

<style>
  .gs { padding: 10px 16px; display: flex; flex-direction: column; gap: 12px; }
  .row { position: relative; overflow: hidden; display: flex; align-items: center; gap: 12px; height: 16px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--row-hover); flex: none; }
  .bar { height: 9px; border-radius: 999px; background: var(--row-hover); }
  .meta { height: 9px; width: 12%; border-radius: 999px; background: var(--row-hover); margin-left: auto; }
  .glow { position: absolute; inset: 0; background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 22%, transparent), transparent); animation: gs-sweep 1.5s ease-in-out infinite; }
  @keyframes gs-sweep { 0% { transform: translateX(-100%); } 100% { transform: translateX(280%); } }
  @media (prefers-reduced-motion: reduce) { .glow { animation: none; opacity: 0.25; } }
</style>
```

(Use the actual hover/placeholder token the app uses — `--row-hover` exists in RefTree.svelte; if a
dedicated skeleton token exists in `github/Skeleton.svelte`, prefer matching it. Read that file.)

- [ ] **Step 5: Wire into `+page.svelte`**

Read `src/routes/+page.svelte`. Mount `<LoadingBar/>` once at the top of the app shell (above the
tabs/header content, so the sweep sits at the window's top edge — alongside where `RemoteProgress` mounts,
~line 359). Then, in the commit-graph area, when `appState.repoLoading` is true AND there are no graph rows
yet (`appState.graphCommits.length === 0`), render `<GraphSkeleton/>` in place of the empty graph. Import
both components. Keep it minimal — do not disturb the existing graph virtualization markup; gate the
skeleton with `{#if appState.repoLoading && appState.graphCommits.length === 0}…{/if}` near the graph host.

- [ ] **Step 6: Verify in the browser preview**

The browser preview can't switch real repos, but the components render. Use the preview tools to confirm
`LoadingBar` and `GraphSkeleton` mount without console errors and animate. (Do `appState.setNavBusy(true)`
via `preview_eval` if needed to force the bar visible.) Capture a screenshot.

- [ ] **Step 7: Gates** — `cd git-it && npm run check && npm test && cargo build -p git-it`.

- [ ] **Step 8: Commit**

```bash
cd git-it && git add -A
git commit -m "feat(loading): top accent sweep + commit-graph skeleton during repo/branch load

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: PR detail timeline + inline review comments + CI history

**Files:**
- Modify: `crates/git-core/src/github/mod.rs` (`pr_detail` + new structs/types, ~lines 1010-1164).
- Modify: `src/lib/types.ts:261-274` (new Gh types + extend `GhPullDetail`).
- Create: `src/lib/github/prTimeline.ts` (+ `src/lib/github/prTimeline.test.ts`).
- Create: `src/lib/components/github/PrTimeline.svelte`.
- Modify: `src/lib/components/github/GithubDetail.svelte` (pinned checks strip + render `PrTimeline`).

### 5a — Backend: fetch commits, review threads, CI runs

- [ ] **Step 1: Add commits to the `gh pr view` field list** in `github/mod.rs` `pr_detail`

Find the JSON field list (~line 1157) and append `,commits`. Add a `RawCommit` deser struct and a public
`GhCommit`:

```rust
#[derive(Deserialize)]
struct RawCommit {
    oid: String,
    #[serde(rename = "messageHeadline", default)]
    message_headline: String,
    #[serde(rename = "committedDate", default)]
    committed_date: String,
    #[serde(default)]
    authors: Vec<RawUser>,
}

#[derive(Serialize, Clone)]
pub struct GhCommit {
    pub oid: String,
    pub message: String,
    pub author: String,
    pub committed_date: String,
}
```

(Read the existing `RawUser`/`RawPullDetail`/`map_*` helpers and match their exact shapes — `gh pr view`
returns commits as `[{ oid, messageHeadline, committedDate, authors: [{ login, name }] }]`.)

- [ ] **Step 2: Fetch review threads via GraphQL** (new helper in `github/mod.rs`)

```rust
fn fetch_review_threads(owner: &str, repo: &str, number: i64) -> Vec<GhReviewThread> {
    let query = r#"query($owner:String!,$repo:String!,$num:Int!){
      repository(owner:$owner,name:$repo){
        pullRequest(number:$num){
          reviewThreads(first:100){ nodes{
            isResolved path line
            comments(first:50){ nodes{ author{login} body path originalLine line createdAt } }
          } }
        }
      }
    }"#;
    let args = [
        "api", "graphql",
        "-f", &format!("query={}", query),
        "-F", &format!("owner={}", owner),
        "-F", &format!("repo={}", repo),
        "-F", &format!("num={}", number),
    ];
    match run_gh(&args, None) {
        Ok(out) => parse_review_threads(&out),
        Err(_) => Vec::new(), // best-effort: no perms / no threads → empty
    }
}
```

Add `parse_review_threads` (serde_json navigation of
`data.repository.pullRequest.reviewThreads.nodes[]`) returning:

```rust
#[derive(Serialize, Clone)]
pub struct GhInlineComment {
    pub author: String,
    pub body: String,
    pub path: String,
    pub line: i64,
    pub created_at: String,
}
#[derive(Serialize, Clone)]
pub struct GhReviewThread {
    pub resolved: bool,
    pub path: String,
    pub line: i64,
    pub comments: Vec<GhInlineComment>,
}
```

Match `run_gh`'s actual signature (the file already shells out to gh — read an existing call, e.g. inside
`pr_detail`, and reuse the exact helper + arg-passing convention, including how `-f`/`-F` are quoted).

- [ ] **Step 3: Fetch CI run history** (new helper in `github/mod.rs`)

```rust
fn fetch_check_runs(owner: &str, repo: &str, head_ref: &str) -> Vec<GhCheckRun> {
    let path = format!(
        "repos/{}/{}/actions/runs?branch={}&per_page=30",
        owner, repo, head_ref
    );
    match run_gh(&["api", &path], None) {
        Ok(out) => parse_check_runs(&out),
        Err(_) => Vec::new(),
    }
}
```

with:

```rust
#[derive(Serialize, Clone)]
pub struct GhCheckRun {
    pub name: String,
    pub status: String,      // queued | in_progress | completed
    pub conclusion: String,  // success | failure | cancelled | "" (null)
    pub started_at: String,  // run_started_at
    pub updated_at: String,
    pub url: String,         // html_url
    pub head_sha: String,
}
```

`parse_check_runs` reads `workflow_runs[]` from the response, mapping `name`, `status`, `conclusion`
(null → ""), `run_started_at`, `updated_at`, `html_url`, `head_sha`.

- [ ] **Step 4: Extend `GhPullDetail` + assemble** in `pr_detail`

Add to the public `GhPullDetail` struct:

```rust
    pub commits: Vec<GhCommit>,
    pub review_threads: Vec<GhReviewThread>,
    pub check_runs: Vec<GhCheckRun>,
```

In `pr_detail`, after the existing `gh pr view` parse, populate the three new fields by calling the helpers
(owner/repo are already resolved in this module; `head_ref_name` is on the parsed detail). All three are
best-effort and must never fail the whole command.

- [ ] **Step 5: Backend gate** — `cd git-it && cargo test && cargo build -p git-it` (green; the existing
github tests must still pass — the new fields are additive).

- [ ] **Step 6: Commit backend**

```bash
cd git-it && git add crates/git-core/src/github/mod.rs
git commit -m "feat(github): pr_detail fetches commits, review threads (isResolved) and CI run history

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### 5b — Frontend: timeline builder + component

- [ ] **Step 7: Extend `types.ts`** (after line 274)

```ts
export type GhCommit = { oid: string; message: string; author: string; committedDate: string };
export type GhInlineComment = { author: string; body: string; path: string; line: number; createdAt: string };
export type GhReviewThread = { resolved: boolean; path: string; line: number; comments: GhInlineComment[] };
export type GhCheckRun = {
  name: string; status: string; conclusion: string;
  startedAt: string; updatedAt: string; url: string; headSha: string;
};
export type PrTimelineEvent =
  | { kind: "commit"; at: string; commit: GhCommit }
  | { kind: "comment"; at: string; comment: GhComment }
  | { kind: "review"; at: string; review: GhReview; threads: GhReviewThread[] }
  | { kind: "ciRun"; at: string; run: GhCheckRun };
```

Extend `GhPullDetail` with `commits: GhCommit[]; reviewThreads: GhReviewThread[]; checkRuns: GhCheckRun[];`.

**Important — serde casing:** the Rust structs serialize snake_case (`committed_date`, `review_threads`,
`check_runs`, `started_at`, `head_sha`). Check how the rest of `GhPullDetail` is cased: existing fields are
camelCase (`createdAt`, `headRefName`), which means the github module already applies
`#[serde(rename_all = "camelCase")]` somewhere or the gh JSON is camelCase. Verify the serialize casing of
the NEW structs and make the TS types match exactly (add `#[serde(rename_all = "camelCase")]` to the new
public structs if the others use it, so TS sees `committedDate`, `reviewThreads`, `checkRuns`, `startedAt`,
`headSha`). The TS above assumes camelCase — keep Rust + TS consistent.

- [ ] **Step 8: Write the timeline builder test** `src/lib/github/prTimeline.test.ts`

```ts
import { describe, it, expect } from "vitest";
import { buildPrTimeline } from "./prTimeline";
import type { GhPullDetail } from "../types";

function base(): GhPullDetail {
  return {
    number: 1, title: "t", body: "", author: "a", state: "OPEN", isDraft: false,
    labels: [], assignees: [], milestone: null, baseRefName: "main", headRefName: "f",
    reviewDecision: "", mergeable: "", mergeStateStatus: "",
    additions: 0, deletions: 0, changedFiles: 0,
    files: [], reviews: [], checks: [], comments: [],
    commits: [], reviewThreads: [], checkRuns: [],
    createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z", url: "",
  };
}

describe("buildPrTimeline", () => {
  it("orders events ascending by timestamp across kinds", () => {
    const d = base();
    d.commits = [{ oid: "aaaaaaa", message: "c", author: "a", committedDate: "2026-01-02T00:00:00Z" }];
    d.comments = [{ author: "b", body: "hi", createdAt: "2026-01-04T00:00:00Z" }];
    d.reviews = [{ author: "c", state: "APPROVED", body: "", submittedAt: "2026-01-03T00:00:00Z" }];
    d.checkRuns = [{ name: "ci", status: "completed", conclusion: "success", startedAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:05:00Z", url: "", headSha: "aaaaaaa" }];
    const ev = buildPrTimeline(d);
    expect(ev.map((e) => e.kind)).toEqual(["ciRun", "commit", "review", "comment"]);
  });

  it("attaches review threads to the nearest review and marks resolved", () => {
    const d = base();
    d.reviews = [{ author: "c", state: "CHANGES_REQUESTED", body: "", submittedAt: "2026-01-03T00:00:00Z" }];
    d.reviewThreads = [
      { resolved: false, path: "a.rs", line: 5, comments: [{ author: "c", body: "fix", path: "a.rs", line: 5, createdAt: "2026-01-03T00:00:01Z" }] },
    ];
    const ev = buildPrTimeline(d);
    const review = ev.find((e) => e.kind === "review");
    expect(review && review.kind === "review" && review.threads.length).toBe(1);
  });
});
```

- [ ] **Step 9: Run — verify FAIL.** `cd git-it && npm test -- prTimeline` (module not found).

- [ ] **Step 10: Implement `src/lib/github/prTimeline.ts`**

```ts
import type { GhPullDetail, PrTimelineEvent, GhReviewThread } from "../types";

// Attach each inline review thread to the review submitted closest at/after its
// first comment time (GitHub groups inline comments under the review that created
// them). Falls back to the latest review. Threads with no review still surface as
// their own review-less event is avoided — they ride the nearest review.
export function buildPrTimeline(d: GhPullDetail): PrTimelineEvent[] {
  const events: PrTimelineEvent[] = [];

  for (const commit of d.commits) {
    events.push({ kind: "commit", at: commit.committedDate, commit });
  }
  for (const comment of d.comments) {
    events.push({ kind: "comment", at: comment.createdAt, comment });
  }
  for (const run of d.checkRuns) {
    events.push({ kind: "ciRun", at: run.startedAt, run });
  }

  // Group threads onto reviews by nearest-preceding/﻿earliest review time.
  const reviewsSorted = [...d.reviews].sort((a, b) => a.submittedAt.localeCompare(b.submittedAt));
  const threadsFor = new Map<number, GhReviewThread[]>();
  const threadTime = (t: GhReviewThread) => t.comments[0]?.createdAt ?? "";
  for (const t of d.reviewThreads) {
    const tt = threadTime(t);
    let idx = reviewsSorted.length - 1; // default: latest review
    for (let i = 0; i < reviewsSorted.length; i++) {
      if (reviewsSorted[i].submittedAt >= tt) { idx = i; break; }
    }
    if (idx < 0) idx = 0;
    const list = threadsFor.get(idx) ?? [];
    list.push(t);
    threadsFor.set(idx, list);
  }
  reviewsSorted.forEach((review, i) => {
    events.push({ kind: "review", at: review.submittedAt, review, threads: threadsFor.get(i) ?? [] });
  });

  return events.sort((a, b) => a.at.localeCompare(b.at));
}
```

- [ ] **Step 11: Run — verify PASS.** `cd git-it && npm test -- prTimeline`.

- [ ] **Step 12: Build `PrTimeline.svelte`** `src/lib/components/github/PrTimeline.svelte`

A vertical timeline (left rail + dots) rendering the events from `buildPrTimeline(pr)`. Read the existing
`GithubDetail.svelte` + `Markdown.svelte` + `github/format.ts` (for relative dates / glyphs) to reuse the
app's classes and helpers. Event rendering:
- `commit` → author + 7-char sha (mono) + headline.
- `comment` → author + `<Markdown src={body}/>`.
- `review` → author + decision label (state colour: APPROVED green / CHANGES_REQUESTED amber / COMMENTED
  neutral) + optional body; **nested threads** grouped by `path:line`. Each thread shows the file:line
  header + each comment body. A `resolved` thread renders **dimmed + collapsed to a one-line summary**
  (`{author} commented on {path}:{line} · resolved`) that expands on click (a `<details>` or a local
  `$state` toggle); **unresolved** threads render fully expanded with an "unresolved" badge.
- `ciRun` → workflow name + a conclusion glyph (success ✓ / failure ✗ / running …) + "started {rel} ·
  ran {duration}" where duration = updatedAt − startedAt (format mm:ss). Link the name to `run.url`.

Match the mockup in the spec. Use the design tokens already in the github components (accent, text-muted,
status colours). Sort is ascending (oldest → newest).

- [ ] **Step 13: Wire `GithubDetail.svelte`**

Read the current PR block (`{#if pr}` … `<details>` sections, ~lines 85-134). Replace the Checks +
Reviews `<details>` sections with:
1. A **pinned checks strip** above the timeline, summarising the latest `pr.checks` (the existing
   `statusCheckRollup`-derived array): count of pass/fail/pending with glyphs, expandable to the per-check
   list (a `<details>` is fine for the expand).
2. `<PrTimeline {pr} />` for the chronological body (commits + comments + reviews + inline threads + CI
   runs).
Keep the Files list as its own collapsible section below the timeline (unchanged). Import `PrTimeline`.

- [ ] **Step 14: Preview-verify** the PR detail layout. The browser preview can't run live `gh`, but you
can confirm `PrTimeline` renders against a mocked `GhPullDetail` (or verify no svelte-check/runtime errors
and that the component compiles). Capture a screenshot if a mock path is wired; otherwise rely on the
Vitest builder coverage + the user's `tauri dev` confirmation.

- [ ] **Step 15: Gates** — `cd git-it && npm run check && npm test && cargo test && cargo build -p git-it`.

- [ ] **Step 16: Commit frontend**

```bash
cd git-it && git add -A
git commit -m "feat(github): PR detail as a chronological timeline with inline review comments + CI runs

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## After all tasks
Dispatch a final adversarial review across the whole batch (focus: the `-uall` perf/ignore behaviour, the
FF safe-failure on divergence, the two-phase delete partial-failure messaging, loading-flag clear on every
error path, and the timeline ordering / thread-grouping / resolved handling + best-effort gh degradation).
Then run all gates once more and use superpowers:finishing-a-development-branch to ff-merge
`batch7-desktop-feedback` → `main`. The user runs `npm run tauri build` / `npm run tauri dev` to confirm
the Tauri-only paths (real `git status -uall`, FF op, remote delete, live `gh` PR detail, loading visuals).
