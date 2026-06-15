# Phase 1 · Plan 1 — Commit graph lane engine (implementation plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure, fully-tested TypeScript engine that turns a list of commits (with parents) into a renderable lane/edge graph layout, plus the color resolver and SVG path builders — the linchpin every later graph piece depends on.

**Architecture:** Three dependency-free modules under `src/lib/graph/`: `lanes.ts` (lane assignment via a two-pass snapshot algorithm), `colors.ts` (per-lane palette + per-branch override resolution), `paths.ts` (curved + angular SVG path strings from edges). No DOM, no Tauri, no Svelte — so each is unit-tested with Vitest. The shell UI (Plan 3) and Rust data layer (Plan 2) are separate plans; this plan ships a verified library only.

**Tech Stack:** TypeScript 5.6, Vitest (added here), no other runtime deps.

**Reference spec:** `docs/specs/2026-06-14-git-graph-client-design.md` §5.2 (lane engine), §5.3 (rendering/colors).

**Scope note:** This is plan 1 of 3 for Phase 1. Plan 2 = Rust data layer (`load_graph`/`list_refs`/`repo_status`). Plan 3 = shell UI + state + integration. This plan has no dependency on the other two and can be executed immediately.

---

## File structure

| File | Responsibility |
|---|---|
| `git-it/vitest.config.ts` | Vitest config (node env, `src/**/*.test.ts`) — created Task 1 |
| `git-it/src/lib/graph/types.ts` | Shared engine types: `LaneCommit`, `Edge`, `RowLayout` |
| `git-it/src/lib/graph/colors.ts` | `LANE_PALETTE`, `laneColor()` resolver |
| `git-it/src/lib/graph/colors.test.ts` | Tests for the color resolver |
| `git-it/src/lib/graph/lanes.ts` | `computeLanes()` — the two-pass lane algorithm |
| `git-it/src/lib/graph/lanes.test.ts` | Tests: linear, branch, merge, octopus, multi-root, color stability |
| `git-it/src/lib/graph/paths.ts` | `laneX()`, `curvedEdgePath()`, `angularEdgePath()` |
| `git-it/src/lib/graph/paths.test.ts` | Tests for path strings (exact output) |
| `git-it/src/lib/graph/index.ts` | Barrel re-export |

All commands below run from `git-it/` (the npm project root) unless noted.

---

## Task 1: Project setup — git, Vitest, smoke test

**Files:**
- Create: `git-it/vitest.config.ts`
- Modify: `git-it/package.json` (scripts + devDependency)
- Create: `git-it/src/lib/graph/smoke.test.ts` (temporary; deleted at end of task)

- [ ] **Step 1: Initialize git at the repo root and make a baseline commit**

The project is not yet under version control; the TDD loop needs commits. Run from the repo root `/Users/ashshah/Projects/GIT-GUI`:

```bash
cd /Users/ashshah/Projects/GIT-GUI
git init
printf '%s\n' 'node_modules/' 'dist/' '.DS_Store' 'src-tauri/target/' 'build/' '.svelte-kit/' > .gitignore
git add -A
git commit -m "chore: baseline commit before git-client work"
```

Expected: a first commit containing the existing app. (If `git init` reports an existing repo, skip straight to verifying `git status` is clean enough to proceed.)

- [ ] **Step 2: Add Vitest as a dev dependency**

Run from `git-it/`:

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm install -D vitest@^2
```

Expected: `vitest` appears under `devDependencies` in `package.json`.

- [ ] **Step 3: Create the Vitest config**

Create `git-it/vitest.config.ts`:

```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
```

- [ ] **Step 4: Add test scripts to package.json**

In `git-it/package.json`, add these two entries to the `"scripts"` object (keep existing scripts):

```json
"test": "vitest run",
"test:watch": "vitest"
```

- [ ] **Step 5: Add a temporary smoke test**

Create `git-it/src/lib/graph/smoke.test.ts`:

```ts
import { describe, it, expect } from "vitest";

describe("vitest smoke", () => {
  it("runs", () => {
    expect(1 + 1).toBe(2);
  });
});
```

- [ ] **Step 6: Run the smoke test**

Run: `npm test`
Expected: PASS — 1 test passed, runner works.

- [ ] **Step 7: Delete the smoke test and commit setup**

```bash
rm src/lib/graph/smoke.test.ts
git add -A
git commit -m "chore: add vitest test runner"
```

---

## Task 2: Engine types + color resolver

**Files:**
- Create: `git-it/src/lib/graph/types.ts`
- Create: `git-it/src/lib/graph/colors.test.ts`
- Create: `git-it/src/lib/graph/colors.ts`

- [ ] **Step 1: Write the color resolver tests (failing)**

Create `git-it/src/lib/graph/colors.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { LANE_PALETTE, laneColor } from "./colors";

