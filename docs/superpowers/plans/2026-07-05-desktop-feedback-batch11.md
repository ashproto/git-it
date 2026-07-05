# Desktop Feedback Batch 11 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship two Git It refinements — deleting a remote branch from the sidebar, and a high-fidelity Chrome/Safari-style tab drag (floating clone + neighbors reflow).

**Architecture:** Item 2 extracts the existing `git push --delete` from `delete_branch` into a reusable `delete_remote_branch` command wired to a sidebar danger menu with a confirm. Item 1 rewrites the `RepoTabs.svelte` pointer-drag to live-reorder a local display list with Svelte `animate:flip` (neighbors slide aside) plus a `position: fixed` clone that follows the cursor; hit-testing reads `offsetLeft/offsetWidth` (which ignore the flip `transform`) so the reflow can't feed back into the target calc.

**Tech Stack:** Rust (git-core + Tauri shell), SvelteKit 5 runes, `svelte/animate` `flip`, vitest, cargo test.

**Branch:** `desktop-batch9` (stacked; already carries batch 10, tip `a2e6fae`). Execute tasks sequentially — the shared git index means no parallel tree mutations while an implementer is active.

**Full green gate before any task is "done":** `npm run check` (0 errors) + `npm test` + `cargo test`. Frontend-visible changes also get a browser-preview check.

---

## Task 1: Delete a remote branch from the sidebar

**Files:**
- Modify: `git-it/crates/git-core/src/ops.rs` (add `delete_remote_branch`; refactor `delete_branch` to reuse it; add a test)
- Modify: `git-it/src-tauri/src/commands.rs` (async command)
- Modify: `git-it/src-tauri/src/lib.rs` (register)
- Modify: `git-it/src/lib/api.ts` (binding)
- Modify: `git-it/src/lib/gitActions.ts` (action)
- Modify: `git-it/src/lib/components/Sidebar.svelte` (remote menu item + confirm)

- [ ] **Step 1: Write the failing Rust test**

Add to the `tests` module in `crates/git-core/src/ops.rs` (it already has `TempRepo`, `unique_dir`, `fetch`). This pushes `feature` to a bare "origin", clones, deletes the remote branch, and asserts it's gone on both the bare repo and the clone's remote-tracking refs.

```rust
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

        // origin/feature exists in the clone before deletion.
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

        // Gone on the bare remote…
        assert!(
            !Command::new("git")
                .args(["--git-dir", bare.to_str().unwrap(), "show-ref", "--verify", "--quiet", "refs/heads/feature"])
                .status()
                .unwrap()
                .success(),
            "feature should be deleted on the remote"
        );
        // …and the clone's remote-tracking ref is pruned by push --delete.
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p git-core delete_remote_branch`
Expected: FAIL to compile — `delete_remote_branch` is not defined.

- [ ] **Step 3: Add `delete_remote_branch` and refactor `delete_branch` to reuse it**

In `crates/git-core/src/ops.rs`, add this function immediately after `delete_branch`:

```rust
/// Delete a branch on a remote via `git push <remote> --delete <branch>`. This also
/// removes the local `refs/remotes/<remote>/<branch>` tracking ref, so callers don't
/// need a separate prune. Authenticates via the same path as any other push
/// (GIT_TERMINAL_PROMPT=0 so a missing credential fails instead of hanging).
pub fn delete_remote_branch(repo: &Path, remote: &str, branch: &str) -> Result<(), String> {
    let mut p = Command::new("git");
    p.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        // Options first, then --end-of-options, so BOTH the remote name and the
        // branch operand are guarded against leading-dash flag injection.
        .args(["push", "--delete", "--end-of-options", remote, branch]);
    git_ops::run(&mut p)?;
    Ok(())
}
```

Then refactor the remote-delete block inside `delete_branch` (the `if delete_remote { … }` body) to call it:

```rust
    if delete_remote {
        if let (Some(rem), Some(rb)) = (remote, remote_branch) {
            delete_remote_branch(repo, rem, rb)
                .map_err(|e| format!("Deleted local branch, but remote delete failed: {}", e))?;
        }
    }
    Ok(())
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p git-core delete_remote_branch && cargo test -p git-core delete_branch`
Expected: PASS (`delete_remote_branch_removes_upstream` plus the existing `delete_branch_can_also_delete_the_remote` still green).

- [ ] **Step 5: Add the Tauri command**

In `src-tauri/src/commands.rs`, add after the `fast_forward_branch` command:

