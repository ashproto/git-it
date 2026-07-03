# Batch 9 — Desktop Feedback (search, tab reorder, fetch async, detached-HEAD, PR commits, visual tweaks)

**Date:** 2026-07-03
**Branch:** `desktop-batch9` (off `main` @ `a654509`, which includes the merged batch-8 PR workflow)
**Status:** Approved design (user picked curve option C — "the most curvy")

## Items

### 1. Fetch freeze fix + visible busy indicator (bug)

**Root cause:** `fetch`, `fast_forward_branch`, and `delete_branch` in `src-tauri/src/commands.rs`
are synchronous `#[tauri::command]`s; `fetch`/`fast_forward_branch` do network work and
`delete_branch` pushes when delete-remote is enabled — all block the UI thread (same class as
the historic GitHub-screen freeze).

**Fix:**
- Mark those three commands `#[tauri::command(async)]`. No signature changes.
- New `busyOp: string | null` signal in `store.svelte.ts`, set/cleared by `gitActions.run(label, …)`
  (try/finally, exactly like `navBusy`) carrying the human label ("Fetching origin…",
  "Deleting branch…").
- Toolbar buttons that trigger git ops (Fetch, Pull, Push already stream — leave their existing
  progress UI alone; Fetch is the one that needs it) render a spinner + label swap
  ("Fetch" → "⟳ Fetching…") and disable while `busyOp` matches their op. Implementation detail:
  the button reads a scoped derived (e.g. `busyOp?.startsWith("Fetch")`), not a global disable.
- Context-menu-triggered ops (fast-forward, branch delete) keep the top sweep + status-bar
  spinner and benefit from `busyOp` via the status bar showing the label text instead of a bare
  spinner.

### 2. Search — one field per screen (⌘F)

**Graph screen:**
- ⌘F (and a small 🔍 toolbar affordance) opens a compact search bar pinned above the commit list;
  Esc closes it and clears the filter.
- Pure matcher `src/lib/graph/commitSearch.ts`: `matchCommits(commits, query) → number[]`
  (indices) — case-insensitive substring on subject + author name, plus SHA **prefix** match
  (query looks hex-ish ⇒ compare against sha start). Debounced ~150 ms in the component.
- Matches are highlighted rows; "N of M" count; Enter / ↑↓ (and next/prev buttons) move the
  active match using the existing `graphView` index-jump controller. No filtering-out of
  non-matches in v1 (jump-and-highlight model — preserves lane rendering).
- Searches only the **loaded** window of commits; when matches < 1 and more pages exist, show
  "Load more to search older history" button that pages in (reuses infinite-scroll loading).

**GitHub screen:** PRs, Issues, and Releases tabs each get a small filter input above the list
(⌘F focuses the visible one). Client-side filter of the cached panel data:
PR/issue — title, author, `#number`; releases — name + tag. Pure helper
`src/lib/github/listFilter.ts` with tests. Empty query ⇒ full list. No new gh calls.

### 3. Repo-tab drag reorder

- `RepoTabs.svelte`: pointer-based drag to reorder (drag threshold, drop-indicator line between
  tabs, no ghost-image jank; consistent with the app's motion feel).
- New pure `reorder(list, from, to)` helper + `appState.reorderRepos(from, to)` mutating the
  repos array; order persists through the existing repo-list persistence (verify where the repo
  list is saved and ensure order round-trips).
- Sidebar repo-list mode renders the same array so it reorders too (drag support in the sidebar
  list itself is NOT required this batch — tabs only; the order just stays consistent).

### 4. Delete-remote toggle only when the remote branch exists

`Sidebar.confirmDeleteBranch` derives `upstream` from tracking config, which survives remote
deletion ("gone"). New rule:
- Toggle shown only when the upstream ref **exists** in `refsByKind.remote`
  (i.e. `remote.some(r => r.name === upstream)`).
- When tracking exists but the remote ref is gone: no toggle; small muted note in the dialog —
  "Remote branch already deleted." (`branchDelete` dialog state gains `remoteGone: boolean`).
- When there is no upstream at all: unchanged (no toggle, no note).

### 5. Detached-HEAD presentation

Not a bug (HEAD pointing at a commit, not a branch — can't be in the branch lists), but present
it better:
- Move the entry INTO the Local panel as a pinned FIRST row, visually distinct (warning-tinted
  icon + `HEAD detached @ <short-sha>` label), replacing the current standalone button between
  panels.
- Tooltip: "HEAD points directly at a commit instead of a branch. New commits here are lost
  when you switch away unless you create a branch."
- Right-click menu: "Create branch here…" (opens the existing create-branch prompt prefilled
  with the SHA as start point) and "Checkout <default-or-main> branch".
- Clicking still jumps to the commit in the graph (existing behavior).

### 6. PR Commits tab

PR detail segmented control becomes `Conversation | Files | Commits (N)`:
- N = `pr.commits.length` (already fetched by `pr_detail` — no backend change).
- List newest-first: short SHA (click = copy to clipboard, with a copied tick), subject,
  author, relative date — row styling consistent with the timeline's commit events.
- Tab remembers nothing (resets to Conversation on PR switch, same as Files).

### 7. Solid PR-timeline dots

`PrTimeline.svelte` rail markers: outline/open circles → solid filled, keeping their existing
state colors and sizes.

### 8. Curvier graph edges (option C — very round)

`src/lib/graph/paths.ts`: retune `curvedEdgePath` (and its sibling used for merge/branch
transitions if separate) so the cubic control points span most of the row gap — per the approved
mock (option C): control points at ~75% of the vertical span toward the destination, producing a
full-round S with no hard elbow. Angular mode untouched. The exact-output tests in
`paths.test.ts` are UPDATED to the new expected strings (they exist to pin behavior — the pin
moves deliberately). Settings toggle (Curved/Angular) unchanged.

## Error handling

- Async-ified commands keep their existing error strings; `busyOp` cleared in `finally` so a
  failure can't stick the spinner.
- Search never throws on weird input (regex-free substring matching).
- Reorder is bounds-guarded (no-op on out-of-range indices).

## Testing

Vitest: `commitSearch` (subject/author/SHA-prefix/case/empty), `listFilter` (PR/issue/release
fields), `reorder` (moves, bounds, identity), updated `paths.test.ts` for curve C.
Cargo: attribute-only changes — existing suites must stay green.
Gates: `npm run check` 0/0 · `npx vitest run` · `cargo test --workspace`; per-task review +
final adversarial review; manual `tauri build` for live checks (fetch spinner, drag reorder feel,
detached-HEAD flow, curve feel).

## Slices (implementation order)

1. Async commands + `busyOp` + Fetch button spinner (the bug fix).
2. Graph commit search (matcher + bar + jump).
3. GitHub list filters.
4. Tab drag reorder.
5. Delete-remote gating + detached-HEAD presentation (both Sidebar/dialog work).
6. PR Commits tab + solid dots + curve C (visual cluster).

## Out of scope

- Global ⌘K search palette; server-side (gh) search.
- Sidebar-list drag reorder (tabs only).
- Filtering the graph view down to matches (jump-and-highlight only).
