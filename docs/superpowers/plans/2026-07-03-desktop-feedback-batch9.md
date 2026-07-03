# Batch 9 — Desktop Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the fetch UI-freeze with a visible busy indicator, add per-screen search (⌘F), repo-tab drag reorder, smarter delete-remote gating, clearer detached-HEAD presentation, a PR Commits tab, solid timeline dots, and much rounder graph curves (option C).

**Architecture:** Attribute-only async fix on three Tauri commands + a `busyOp` label signal driven by `gitActions.run()`; pure, unit-tested helpers for search/filter/reorder consumed by small UI additions; presentation-only changes for detached-HEAD, PR commits, dots, and curve geometry. Spec: `docs/superpowers/specs/2026-07-03-desktop-feedback-batch9-design.md` — read it first.

**Tech Stack:** Tauri 2, SvelteKit 5 (runes), Rust workspace, Vitest, cargo test.

**Branch:** `desktop-batch9` (exists, current). Commit per task. Repo conventions in CLAUDE.md apply (async commands for slow processes; pure logic in tested `src/lib/*.ts` modules; `--`/`--end-of-options` guards).

---

### Task 1: Async commands + `busyOp` + Fetch button spinner (the bug fix)

**Files:**
- Modify: `src-tauri/src/commands.rs` (three attributes), `src/lib/store.svelte.ts`, `src/lib/gitActions.ts`, `src/routes/+page.svelte` (Fetch button ~line 315), `src/lib/components/StatusBar.svelte`

- [ ] **Step 1:** In `commands.rs`, change `fetch`, `fast_forward_branch`, and `delete_branch` from `#[tauri::command]` to `#[tauri::command(async)]` (they spawn network-touching git processes; sync commands run on the UI thread and freeze the app — see CLAUDE.md). No signature changes. `cargo check -p git-it` → clean.
- [ ] **Step 2:** In `store.svelte.ts`, next to the existing `navBusy` signal: `let busyOp = $state<string | null>(null);` + getter `get busyOp()` + `setBusyOp(v: string | null)` — mirror `navBusy`'s shape exactly.
- [ ] **Step 3:** In `gitActions.run(label, …)` (find where `navBusy` is set/cleared in try/finally): also `appState.setBusyOp(label)` on entry and `appState.setBusyOp(null)` in the same `finally`. Verify every `run()` call passes a human-readable label (they do — e.g. "Fetch", "Checkout PR #N").
- [ ] **Step 4:** Make `gitActions.fetch()`'s run-label exactly `"Fetching…"` (or map: check the current label and set it to a user-facing gerund). In `+page.svelte`, the Fetch button becomes:

```svelte
{@const fetching = appState.busyOp === "Fetching…"}
<button class="fetch-btn" class:busy={fetching} disabled={fetching}
  onclick={() => gitActions.fetch()} title="Fetch all remotes">
  {#if fetching}<span class="spin">⟳</span> Fetching…{:else}Fetch{/if}
</button>
```

with a `.spin { display:inline-block; animation: spin 0.9s linear infinite; }` + `@keyframes spin { to { transform: rotate(360deg); } }` + reduced-motion fallback (opacity pulse), styled to not change button height. (Adapt the `{@const}` placement to valid Svelte syntax — a `$derived` in the script is fine.)
- [ ] **Step 5:** In `StatusBar.svelte`, where the spinner shows for `navBusy || repoLoading`: when `appState.busyOp` is non-null, render its text next to the spinner (e.g. `⟳ Fetching…`) instead of the bare spinner, so menu-triggered ops (fast-forward, delete) also read as active.
- [ ] **Step 6:** Gates: `cargo test --workspace` (attribute change only), `npm run check`, `npx vitest run`. Commit: `fix(ops): async fetch/ff/delete-branch (UI freeze) + visible busyOp indicator`.

### Task 2: Graph commit search (⌘F)

