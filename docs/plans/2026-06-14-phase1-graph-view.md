# Phase 1 · Plan 3 — Commit graph view (implementation plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat commit table with a real Fork/SourceTree-style commit graph — an SVG lane gutter (from the Plan 1 engine) aligned to commit rows with ref badges — wired to the Plan 2 `load_graph` data, with a curved/angular line-style toggle, while preserving the existing multi-select + timestamp-editing flow.

**Architecture:** The store gains `graphCommits` (from `api.loadGraph`, or a bundled sample when not in Tauri so the browser preview renders), a `$derived` `rows = computeLanes(...)`, and a persisted `graphLineStyle`. A new `GraphGutter.svelte` draws the lanes/dots from `RowLayout[]` using the engine's path builders + palette; `GraphHistory.svelte` lays out the rows (badges/subject/author/date/sha/new-date) with the gutter overlaid, reusing the existing selection model; `+page.svelte` mounts it in place of `CommitTable`. The 3-pane sidebar/toolbar/detail chrome is a deliberate follow-up (Plan 3b) — this plan delivers the graph itself.

**Tech Stack:** Svelte 5 runes, TypeScript, the Plan 1 engine (`src/lib/graph`), the Plan 2 commands (`api.loadGraph`).

**Reference spec:** `docs/specs/2026-06-14-git-graph-client-design.md` §5.3 (rendering), §5.4 (shell — partial), §5.5 (settings).

**Prerequisites:** Plans 1 (engine) and 2 (data layer) merged to `main`. `appState` lives in `src/lib/store.svelte.ts`; `isTauri()` already exists there.

**Branch:** `git checkout -b feat/phase1-graph-view` off `main` before Task 1.

**Commit trailer:** every commit ends with `-m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"`.

---

## File structure

| File | Change |
|---|---|
| `src/lib/graph/sample.ts` | New: `SAMPLE_GRAPH: GraphCommit[]` (browser-preview fallback) |
| `src/lib/store.svelte.ts` | Add `graphCommits`, derived `rows`, `graphLineStyle` (+persist), `setGraphCommits`, `setGraphLineStyle`, `graphToCommit` mapping |
| `src/lib/components/GraphGutter.svelte` | New: SVG lane/dot renderer from `RowLayout[]` |
| `src/lib/components/GraphHistory.svelte` | New: graph rows + selection + line-style toggle (replaces CommitTable) |
| `src/lib/components/RepoLoader.svelte` | Route Reload through `appState`-level graph load |
| `src/routes/+page.svelte` | Mount `GraphHistory` instead of `CommitTable`; sample auto-load in browser |
| `src/lib/components/CommitTable.svelte` | Delete (superseded) |

JS checks run from `git-it/`.

---

## Task 1: Store + sample data

**Files:**
- Create: `src/lib/graph/sample.ts`
- Modify: `src/lib/store.svelte.ts`

- [ ] **Step 1: Create the sample graph** at `src/lib/graph/sample.ts`:

```ts
import type { GraphCommit } from "../types";

// A representative history (one merge, a merged feature branch, a tag, a separate
// unmerged branch, HEAD on main, a root) used when not running in Tauri so the
// browser preview can render the graph. Topologically ordered, children first.
export const SAMPLE_GRAPH: GraphCommit[] = [
  { sha: "a1b2c3d4e5f6", parents: ["b2c3d4e5f6a7"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T15:40:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T15:40:00Z", refs: [{ name: "main", kind: "local", is_head: true }], subject: "Render commit graph gutter" },
  { sha: "b2c3d4e5f6a7", parents: ["c3d4e5f6a7b8", "f6a7b8c9d0e1"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T14:10:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T14:10:00Z", refs: [], subject: "Merge branch 'feature/graph-view'" },
  { sha: "f6a7b8c9d0e1", parents: ["e5f6a7b8c9d0"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T13:05:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T13:05:00Z", refs: [{ name: "feature/graph-view", kind: "local", is_head: false }], subject: "Add lane assignment algorithm" },
  { sha: "c3d4e5f6a7b8", parents: ["e5f6a7b8c9d0"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T11:30:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T11:30:00Z", refs: [{ name: "v1.2.0", kind: "tag", is_head: false }], subject: "Fetch parent SHAs + ref decoration" },
  { sha: "e5f6a7b8c9d0", parents: ["d0e1f2a3b4c5"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-13T17:00:00Z", committer_name: "A. Shah", committer_date: "2026-06-13T17:00:00Z", refs: [], subject: "Scaffold graph data layer" },
  { sha: "9a8b7c6d5e4f", parents: ["d0e1f2a3b4c5"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-13T16:20:00Z", committer_name: "A. Shah", committer_date: "2026-06-13T16:20:00Z", refs: [{ name: "fix/dates", kind: "local", is_head: false }, { name: "origin/fix/dates", kind: "remote", is_head: false }], subject: "Fix Feb-30 date validation" },
  { sha: "d0e1f2a3b4c5", parents: [], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-13T09:00:00Z", committer_name: "A. Shah", committer_date: "2026-06-13T09:00:00Z", refs: [], subject: "Round-2 audit hardening" },
];
```

