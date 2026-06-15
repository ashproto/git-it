# Redesign Slice R1 — Multi-Repo Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Turn the single-repo app into a multi-repo shell: a top **tab strip** of open repos AND a **sidebar repo list**, switchable by a user setting; retire the "Repository & Load" panel; open via folder-pick + a recent-repos menu. Everything else keeps working because the active repo still drives all existing code.

**Architecture (the key idea):** `appState.repo` keeps returning the **active** repo, so all existing `gitActions`/graph/diff code is untouched. Multi-repo is a thin layer: a persisted `openRepos: string[]` + `recentRepos: string[]` + the active repo (the existing `repo` state). Switching = set the active repo (the existing `set repo(v)` already clears per-repo state) → a single `$effect` in `+page` reloads the graph when `appState.repo` changes. No backend changes (git commands already take a repo path; `pickRepoFolder`/`isGitRepo` already exist). This is **frontend-only**.

**Scope (R1 only):** multi-repo state + RepoTabs + RepoList + mode toggle + open/recent + retire RepoLoader + the reload-on-switch effect + an empty state. **NOT in R1** (later slices): slimming the header/toolbar, the bottom status bar, infinite-scroll commit loading, grouping recovery panels, the time-edit drawer, the rename. Keep the rest of the layout as-is for now.

**Tech Stack:** SvelteKit 5 runes; reuse the Tauri Store settings pattern + `pickRepoFolder`/`api.isGitRepo`/`gitActions.reloadGraph`.

**Branch:** `redesign/r1-multi-repo-shell` (created).

## File structure
- Modify `src/lib/store.svelte.ts` (multi-repo state + persistence + `repoSwitcherMode`).
- Create `src/lib/components/RepoTabs.svelte`, `src/lib/components/RepoList.svelte`.
- Modify `src/routes/+page.svelte` (mount tabs/list, reload-on-switch effect, retire RepoLoader, empty state, mode toggle in toolbar).
- Modify `src/lib/components/DateFormatMenu.svelte` is NOT needed (mode toggle goes in the toolbar per the approved mockup).
- Delete/retire `src/lib/components/RepoLoader.svelte` usage (the folder-pick logic moves into RepoTabs/RepoList; the component can be removed from the tree).

---

## Task 1 — Store: multi-repo state + persistence

**Files:** modify `src/lib/store.svelte.ts`.

- [ ] **Step 1: Keys + sync loaders.** Near the other persistence keys add:
```ts
const OPENREPOS_KEY = "gte.openRepos.v1";       const OPENREPOS_STORE_KEY = "openRepos";
const RECENTREPOS_KEY = "gte.recentRepos.v1";   const RECENTREPOS_STORE_KEY = "recentRepos";
const REPOMODE_KEY = "gte.repoSwitcherMode.v1"; const REPOMODE_STORE_KEY = "repoSwitcherMode";
const RECENT_CAP = 12;
```
Add sync loaders (mirror `loadSyncDiffSplit`): `loadSyncStringList(lsKey)` → `string[]` (parse JSON array from localStorage; `[]` on error/Tauri), and `loadSyncRepoMode()` → `"tabs" | "sidebar"` (default `"tabs"`).

- [ ] **Step 2: State.** Inside `makeState()` add (the existing `repo` state stays as the ACTIVE repo — do NOT rename it):
```ts
let openRepos = $state<string[]>(loadSyncStringList(OPENREPOS_KEY));
let recentRepos = $state<string[]>(loadSyncStringList(RECENTREPOS_KEY));
let repoSwitcherMode = $state<"tabs" | "sidebar">(loadSyncRepoMode());
```
Add async hydrate blocks (mirror the `graphLineStyle` hydrate) for all three from the Tauri Store (`store.get<string[]>(OPENREPOS_STORE_KEY)` etc.), guarded by `…Touched` flags so a user action during the load window isn't clobbered. Add `persistStringList(storeKey, lsKey, value)` and `persistRepoMode()` write-through helpers (mirror `persistDiffSplit`; JSON-stringify the lists).