**Files:**
- Create: `src/lib/graph/commitSearch.ts`, `src/lib/graph/commitSearch.test.ts`
- Create: `src/lib/components/GraphSearchBar.svelte`
- Modify: the graph screen host (where GraphHistory mounts — grep `GraphHistory` in `+page.svelte`), `src/lib/components/GraphHistory.svelte` (row highlight class), `src/lib/store.svelte.ts` (only if a signal is needed; prefer local state in the host)

- [ ] **Step 1 (TDD): failing tests** in `commitSearch.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { matchCommits } from "./commitSearch";

const C = (sha: string, subject: string, author: string) =>
  ({ sha, subject, authorName: author }) as never; // adapt the cast to the real GraphCommit shape

const commits = [
  C("a1b2c3d4e5", "Fix login bug", "Ash"),
  C("f6e5d4c3b2", "Add search feature", "Bob"),
  C("09876fedcb", "fix Search index", "ash shah"),
];

describe("matchCommits", () => {
  it("matches subject case-insensitively", () => {
    expect(matchCommits(commits, "search")).toEqual([1, 2]);
  });
  it("matches author name", () => {
    expect(matchCommits(commits, "ash")).toEqual([0, 2]);
  });
  it("matches SHA prefix only when the query is hex-like (≥4 chars)", () => {
    expect(matchCommits(commits, "a1b2")).toEqual([0]);
    expect(matchCommits(commits, "a1")).toEqual([]);   // too short for SHA, no text match
  });
  it("empty/whitespace query matches nothing", () => {
    expect(matchCommits(commits, "")).toEqual([]);
    expect(matchCommits(commits, "  ")).toEqual([]);
  });
  it("no matches → empty array", () => {
    expect(matchCommits(commits, "zzz")).toEqual([]);
  });
});
```

Adapt `C()` to the REAL `GraphCommit` fields (check `src/lib/types.ts` — subject/author field names) so the test compiles against the true type. Run → FAIL.
- [ ] **Step 2: implement** `matchCommits(commits: GraphCommit[], query: string): number[]` — trim; empty → `[]`; lowercase substring on subject + author; additionally, if `/^[0-9a-f]{4,}$/i.test(trimmed)`, include commits whose `sha.toLowerCase().startsWith(trimmed.toLowerCase())`; dedupe indices, ascending. PASS.
- [ ] **Step 3: `GraphSearchBar.svelte`.** Props: `{ open: boolean; count: number; active: number; onQuery: (q: string) => void; onNext: () => void; onPrev: () => void; onClose: () => void }`. Compact bar: 🔍 input (autofocus when opened), "N of M" (or "No matches"), ▲ ▼ buttons, ✕. Keys inside the input: Enter → onNext, Shift+Enter → onPrev, Escape → onClose. Debounce input ~150 ms before calling onQuery. Styling: same chrome tokens as the commit-list header (borders, 12px text); pinned as a slim row directly ABOVE the commit list, pushing content (not overlaying).
- [ ] **Step 4: host wiring** (graph screen in `+page.svelte`): local `$state` for `searchOpen/query/matches/activeIdx`; recompute `matches = matchCommits(appState.graphCommits, query)` in a `$derived`; window ⌘F listener (only when the graph screen is active and no dialog open; `preventDefault`) opens the bar; jump on next/prev via the existing `graphView` index-jump controller (grep `graphView` for the jump API used by jump-to-ref) + set `activeIdx` circularly. Highlight: pass the match set down so `GraphHistory` adds a `.search-hit` class on matching rows and `.search-active` on the current one (accent-tinted backgrounds; find the row-class block in GraphHistory and extend). When matches is empty and query non-empty and more commits are loadable (check how infinite scroll knows more pages exist), show a "Load more to search older history" button in the bar that triggers the same load-more path.
- [ ] **Step 5:** Gates + commit: `feat(graph): ⌘F commit search — match subject/author/SHA, jump + highlight`.

### Task 3: GitHub list filters

