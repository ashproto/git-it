# Desktop Feedback Batch 11 — Design

**Date:** 2026-07-04
**Branch:** stacked on `desktop-batch9` (tip `a2e6fae`, which already carries batch 10); one combined live test → ff-merge to `main` later.
**Status:** approved design, pending spec review.

Two user-reported refinements from live use of Git It:

1. **Higher-fidelity tab drag-reorder** — see a floating image of the tab being dragged, and have the other tabs animate out of the way to preview where it will land (Chrome/Safari-style live reflow), refining the batch-9 pointer-drag in `RepoTabs.svelte`.
2. **Delete a remote branch** — there is currently no way to delete a remote branch from the sidebar remotes list.

---

## Item 1 — High-fidelity tab drag-reorder

### Current behavior (batch 9)
`RepoTabs.svelte` drags with pointer events (4px threshold, `setPointerCapture`). The dragged tab stays in place at `opacity: 0.5`; a 2px accent `.drop-indicator` line marks the insertion gap (`updateGap` → `indicatorLeft`); the other tabs do not move. On drop, `appState.reorderRepos(dragFrom, gapToIndex(dragFrom, dropGap))` commits.

### Target behavior
- A **floating clone** of the dragged tab follows the cursor (elevated, full opacity) — you can see what you're dragging.
- The remaining tabs **reflow** as you drag: when the cursor crosses a neighbor's midpoint, the tabs slide to open the slot where the drop will land (the opening gap replaces the 2px line as the drop preview).
- Drop commits the new order; Escape/cancel reverts to the original order with no change.

### Design (approach B — live reorder + Svelte `animate:flip` + floating clone)

The tab strip iterates a **display list** that is the live-reordered copy during a drag and the store order otherwise:
```
const displayRepos = $derived(dragOrder ?? appState.openRepos);
```
`{#each displayRepos as path (path)}` gets `animate:flip={{ duration: 160, easing: quintOut }}` (the `flip`/`quintOut` primitives are already used in `WorkingCopyView`/`FileTree`). Keying by `path` (unchanged) means a reorder of `dragOrder` triggers `flip` on the moved neighbors automatically — that IS the "move out of the way" animation.

**Drag state (module-level `$state`):**
- `dragOrder: string[] | null` — the working order during a drag (`null` when idle).
- `dragPath: string | null` — the path being dragged.
- `dragGhost: { x: number; y: number; label: string; width: number } | null` — the floating clone.
- `grabDX: number` — pointer offset within the tab at grab time, so the clone stays under the cursor.
- Existing `candidate` / `DRAG_THRESHOLD` / `didDrag` remain (press-vs-drag disambiguation, and swallowing the click that trails a drag).