- [ ] **Step 3: Methods (on the returned object).** Preserve the existing `get repo()` / `set repo(v)` exactly (the setter already clears per-repo state — keep that). Add:
```ts
get openRepos() { return openRepos; },
get recentRepos() { return recentRepos; },
get repoSwitcherMode() { return repoSwitcherMode; },
setRepoSwitcherMode(m: "tabs" | "sidebar") { repoSwitcherModeTouched = true; repoSwitcherMode = m; persistRepoMode(); },

// Add `path` to the open set + recents (MRU) and make it active.
openRepo(path: string) {
  if (!path) return;
  if (!openRepos.includes(path)) openRepos = [...openRepos, path];
  recentRepos = [path, ...recentRepos.filter((p) => p !== path)].slice(0, RECENT_CAP);
  persistStringList(OPENREPOS_STORE_KEY, OPENREPOS_KEY, openRepos);
  persistStringList(RECENTREPOS_STORE_KEY, RECENTREPOS_KEY, recentRepos);
  this.repo = path; // existing setter: clears per-repo state; the +page effect reloads
},

// Switch active to an already-open repo.
setActiveRepo(path: string) {
  if (path && path !== repo) this.repo = path;
},

// Close a tab; if it was active, fall back to a neighbor (or empty).
closeRepo(path: string) {
  const idx = openRepos.indexOf(path);
  if (idx === -1) return;
  openRepos = openRepos.filter((p) => p !== path);
  persistStringList(OPENREPOS_STORE_KEY, OPENREPOS_KEY, openRepos);
  if (repo === path) this.repo = openRepos[idx] ?? openRepos[idx - 1] ?? openRepos[0] ?? "";
},
```
> Note: `this.repo = …` inside these methods invokes the existing `set repo(v)` (state-clearing). Confirm `this` binds to the returned object (these are object methods, so it does). If the existing setter has a `v !== repo` guard, opening the same path again won't reload — that's fine.

- [ ] **Step 4: Verify.** `npm run check` (0/0). Commit: `feat(redesign): multi-repo store state (open/recent/active + switcher mode) (R1)` (+ trailer).

---

## Task 2 — RepoTabs + RepoList components

**Files:** create `RepoTabs.svelte`, `RepoList.svelte`.

- [ ] **Step 1: Shared open flow.** Both components need: an "open" action = `const p = await pickRepoFolder(appState.repo || undefined); if (!p) return; if (!(await api.isGitRepo(p))) { appState.status = `${p} is not a git repo.`; return; } appState.openRepo(p);` (guard: in browser, `pickRepoFolder`/`isGitRepo` need Tauri — show a status note if not in Tauri). A helper `basename(path)` = `path.split("/").filter(Boolean).pop() ?? path` for tab labels (full path in `title`).

- [ ] **Step 2: `RepoTabs.svelte`** — a horizontal tab strip: for each `appState.openRepos`, a tab showing `basename` (title = full path), highlighted when `=== appState.repo`, click → `appState.setActiveRepo(path)`, an `×` → `appState.closeRepo(path)` (stopPropagation). A trailing `+` button → the open flow. A recent-repos dropdown (a small `▾` button toggling a menu of `appState.recentRepos` not already open → `appState.openRepo`). Style consistent with the app (use the CSS tokens; tabs read against `--header-bg`). Render nothing (or just the `+`) when there are no open repos.

