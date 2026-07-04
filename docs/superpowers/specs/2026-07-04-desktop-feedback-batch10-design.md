# Desktop Feedback Batch 10 — Design

**Date:** 2026-07-04
**Branch:** stacked on `desktop-batch9` (tip `d3d7845`); one combined live test → ff-merge to `main` later.
**Status:** approved design, pending spec review.

Three user-reported items from live use of Git It:

1. Right-click a file in Local Changes → **Open**, **Open With ▸**, **Show in Finder**.
2. **Bug:** right-click a branch → *Fast-forward to origin* silently fails on some branches.
3. Commit-graph **merge edges** should read as a decisive "merge-in", not a symmetric ease. Make the chosen look the default and expose the alternatives (this round + the prior curviness round) in Settings.

---

## Item 1 — Local Changes context menu: Open / Open With / Show in Finder

### Goal
Add file actions to the existing right-click menu on Local Changes rows (staged, unstaged, untracked): **Open** (default app), **Open With ▸** (submenu of apps that can open the file, Finder-style), **Show in Finder** (reveal).

### What already exists
- `WorkingCopyView.svelte` already builds a per-file menu in `onRowContext(e, f, section)` and opens the shared `contextMenu` singleton.
- `contextMenu.svelte.ts` exposes `MenuItem { label?, action?, danger?, disabled?, separator? }` and `openAt(x, y, items)`.
- `tauri-plugin-opener` (2.5.4) is a dependency with `opener:default` capability. Its JS API `@tauri-apps/plugin-opener` provides:
  - `revealItemInDir(path)` → Show in Finder.
  - `openPath(path, openWith?)` → open with default app, or with a specific app path when `openWith` is given.

### Design

**Menu additions.** In `onRowContext`, before the existing section-specific stage/discard items, prepend:
```
Open
Open With ▸   (submenu; only when ≥1 handler app and file exists on disk)
Show in Finder
──────────────
<existing Stage / Unstage / Discard / Remove items>
```
Absolute path is `join(appState.repo, f.path)` (a small helper; repo path + "/" + relative path — repo paths in this app are absolute and forward-slashed on macOS).

**Missing-file guard.** A staged **deletion** has no working-tree file. For such a row:
- **Open** and **Open With ▸** are `disabled: true`.
- **Show in Finder** still reveals the parent directory (`revealItemInDir` on a non-existent leaf reveals the containing folder; if that also fails it is a no-op — acceptable).

Deletion detection: `WorkingFile` carries status flags; a staged row whose status marks a deletion (no on-disk file) disables Open/Open With. If the flag is ambiguous, fall back to letting `apps_for_file` return `[]` (→ submenu hidden) and letting `openPath` surface an error via the normal error toast. The plan pins the exact `WorkingFile` field.

**Submenu support (shared infra).** `MenuItem` gains `submenu?: MenuItem[]`. The context-menu component renders a nested flyout for items that have a `submenu`:
- Hover (or click) a submenu parent → the child list opens to the right (flips left if it would overflow the viewport).
- Parent items with a submenu have no `action`; selecting a leaf runs its `action` and closes the whole menu.
- Keyboard is out of scope (the existing menu is pointer-driven); no regression to existing single-level menus (absence of `submenu` behaves exactly as today).

**Open With population.** New Tauri command:
```
apps_for_file(path: String) -> Result<Vec<AppEntry>, String>
struct AppEntry { name: String, path: String }   // display name + .app bundle path
```
Placed in the **desktop shell** (`src-tauri/src/openwith.rs`), not `git-core` (this is a macOS/LaunchServices concern, not git). Implementation uses Cocoa `NSWorkspace.urlsForApplications(toOpen:)` for the file URL (macOS 12+; `objc2_app_kit` is already transitively available via the opener plugin, added explicitly to `src-tauri/Cargo.toml`). Each returned app URL yields:
- `path` = the `.app` bundle path (file URL → path).
- `name` = `NSFileManager.displayName(atPath:)` of that path, minus a trailing `.app` (e.g. "Visual Studio Code", "Xcode").
De-duplicate by path; drop entries whose path doesn't exist. Empty result → the "Open With ▸" item is omitted.

Each submenu leaf calls `openPath(abs, app.path)`.

### Files
- Create: `git-it/src-tauri/src/openwith.rs` (the `apps_for_file` command + `AppEntry`).
- Modify: `git-it/src-tauri/src/commands.rs` or `lib.rs` (register `apps_for_file`); `src-tauri/Cargo.toml` (explicit `objc2-app-kit`/`objc2-foundation` if not already direct deps).
- Modify: `git-it/src/lib/contextMenu.svelte.ts` (add `submenu?`), the context-menu render component (nested flyout).
- Modify: `git-it/src/lib/api.ts` (`appsForFile` binding), `git-it/src/lib/components/WorkingCopyView.svelte` (menu items + abs-path helper).