**Flow:**
1. `onTabPointerDown(e, index)` — unchanged press-candidate recording (ignore the close button, left button only).
2. `onDragMove` — once travel ≥ threshold, enter drag: set `dragOrder = [...appState.openRepos]`, `dragPath = path`, capture the pointer, compute `grabDX = e.clientX - tabRect.left` and the tab `width`, and set `didDrag = true`. Then on every move, update the ghost position and recompute the target slot.
3. **Target slot** — measure the current in-flow `.tab` rects (excluding the floating clone). The insertion index is the count of tab midpoints left of the pointer (same midpoint logic as today's `updateGap`, adapted to produce an index in the display list). If it differs from `dragPath`'s current index in `dragOrder`, splice `dragPath` to the new index (`dragOrder` = reorder(dragOrder, cur, next)) — Svelte reorders + flips.
4. `onDragUp` — commit: `appState.reorderRepos(appState.openRepos.indexOf(dragPath), dragOrder.indexOf(dragPath))`, then clear drag state.
5. `onDragCancel` / Escape — clear drag state WITHOUT committing (`dragOrder = null` reverts the display to the store order); `didDrag` stays true so the trailing click doesn't select.

**Rendering:**
- The in-flow tab whose `path === dragPath` renders with `visibility: hidden` (keeps its box as the placeholder that flips around, but is not visible — the clone is what you see). All other tabs render normally and flip.
- The **floating clone** is a `position: fixed` element (`pointer-events: none`, `z-index` above the strip, subtle shadow + slight scale) at `left = ghost.x`, `top = ghost.y`, showing the tab label. `ghost.x = e.clientX - grabDX`; `ghost.y` = the strip's top (fixed vertically — the strip is horizontal).
- The old `.drop-indicator` element and `indicatorLeft`/`dropGap` are removed (the reflow gap is the indicator now).

**Interaction guards (preserve today's correctness):**
- Close button still cannot start a drag; the trailing click after a drag is still swallowed via `didDrag`.
- Global `pointermove`/`pointerup`/`pointercancel`/`keydown(Escape)` listeners are attached on drag start and removed in `endDrag` (as today).
- `reorderRepos` persistence (the `openRepos` Store write-through) is unchanged — commit still goes through it.

**Out of scope:** auto-scrolling the strip when dragging past its edge (nice-to-have; noted, not built).

### Files
- Modify: `src/lib/components/RepoTabs.svelte` (drag section rewrite: display list + flip + floating clone; remove `.drop-indicator`).
- Reuse: `src/lib/reorder.ts` (`reorder`; `gapToIndex` may no longer be needed once the target is expressed as a direct index — remove its import from RepoTabs if it becomes unused, but leave `reorder.ts` itself intact since `reorder` is used).
- Reuse: `appState.reorderRepos(from, to)` — unchanged.

### Testing
- Pure index math (target-slot-from-pointer, and the splice reorder) stays in / continues to use `src/lib/reorder.ts`, which is vitest-covered; add a test only if new pure logic is introduced (e.g. a `targetIndex(rects, pointerX)` helper) — extract that helper and unit-test it.
- The drag animation itself is component-level: verified in browser preview (drag a tab, confirm the clone follows and neighbors reflow; drop persists; Escape reverts). Tauri is not required (pointer DnD works in the browser).

---

## Item 2 — Delete a remote branch from the sidebar

### Current state
`Sidebar.svelte`'s remote-ref context menu (`onRefContext`, `kind === "remote"`) offers only "Checkout `<name>` (detached)" and "Create local branch…". There is no delete. `ops::delete_branch` already deletes an upstream branch via `git push --delete --end-of-options <remote> <branch>` (with `GIT_TERMINAL_PROMPT=0`) as part of deleting a local branch.

### Design

**Backend** (`crates/git-core/src/ops.rs`):
- Add `pub fn delete_remote_branch(repo: &Path, remote: &str, branch: &str) -> Result<(), String>` that runs the same guarded push-delete:
  ```rust
  pub fn delete_remote_branch(repo: &Path, remote: &str, branch: &str) -> Result<(), String> {
      let mut p = Command::new("git");
      p.current_dir(repo)
          .env("GIT_TERMINAL_PROMPT", "0")
          .args(["push", "--delete", "--end-of-options", remote, branch]);
      git_ops::run(&mut p)?;
      Ok(())
  }
  ```
- Refactor `delete_branch`'s remote-delete block to call `delete_remote_branch` (single source of truth; wrap its error with the existing "Deleted local branch, but remote delete failed: {e}" context). Behavior is byte-identical to today's "also delete remote", so auth works the same way.
- Note: `git push --delete` also removes the local `refs/remotes/<remote>/<branch>` tracking ref, so the sidebar's remote list refresh reflects the deletion without a separate prune.

**Tauri command** (`src-tauri/src/commands.rs`): `#[tauri::command(async)]` (network op) `delete_remote_branch(repo, remote, branch)` → `ops::delete_remote_branch`. Register in `lib.rs`.

**Frontend:**
- `src/lib/api.ts`: `deleteRemoteBranch(repo, remote, branch)` → `invoke("delete_remote_branch", { repo, remote, branch })`.
- `src/lib/gitActions.ts`: `deleteRemoteBranch(remote, branch)` wrapped in `run("Delete remote branch " + remote + "/" + branch, …)` (so it gets the busy spinner + refresh that `run` provides).
- `src/lib/components/Sidebar.svelte`: in the `kind === "remote"` menu, append a separator + a danger item "Delete remote branch". Its action confirms first, then calls the action:
  ```ts
  items.push({ separator: true });
  items.push({
    label: "Delete remote branch",
    danger: true,
    action: async () => {
      const slash = r.name.indexOf("/");
      const remote = r.name.slice(0, slash);
      const branch = r.name.slice(slash + 1);
      const ok = await dialogs.confirm({
        title: "Delete remote branch",
        message: `Delete "${r.name}" on the remote? This removes it for everyone with access to ${remote}.`,
        confirmLabel: "Delete",
      });
      if (ok) gitActions.deleteRemoteBranch(remote, branch);
    },
  });
  ```
  (`r.name` for a remote ref is `"<remote>/<branch>"`; remote names cannot contain `/`, so the first-slash split is correct.)

### Files
- Modify: `crates/git-core/src/ops.rs` (add `delete_remote_branch`; refactor `delete_branch`), plus a Rust test.
- Modify: `src-tauri/src/commands.rs` (command), `src-tauri/src/lib.rs` (register).
- Modify: `src/lib/api.ts`, `src/lib/gitActions.ts`, `src/lib/components/Sidebar.svelte`.

### Testing
- Rust test `delete_remote_branch_removes_upstream` in `ops.rs`: a bare "origin" with a pushed `feature`; a clone; `delete_remote_branch(&clone, "origin", "feature")` succeeds; assert `feature` no longer exists on the bare remote (`git --git-dir=<bare> show-ref refs/heads/feature` fails) and the clone's `refs/remotes/origin/feature` is gone. Leading-dash/empty guards are unnecessary here because `--end-of-options` already neutralizes operand injection (matching the existing `delete_branch` pattern), but the command still places `--end-of-options` before the operands.
- Frontend: the menu item + confirm is verified in the live test (Tauri-only network path). Browser preview confirms the menu renders the new item.

---

## Cross-cutting
- **Branch:** both items land on `desktop-batch9`. Execute sequentially (shared git index — no parallel tree mutations while an implementer is active). Item 1 touches `RepoTabs.svelte` only; item 2 touches `ops.rs`/`commands.rs`/`lib.rs`/`api.ts`/`gitActions.ts`/`Sidebar.svelte` — no overlap between the two items.
- **Async commands:** `delete_remote_branch` is a network `git push` → `#[tauri::command(async)]` (repo hard rule).
- **Shell-out safety:** the push-delete keeps `--end-of-options` before the remote + branch operands.
- **Gates:** `npm run check` (0 errors) + `npm test` + `cargo test`, plus browser preview for the drag reflow and the new menu item. Then the user's `tauri dev`/`build` live test. **Stop before merge.**

## Out of scope / non-goals
- No auto-scroll of the tab strip during a drag past its edge.
- No multi-tab drag selection.
- No change to `delete_branch`'s local-delete behavior or its "also delete remote" toggle (item 2 only adds a standalone path + reuses the shared push-delete helper).
- No new credential mechanism — remote-branch delete authenticates via the same path as the existing "also delete remote".