```rust
// async: `git push --delete` is a network op (same gotcha as fetch/push).
#[tauri::command(async)]
pub fn delete_remote_branch(repo: String, remote: String, branch: String) -> Result<String, String> {
    ops::delete_remote_branch(&PathBuf::from(repo), &remote, &branch).map(|()| String::new())
}
```

- [ ] **Step 6: Register the command**

In `src-tauri/src/lib.rs`, add to the `tauri::generate_handler![…]` list, right after `commands::fast_forward_branch,`:

```rust
            commands::delete_remote_branch,
```

- [ ] **Step 7: Add the TS binding**

In `src/lib/api.ts`, add after the `appsForFile` binding:

```ts
  deleteRemoteBranch: (repo: string, remote: string, branch: string) =>
    invoke<string>("delete_remote_branch", { repo, remote, branch }),
```

- [ ] **Step 8: Add the gitActions wrapper**

In `src/lib/gitActions.ts`, add to the returned actions object next to `deleteBranch`:

```ts
  deleteRemoteBranch: (remote: string, branch: string) =>
    run(`Delete remote branch ${remote}/${branch}`, () =>
      api.deleteRemoteBranch(appState.repo, remote, branch),
    ),
```

- [ ] **Step 9: Add the sidebar menu item + confirm**

In `src/lib/components/Sidebar.svelte`, in `onRefContext` under the `else if (kind === "remote")` branch, after the existing "Create local branch…" item, append:

```svelte
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

(`r.name` for a remote ref is `"<remote>/<branch>"`; remote names cannot contain `/`, so the first-slash split is correct. `dialogs.confirm` resolves `true`/`false`; `dialogs` and `gitActions` are already imported in Sidebar.)

- [ ] **Step 10: Run the full gate**

Run: `npm run check && npm test && cargo test -p git-core`
Expected: 0 check errors; vitest all pass; cargo git-core tests pass.

- [ ] **Step 11: Commit**

```bash
git add crates/git-core/src/ops.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/api.ts src/lib/gitActions.ts src/lib/components/Sidebar.svelte
git commit -m "feat(remotes): delete a remote branch from the sidebar (push --delete)"
```

---

## Task 2: High-fidelity tab drag-reorder

**Why:** The batch-9 drag dims the tab in place with a static drop-line and doesn't move the neighbors. Replace it with a floating clone that follows the cursor and neighbors that reflow via Svelte `flip`.

**Files:**
- Modify: `git-it/src/lib/reorder.ts` (add pure `insertionIndex`)
- Modify: `git-it/src/lib/reorder.test.ts` (test it)
- Modify: `git-it/src/lib/components/RepoTabs.svelte` (drag rewrite)

- [ ] **Step 1: Write the failing test for `insertionIndex`**

Add to `src/lib/reorder.test.ts`:

```ts
import { insertionIndex } from "./reorder";

describe("insertionIndex", () => {
  // Three tabs with midpoints at x = 20, 60, 100.
  const mids = [20, 60, 100];
  it("returns 0 left of the first midpoint", () => {
    expect(insertionIndex(mids, 5)).toBe(0);
  });
  it("returns the index of the first midpoint to the right of the pointer", () => {
    expect(insertionIndex(mids, 40)).toBe(1); // between mid0 and mid1
    expect(insertionIndex(mids, 80)).toBe(2); // between mid1 and mid2
  });
  it("clamps to the last index past the final midpoint", () => {
    expect(insertionIndex(mids, 999)).toBe(2);
  });
  it("returns 0 for an empty list", () => {
    expect(insertionIndex([], 10)).toBe(0);
  });
});
```

(If `reorder.test.ts` doesn't already `import { describe, it, expect } from "vitest"` at the top, that import is present — it tests `reorder`/`gapToIndex` today; reuse it.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/lib/reorder.test.ts`
Expected: FAIL — `insertionIndex` is not exported.

- [ ] **Step 3: Implement `insertionIndex`**

Add to `src/lib/reorder.ts`:

```ts
/**
 * Insertion index for a pointer at `pointerX` given ascending tab `midpoints`
 * (x of each tab's centre). Returns the first index whose midpoint is right of
 * the pointer, clamped to the last index when the pointer is past every tab.
 */
export function insertionIndex(midpoints: number[], pointerX: number): number {
  for (let i = 0; i < midpoints.length; i++) {
    if (pointerX < midpoints[i]) return i;
  }
  return Math.max(0, midpoints.length - 1);
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/lib/reorder.test.ts`
Expected: PASS (all `insertionIndex` cases plus the existing `reorder`/`gapToIndex` cases).

- [ ] **Step 5: Rewrite the drag section of `RepoTabs.svelte`**