- [ ] **Step 2: Extend the store.** In `src/lib/store.svelte.ts`:

(a) Add imports near the top (after the existing `import type { Commit } ...` lines):
```ts
import type { GraphCommit } from "./types";
import { computeLanes, type RowLayout } from "./graph";
```

(b) Add a snake_case→`Commit` mapper at module scope (after the imports, before `makeState`):
```ts
// The edit/apply features consume the flat Commit shape; map from GraphCommit.
function graphToCommit(g: GraphCommit): Commit {
  return {
    sha: g.sha,
    author_name: g.author_name,
    author_date: g.author_date,
    committer_name: g.committer_name,
    committer_date: g.committer_date,
    subject: g.subject,
  };
}
```

(c) Add persistence constants next to the existing date-format ones:
```ts
const LINESTYLE_KEY = "gte.graphLineStyle.v1";
const LINESTYLE_STORE_KEY = "graphLineStyle";
```

(d) Inside `makeState()`, after the `dateFormat` state declarations, add:
```ts
  let graphCommits = $state<GraphCommit[]>([]);
  const rows = $derived(
    computeLanes(graphCommits.map((c) => ({ sha: c.sha, parents: c.parents }))),
  );
  let graphLineStyle = $state<"curved" | "angular">(loadSyncLineStyle());
  let graphLineStyleTouched = false;

  // Hydrate the durable line-style (Tauri) like dateFormat.
  const lsHydrate = getStore();
  if (lsHydrate) {
    lsHydrate
      .then((store) => store.get<string>(LINESTYLE_STORE_KEY))
      .then((saved) => {
        if ((saved === "curved" || saved === "angular") && !graphLineStyleTouched) {
          graphLineStyle = saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load line style", e));
  }

  function persistLineStyle() {
    const snapshot = graphLineStyle;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(LINESTYLE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist line style", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(LINESTYLE_KEY, snapshot);
    } catch (e) {
      console.warn("[gte] could not persist line style", e);
    }
  }
```

(e) Add a sync loader at module scope (next to `loadSyncDateFormat`):
```ts
function loadSyncLineStyle(): "curved" | "angular" {
  if (isTauri()) return "curved";
  try {
    if (typeof localStorage === "undefined") return "curved";
    const raw = localStorage.getItem(LINESTYLE_KEY);
    return raw === "angular" ? "angular" : "curved";
  } catch {
    return "curved";
  }
}
```

(f) In the object returned by `makeState()`, add these members (alongside the existing getters/setters):
```ts
    get graphCommits() {
      return graphCommits;
    },
    get rows() {
      return rows;
    },
    get graphLineStyle() {
      return graphLineStyle;
    },
    setGraphLineStyle(v: "curved" | "angular") {
      graphLineStyleTouched = true;
      graphLineStyle = v;
      persistLineStyle();
    },
    // Set the graph commits and keep the flat `commits` list (used by the edit
    // tools) in sync; clear stale selection + staged dates.
    setGraphCommits(gc: GraphCommit[]) {
      graphCommits = gc;
      commits = gc.map(graphToCommit);
      newDates = new Map();
      selected = new Set();
    },
```