**Files:**
- Create: `src/lib/github/listFilter.ts`, `src/lib/github/listFilter.test.ts`
- Modify: `src/lib/components/github/GithubPulls.svelte`, `GithubIssues.svelte`, `GithubReleases.svelte`

- [ ] **Step 1 (TDD): failing tests** in `listFilter.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { filterItems } from "./listFilter";

const pulls = [
  { number: 12, title: "Fix crash on launch", author: "ash" },
  { number: 34, title: "Add dark mode", author: "bob" },
];
const releases = [{ name: "v1.2 — Spring", tagName: "v1.2.0" }, { name: "Beta", tagName: "v0.9" }];

describe("filterItems", () => {
  it("filters by title substring, case-insensitive", () => {
    expect(filterItems(pulls, "crash", ["title", "author"])).toEqual([pulls[0]]);
  });
  it("filters by author", () => {
    expect(filterItems(pulls, "BOB", ["title", "author"])).toEqual([pulls[1]]);
  });
  it("matches #number and bare number", () => {
    expect(filterItems(pulls, "#34", ["title", "author"])).toEqual([pulls[1]]);
    expect(filterItems(pulls, "12", ["title", "author"])).toEqual([pulls[0]]);
  });
  it("releases: name + tagName fields", () => {
    expect(filterItems(releases, "v1.2", ["name", "tagName"])).toEqual([releases[0]]);
  });
  it("empty query returns the input array", () => {
    expect(filterItems(pulls, " ", ["title"])).toEqual(pulls);
  });
});
```