- [ ] **Step 3: `RepoList.svelte`** — a sidebar block titled "Workspace" (reuse `CollapsiblePanel` or a simple `<section>` matching `Sidebar.svelte`'s look): list `appState.openRepos` (active highlighted, click → setActiveRepo, a small remove on hover → closeRepo), an "Open…" row → open flow, and a "Recent" subsection (recentRepos not open → openRepo). Same `basename`/title treatment.

- [ ] **Step 4: Verify.** `npm run check` (0/0) + `npm test` (existing green). Commit: `feat(redesign): RepoTabs + RepoList components (R1)`.

---

## Task 3 — Wire into the shell + retire RepoLoader

**Files:** modify `src/routes/+page.svelte`; remove `RepoLoader` from the tree.

- [ ] **Step 1: Reload-on-switch effect.** In `+page.svelte` add a single effect that reloads the active repo's graph whenever the active repo changes (Tauri only; the browser keeps the sample graph):
```ts
import { reloadGraph } from "$lib/gitActions";
let lastLoaded = "";
$effect(() => {
  const r = appState.repo;
  const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  if (inTauri && r && r !== lastLoaded) { lastLoaded = r; reloadGraph(); }
});
```
(The `lastLoaded` guard prevents re-running on unrelated state changes; switching tabs changes `appState.repo` → reloads once.)

- [ ] **Step 2: Mount the switcher.** In the header area, in **tabs mode** render `<RepoTabs />` (e.g. a row under the title strip / above the toolbar). In the `.side-col` (sidebar), in **sidebar mode** render `<RepoList />` at the top. Drive both off `appState.repoSwitcherMode`. Add a compact mode toggle to the toolbar (a segmented `Tabs | Sidebar` control → `appState.setRepoSwitcherMode`) per the approved mockup.

- [ ] **Step 3: Retire RepoLoader.** Remove `<RepoLoader />` from `.side-col` and its import. (Leave the file in place but unused, or delete it — your call; if deleted, ensure no other imports.) The folder-pick + git-repo validation now live in the open flow (Task 2). The "Recent N" count input is intentionally dropped (full reload for now; infinite-scroll lands in R2).

- [ ] **Step 4: Empty state.** When `appState.openRepos.length === 0` (and no active repo), show a centered "Open a repository to get started" prompt with an Open button (the open flow), in place of the graph/main column. (In the browser preview the sample graph still loads via the existing `onMount`, so the empty state shows mainly in the real app with no repos.)

- [ ] **Step 5: Verify.** `npm run check` (0/0) + `npm test`. Commit: `feat(redesign): mount repo tabs/list, reload-on-switch, retire RepoLoader (R1)`.

---

## Task 4 — Verification + preview + review + merge

- [ ] **Step 1: Gates.** `cargo check` (no Rust change, but confirm) + `npm run check` (0/0) + `npm test` (green) + `npm run build` (succeeds).
- [ ] **Step 2: Preview.** Start preview. Verify (seed `appState` via a temporary mock where Tauri-guarded, then revert): tabs mode renders a tab strip + `+`; the mode toggle switches to sidebar mode (Workspace list appears, tabs hide); recent menu lists recents; the empty state shows when openRepos is empty; switching the active repo updates the branch chip/graph (in the real app it reloads — in preview, simulate by setting two fake open repos and confirm the active highlight + toggle behavior). Screenshot tabs mode + sidebar mode (light + dark). Console clean. Revert seeds; re-run `npm run check`.
- [ ] **Step 3: Adversarial review** (Agent `superpowers:code-reviewer`, opus) over the branch diff. Focus: `appState.repo` still returns the active repo so existing code is unbroken; switching repos cleanly clears + reloads per-repo state (no stale graph/status/working-copy from the previous repo — the existing `set repo` clearing covers it); `openRepo`/`closeRepo` edge cases (close active → sensible fallback; close last → empty state; reopen same path); persistence round-trips (openRepos/recent/mode survive reload); the reload effect doesn't loop or double-load; no backend/security surface touched. Fix findings.
- [ ] **Step 4: Merge.** `superpowers:finishing-a-development-branch` → ff-merge to `main`, delete branch. Update memory (`git-client-redesign.md`: R1 done) + checkpoint with the user before R2.

## Patterns to REUSE
- Tauri Store settings pattern (`diffSplit`/`graphLineStyle`): sync load + async hydrate (touched-guard) + write-through.
- `pickRepoFolder` + `api.isGitRepo` (the existing open/validate logic from `RepoLoader`).
- `gitActions.reloadGraph` (reloads graph + status + working changes + refs for the active repo).
- `Sidebar.svelte` / `CollapsiblePanel` look for `RepoList`; the existing branch chip + toolbar in `+page.svelte`.