- [ ] **Step 3: Type-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -4`
Expected: svelte-check 0 errors, 0 warnings. (`RowLayout` is imported for the derived type; if svelte-check flags it as unused, drop the `type RowLayout` from the import — only `computeLanes` is strictly required.)

- [ ] **Step 4: Commit.**
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/graph/sample.ts src/lib/store.svelte.ts
git commit -m "feat(graph): add graph commits + line-style state to store" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: GraphGutter component

**Files:**
- Create: `src/lib/components/GraphGutter.svelte`

- [ ] **Step 1: Create `src/lib/components/GraphGutter.svelte`:**

```svelte
<script lang="ts">
  import {
    laneX,
    curvedEdgePath,
    angularEdgePath,
    laneColor,
    type RowLayout,
    type Edge,
    type GeomConfig,
  } from "../graph";

  let {
    rows,
    heads,
    rowHeight = 30,
    lineStyle = "curved",
  }: {
    rows: RowLayout[];
    heads: boolean[];
    rowHeight?: number;
    lineStyle?: "curved" | "angular";
  } = $props();

  const laneWidth = 16;
  const offsetX = 12;
  const g: GeomConfig = $derived({ laneWidth, rowHeight, offsetX });
  const maxLanes = $derived(rows.reduce((m, r) => Math.max(m, r.width), 1));
  const width = $derived(offsetX + maxLanes * laneWidth);
  const height = $derived(Math.max(rows.length * rowHeight, rowHeight));

  const dotY = (i: number) => i * rowHeight + rowHeight / 2;
  const pathFor = (edge: Edge, topY: number) =>
    lineStyle === "angular" ? angularEdgePath(edge, topY, g) : curvedEdgePath(edge, topY, g);
</script>

<svg
  class="gutter"
  width={width}
  height={height}
  viewBox={`0 0 ${width} ${height}`}
  aria-hidden="true"