- [ ] **Step 2: implement** `filterItems<T extends Record<string, unknown>>(items: T[], query: string, fields: (keyof T & string)[]): T[]` — trim, empty → items; lowercase substring across the given string fields; if the item has a numeric `number` property and the query is `#?\d+`, also match `String(item.number)` against the digits. PASS.
- [ ] **Step 3: wire into the three tabs.** Each gets a small filter input above its list (placeholder "Filter pull requests…" / "Filter issues…" / "Filter releases…", ✕ clear button, match count shown when filtering like "3 of 24"). Local `$state` query per component (resets on repo switch via the components' existing lifecycle). Derived filtered list feeds the existing `{#each}`. ⌘F: when the GitHub screen is active, focus the visible tab's filter input (window listener in `GithubView.svelte`, delegated via an exported focus function or an `id` per input — pick the simplest working mechanism; only ONE tab's input is mounted at a time). Fields: pulls/issues → title + author (+number rule); releases → name + tagName.
- [ ] **Step 4:** Gates + commit: `feat(github): client-side filter fields for PRs/Issues/Releases (⌘F)`.

### Task 4: Repo-tab drag reorder

**Files:**
- Create: `src/lib/reorder.ts`, `src/lib/reorder.test.ts`
- Modify: `src/lib/components/RepoTabs.svelte`, `src/lib/store.svelte.ts`

- [ ] **Step 1 (TDD): failing tests** in `reorder.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { reorder } from "./reorder";

describe("reorder", () => {
  it("moves an item forward", () => expect(reorder(["a","b","c","d"], 0, 2)).toEqual(["b","c","a","d"]));
  it("moves an item backward", () => expect(reorder(["a","b","c","d"], 3, 1)).toEqual(["a","d","b","c"]));
  it("same index is identity", () => expect(reorder(["a","b"], 1, 1)).toEqual(["a","b"]));
  it("out-of-range is identity", () => {
    expect(reorder(["a","b"], -1, 0)).toEqual(["a","b"]);
    expect(reorder(["a","b"], 0, 5)).toEqual(["a","b"]);
  });
  it("does not mutate the input", () => {
    const src = ["a","b","c"]; reorder(src, 0, 2); expect(src).toEqual(["a","b","c"]);
  });
});
```

- [ ] **Step 2: implement** pure `reorder<T>(list: T[], from: number, to: number): T[]`. PASS.
- [ ] **Step 3:** `appState.reorderRepos(from: number, to: number)` in `store.svelte.ts` — applies `reorder` to the open-repos array (find it: the array RepoTabs iterates). INVESTIGATE persistence: if the open-repos list (not just recentRepos/last-active) persists anywhere, write the new order through the same mechanism; if it's session-only, session-only reordering is accepted — note which in the commit message.
- [ ] **Step 4: drag UI in `RepoTabs.svelte`.** Pointer-events based (pointerdown → threshold 4px → dragging): the dragged tab gets `.dragging` (reduced opacity), a 2px accent drop-indicator line renders between tabs at the computed insertion index (from pointer x vs tab midpoints), pointerup → `appState.reorderRepos(from, insertionIndex)`. No HTML5 DnD ghosts. Click still selects (only when the threshold was never crossed). Keyboard/a11y unchanged.
- [ ] **Step 5:** Gates + commit: `feat(tabs): drag to reorder repo tabs`.

### Task 5: Delete-remote gating (remote branch must exist)

**Files:**
- Modify: `src/lib/components/Sidebar.svelte` (`confirmDeleteBranch`, ~line 70), `src/lib/dialogs.svelte.ts` (`branchDelete` state), `src/lib/components/Modal.svelte` (branchDelete render)

- [ ] **Step 1:** In `Sidebar.confirmDeleteBranch`: after resolving `upstream` from `refsDetailed`, compute `const remoteExists = upstream !== null && appState.refsByKind.remote.some((r) => r.name === upstream);` (verify the exact remote-ref name format in `refsByKind.remote` — e.g. `origin/feature` — matches the `upstream` string; adapt if one side is prefixed differently).
- [ ] **Step 2:** `dialogs.confirmBranchDelete` gains a `remoteGone: boolean` field (state + param). Rules: pass `upstream` only when `remoteExists` (toggle renders as today); pass `remoteGone: true` + `upstream: null` when tracking exists but the ref doesn't.
- [ ] **Step 3:** In `Modal.svelte`'s branchDelete block: when `remoteGone`, render a muted note `Remote branch already deleted.` where the toggle would be; no checkbox. Existing behavior for the other two cases unchanged.
- [ ] **Step 4:** Gates (`npm run check`, vitest untouched) + commit: `fix(branches): only offer "also delete remote" when the remote branch actually exists`.

### Task 6: Detached-HEAD presentation

**Files:**
- Modify: `src/lib/components/Sidebar.svelte` (the standalone `.ref.detached` button ~line 225 moves INTO the Local panel as a pinned first row; context-menu additions)

- [ ] **Step 1:** Move the detached entry inside the Local `CollapsiblePanel`, rendered BEFORE the `RefTree`, only when detached (same condition that renders it today — `refs.head[0]` shape; verify). Style: warning-tinted (use the existing warn/orange token if one exists, else `color-mix` with `#d97706`), ⚠ or ◦ icon, label `HEAD detached @ {shortSha}` (7 chars). Keep click = jump to the commit.
- [ ] **Step 2:** `title` tooltip: `HEAD points directly at a commit instead of a branch. New commits here are lost when you switch away unless you create a branch.`
- [ ] **Step 3:** Right-click context menu on the row (reuse `contextMenu` the way branch rows do): `Create branch here…` → the existing create-branch prompt/dialog prefilled with the detached SHA as start point (find how "create branch" is invoked from the branch context menu and reuse, passing the SHA); `Checkout main` (label uses the repo's default local branch: `main` if present else `master` else the first local; hidden if none) → `gitActions.checkout(<that branch>)`.
- [ ] **Step 4:** Remove the old standalone button + its now-dead CSS. Gates + commit: `feat(sidebar): detached HEAD as an explained, pinned Local entry with escape hatches`.

### Task 7: PR Commits tab + solid timeline dots

**Files:**
- Modify: `src/lib/components/github/GithubDetail.svelte` (segmented control + new tab), `src/lib/components/github/PrTimeline.svelte` (dot fill)
- Create: `src/lib/components/github/PrCommitsTab.svelte`

- [ ] **Step 1:** Segmented control becomes `Conversation | Files (N) | Commits (M)` (`detailTab` union gains `"commits"`; M = `pr.commits.length`; reset-to-conversation-on-PR-change logic already exists — extend it). PRs only; issues unchanged.
- [ ] **Step 2:** `PrCommitsTab.svelte` props `{ commits: GhCommit[] }` (type exists in `src/lib/types.ts`: `{ oid, message, author, committedDate }`). Render newest-first (`committedDate` desc; the fetched order may already be oldest-first — sort explicitly): row = mono short SHA button (7 chars; click → `navigator.clipboard.writeText(oid)` + transient ✓ swap, ~1.2 s), first line of `message`, author, relative date (reuse the `rel()`/date helper PrTimeline uses — import from the same place). Empty state: "No commits." Styling consistent with timeline commit events.
- [ ] **Step 3:** Solid dots: in `PrTimeline.svelte`, find the rail marker styles (open circles — `border` + transparent/`background: var(--panel-bg)` fill) and change to solid fill using each marker's existing state color (border color becomes the fill; keep size). Check both light/dark render sensibly (fill with the same token previously used for the border).
- [ ] **Step 4:** Gates + commit: `feat(github): PR Commits tab (count, copyable SHAs) + solid timeline dots`.

### Task 8: Curve C — very round graph edges

**Files:**
- Modify: `src/lib/graph/paths.ts`, `src/lib/graph/paths.test.ts`

- [ ] **Step 1:** In `paths.ts`, replace the mid-point control points in `curvedEdgePath` with full-round tangents (approved option C):

```ts
/** 0.5 = the old gentle S; higher pulls the tangents vertical for a rounder sweep. */
const CURVE_TENSION = 0.8;

export function curvedEdgePath(edge: Edge, topY: number, g: GeomConfig): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const a = g.rowHeight * CURVE_TENSION;
  return `M${x1} ${y1} C${x1} ${y1 + a} ${x2} ${y2 - a} ${x2} ${y2}`;
}
```

(Vertical tangents at both endpoints, sweeping across in the middle — no elbow. `angularEdgePath` untouched.)
- [ ] **Step 2:** Update `paths.test.ts` exact-output expectations: with the test geometry (`rowHeight: 30`, lanes 0→1 ⇒ x 12→28): `curvedEdgePath(edge(0, 1, "branch"), 0, g)` → `"M12 0 C12 24 28 6 28 30"`; same-lane case unchanged (`"M12 0 L12 30"`). Update every curved expectation in the file; run `npx vitest run src/lib/graph/paths.test.ts` → PASS.
- [ ] **Step 3:** Visual sanity note for the reviewer: multi-row bands — check whether `curvedEdgePath` is ALSO used for edges spanning taller bands (search call sites; if `topY`/`rowHeight` is always one row, fine). If a taller variant exists, apply the same tension logic proportionally.
- [ ] **Step 4:** Gates + commit: `feat(graph): rounder curved edges (tension 0.8, option C)`.

### Task 9: Full gates + whole-batch adversarial review

- [ ] `npm run check` 0/0 · `npx vitest run` all green · `cargo test --workspace` green · `npm run build` ok.
- [ ] Final adversarial review over `git diff main...desktop-batch9` (focus: ⌘F listener conflicts/leaks across screens and dialogs, drag-reorder vs tab-click interactions, async attribute side effects, search-jump vs virtualization, remote-name format assumption in Task 5, curve regressions at lane distances > 1). Fix Critical/Important, re-run gates.
- [ ] Update memory; STOP before merging — user live-tests (`npm run tauri dev/build`): fetch spinner + no freeze, ⌘F on both screens, drag feel, detached-HEAD flow, delete-remote gating on a "gone" branch, curve feel, solid dots, PR Commits tab.