describe("laneColor", () => {
  it("returns the palette color for a color index", () => {
    expect(laneColor(0, null, {})).toBe(LANE_PALETTE[0]);
    expect(laneColor(2, null, {})).toBe(LANE_PALETTE[2]);
  });

  it("cycles through the palette when the index exceeds its length", () => {
    expect(laneColor(LANE_PALETTE.length, null, {})).toBe(LANE_PALETTE[0]);
    expect(laneColor(LANE_PALETTE.length + 1, null, {})).toBe(LANE_PALETTE[1]);
  });

  it("prefers a per-branch override when one exists for the branch", () => {
    expect(laneColor(0, "main", { main: "#ff0000" })).toBe("#ff0000");
  });

  it("ignores overrides for other branches", () => {
    expect(laneColor(0, "feature", { main: "#ff0000" })).toBe(LANE_PALETTE[0]);
  });

  it("falls back to palette when branch name is null", () => {
    expect(laneColor(1, null, { main: "#ff0000" })).toBe(LANE_PALETTE[1]);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- colors`
Expected: FAIL — cannot resolve `./colors`.

- [ ] **Step 3: Create the engine types**

Create `git-it/src/lib/graph/types.ts`:

```ts
export interface LaneCommit {
  sha: string;
  parents: string[];
}

export type EdgeKind = "straight" | "merge" | "branch";

export interface Edge {
  fromLane: number;
  toLane: number;
  colorIndex: number;
  kind: EdgeKind;
}

export interface RowLayout {
  sha: string;
  lane: number;
  colorIndex: number;
  isMerge: boolean;
  edges: Edge[];
  width: number;
}
```

- [ ] **Step 4: Implement the color resolver**

Create `git-it/src/lib/graph/colors.ts`:

```ts
export const LANE_PALETTE: string[] = [
  "#378ADD",
  "#1D9E75",
  "#D85A30",
  "#7F77DD",
  "#BA7517",
  "#D4537E",
  "#639922",
  "#5F5E5A",
];

export function laneColor(
  colorIndex: number,
  branchName: string | null,
  overrides: Record<string, string>,
): string {
  if (branchName && overrides[branchName]) return overrides[branchName];
  return LANE_PALETTE[colorIndex % LANE_PALETTE.length];
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm test -- colors`
Expected: PASS — 5 tests.

- [ ] **Step 6: Commit**

```bash
git add src/lib/graph/types.ts src/lib/graph/colors.ts src/lib/graph/colors.test.ts
git commit -m "feat(graph): add engine types and lane color resolver"
```

---

## Task 3: Lane algorithm — write the full test suite (failing)

The lane algorithm is holistic (it can't be grown one branch at a time without rewriting), so we write a representative test suite covering every §5.2 edge case first, then implement the whole function in Task 4 and iterate to green. Each fixture's expected values were hand-derived from the algorithm in the spec.

**Files:**
- Create: `git-it/src/lib/graph/lanes.test.ts`

- [ ] **Step 1: Write the lane test suite**

Create `git-it/src/lib/graph/lanes.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { computeLanes } from "./lanes";
import type { LaneCommit } from "./types";

const c = (sha: string, parents: string[] = []): LaneCommit => ({ sha, parents });

describe("computeLanes — linear history", () => {
  it("places a straight chain in lane 0", () => {
    const rows = computeLanes([c("A", ["B"]), c("B", ["C"]), c("C", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 0]);
    expect(rows.map((r) => r.colorIndex)).toEqual([0, 0, 0]);
    expect(rows.map((r) => r.isMerge)).toEqual([false, false, false]);
    expect(rows[0].edges).toEqual([
      { fromLane: 0, toLane: 0, colorIndex: 0, kind: "straight" },
    ]);
    expect(rows[2].edges).toEqual([]);
  });
});

describe("computeLanes — branch point", () => {
  it("converges a second tip into the shared parent's lane", () => {
    // X and Y are both tips sharing parent Z.
    const rows = computeLanes([c("X", ["Z"]), c("Y", ["Z"]), c("Z", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 1, 0]);
    // Y's band converges lane 1 into lane 0 at Z, lane 0 continues straight.
    expect(rows[1].edges).toContainEqual({
      fromLane: 1,
      toLane: 0,
      colorIndex: 1,
      kind: "merge",
    });
    expect(rows[1].edges).toContainEqual({
      fromLane: 0,
      toLane: 0,
      colorIndex: 0,
      kind: "straight",
    });
  });
});

describe("computeLanes — merge commit", () => {
  it("forks at the merge and reconverges at the base", () => {
    // M merges A (first parent) and B (second parent); both descend to base C.
    const rows = computeLanes([
      c("M", ["A", "B"]),
      c("A", ["C"]),
      c("B", ["C"]),
      c("C", []),
    ]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 1, 0]);
    expect(rows[0].isMerge).toBe(true);
    // M's color stays on its first-parent chain (M and A share color 0); B is color 1.
    expect(rows[0].colorIndex).toBe(0);
    expect(rows[1].colorIndex).toBe(0);
    expect(rows[2].colorIndex).toBe(1);
    // M's band: straight down lane 0 plus a branch out to lane 1.
    expect(rows[0].edges).toContainEqual({
      fromLane: 0,
      toLane: 1,
      colorIndex: 1,
      kind: "branch",
    });
    expect(rows[0].edges).toContainEqual({
      fromLane: 0,
      toLane: 0,
      colorIndex: 0,
      kind: "straight",
    });
    // B's band: lane 1 converges into lane 0 at C.
    expect(rows[2].edges).toContainEqual({
      fromLane: 1,
      toLane: 0,
      colorIndex: 1,
      kind: "merge",
    });
  });
});

describe("computeLanes — octopus merge", () => {
  it("forks to three lanes and reconverges", () => {
    const rows = computeLanes([
      c("O", ["A", "B", "C"]),
      c("A", ["D"]),
      c("B", ["D"]),
      c("C", ["D"]),
      c("D", []),
    ]);
    expect(rows[0].isMerge).toBe(true);
    const branchEdges = rows[0].edges.filter((e) => e.kind === "branch");
    expect(branchEdges).toHaveLength(2); // forks to the 2nd and 3rd parents
    const mergeEdges = rows[3].edges.filter((e) => e.kind === "merge");
    expect(mergeEdges).toHaveLength(2); // B and C reconverge into D's lane at C's row
  });
});

describe("computeLanes — multiple roots", () => {
  it("recycles a freed lane and assigns a fresh color", () => {
    const rows = computeLanes([c("P", []), c("Q", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0]);
    expect(rows[0].colorIndex).toBe(0);
    expect(rows[1].colorIndex).toBe(1);
    expect(rows[0].edges).toEqual([]);
    expect(rows[1].edges).toEqual([]);
  });
});

describe("computeLanes — width", () => {
  it("reports the column count spanning each row's band", () => {
    const rows = computeLanes([
      c("M", ["A", "B"]),
      c("A", ["C"]),
      c("B", ["C"]),
      c("C", []),
    ]);
    expect(rows[0].width).toBe(2);
    expect(rows[3].width).toBe(1);
  });
});
```

- [ ] **Step 2: Run the suite to verify it fails**

Run: `npm test -- lanes`
Expected: FAIL — cannot resolve `./lanes` (`computeLanes` not defined).

- [ ] **Step 3: Commit the failing tests**

```bash
git add src/lib/graph/lanes.test.ts
git commit -m "test(graph): add lane algorithm spec suite"
```

---

## Task 4: Implement `computeLanes`

**Files:**
- Create: `git-it/src/lib/graph/lanes.ts`

- [ ] **Step 1: Implement the two-pass lane algorithm**

Create `git-it/src/lib/graph/lanes.ts`:

```ts
import type { Edge, LaneCommit, RowLayout } from "./types";

interface LaneSlot {
  sha: string | null;
  color: number;
}

interface RowRec {
  sha: string;
  lane: number;
  color: number;
  isMerge: boolean;
  descend: LaneSlot[]; // snapshot of active lanes AFTER this commit's parents are set up
  forks: number[]; // lane indices newly created for 2nd+ (merge) parents
}

/**
 * Turn a topologically ordered commit list (children before parents, as from
 * `git log --topo-order`) into per-row lane/edge layout. Pure + deterministic.
 */
export function computeLanes(commits: LaneCommit[]): RowLayout[] {
  const lanes: LaneSlot[] = [];
  let nextColor = 0;

  const firstFree = (): number => {
    for (let i = 0; i < lanes.length; i++) {
      if (lanes[i].sha === null) return i;
    }
    lanes.push({ sha: null, color: 0 });
    return lanes.length - 1;
  };

  // Pass 1 — assign lanes, snapshot descending state per row.
  const recs: RowRec[] = [];
  for (const commit of commits) {
    const incoming: number[] = [];
    for (let i = 0; i < lanes.length; i++) {
      if (lanes[i].sha === commit.sha) incoming.push(i);
    }

    let lane: number;
    let color: number;
    if (incoming.length > 0) {
      lane = incoming[0];
      color = lanes[lane].color;
      for (let k = 1; k < incoming.length; k++) {
        lanes[incoming[k]] = { sha: null, color: 0 };
      }
    } else {
      lane = firstFree();
      color = nextColor++;
      lanes[lane] = { sha: commit.sha, color };
    }

    const forks: number[] = [];
    if (commit.parents.length === 0) {
      lanes[lane] = { sha: null, color: 0 };
    } else {
      lanes[lane] = { sha: commit.parents[0], color };
      for (let p = 1; p < commit.parents.length; p++) {
        const parent = commit.parents[p];
        let target = lanes.findIndex((s) => s.sha === parent);
        if (target === -1) {
          target = firstFree();
          lanes[target] = { sha: parent, color: nextColor++ };
        }
        forks.push(target);
      }
    }

    recs.push({
      sha: commit.sha,
      lane,
      color,
      isMerge: commit.parents.length > 1,
      descend: lanes.map((s) => ({ ...s })),
      forks,
    });
  }

  // Pass 2 — derive each row's below-band edges from consecutive snapshots.
  const rows: RowLayout[] = [];
  for (let r = 0; r < recs.length; r++) {
    const rec = recs[r];
    const next = recs[r + 1];
    const forkSet = new Set(rec.forks);
    const edges: Edge[] = [];

    for (let k = 0; k < rec.descend.length; k++) {
      const slot = rec.descend[k];
      if (slot.sha === null) continue;
      if (forkSet.has(k)) {
        edges.push({ fromLane: rec.lane, toLane: k, colorIndex: slot.color, kind: "branch" });
      } else if (next && slot.sha === next.sha) {
        edges.push({
          fromLane: k,
          toLane: next.lane,
          colorIndex: slot.color,
          kind: k === next.lane ? "straight" : "merge",
        });
      } else {
        edges.push({ fromLane: k, toLane: k, colorIndex: slot.color, kind: "straight" });
      }
    }

    let maxLane = rec.lane;
    for (const e of edges) maxLane = Math.max(maxLane, e.fromLane, e.toLane);
    for (let k = 0; k < rec.descend.length; k++) {
      if (rec.descend[k].sha !== null) maxLane = Math.max(maxLane, k);
    }

    rows.push({
      sha: rec.sha,
      lane: rec.lane,
      colorIndex: rec.color,
      isMerge: rec.isMerge,
      edges,
      width: maxLane + 1,
    });
  }

  return rows;
}
```

- [ ] **Step 2: Run the suite to verify it passes**

Run: `npm test -- lanes`
Expected: PASS — all describe blocks green. If any fail, compare the actual `edges`/`lane` arrays against the fixtures and fix `lanes.ts` (not the tests) until green.

- [ ] **Step 3: Run the whole test run + type check**

Run: `npm test && npm run check`
Expected: all tests pass; svelte-check reports 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/graph/lanes.ts
git commit -m "feat(graph): implement two-pass lane assignment engine"
```

---

## Task 5: SVG path builders (curved + angular)

**Files:**
- Create: `git-it/src/lib/graph/paths.test.ts`
- Create: `git-it/src/lib/graph/paths.ts`

- [ ] **Step 1: Write the path-builder tests (failing)**

Create `git-it/src/lib/graph/paths.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { laneX, curvedEdgePath, angularEdgePath } from "./paths";
import type { Edge } from "./types";

const g = { laneWidth: 16, rowHeight: 30, offsetX: 12 };
const edge = (fromLane: number, toLane: number, kind: Edge["kind"]): Edge => ({
  fromLane,
  toLane,
  colorIndex: 0,
  kind,
});

describe("laneX", () => {
  it("maps a lane index to an x coordinate", () => {
    expect(laneX(0, g)).toBe(12);
    expect(laneX(2, g)).toBe(44);
  });
});

describe("curvedEdgePath", () => {
  it("draws a vertical line for a straight (same-lane) edge", () => {
    expect(curvedEdgePath(edge(0, 0, "straight"), 0, g)).toBe("M12 0 L12 30");
  });
  it("draws a bezier for a lane change", () => {
    expect(curvedEdgePath(edge(0, 1, "branch"), 0, g)).toBe("M12 0 C12 15 28 15 28 30");
  });
});

describe("angularEdgePath", () => {
  it("draws a vertical line for a straight edge", () => {
    expect(angularEdgePath(edge(0, 0, "straight"), 0, g)).toBe("M12 0 L12 30");
  });
  it("forks late for a branch edge (down then across)", () => {
    expect(angularEdgePath(edge(0, 1, "branch"), 0, g)).toBe("M12 0 L12 15 L28 30");
  });
  it("converges early for a merge edge (across then down)", () => {
    expect(angularEdgePath(edge(1, 0, "merge"), 0, g)).toBe("M28 0 L12 15 L12 30");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm test -- paths`
Expected: FAIL — cannot resolve `./paths`.

- [ ] **Step 3: Implement the path builders**

Create `git-it/src/lib/graph/paths.ts`:

```ts
import type { Edge } from "./types";

export interface GeomConfig {
  laneWidth: number;
  rowHeight: number;
  offsetX: number;
}

export function laneX(lane: number, g: GeomConfig): number {
  return g.offsetX + lane * g.laneWidth;
}

/** Cubic-bezier connector (Fork style). `topY` is the y of the band's top row dot. */
export function curvedEdgePath(edge: Edge, topY: number, g: GeomConfig): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const my = y1 + g.rowHeight / 2;
  return `M${x1} ${y1} C${x1} ${my} ${x2} ${my} ${x2} ${y2}`;
}

/** Right-angle connector (SourceTree style). */
export function angularEdgePath(edge: Edge, topY: number, g: GeomConfig): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const my = y1 + g.rowHeight / 2;
  // Branch forks out of the top commit: descend, then cross to the new lane.
  if (edge.kind === "branch") return `M${x1} ${y1} L${x1} ${my} L${x2} ${y2}`;
  // Merge/other converge into the bottom commit: cross early, then descend.
  return `M${x1} ${y1} L${x2} ${my} L${x2} ${y2}`;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm test -- paths`
Expected: PASS — all path tests green.

- [ ] **Step 5: Commit**

```bash
git add src/lib/graph/paths.ts src/lib/graph/paths.test.ts
git commit -m "feat(graph): add curved and angular SVG path builders"
```

---

## Task 6: Barrel export + full verification

**Files:**
- Create: `git-it/src/lib/graph/index.ts`

- [ ] **Step 1: Create the barrel re-export**

Create `git-it/src/lib/graph/index.ts`:

```ts
export type { LaneCommit, Edge, EdgeKind, RowLayout } from "./types";
export { computeLanes } from "./lanes";
export { LANE_PALETTE, laneColor } from "./colors";
export { laneX, curvedEdgePath, angularEdgePath } from "./paths";
export type { GeomConfig } from "./paths";
```

- [ ] **Step 2: Run the full test + type-check gate**

Run: `npm test && npm run check`
Expected: all tests pass (colors, lanes, paths); svelte-check 0 errors.

- [ ] **Step 3: Commit**

```bash
git add src/lib/graph/index.ts
git commit -m "feat(graph): export lane engine barrel"
```

---

## Self-review (completed by plan author)

- **Spec coverage:** §5.2 edge cases — linear ✓ (Task 3), branch point ✓, merge ✓, octopus ✓, multiple roots ✓, color stability on first-parent chain ✓ (merge test), lane recycling ✓ (multi-root test), width/sizing ✓. §5.3 — curved + angular renderers ✓ (Task 5), palette + per-branch override ✓ (Task 2). Paging-continuity is satisfied structurally: `computeLanes` is pure over the full loaded set, so "load more" re-runs it on the larger set (no incremental drift) — exercised once the data layer exists in Plan 2.
- **Placeholder scan:** none — every code/test/command step is concrete.
- **Type consistency:** `LaneCommit`/`Edge`/`RowLayout` defined in Task 2 and used unchanged in Tasks 3–6; `computeLanes`, `laneColor`, `laneX`, `curvedEdgePath`, `angularEdgePath`, `GeomConfig` names match across tests, implementations, and the barrel.

## Out of scope for this plan (later Phase 1 plans)

- Rust `load_graph`/`list_refs`/`repo_status` + types mirror (Plan 2).
- `GraphGutter`/`CommitRow`/`GraphHistory`/`Sidebar`/`Toolbar`/`CommitDetail`/`AppShell`, state modules, settings UI, glass tokens, virtualization, and wiring `computeLanes` to live data (Plan 3).