Replace the imports line `import { gapToIndex } from "../reorder";` (line 4) with:

```ts
  import { reorder, insertionIndex } from "../reorder";
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
```

Replace the entire drag block — from the `// ── Drag-to-reorder tabs …` comment (line 76) through the `onTabClick` function (ending line 189) — with:

```ts
  // ── Drag-to-reorder tabs (pointer events; live reorder + flip + floating clone) ──
  const DRAG_THRESHOLD = 4; // px of horizontal travel before a press becomes a drag
  let stripEl = $state<HTMLDivElement>();
  // Live-reordered copy of openRepos during a drag; null when idle. The strip renders
  // from `displayRepos` so a splice here animates the neighbors via `flip`.
  let dragOrder = $state<string[] | null>(null);
  let dragPath = $state<string | null>(null);
  // The floating clone that follows the cursor (viewport coords; position: fixed).
  let ghost = $state<{ x: number; y: number; label: string; width: number } | null>(null);
  let grabDX = 0; // pointer offset within the grabbed tab, so the clone stays under the cursor
  // Pre-threshold press candidate (non-reactive — nothing renders until drag mode).
  let candidate: { path: string; startX: number; pointerId: number; el: HTMLElement } | null = null;
  // Set when a drag actually happened, so the click that follows pointerup on the
  // same tab doesn't also activate it. Reset on the next pointerdown.
  let didDrag = false;

  const displayRepos = $derived(dragOrder ?? appState.openRepos);

  function onTabPointerDown(e: PointerEvent, path: string) {
    if (e.button !== 0) return;
    // The close button must not start a drag (its click closes the tab).
    if ((e.target as HTMLElement).closest(".tab-close")) return;
    didDrag = false;
    candidate = {
      path,
      startX: e.clientX,
      pointerId: e.pointerId,
      el: e.currentTarget as HTMLElement,
    };
    window.addEventListener("pointermove", onDragMove);
    window.addEventListener("pointerup", onDragUp);
    window.addEventListener("pointercancel", onDragCancel);
    window.addEventListener("keydown", onDragKeydown, true);
  }

  function onDragMove(e: PointerEvent) {
    if (!candidate) return;
    if (dragOrder === null) {
      if (Math.abs(e.clientX - candidate.startX) < DRAG_THRESHOLD) return;
      // Enter drag mode.
      const rect = candidate.el.getBoundingClientRect();
      grabDX = candidate.startX - rect.left;
      dragOrder = [...appState.openRepos];
      dragPath = candidate.path;
      ghost = { x: e.clientX - grabDX, y: rect.top, label: basename(candidate.path), width: rect.width };
      didDrag = true;
      try {
        candidate.el.setPointerCapture(candidate.pointerId);
      } catch {
        /* capture is best-effort (pointer may already be gone) */
      }
    }
    if (ghost) ghost = { ...ghost, x: e.clientX - grabDX };
    updateTarget(e.clientX);
  }

  // Move the dragged path to the slot the pointer is over. Hit-testing uses
  // offsetLeft/offsetWidth (the RESTING layout box), NOT getBoundingClientRect —
  // `flip` animates via `transform`, which offsetLeft ignores, so an in-flight
  // reflow can't jitter the target back and forth.
  function updateTarget(pointerX: number) {
    if (!stripEl || dragOrder === null || dragPath === null) return;
    const tabs = Array.from(stripEl.querySelectorAll<HTMLElement>(".tab"));
    if (tabs.length === 0) return;
    const stripRect = stripEl.getBoundingClientRect();
    const contentX = pointerX - stripRect.left + stripEl.scrollLeft;
    const mids = tabs.map((el) => el.offsetLeft + el.offsetWidth / 2);
    const target = insertionIndex(mids, contentX);
    const cur = dragOrder.indexOf(dragPath);
    if (target !== cur) dragOrder = reorder(dragOrder, cur, target);
  }

  function onDragUp() {
    if (dragOrder !== null && dragPath !== null) {
      const from = appState.openRepos.indexOf(dragPath);
      const to = dragOrder.indexOf(dragPath);
      if (from !== -1 && from !== to) appState.reorderRepos(from, to);
    }
    endDrag();
  }

  function onDragCancel() {
    endDrag(); // abort — dragOrder = null reverts the display to the store order
  }

  function onDragKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && dragOrder !== null) {
      e.stopPropagation();
      endDrag(); // abort without committing; didDrag stays true so the trailing click is swallowed
    }
  }

  function endDrag() {
    if (candidate) {
      try {
        candidate.el.releasePointerCapture(candidate.pointerId);
      } catch {
        /* already released */
      }
    }
    candidate = null;
    dragOrder = null;
    dragPath = null;
    ghost = null;
    window.removeEventListener("pointermove", onDragMove);
    window.removeEventListener("pointerup", onDragUp);
    window.removeEventListener("pointercancel", onDragCancel);
    window.removeEventListener("keydown", onDragKeydown, true);
  }

  function onTabClick(path: string) {
    if (didDrag) {
      didDrag = false; // this click is the tail end of a drag, not a select
      return;
    }
    appState.setActiveRepo(path);
  }
```