### Testing
- Rust unit test for `apps_for_file`: a known extension (e.g. a temp `.txt`) returns a non-empty list containing at least one app with a non-empty name and an existing path; a path with a leading dash is rejected before use (shell-out safety parity, even though this is an API call not a shell-out).
- Frontend: submenu rendering is component-level (not unit-tested logic); verified in browser preview. Any pure helper (abs-path join) gets a vitest.

---

## Item 2 — Fast-forward to origin: ref-ambiguity fix

### Root cause (confirmed against the live `Resume-Designer` repo)
`Resume-Designer` has **both** `refs/heads/next` (a branch) and `refs/tags/next` (a tag). The current backend runs:
```
git fetch --end-of-options origin next:next
```
Because the destination `next` is **unqualified and ambiguous**, git resolves it to the **tag** and rejects the update:
```
 ! [rejected]  next -> next  (non-fast-forward)      # exit 1
```
`main` has no same-named tag, so `main:main` unambiguously hits the branch and works. Verified fix:
```
git fetch --end-of-options origin refs/heads/next:refs/heads/next
   f55a574..a6bf1eb  next -> next                    # exit 0, advances 143 commits
```

### Design

**Backend** `crates/git-core/src/ops.rs::fast_forward_branch`:
- New signature: `fast_forward_branch(repo, local_branch, remote, remote_branch) -> Result<String, String>`.
- Refspec fully qualified on **both** sides: `refs/heads/{remote_branch}:refs/heads/{local_branch}`.
- Keep `--end-of-options` and `GIT_TERMINAL_PROMPT=0`.
- Validate `local_branch` / `remote_branch` are non-empty and do not start with `-` (shell-out safety), matching the pattern in `branch_subjects`.
- On error, map git's `non-fast-forward` stderr to a friendlier message: `Can't fast-forward {local_branch}: it has diverged from {remote}/{remote_branch}.` Other errors pass through their git text.
- On success, return the fetch summary (git prints `old..new  branch -> branch`); the frontend already surfaces the returned string.

**Frontend** `src/lib/components/Sidebar.svelte` (`onRefContext`, the local-branch FF item):
- Derive `remoteBranch` from the upstream: `upstream = "origin/next"` → `remote = "origin"`, `remoteBranch = "next"` (everything after the first `/`). This also handles a renamed upstream (`origin/foo` tracking local `bar`).
- Call `gitActions.fastForwardBranch(localName, remote, remoteBranch)`.

**Plumbing:** `src-tauri/src/commands.rs::fast_forward_branch` gains `remote_branch`; `src/lib/api.ts::fastForwardBranch` and `src/lib/gitActions.ts::fastForwardBranch` gain the parameter.

### Testing
- New Rust test in `ops.rs`: build a temp repo with a branch `feature` and a **tag `feature`**, a bare "remote" ahead of `feature`, wire the upstream, then assert `fast_forward_branch` advances `refs/heads/feature` (not the tag) and returns Ok. A companion assertion that a diverged branch yields the friendly diverged error.
- The existing `fast_forward_branch_advances_non_current_branch` test is updated to the new signature.

### Non-goals
- No force-update of a diverged branch (destructive; out of scope). We only report that it can't fast-forward.
- No change to the `!r.isHead` gating (FF stays offered only for non-current branches).

---

## Item 3 — Merge-in curve (default A) + graph curve Settings

### Current behavior
`curvedEdgePath` (in `src/lib/graph/paths.ts`) ignores `edge.kind` and draws a symmetric S (both tangents vertical, `CURVE_TENSION = 0.8`) for every lane change — the "eases in" look. `GraphGutter.svelte` already switches between `curvedEdgePath` and `angularEdgePath` via a `lineStyle: "curved" | "angular"` prop, and `lineStyle` is already a persisted user setting (`gitit.graphLineStyle.v1`).

### Edge-kind mapping (from `src/lib/graph/lanes.ts`)
- `kind: "branch"` — emitted only for a **merge commit's fork-out** to its second parent (merge dot at top → parent lane at bottom). This is the **merge-in** edge.
- `kind: "merge"` — emitted when a descending side lane lands on a next-row commit in a different lane. This is the **branch-off** point.
- `kind: "straight"` — same-lane; renders as a vertical line (`x1 === x2`), unaffected.

### Design

**`curvedEdgePath(edge, topY, g, opts?)`** where `opts = { tension: number, mergeInStyle: "hooked" | "featureSide" | "symmetric" }` (defaults `{ tension: 0.8, mergeInStyle: "hooked" }` so existing callers/tests can pass nothing and get the new default). `x1 === x2` still returns the straight `L` line regardless of kind.

For a lane-changing edge (top `(x1,y1)` → bottom `(x2,y2)`, `h = y2 - y1`, `a = h * tension`):

- **`kind: "merge"` (branch-off) → symmetric** (unchanged):
  `M x1 y1 C x1 (y1+a) x2 (y2-a) x2 y2`
- **`kind: "branch"` (merge-in) → depends on `mergeInStyle`:**
  - `"hooked"` (**A**, default): feature lane runs straight, hooks into the node at the top.
    `M x1 y1 C x2 y1 x2 (y2 - h*(1-tension)) x2 y2` — leaves the node horizontally toward the feature lane, arrives vertical. (For `tension = 0.8`, the second control sits at `y2 - 0.2h`.)
  - `"featureSide"` (**B**): main lane stays straight, bend near the feature dot.
    `M x1 y1 C x1 (y1 + h*(1-tension)) x1 y2 x2 y2` — leaves the node vertical down its own lane, curves into the feature lane near the bottom.
  - `"symmetric"`: same symmetric S as the `merge` kind (the pre-batch-10 look).

Note: `x1` is the **merge dot** lane (top), `x2` the **second-parent** lane (bottom), per `lanes.ts` line 109 (`fromLane: rec.lane, toLane: k`). The "hooked into the node" reading places the hook at the top (merge dot), which is correct for a merge-in.

`angularEdgePath` is unchanged (its own kind-based elbow already exists).

**Settings (two new, curved-only).** Following the `lineStyle` dual-persist pattern in `store.svelte.ts` (a `gitit.*.vN` localStorage key + a Tauri Store key + `loadSync*` + async durable load + getter/setter + write-through persist):

1. `graphMergeInStyle: "hooked" | "featureSide" | "symmetric"` — default `"hooked"`. Keys `gitit.graphMergeInStyle.v1` / store `graphMergeInStyle`.
2. `graphCurviness: number` via presets — default `0.8`. Keys `gitit.graphCurviness.v1` / store `graphCurviness`. Exposed as three presets:
   - **Subtle** = `0.55`
   - **Balanced** = `0.8` (default; today's look)
   - **Sweeping** = `0.95` (kept < 1 so the curve stays monotone / no kink, per the batch-9 proof)

Both only take effect when line style = **Curved**; the Settings panel shows them indented/disabled under the existing Curved/Angular control.

**Threading.** Store getters → the graph host (`+page.svelte` / `GraphHistory.svelte`) → `GraphGutter` props (`tension`, `mergeInStyle`) → `curvedEdgePath(edge, topY, g, { tension, mergeInStyle })`. `GraphGutter` keeps `lineStyle` as today; the two new props default to the current look so nothing breaks if unset.

### Files
- Modify: `src/lib/graph/paths.ts` (kind-aware `curvedEdgePath` + `opts`), `src/lib/graph/paths.test.ts` (exact-output cases for each `mergeInStyle` and a couple of tensions; branch-off symmetric case; straight case).
- Modify: `src/lib/components/GraphGutter.svelte` (two new props → pass to `curvedEdgePath`), `src/lib/components/GraphHistory.svelte` / `+page.svelte` (thread store settings in).
- Modify: `src/lib/store.svelte.ts` (two settings: keys, loadSync, async load, getters/setters, persist), `src/lib/components/SettingsPanel.svelte` (Curviness + Merge-in curve controls under the graph line-style section).

### Testing
- `paths.test.ts`: assert exact path strings for `branch`-kind `hooked` / `featureSide` / `symmetric` at `tension ∈ {0.55, 0.8}`; assert `merge`-kind stays the symmetric S; assert `straight` (`x1 === x2`) is the vertical `L` for every style.
- Store: default values and setter round-trip covered by the existing settings test approach if present; otherwise browser-preview verified.
- Browser preview: toggle each setting and confirm the graph re-renders (merge-in hook vs feature-side vs symmetric; curviness presets).

---

## Cross-cutting

- **Branch:** all work lands on `desktop-batch9`. Items are independent; execute sequentially (shared git index — no parallel tree mutations).
- **File overlap awareness:** item 3 touches `paths.ts` / `GraphGutter` (same area as batch-9 curve C) and item 2 touches `ops.rs` / `Sidebar.svelte` (same area as batch-9 delete-remote/FF) — stacking on batch 9 avoids conflicts.
- **Gates:** `npm run check` (0 errors) + `npm test` + `cargo test`, plus browser preview for graph/menu. Then the user's combined `tauri dev`/`build` live test. **Stop before merge.**
- **Shell-out safety:** the FF refspec keeps `--end-of-options`; new operands are validated for leading `-`.
- **Async commands:** `apps_for_file` is a fast local LaunchServices call (not network); it may remain a normal `#[tauri::command]`. `fast_forward_branch` is already `#[tauri::command(async)]` (batch 9) and stays async.

## Out of scope / non-goals
- No "Open With" for commit-detail file lists (Local Changes only, per request).
- No keyboard navigation for the context submenu (pointer-driven, matching existing menus).
- No force fast-forward of a diverged branch.
- No change to the Curved/Angular toggle or to `angularEdgePath`.
