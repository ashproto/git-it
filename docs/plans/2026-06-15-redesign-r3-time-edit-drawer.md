# Redesign Slice R3 — Time-Edit On-Demand Drawer

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Time-editing is now one feature among many, so it should not occupy the main column at all times. Move the Offset/Exact/Compress editor + the Rewrite-history apply panel into an **on-demand slide-in drawer** opened from a commit selection, and remove the always-on "New date" column from the commits table. The freed vertical space lets the graph + diff/detail fill the shell.

**Architecture:** A new singleton controller `timeEditDrawer.svelte.ts` (mirrors `amendDialog.svelte.ts`) gates a new `TimeEditDrawer.svelte` overlay. The drawer **reuses the existing `EditTabs` + `ApplyPanel` components unchanged** — they already operate on `appState.selected` / `appState.newDates`, so only their mount point moves (out of `.main-col`, into the drawer). Entry points: an "Edit timestamps…" button in the Commits-panel header (enabled when something is selected) and a context-menu item. The "New date" column is removed; edited rows keep a compact inline accent indicator so pending edits stay visible.

**Tech Stack:** SvelteKit 5 runes; reuse the `*.svelte.ts` controller pattern + the `AmendDialog` overlay/focus/Escape pattern.

**Branch:** `redesign/r3-time-edit-drawer` (created). Base `main`.

**Scope (R3 only):** the drawer controller + component · re-host EditTabs/ApplyPanel in the drawer · remove the always-on `.two-col` · entry points (header button + context-menu item) · remove the "New date" column + edited-row indicator · raise the graph height cap. **NOT in R3:** the rename (R4). Do not change `EditTabs.svelte` / `ApplyPanel.svelte` logic — only re-host them.

## File structure
- Create `src/lib/timeEditDrawer.svelte.ts` — singleton open/close controller.
- Create `src/lib/components/TimeEditDrawer.svelte` — slide-in overlay hosting `<EditTabs/>` + `<ApplyPanel/>`.
- Modify `src/routes/+page.svelte` — drop the always-on `.two-col` (+ EditTabs/ApplyPanel imports), mount `<TimeEditDrawer/>`.
- Modify `src/lib/components/GraphHistory.svelte` — header "Edit timestamps…" button + context-menu item; remove the "New date" column; edited-row indicator; raise `.wrap` height.

---

## Task 1 — Drawer controller + TimeEditDrawer component

**Files:** create `src/lib/timeEditDrawer.svelte.ts`, `src/lib/components/TimeEditDrawer.svelte`.

- [ ] **Step 1: Controller.** Create `src/lib/timeEditDrawer.svelte.ts` (mirror `amendDialog.svelte.ts` — no seed data needed, it reads `appState.selected` live):
```ts
// Tiny store for the TimeEditDrawer: just open/close state. The drawer reads the
// live selection (appState.selected) — there is no per-open seed to capture.
function makeTimeEditDrawer() {
  let open = $state(false);

  return {
    get open() {
      return open;
    },
    openDrawer() {
      open = true;
    },
    close() {
      open = false;
    },
  };
}

export const timeEditDrawer = makeTimeEditDrawer();
```

- [ ] **Step 2: Component.** Create `src/lib/components/TimeEditDrawer.svelte`. A right-anchored slide-in panel hosting the existing editor + apply components. Mirror `AmendDialog.svelte`'s overlay / Escape / overlay-click / focus pattern, but anchor right and full-height. Reuse `EditTabs` + `ApplyPanel` verbatim (do NOT reimplement their logic):
```svelte
<script lang="ts">
  import { timeEditDrawer } from "../timeEditDrawer.svelte";
  import { appState } from "../store.svelte";
  import EditTabs from "./EditTabs.svelte";
  import ApplyPanel from "./ApplyPanel.svelte";

  let closeBtn = $state<HTMLButtonElement | undefined>();

  // Focus the close button when the drawer opens (a predictable, always-present
  // focus target; the editor fields live in a reused child component).
  $effect(() => {
    if (timeEditDrawer.open) {
      Promise.resolve().then(() => closeBtn?.focus());
    }
  });

  const selectedCount = $derived(appState.selected.size);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      timeEditDrawer.close();
    }
  }
</script>

{#if timeEditDrawer.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) timeEditDrawer.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="drawer panel" role="dialog" aria-modal="true" aria-label="Edit timestamps">
      <header class="drawer-head">
        <div class="titles">
          <h3>Edit timestamps</h3>
          <span class="sel-count">
            {selectedCount}
            {selectedCount === 1 ? "commit" : "commits"} selected
          </span>
        </div>
        <button
          class="close"
          bind:this={closeBtn}
          onclick={() => timeEditDrawer.close()}
          aria-label="Close timestamp editor"
          title="Close"
        >✕</button>
      </header>

      {#if selectedCount === 0}
        <p class="empty-hint">
          Select one or more commits in the list, then choose an edit mode below.
        </p>
      {/if}

      <div class="body">
        <EditTabs />
        <ApplyPanel />
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 3000;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    justify-content: flex-end;
  }
  .drawer {
    width: 560px;
    max-width: calc(100vw - 32px);
    height: 100%;
    background: var(--popover-bg, var(--panel-bg));
    border-left: 1px solid var(--border);
    box-shadow: -12px 0 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    box-sizing: border-box;
    overflow-y: auto;
    animation: slide-in 0.16s ease-out;
  }
  @keyframes slide-in {
    from { transform: translateX(16px); opacity: 0.4; }
    to   { transform: translateX(0);    opacity: 1; }
  }
  .drawer-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .sel-count {
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .close {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
    line-height: 1;
  }
  .close:hover {
    background: var(--btn-hover);
  }
  .empty-hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
```
> Note: the `.drawer` carries the `panel` class so glass mode applies its `backdrop-filter` (same as `.panel` elsewhere). It also sets `backdrop-filter` directly (like `AmendDialog`) so the blur is present even outside glass mode's `.panel` rule.