- [ ] **Step 6: Update the tab-strip markup**

Replace the `{#each appState.openRepos …}` block plus the `{#if dragFrom …}` drop-indicator (RepoTabs.svelte lines 222–247) with a `displayRepos` each that adds `animate:flip`, a `.placeholder` class for the dragged tab, and a floating clone after the loop:

```svelte
  {#each displayRepos as path (path)}
    <button
      class="tab"
      class:active={path === appState.repo}
      class:placeholder={path === dragPath}
      title={path}
      animate:flip={{ duration: 160, easing: quintOut }}
      onpointerdown={(e) => onTabPointerDown(e, path)}
      onclick={() => onTabClick(path)}
    >
      <span class="tab-name">{basename(path)}</span>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="tab-close"
        title="Close"
        onclick={(e) => { e.stopPropagation(); appState.closeRepo(path); }}
        role="button"
        tabindex="-1"
        aria-label="Close {basename(path)}"
      >×</span>
    </button>
  {/each}

  {#if ghost}
    <div class="tab-ghost" style={`left:${ghost.x}px; top:${ghost.y}px; width:${ghost.width}px`}>
      <span class="tab-name">{ghost.label}</span>
    </div>
  {/if}
```

- [ ] **Step 7: Swap the drag CSS**

In the `<style>` block, replace the `.tab.dragging` rule and the `.drop-indicator` rule with the placeholder + ghost styles:

```css
  /* The dragged tab keeps its slot as an invisible placeholder that `flip` slides
     around; the floating clone is what the user sees. */
  .tab.placeholder {
    visibility: hidden;
  }

  .tab-ghost {
    position: fixed;
    z-index: 3500;
    display: inline-flex;
    align-items: center;
    height: 34px;
    padding: 0 10px;
    box-sizing: border-box;
    background: var(--header-bg);
    color: var(--text);
    border-radius: 6px;
    border: 1px solid var(--border);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.28);
    font-size: 12.5px;
    white-space: nowrap;
    pointer-events: none;
    transform: scale(1.02);
  }
```

- [ ] **Step 8: Run the full gate**

Run: `npm run check && npm test`
Expected: 0 check errors; all vitest pass (incl. the new `insertionIndex` tests).

- [ ] **Step 9: Browser-preview the drag**

Start the dev server. With 3+ repos open, drag a tab: the floating clone follows the cursor, the other tabs slide aside to open the drop slot, dropping persists the new order, and Escape mid-drag reverts. Confirm a normal click still switches tabs and the × still closes. (`preview_screenshot` for proof.) Tauri is not required — pointer DnD works in the browser.

- [ ] **Step 10: Commit**

```bash
git add src/lib/reorder.ts src/lib/reorder.test.ts src/lib/components/RepoTabs.svelte
git commit -m "feat(tabs): high-fidelity drag — floating clone + neighbors reflow (flip)"
```

---

## Final verification (whole batch)

- [ ] Run the full gate on the tip: `npm run check` (0 errors) + `npm test` + `cargo test`.
- [ ] `npm run build` (production frontend) succeeds.
- [ ] Browser preview: tab drag reflow works; sidebar remote menu shows "Delete remote branch".
- [ ] Dispatch a final whole-batch adversarial code review; fix Critical/Important before stopping.
- [ ] **STOP before merge.** Hand off for the user's `tauri dev`/`build` live test:
  - Right-click a remote branch in the sidebar → Delete remote branch → confirm → it's removed and the remotes list updates.
  - Drag a repo tab: clone follows the cursor, neighbors reflow, drop persists across restart, Escape reverts.
- [ ] After user approval: ff-merge `desktop-batch9` → `main`, push (gh credential-helper pattern), delete the branch.

## Notes / non-goals
- No auto-scroll of the tab strip while dragging past its edge.
- No multi-tab drag; no change to `delete_branch`'s local-delete or "also delete remote" toggle.
- Remote-branch delete authenticates via the same path as the existing "also delete remote".