>
  {#each rows as row, i}
    {#each row.edges as edge}
      <path
        d={pathFor(edge, dotY(i))}
        stroke={laneColor(edge.colorIndex, null, {})}
        stroke-width="2"
        fill="none"
      />
    {/each}
  {/each}
  {#each rows as row, i}
    {#if heads[i]}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="7.5"
        fill="none"
        stroke="var(--accent)"
        stroke-width="1.5"
      />
    {/if}
    {#if row.isMerge}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="4.5"
        fill="var(--panel-bg)"
        stroke={laneColor(row.colorIndex, null, {})}
        stroke-width="2"
      />
    {:else}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="4.5"
        fill={laneColor(row.colorIndex, null, {})}
        stroke="var(--panel-bg)"
        stroke-width="1.5"
      />
    {/if}
  {/each}
</svg>

<style>
  .gutter {
    display: block;
    pointer-events: none;
  }
</style>
```

- [ ] **Step 2: Type-check.**
Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -4`
Expected: 0 errors, 0 warnings. (The component is not mounted yet, so svelte-check only validates it in isolation.)

- [ ] **Step 3: Commit.**
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/GraphGutter.svelte
git commit -m "feat(graph): add GraphGutter SVG lane renderer" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: GraphHistory component (replaces CommitTable)

**Files:**
- Create: `src/lib/components/GraphHistory.svelte`

- [ ] **Step 1: Create `src/lib/components/GraphHistory.svelte`:**

```svelte
<script lang="ts">
  import { appState } from "../store.svelte";
  import { parseISO, formatCommitDate } from "../dates";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import GraphGutter from "./GraphGutter.svelte";

  const rowHeight = 30;
  let anchorIndex = $state<number | null>(null);

  const commits = $derived(appState.graphCommits);
  const rows = $derived(appState.rows);
  const heads = $derived(commits.map((c) => c.refs.some((r) => r.is_head)));
  const gutterWidth = $derived(
    12 + Math.max(1, rows.reduce((m, r) => Math.max(m, r.width), 1)) * 16,
  );

  function localDate(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat) : iso;
  }
  function newDateLabel(sha: string): string {
    const d = appState.newDates.get(sha);
    return d ? formatCommitDate(d, appState.dateFormat) : "";
  }

  function onRowMouseDown(event: MouseEvent, sha: string, index: number) {
    event.preventDefault();
    const cmd = event.metaKey || event.ctrlKey;
    const shift = event.shiftKey;
    if (shift && anchorIndex !== null) {
      const [lo, hi] = anchorIndex <= index ? [anchorIndex, index] : [index, anchorIndex];
      const next = new Set<string>(cmd ? appState.selected : []);
      for (let i = lo; i <= hi; i++) next.add(commits[i].sha);
      appState.selected = next;
    } else if (cmd) {
      const next = new Set(appState.selected);
      if (next.has(sha)) next.delete(sha);
      else next.add(sha);
      appState.selected = next;
      anchorIndex = index;
    } else {
      appState.selected = new Set([sha]);
      anchorIndex = index;
    }
  }

  function selectAll() {
    appState.selectAll();
    anchorIndex = commits.length > 0 ? 0 : null;
  }
  function clearSel() {
    appState.clearSelection();
    anchorIndex = null;
  }
</script>

<CollapsiblePanel title="Commits">
  {#snippet headerActions()}
    <span class="count">{appState.selected.size} selected of {commits.length}</span>
    <div class="seg" role="group" aria-label="Graph line style">
      <button
        type="button"
        class:active={appState.graphLineStyle === "curved"}
        onclick={() => appState.setGraphLineStyle("curved")}
      >Curved</button>
      <button
        type="button"
        class:active={appState.graphLineStyle === "angular"}
        onclick={() => appState.setGraphLineStyle("angular")}
      >Angular</button>
    </div>
    <button type="button" onclick={selectAll}>Select all</button>
    <button type="button" onclick={clearSel}>Clear</button>
  {/snippet}

  <div class="wrap">
    <div class="head-row" style={`padding-left:${gutterWidth}px`}>
      <span class="h subject">Description</span>
      <span class="h author">Author</span>
      <span class="h date">Date</span>
      <span class="h sha">Commit</span>
      <span class="h newdate">New date</span>
    </div>

    <div class="history">
      <div class="gutter-layer" style={`width:${gutterWidth}px`}>
        <GraphGutter {rows} {heads} {rowHeight} lineStyle={appState.graphLineStyle} />
      </div>

      {#each commits as commit, i (commit.sha)}
        <div
          class="row"
          class:selected={appState.selected.has(commit.sha)}
          class:edited={appState.newDates.has(commit.sha)}
          style={`height:${rowHeight}px`}
          onmousedown={(e) => onRowMouseDown(e, commit.sha, i)}
        >
          <div class="spacer" style={`width:${gutterWidth}px`}></div>
          <div class="subject">
            {#each commit.refs as r}
              <span class="badge {r.kind}" class:head={r.is_head}>{r.name}</span>
            {/each}
            <span class="msg">{commit.subject}</span>
          </div>
          <div class="author">{commit.author_name}</div>
          <div class="date mono">{localDate(commit.author_date)}</div>
          <div class="sha mono">{commit.sha.slice(0, 9)}</div>
          <div class="newdate mono">{newDateLabel(commit.sha)}</div>
        </div>
      {/each}

      {#if commits.length === 0}
        <div class="empty">No commits loaded. Pick a repository and click Reload.</div>
      {/if}
    </div>
  </div>
  <p class="hint">Click to select · ⌘-click to add/remove · Shift-click to select range</p>
</CollapsiblePanel>

<style>
  .count {
    color: var(--text-muted);
    font-size: 12px;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .seg button {
    padding: 3px 9px;
    border: none;
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .seg button.active {
    background: var(--accent);
    color: #fff;
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
  .wrap {
    border-radius: 6px;
    border: 1px solid var(--border);
    overflow: auto;
    max-height: 440px;
    user-select: none;
    -webkit-user-select: none;
  }
  .head-row {
    display: flex;
    align-items: center;
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--header-bg);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    color: var(--text-muted);
    padding-top: 6px;
    padding-bottom: 6px;
  }
  .history {
    position: relative;
  }
  .gutter-layer {
    position: absolute;
    left: 0;
    top: 0;
    pointer-events: none;
    z-index: 1;
  }
  .row {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    font-size: 12.5px;
    position: relative;
    z-index: 0;
  }
  .row:hover {
    background: var(--row-hover);
  }
  .row.selected {
    background: var(--row-selected);
  }
  .spacer {
    flex: 0 0 auto;
  }
  .subject {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }
  .msg {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .author {
    flex: 0 0 110px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 8px;
  }
  .date {
    flex: 0 0 168px;
    color: var(--text-muted);
  }
  .sha {
    flex: 0 0 84px;
    color: var(--text-muted);
  }
  .newdate {
    flex: 0 0 168px;
    color: var(--accent);
    padding-right: 10px;
  }
  .row.edited .newdate {
    font-weight: 600;
  }
  .badge {
    flex: 0 0 auto;
    font-size: 11px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--text-muted);
    white-space: nowrap;
  }
  .badge.head {
    border-color: var(--accent);
    color: var(--accent);
  }
  .badge.tag {
    border-color: var(--err);
    color: var(--err);
  }
  .badge.remote {
    opacity: 0.7;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .empty {
    text-align: center;
    color: var(--text-muted);
    padding: 24px;
    font-style: italic;
  }
  .hint {
    margin: 6px 0 0 0;
    font-size: 11px;
    color: var(--text-muted);
  }
</style>
```

- [ ] **Step 2: Type-check.**
Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -4`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Commit.**
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/GraphHistory.svelte
git commit -m "feat(graph): add GraphHistory graph rows + line-style toggle" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Wire it in

**Files:**
- Modify: `src/lib/components/RepoLoader.svelte`
- Modify: `src/routes/+page.svelte`
- Delete: `src/lib/components/CommitTable.svelte`

- [ ] **Step 1: Route Reload through the graph load.** In `src/lib/components/RepoLoader.svelte`, replace the body of the `try` block in `load()` (the part from `const commits = await api.loadCommits(...)` through `appState.status = ...Loaded...`) with:
```ts
      const gc = await api.loadGraph(appState.repo, count, 0);
      appState.setGraphCommits(gc);
      appState.status = `Loaded ${gc.length} commit(s) across all branches.`;
```
(Leave the `isGitRepo` guard, the `loading`/`status` scaffolding, and the `catch`/`finally` exactly as they are. `useRange`/`range` no longer affect the all-branches graph — leave the inputs for now; they are cleaned up in Plan 3b.)

- [ ] **Step 2: Mount the graph.** In `src/routes/+page.svelte`:
(a) change the import `import CommitTable from "$lib/components/CommitTable.svelte";` to `import GraphHistory from "$lib/components/GraphHistory.svelte";`
(b) change `<CommitTable />` in the markup to `<GraphHistory />`
(c) add a browser-only sample auto-load so the graph shows without a Tauri backend. Add to the `<script>` block:
```ts
  import { onMount } from "svelte";
  import { appState } from "$lib/store.svelte";
  import { SAMPLE_GRAPH } from "$lib/graph/sample";

  onMount(() => {
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!inTauri && appState.graphCommits.length === 0) {
      appState.setGraphCommits(SAMPLE_GRAPH);
    }
  });
```

- [ ] **Step 3: Delete the superseded table.**
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
rm src/lib/components/CommitTable.svelte
```
Then confirm nothing else imports it:
`grep -rn "CommitTable" src/ || echo "no references"` — expected: `no references`.

- [ ] **Step 4: Type-check.**
Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -4`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit.**
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add -A
git commit -m "feat(graph): mount GraphHistory, load via load_graph, drop CommitTable" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Verify (controller)

Not a subagent task — the controller verifies visually.

- [ ] **Step 1:** `npm test` (vitest still 25) and `npm run check` (0/0).
- [ ] **Step 2:** Start the Claude_Preview dev server, navigate to `?translucent`, confirm the graph renders the sample (lanes, colored edges, the merge ring, the HEAD ring on `main`, ref badges including the `v1.2.0` tag and `fix/dates`/`origin/fix/dates`).
- [ ] **Step 3:** Toggle Curved/Angular and confirm the connector geometry changes; check console/logs for errors; take a screenshot for the user. Click a row and a ⌘/shift range and confirm selection still highlights and the edit tools still see the selection.
- [ ] **Step 4:** Resize / dark mode spot check.

---

## Self-review (completed by plan author)

- **Spec coverage:** §5.3 SVG lane gutter + dot styles (commit/merge ring/HEAD ring) + curved/angular renderers + per-lane palette ✓; §5.5 persisted `graphLineStyle` ✓ (per-branch color overrides deferred — they need the override UI, a Plan 3b item, noted). Sidebar/toolbar/bottom-detail (§5.4 full shell) intentionally deferred to Plan 3b.
- **Placeholder scan:** none — full code for every file.
- **Type consistency:** `setGraphCommits`/`graphToCommit` keep `appState.commits: Commit[]` populated so `EditTabs`/`ApplyPanel` keep working unchanged; `rows`/`graphLineStyle` getters match `GraphGutter`/`GraphHistory` reads; `RefDecoration.is_head`/`kind` field names match the Plan 2 TS mirror; `graphLineStyle` persists through the same Tauri Store used by `dateFormat`.
- **Browser verifiability:** the sample-data fallback renders the exact same `computeLanes`→path-builder→component pipeline the real data uses, so the preview is a faithful check of the rendering even without a git backend.

## Out of scope (Plan 3b / later)
3-pane shell chrome (sidebar ref tree from `list_refs`, toolbar, bottom `CommitDetail` panel), branch color-override UI, status bar from `repo_status`, virtualization for very large graphs, removing the now-vestigial custom-range input.