- [ ] **Step 3: Verify.** `cd git-it && npm run check` (0/0). Commit: `feat(redesign): time-edit drawer controller + component (R3)` (+ trailer).

---

## Task 2 — Integrate drawer, remove always-on panels + "New date" column

**Files:** modify `src/routes/+page.svelte`, `src/lib/components/GraphHistory.svelte`.

### `+page.svelte`

- [ ] **Step 1: Drop the always-on editor block.** Remove the `.two-col` block from `.main-col`:
```svelte
            <div class="two-col">
              <EditTabs />
              <ApplyPanel />
            </div>
```
So the `.main-col` becomes:
```svelte
          <div class="main-col">
            <UndoBar />
            <GraphHistory />
            <ConflictView />
            {#if appState.workingCopySelected}
              <WorkingCopyView />
            {:else}
              <CommitDetail />
            {/if}
            <LogPanel />
          </div>
```

- [ ] **Step 2: Swap imports.** Remove the now-unused direct imports:
```ts
  import EditTabs from "$lib/components/EditTabs.svelte";
  import ApplyPanel from "$lib/components/ApplyPanel.svelte";
```
and add:
```ts
  import TimeEditDrawer from "$lib/components/TimeEditDrawer.svelte";
```
(EditTabs/ApplyPanel are still used — but now imported by `TimeEditDrawer.svelte`, not here.)

- [ ] **Step 3: Mount the drawer.** Add `<TimeEditDrawer />` alongside the other overlays at the end of `<main>`, just before `<StatusBar />`:
```svelte
  <ContextMenu />
  <Modal />
  <AmendDialog />
  <RebaseTodo />
  <TimeEditDrawer />
  <StatusBar />
```

- [ ] **Step 4: Remove the now-dead `.two-col` CSS** (the grid rule and its `@media` override). Delete:
```css
  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
```
and inside the `@media (max-width: 900px)` block delete:
```css
    .two-col {
      grid-template-columns: 1fr;
    }
```

### `GraphHistory.svelte`

- [ ] **Step 5: Import the controller.** Add to the imports:
```ts
  import { timeEditDrawer } from "../timeEditDrawer.svelte";
```

- [ ] **Step 6: Header entry button.** In the `{#snippet headerActions()}`, after the `Select all` / `Clear` buttons, add an "Edit timestamps…" button enabled only when there's a selection:
```svelte
    <button type="button" onclick={selectAll}>Select all</button>
    <button type="button" onclick={clearSel}>Clear</button>
    <button
      type="button"
      class="edit-ts"
      disabled={appState.selected.size === 0}
      title={appState.selected.size === 0 ? "Select one or more commits first" : "Edit timestamps for the selected commits"}
      onclick={() => timeEditDrawer.openDrawer()}
    >Edit timestamps…</button>
```
Add a style so it reads as the primary action in the header:
```css
  button.edit-ts:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  button.edit-ts:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
```

- [ ] **Step 7: Context-menu item.** In `onRowContext`, add an "Edit timestamps…" entry. Right-click already sets `appState.selected = new Set([sha])` at the top of the handler, so opening the drawer here edits at least that commit (and any wider selection the user keeps via the header button instead). Insert after the `Copy SHA` item's separator group — add a new separator + item at the end of the items array, before the final `Copy SHA`, or right after it. Place it as its own group just above `Copy SHA`:
```ts
      { separator: true },
      {
        label: "Edit timestamps…",
        action: () => timeEditDrawer.openDrawer(),
      },
      { separator: true },
      { label: "Copy SHA", action: () => navigator.clipboard?.writeText(sha) },
```

- [ ] **Step 8: Remove the "New date" column — header cell.** In the `.head-row`, delete:
```svelte
      <span class="h newdate">New date</span>
```

- [ ] **Step 9: Remove the "New date" cell — working-copy row.** In the `wc-row`, delete:
```svelte
          <div class="newdate mono"></div>
```

