# Redesign Slice R2 — Toolbar + Status Bar + Lazy Loading + Panel Grouping

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the shell feel like a git client: load commits **on demand (infinite scroll)** instead of a fixed count; add a **bottom status bar**; **slim the header** (the timestamp tagline shouldn't dominate); and **group the recovery panels** (Backups/Reflog/Stashes/Remotes) under one collapsible section. Frontend-only (the backend `load_graph(repo, count, skip)` already pages).

**Branch:** `redesign/r2-toolbar-statusbar-lazyload` (created). Base `main`.

**Scope (R2 only):** lazy graph loading · StatusBar · header slimming · panel grouping (+ the `BackupsPanel` `isTauri` guard noted in R1). **NOT in R2:** the time-edit drawer (R3) and the rename (R4) — leave the EditTabs/ApplyPanel/title text as-is except where the status move touches them.

## File structure
- Modify `src/lib/store.svelte.ts` (append/hasMore/loading graph state).
- Modify `src/lib/gitActions.ts` (`reloadGraph` → page 1; new `loadMoreGraph`).
- Modify `src/lib/components/GraphHistory.svelte` (scroll-to-load-more).
- Create `src/lib/components/StatusBar.svelte`; mount in `+page.svelte`.
- Modify `src/routes/+page.svelte` (slim header; mount StatusBar).
- Create `src/lib/components/RecoveryPanels.svelte` (groups Backups/Reflog/Stashes/Remotes); modify `Sidebar.svelte` to use it; fix `BackupsPanel` `isTauri` guard.

---

## Task 1 — Infinite-scroll commit loading

**Files:** `store.svelte.ts`, `gitActions.ts`, `GraphHistory.svelte`.

- [ ] **Step 1: Store.** Add `let graphHasMore = $state(false)` and `let graphLoadingMore = $state(false)` (+ getters). Add `appendGraphCommits(gc: GraphCommit[])`: dedup against current by sha, concat to `graphCommits`, and re-map `commits` (the flat list) for the appended ones — do NOT reset selection/newDates/currentSha (this is an append, not a fresh load). Add a `PAGE` constant concept (the page size lives in gitActions). Keep `setGraphCommits` as the fresh-load reset (it already clears selection etc.).

- [ ] **Step 2: gitActions.** Define `const PAGE = 150;`. Change `reloadGraph` (Tauri branch) to load page 1: `const gc = await api.loadGraph(repo, PAGE, 0); appState.setGraphCommits(gc); appState.setGraphHasMore(gc.length === PAGE);` then the existing `refreshStatus`/`refreshWorkingChanges`/`refreshRefs`. Add:
```ts
export async function loadMoreGraph(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  if (!appState.graphHasMore || appState.graphLoadingMore) return;
  appState.setGraphLoadingMore(true);
  try {
    const gc = await api.loadGraph(appState.repo, PAGE, appState.graphCommits.length);
    appState.appendGraphCommits(gc);
    appState.setGraphHasMore(gc.length === PAGE);
  } catch (e) {
    console.warn("[gte] load more failed", e);
  } finally {
    appState.setGraphLoadingMore(false);
  }
}
```
(`skip` = the count already loaded; `git log --all --topo-order --skip --max-count` paging — deterministic for a static repo. New commits between pages can shift offsets; acceptable for browsing.)

- [ ] **Step 3: GraphHistory.** On the scrollable `.wrap` add an `onscroll` handler: when scrolled near the bottom (`scrollTop + clientHeight >= scrollHeight - 200`), call `gitActions.loadMoreGraph()`. Show a subtle "Loading more…" row at the bottom when `appState.graphLoadingMore`, and a "— end of history —" hint when `!graphHasMore && commits.length > 0`. (No DOM virtualization in R2 — that's a later perf follow-on; note it.)

- [ ] **Step 4: Verify + commit.** `npm run check` (0/0) + `npm test`. Commit: `feat(redesign): infinite-scroll commit loading (R2)` (+ trailer).

---

## Task 2 — Bottom status bar + slim header

**Files:** create `StatusBar.svelte`; modify `+page.svelte`.

- [ ] **Step 1: `StatusBar.svelte`.** A slim bar (full width, bottom of `<main>`): left = current branch (`appState.refsByKind.local.find(r=>r.isHead)?.name` or "detached HEAD" / "no repo") + ahead/behind (`↑{currentAhead} ↓{currentBehind}` when upstream) ; middle/right = working-tree summary from `appState.repoStatus` ("clean" or "N staged · M unstaged · K untracked" / "N conflicts") ; far right = `appState.status` (the latest op/status line) + a spinner dot when `appState.remoteOpActive || appState.isRewriting`. Use the CSS tokens; height ~24px; `--header-bg`/`--border-subtle`. Guard empty repo gracefully.

- [ ] **Step 2: Slim the header in `+page.svelte`.** Reduce the title's dominance: drop the always-on tagline `<span class="sub">Batch-edit commit timestamps…</span>` (it's a timestamp-specific tagline; the app is a git client now) OR shrink it to a small muted wordmark. Keep `<h1>` small (it gets the real rename in R4 — leave the text "Git It" for now but reduce font-size/weight so it reads as a wordmark, not a banner). The toolbar row (branch chip + Fetch/Pull/Push + Tabs/Sidebar toggle + gear) stays. Net effect: the header is one tidy bar, not a title banner + tagline.

- [ ] **Step 3: Mount `<StatusBar />`** as the last child of `<main>` (after the `.shell`). Move the "Ready"/status reliance away from forcing the user to read the Output panel — the StatusBar now surfaces `appState.status`. (Leave the Output/LogPanel as-is; it still shows the streamed log.)

- [ ] **Step 4: Verify + commit.** `npm run check` (0/0) + `npm test`. Commit: `feat(redesign): bottom status bar + slimmer header (R2)`.

---

## Task 3 — Group recovery panels + fix BackupsPanel guard

**Files:** create `RecoveryPanels.svelte`; modify `Sidebar.svelte`, `BackupsPanel.svelte`.

- [ ] **Step 1: `RecoveryPanels.svelte`.** A single collapsible "History & recovery" section (use `CollapsiblePanel` or the Sidebar section pattern) that contains, stacked, the existing `BackupsPanel`, `ReflogPanel`, `StashPanel`, and `RemotePanel` — so the sidebar shows one grouped section instead of four loose panels. Keep each sub-panel's own collapse behavior, or render them as sub-items; pick the cleaner look (match `Sidebar.svelte`).

- [ ] **Step 2: Use it in `Sidebar.svelte`.** Replace the four separate `<BackupsPanel/> <ReflogPanel/> <StashPanel/> <RemotePanel/>` mounts (currently each in a `.backups` div) with a single `<RecoveryPanels />`.

- [ ] **Step 3: Fix the `BackupsPanel` `isTauri` guard (from R1 review).** `BackupsPanel` fetches `api.listBundles`/`listSafetyRefs` without an `isTauri` guard, erroring in the browser when a repo is set. Add the same `isTauri()` guard the other panels use (ReflogPanel/StashPanel/RemotePanel all guard + show a "desktop app only" note in the browser) so it doesn't fetch / error outside Tauri.

- [ ] **Step 4: Verify + commit.** `npm run check` (0/0) + `npm test`. Commit: `feat(redesign): group recovery panels + guard BackupsPanel (R2)`.

---

## Task 4 — Verification + preview + review + merge

- [ ] **Step 1: Gates.** `cargo check` (no Rust change, confirm) + `npm run check` (0/0) + `npm test` + `npm run build`.
- [ ] **Step 2: Preview.** Start preview. Verify (mock where needed, then revert): the status bar renders with branch + tree summary + status; the header is slimmer (no big tagline banner); the recovery panels are grouped under one section; infinite-scroll appends (seed a larger sample list or simulate by checking the scroll handler + the "end of history" hint — the browser sample is small so it shows the end hint). Screenshot light + dark. Console clean. Revert seeds; re-run `npm run check`.
- [ ] **Step 3: Adversarial review** (Agent `superpowers:code-reviewer`, opus) over the branch diff. Focus: lazy-load — `appendGraphCommits` dedups + doesn't reset selection/edits; `hasMore`/`loadingMore` guards prevent duplicate/overlapping loads; the scroll handler doesn't thrash (throttle/guard); `skip` paging is correct; `reloadGraph` page-1 path doesn't regress existing reload callers (after every op the graph reloads to page 1 — confirm that's acceptable, i.e. ops don't lose the user's scroll-loaded history in a surprising way → it's fine, ops reset to recent). StatusBar reads the right state; header slimming didn't break the drag region / traffic-light spacing (glass mode). BackupsPanel guard correct. No backend/security change. Fix findings.
- [ ] **Step 4: Merge.** `superpowers:finishing-a-development-branch` → ff-merge to `main`, delete branch. Update memory (`git-client-redesign.md`: R2 done). Continue to R3.

## Deferred (noted)
- DOM virtualization (windowing) for very large histories — R2 does infinite-append only; the DOM grows with loaded rows. Acceptable for now; virtualization is a perf follow-on.

## Patterns to REUSE
- `gitActions` `reloadGraph`/`run` patterns; the Tauri-guard `isTauri()`; `CollapsiblePanel`; the existing `ReflogPanel`/`StashPanel`/`RemotePanel` desktop-only-note pattern (for the BackupsPanel fix); `appState.repoStatus`/`refsByKind`/`currentAhead`/`currentBehind` for the StatusBar.