- [ ] **Step 10: Replace the commit-row "New date" cell with an inline edited indicator.** In the `{#each commits …}` row, delete the trailing cell:
```svelte
          <div class="newdate mono">{newDateLabel(commit.sha)}</div>
```
and instead, inside the `.subject` div, after the `<span class="msg">…</span>`, append a compact pill that only shows for edited rows:
```svelte
          <div class="subject">
            {#each commit.refs as r}
              <span class="badge {r.kind}" class:current={r.is_head}>{r.name}</span>
            {/each}
            <span class="msg">{commit.subject}</span>
            {#if appState.newDates.has(commit.sha)}
              <span class="new-pill mono" title={`New date: ${newDateLabel(commit.sha)}`}
                >→ {newDateLabel(commit.sha)}</span>
            {/if}
          </div>
```
(Keep the `newDateLabel` function and the `class:edited` on the row — both are still used.)

- [ ] **Step 11: Swap the `.newdate` CSS for `.new-pill`.** Delete the `.newdate` and `.row.edited .newdate` rules:
```css
  .newdate {
    flex: 0 0 168px;
    color: var(--accent);
    padding-right: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row.edited .newdate {
    font-weight: 600;
  }
```
and add a `.new-pill` rule (compact accent chip that doesn't fight the subject for space):
```css
  .new-pill {
    flex: 0 0 auto;
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--accent);
    color: var(--accent);
    font-size: 11px;
    line-height: 1.6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }
```
> Note: the `.subject` is `overflow: hidden` with `min-width: 160px`; the `.msg` ellipsizes, so the pill stays visible at the right of the subject cell on a long message. Acceptable — the drawer is the authoritative full list of queued edits.

- [ ] **Step 12: Raise the graph height cap.** The R2 review noted the 440px inner cap (dual scrollbars + a cramped graph). With the editor panels gone, give the graph more room while keeping an inner scroll (required so `onWrapScroll` → `loadMoreGraph` still fires). Change `.wrap`:
```css
  .wrap {
    border-radius: 6px;
    border: 1px solid var(--border);
    overflow: auto;
    max-height: clamp(360px, 58vh, 900px);
    user-select: none;
    -webkit-user-select: none;
  }
```

- [ ] **Step 13: Verify.** `cd git-it && npm run check` (0/0) + `npm test`. Commit: `feat(redesign): on-demand time-edit drawer; drop New date column (R3)`.

---

## Task 3 — Verification + preview + review + merge

- [ ] **Step 1: Gates.** `cd git-it` then: `cargo check` (no Rust change — confirm clean) + `npm run check` (0/0) + `npm test` (green) + `npm run build` (succeeds).
- [ ] **Step 2: Preview.** Start preview (sample graph loads in browser). Verify:
  - The Commits header has an "Edit timestamps…" button, **disabled** with no selection; selecting a row enables it.
  - Clicking it opens a right-side drawer titled "Edit timestamps" with the Offset/Exact/Compress editor + the Rewrite-history apply panel; the "N commits selected" count is live.
  - Right-click a row → context menu has "Edit timestamps…" → opens the drawer.
  - In the drawer, Offset/Exact "Preview" stages a new date; the corresponding row shows the `→ <date>` accent pill (and the `.edited` styling); the drawer's apply-panel count increments.
  - Escape, the ✕ button, and clicking the dim overlay all close the drawer.
  - The "New date" column is gone from the table; the graph is taller (fills more of the shell) and still infinite-scrolls (scroll near the bottom appends / shows the end hint).
  - Screenshot light + dark. Console clean.
- [ ] **Step 3: Adversarial review** (Agent `superpowers:code-reviewer`, opus) over the branch diff. Focus: the drawer reuses EditTabs/ApplyPanel without behavior change; the controller open/close has no leak; Escape/overlay/✕ all close; focus moves into the drawer on open and the page is inert behind the modal overlay; removing the column didn't break header/row alignment or the gutter/`onscroll` paging; `class:edited` + `.new-pill` render only for edited rows; the raised `.wrap` cap preserves the inner scroll that drives `loadMoreGraph`; no orphaned `.two-col`/`.newdate` CSS or dangling imports; no backend/security surface touched. Fix findings.
- [ ] **Step 4: Merge.** `superpowers:finishing-a-development-branch` → ff-merge to `main`, delete branch. Update memory (`git-client-redesign.md`: R3 done; NEXT = R4 rename) + `docs/RESUME.md`. Then checkpoint with the user about R4 (the rename needs a user decision on the new name).

## Patterns to REUSE
- `amendDialog.svelte.ts` controller shape (open/close singleton) for `timeEditDrawer.svelte.ts`.
- `AmendDialog.svelte` overlay: fixed inset overlay, `onpointerdown` self-target close, `onkeydown` Escape, deferred focus via `Promise.resolve().then(...)`, `role="dialog" aria-modal="true"`.
- `EditTabs.svelte` + `ApplyPanel.svelte` reused **unchanged** (they already drive off `appState.selected`/`newDates`).
- The `.panel` + `backdrop-filter` glass convention; the `--popover-bg` opaque token for legible small surfaces.

## Deferred (noted)
- DOM virtualization for very large histories (still infinite-append only) — perf follow-on.
- R4: rename + open-source README (needs the user to pick a distinct name).
