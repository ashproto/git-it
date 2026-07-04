# Desktop Feedback Batch 10 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship three Git It desktop fixes — a Local Changes file context menu (Open / Open With ▸ / Show in Finder), a fast-forward ref-ambiguity bug fix, and a directional "merge-in" commit-graph curve with the alternatives exposed in Settings.

**Architecture:** Frontend is SvelteKit 5 (runes) calling Tauri commands (`src/lib/api.ts` → `src-tauri/src/commands.rs` → `crates/git-core`). Graph edge geometry is pure TS in `src/lib/graph/paths.ts` (vitest). Settings persist through the store's dual localStorage + Tauri-Store pattern. File-open uses the already-installed `tauri-plugin-opener`; the "Open With" app list comes from a new macOS-only Tauri command using Cocoa `NSWorkspace`.

**Tech Stack:** Rust (git-core + Tauri shell, `objc2-app-kit`/`objc2-foundation`), SvelteKit 5 runes, `@tauri-apps/plugin-opener`, vitest, cargo test.

**Branch:** `desktop-batch9` (stacked). Execute tasks sequentially — the shared git index means no parallel tree mutations while an implementer is active.

**Full green gate before any task is "done":** `npm run check` (0 errors) + `npm test` + `cargo test`. Frontend-visible changes also get a browser-preview check. Tauri-only paths (real `apps_for_file`, `openPath`, `revealItemInDir`) are verified in the user's `tauri dev` live test, not preview.

---

## Task 1: Fast-forward ref-ambiguity fix (backend + plumbing)

**Why:** `git fetch origin next:next` uses unqualified refs. When a tag shares the branch's name (`refs/tags/next` alongside `refs/heads/next`), git resolves the ambiguous destination to the *tag* and rejects the update as `non-fast-forward` (exit 1). Fully-qualifying the refspec to `refs/heads/<remote_branch>:refs/heads/<local_branch>` targets the branch unambiguously. Confirmed live against `Resume-Designer`.

**Files:**
- Modify: `git-it/crates/git-core/src/ops.rs:87-100` (`fast_forward_branch` signature + refspec + messaging)
- Modify: `git-it/crates/git-core/src/ops.rs:351-402` (update existing test to new signature; add ambiguity regression test)
- Modify: `git-it/src-tauri/src/commands.rs:186-190` (command signature)
- Modify: `git-it/src/lib/api.ts:86-87` (binding)
- Modify: `git-it/src/lib/gitActions.ts:568-571` (action)
- Modify: `git-it/src/lib/components/Sidebar.svelte:100-106` (derive `remoteBranch`, pass through)

- [ ] **Step 1: Write the ambiguity regression test**

Add to the `tests` module in `crates/git-core/src/ops.rs` (after `fast_forward_branch_advances_non_current_branch`). It builds a clone with a local branch `feature` AND a tag `feature` (the ambiguity), advances the remote, and asserts the *branch* moves while the *tag* stays.

```rust
    #[test]
    fn fast_forward_branch_ignores_same_named_tag() {
        // origin = bare repo with main + feature @ c1.
        let bare = unique_dir("ff-tag-bare");
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

        // Clone; create a LOCAL feature branch tracking origin/feature, stay on main.
        let clone_path = unique_dir("ff-tag-clone");
        Command::new("git")
            .args(["clone", "-q", bare.to_str().unwrap(), clone_path.to_str().unwrap()])
            .output()
            .unwrap();
        let clone = TempRepo { path: clone_path.clone() };
        clone.git(&["config", "user.email", "t@example.com"]);
        clone.git(&["config", "user.name", "Tester"]);
        clone.git(&["branch", "--track", "feature", "origin/feature"]);

        // Create a TAG also named `feature` at the old tip → ref ambiguity.
        let old = clone.rev("refs/heads/feature");
        clone.git(&["tag", "feature", &old]);

        // Advance origin/feature by c2.
        upstream.git(&["checkout", "-q", "feature"]);
        upstream.commit("b.txt", "c2");
        upstream.git(&["push", "-q", "origin", "feature"]);
        upstream.git(&["checkout", "-q", "main"]);

        fetch(&clone.path, Some("origin")).unwrap();
        let target = clone.rev("refs/remotes/origin/feature");

        let res = fast_forward_branch(&clone.path, "feature", "origin", "feature");
        assert!(res.is_ok(), "ff failed (tag ambiguity not handled): {:?}", res);
        assert_eq!(clone.rev("refs/heads/feature"), target, "branch must advance to origin/feature");
        assert_eq!(clone.rev("refs/tags/feature"), old, "tag must be left untouched");

        let _ = fs::remove_dir_all(&bare);
        let _ = fs::remove_dir_all(&clone_path);
    }
```

- [ ] **Step 2: Update the existing test to the new 4-arg signature**

In `fast_forward_branch_advances_non_current_branch` (ops.rs ~line 393), change the call:

```rust
        // Fast-forward main to origin/main without checking it out.
        let res = fast_forward_branch(&clone.path, "main", "origin", "main");
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p git-core fast_forward_branch`
Expected: compile error (4-arg call vs 3-arg fn) — or, once the signature is added, `fast_forward_branch_ignores_same_named_tag` FAILS with a `non-fast-forward` rejection.

- [ ] **Step 4: Rewrite `fast_forward_branch` with a fully-qualified refspec + friendly diverged message**

Replace `crates/git-core/src/ops.rs:87-100`:

```rust
/// Fast-forward a LOCAL branch to its upstream tip WITHOUT checking it out.
///
/// Uses a FULLY-QUALIFIED refspec `refs/heads/<remote_branch>:refs/heads/<local_branch>`
/// so a tag (or any other ref) sharing the branch's name can't shadow the destination:
/// an unqualified `next:next` lets git resolve the ambiguous `next` to a same-named tag
/// and reject the update as non-fast-forward. Git refuses a genuine non-fast-forward, so
/// a diverged branch fails cleanly and the ref is left untouched. Callers only offer this
/// for non-current branches (git also refuses to fetch into the checked-out branch).
pub fn fast_forward_branch(
    repo: &Path,
    local_branch: &str,
    remote: &str,
    remote_branch: &str,
) -> Result<String, String> {
    if local_branch.is_empty() || local_branch.starts_with('-') {
        return Err(format!("Invalid branch: {}", local_branch));
    }
    if remote_branch.is_empty() || remote_branch.starts_with('-') {
        return Err(format!("Invalid remote branch: {}", remote_branch));
    }
    let refspec = format!("refs/heads/{}:refs/heads/{}", remote_branch, local_branch);
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["fetch", "--end-of-options", remote, &refspec]);
    match git_ops::run(&mut c) {
        Ok((o, e)) => Ok(format!("{}{}", o, e).trim().to_string()),
        Err(msg) => {
            if msg.contains("non-fast-forward") || msg.contains("[rejected]") {
                Err(format!(
                    "Can't fast-forward {}: it has diverged from {}/{}.",
                    local_branch, remote, remote_branch
                ))
            } else {
                Err(msg)
            }
        }
    }
}
```

- [ ] **Step 5: Run the Rust tests to verify they pass**

Run: `cargo test -p git-core fast_forward_branch`
Expected: both `fast_forward_branch_advances_non_current_branch` and `fast_forward_branch_ignores_same_named_tag` PASS.

- [ ] **Step 6: Thread the new parameter through the Tauri command**

Replace `src-tauri/src/commands.rs:186-190`:

```rust
// async: `git fetch <remote> refs/heads/<rb>:refs/heads/<lb>` is a network op (same gotcha as fetch).
#[tauri::command(async)]
pub fn fast_forward_branch(
    repo: String,
    branch: String,
    remote: String,
    remote_branch: String,
) -> Result<String, String> {
    ops::fast_forward_branch(&PathBuf::from(repo), &branch, &remote, &remote_branch)
}
```

- [ ] **Step 7: Update the TS binding**

Replace `src/lib/api.ts:86-87`:

```ts
  fastForwardBranch: (repo: string, branch: string, remote: string, remoteBranch: string) =>
    invoke<string>("fast_forward_branch", { repo, branch, remote, remoteBranch }),
```

- [ ] **Step 8: Update the gitActions wrapper**

Replace `src/lib/gitActions.ts:568-571`:

```ts
  fastForwardBranch: (branch: string, remote: string, remoteBranch: string) =>
    run(`Fast-forward ${branch} → ${remote}/${remoteBranch}`, () =>
      api.fastForwardBranch(appState.repo, branch, remote, remoteBranch),
    ),
```

- [ ] **Step 9: Derive `remoteBranch` from the upstream in the Sidebar menu**

Replace `src/lib/components/Sidebar.svelte:100-106`:

```svelte
        const upstream = detail?.upstream ?? null; // e.g. "origin/main"
        if (upstream) {
          const slash = upstream.indexOf("/");
          const remote = upstream.slice(0, slash);
          const remoteBranch = upstream.slice(slash + 1); // handles a renamed upstream
          items.push({
            label: `Fast-forward to ${remote}`,
            action: () => gitActions.fastForwardBranch(r.name, remote, remoteBranch),
          });
        }
```

- [ ] **Step 10: Run the full gate**

Run: `npm run check && npm test && cargo test -p git-core`
Expected: 0 check errors; vitest all pass; cargo git-core tests pass.

- [ ] **Step 11: Commit**

```bash
git add crates/git-core/src/ops.rs src-tauri/src/commands.rs src/lib/api.ts src/lib/gitActions.ts src/lib/components/Sidebar.svelte
git commit -m "fix(ff): fully-qualify fast-forward refspec so a same-named tag can't shadow the branch"
```

---

## Task 2: Merge-in curve geometry (default "hooked" / option A)

**Why:** `curvedEdgePath` ignores `edge.kind`, drawing a symmetric S for every lane change. Merge-in edges (engine `kind: "branch"` — a merge commit reaching to its second parent) should read directionally; branch-off edges (`kind: "merge"`) stay symmetric.

**Files:**
- Modify: `git-it/src/lib/graph/paths.ts:23-39` (kind-aware `curvedEdgePath` + `opts`; export types)
- Modify: `git-it/src/lib/graph/paths.test.ts:20-30` (update branch-kind case; add hooked/featureSide/symmetric cases)

- [ ] **Step 1: Write the failing tests**

Replace the `describe("curvedEdgePath", …)` block in `src/lib/graph/paths.test.ts:20-30` with (note the added import of the default-changing behavior). Config is `g = { laneWidth: 16, rowHeight: 30, offsetX: 12 }`, so lane 0 → x=12, lane 1 → x=28, y1=0, y2=30, h=30. With `tension 0.8`: `a = 24`, `b = h*(1-0.8) = 6`.

```ts
describe("curvedEdgePath", () => {
  it("draws a vertical line for a straight (same-lane) edge", () => {
    expect(curvedEdgePath(edge(0, 0, "straight"), 0, g)).toBe("M12 0 L12 30");
  });
  it("branch (merge-in) defaults to hooked: horizontal off the node, vertical into the feature lane", () => {
    expect(curvedEdgePath(edge(0, 1, "branch"), 0, g)).toBe("M12 0 C28 0 28 24 28 30");
  });
  it("branch (merge-in) featureSide: vertical down the node lane, bend near the feature dot", () => {
    expect(
      curvedEdgePath(edge(0, 1, "branch"), 0, g, { tension: 0.8, mergeInStyle: "featureSide" }),
    ).toBe("M12 0 C12 6 12 30 28 30");
  });
  it("branch (merge-in) symmetric matches the old S", () => {
    expect(
      curvedEdgePath(edge(0, 1, "branch"), 0, g, { tension: 0.8, mergeInStyle: "symmetric" }),
    ).toBe("M12 0 C12 24 28 6 28 30");
  });
  it("merge (branch-off) stays symmetric regardless of mergeInStyle", () => {
    expect(curvedEdgePath(edge(1, 0, "merge"), 0, g)).toBe("M28 0 C28 24 12 6 12 30");
    expect(
      curvedEdgePath(edge(1, 0, "merge"), 0, g, { tension: 0.8, mergeInStyle: "hooked" }),
    ).toBe("M28 0 C28 24 12 6 12 30");
  });
  it("honours a lower tension (0.55)", () => {
    // a = 30*0.55 = 16.5; b = 30*0.45 = 13.5
    expect(curvedEdgePath(edge(0, 1, "branch"), 0, g, { tension: 0.55, mergeInStyle: "hooked" })).toBe(
      "M12 0 C28 0 28 16.5 28 30",
    );
    expect(curvedEdgePath(edge(1, 0, "merge"), 0, g, { tension: 0.55, mergeInStyle: "hooked" })).toBe(
      "M28 0 C28 16.5 12 13.5 12 30",
    );
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/graph/paths.test.ts`
Expected: FAIL — the branch-kind default now expects the hooked path but the code still returns the symmetric S; and `curvedEdgePath` doesn't accept a 4th arg.

- [ ] **Step 3: Implement the kind-aware curve**

Replace `src/lib/graph/paths.ts:23-39` (the `CURVE_TENSION` const and `curvedEdgePath`):

```ts
/** How the merge-in edge (a merge commit reaching to its second parent) is drawn. */
export type MergeInStyle = "hooked" | "featureSide" | "symmetric";

export interface CurveOpts {
  /** Control-point offset as a fraction of row height (0..1). Higher = rounder. */
  tension: number;
  mergeInStyle: MergeInStyle;
}

const DEFAULT_CURVE: CurveOpts = { tension: 0.8, mergeInStyle: "hooked" };

/** Cubic-bezier connector (Fork style). `topY` is the y of the band's top row dot.
 *
 * `edge.kind === "branch"` is the merge-in edge (merge dot at top → second parent
 * lane at bottom); it renders directionally per `mergeInStyle`. Everything else
 * (a `merge`-kind branch-off, lane shifts) keeps the symmetric S so branch-offs
 * read as a gentle ease. Same-lane edges are always a straight vertical line. */
export function curvedEdgePath(
  edge: Edge,
  topY: number,
  g: GeomConfig,
  opts: CurveOpts = DEFAULT_CURVE,
): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const h = g.rowHeight;
  const a = h * opts.tension;
  if (edge.kind === "branch" && opts.mergeInStyle !== "symmetric") {
    const b = h * (1 - opts.tension);
    if (opts.mergeInStyle === "hooked") {
      // Feature lane (x2) runs straight; hook into the node (x1) at the top.
      return `M${x1} ${y1} C${x2} ${y1} ${x2} ${y2 - b} ${x2} ${y2}`;
    }
    // featureSide: node lane (x1) runs straight; bend into the feature lane near the bottom.
    return `M${x1} ${y1} C${x1} ${y1 + b} ${x1} ${y2} ${x2} ${y2}`;
  }
  return `M${x1} ${y1} C${x1} ${y1 + a} ${x2} ${y2 - a} ${x2} ${y2}`;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/lib/graph/paths.test.ts`
Expected: PASS (all cases).

- [ ] **Step 5: Commit**

```bash
git add src/lib/graph/paths.ts src/lib/graph/paths.test.ts
git commit -m "feat(graph): directional merge-in curve (hooked default) via edge.kind + curve opts"
```

---

## Task 3: Graph curve Settings (merge-in style + curviness) wired end to end

**Why:** Expose the merge-in style (default Hooked/A) and the curviness (default Balanced/0.8) as persisted settings, following the existing `graphLineStyle` pattern, and thread them into the graph.

**Files:**
- Modify: `git-it/src/lib/store.svelte.ts` (keys ~61-62; loadSync helpers ~159-168; state/hydrate/persist ~600-633; public getters/setters ~1340-1346)
- Modify: `git-it/src/lib/components/GraphGutter.svelte:14-33,56-57` (two new props → `curvedEdgePath`)
- Modify: `git-it/src/lib/components/GraphHistory.svelte:444-451` (pass the two props)
- Modify: `git-it/src/lib/components/SettingsPanel.svelte:47-61` (two new seg-rows)

- [ ] **Step 1: Add the storage keys**

After `src/lib/store.svelte.ts:62` (the `LINESTYLE_STORE_KEY` line), add:

```ts
const MERGEINSTYLE_KEY = "gitit.graphMergeInStyle.v1";
const MERGEINSTYLE_STORE_KEY = "graphMergeInStyle";

const CURVINESS_KEY = "gitit.graphCurviness.v1";
const CURVINESS_STORE_KEY = "graphCurviness";
```

- [ ] **Step 2: Import the MergeInStyle type**

At the top of `src/lib/store.svelte.ts:6` the graph barrel is already imported (`import { computeLanes, laneColor } from "./graph";`). Extend it to include the type:

```ts
import { computeLanes, laneColor, type MergeInStyle } from "./graph";
```

Ensure `MergeInStyle` is re-exported from the barrel `src/lib/graph/index.ts` (it already re-exports from `./paths`; add `MergeInStyle` to that export):

```ts
export { laneX, curvedEdgePath, angularEdgePath, LANE_WIDTH, OFFSET_X, type MergeInStyle } from "./paths";
```

- [ ] **Step 3: Add the loadSync helpers**

After `loadSyncLineStyle` (`src/lib/store.svelte.ts:168`), add:

```ts
function loadSyncMergeInStyle(): MergeInStyle {
  if (isTauri()) return "hooked";
  try {
    if (typeof localStorage === "undefined") return "hooked";
    const raw = localStorage.getItem(MERGEINSTYLE_KEY);
    return raw === "featureSide" || raw === "symmetric" ? raw : "hooked";
  } catch {
    return "hooked";
  }
}

// Curviness is one of three presets; anything else falls back to Balanced (0.8).
function loadSyncCurviness(): number {
  if (isTauri()) return 0.8;
  try {
    if (typeof localStorage === "undefined") return 0.8;
    const n = Number(localStorage.getItem(CURVINESS_KEY));
    return n === 0.55 || n === 0.8 || n === 0.95 ? n : 0.8;
  } catch {
    return 0.8;
  }
}
```

- [ ] **Step 4: Add state, async hydrate, and persist (mirror graphLineStyle)**

After the `persistLineStyle` function (`src/lib/store.svelte.ts:633`), add:

```ts
  let graphMergeInStyle = $state<MergeInStyle>(loadSyncMergeInStyle());
  let graphMergeInStyleTouched = false;
  let graphCurviness = $state<number>(loadSyncCurviness());
  let graphCurvinessTouched = false;

  const misHydrate = getStore();
  if (misHydrate) {
    misHydrate
      .then((store) => store.get<string>(MERGEINSTYLE_STORE_KEY))
      .then((saved) => {
        if (
          (saved === "hooked" || saved === "featureSide" || saved === "symmetric") &&
          !graphMergeInStyleTouched
        ) {
          graphMergeInStyle = saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load merge-in style", e));
  }

  const cvHydrate = getStore();
  if (cvHydrate) {
    cvHydrate
      .then((store) => store.get<number>(CURVINESS_STORE_KEY))
      .then((saved) => {
        if ((saved === 0.55 || saved === 0.8 || saved === 0.95) && !graphCurvinessTouched) {
          graphCurviness = saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load curviness", e));
  }

  function persistMergeInStyle() {
    const snapshot = graphMergeInStyle;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(MERGEINSTYLE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist merge-in style", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(MERGEINSTYLE_KEY, snapshot);
    } catch (e) {
      console.warn("[gte] could not persist merge-in style", e);
    }
  }

  function persistCurviness() {
    const snapshot = graphCurviness;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(CURVINESS_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist curviness", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(CURVINESS_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist curviness", e);
    }
  }
```

- [ ] **Step 5: Add public getters/setters**

After the `setGraphLineStyle` block (`src/lib/store.svelte.ts:1343-1347`), add inside the returned object:

```ts
    get graphMergeInStyle() {
      return graphMergeInStyle;
    },
    setGraphMergeInStyle(v: MergeInStyle) {
      graphMergeInStyleTouched = true;
      graphMergeInStyle = v;
      persistMergeInStyle();
    },
    get graphCurviness() {
      return graphCurviness;
    },
    setGraphCurviness(v: number) {
      graphCurvinessTouched = true;
      graphCurviness = v;
      persistCurviness();
    },
```

- [ ] **Step 6: Run the type gate**

Run: `npm run check`
Expected: 0 errors (the store compiles with the new getters/setters and the `MergeInStyle` import).

- [ ] **Step 7: Accept the two props in GraphGutter and pass them to curvedEdgePath**

In `src/lib/components/GraphGutter.svelte`, extend the props block (`:14-33`) — add `mergeInStyle` and `curviness` with defaults, and import `MergeInStyle`:

```svelte
  import {
    laneX,
    curvedEdgePath,
    angularEdgePath,
    laneColor,
    LANE_WIDTH,
    OFFSET_X,
    type RowLayout,
    type Edge,
    type GeomConfig,
    type MergeInStyle,
  } from "../graph";

  let {
    rows,
    heads,
    rowHeight = 30,
    lineStyle = "curved",
    mergeInStyle = "hooked",
    curviness = 0.8,
    renderStart = 0,
    renderEnd = undefined,
    colorOf,
  }: {
    rows: RowLayout[];
    heads: boolean[];
    rowHeight?: number;
    lineStyle?: "curved" | "angular";
    mergeInStyle?: MergeInStyle;
    curviness?: number;
    renderStart?: number;
    renderEnd?: number;
    colorOf?: (colorIndex: number) => string;
  } = $props();
```

Then replace `pathFor` (`:56-57`):

```svelte
  const pathFor = (edge: Edge, topY: number) =>
    lineStyle === "angular"
      ? angularEdgePath(edge, topY, g)
      : curvedEdgePath(edge, topY, g, { tension: curviness, mergeInStyle });
```

- [ ] **Step 8: Pass the store settings from GraphHistory**

In `src/lib/components/GraphHistory.svelte:444-451`, add the two props to the `<GraphGutter … />`:

```svelte
        <GraphGutter
          {rows}
          {heads}
          {rowHeight}
          lineStyle={appState.graphLineStyle}
          mergeInStyle={appState.graphMergeInStyle}
          curviness={appState.graphCurviness}
          renderStart={winStart}
          renderEnd={winEnd}
          colorOf={(idx) => appState.colorForIndex(idx)}
        />
```

- [ ] **Step 9: Add the Settings controls**

In `src/lib/components/SettingsPanel.svelte`, right after the "Graph lines" seg-row (ends at `:61`), add two seg-rows. The curviness buttons map presets to numbers; both rows disable when line style is Angular (curves don't apply).

```svelte
      <div class="seg-row">
        <span class="seg-label">Merge-in curve</span>
        <div class="seg" role="group" aria-label="Merge-in curve style">
          <button
            type="button"
            class:active={appState.graphMergeInStyle === "hooked"}
            disabled={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphMergeInStyle("hooked")}
            aria-pressed={appState.graphMergeInStyle === "hooked"}
          >Hooked</button><button
            type="button"
            class:active={appState.graphMergeInStyle === "featureSide"}
            disabled={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphMergeInStyle("featureSide")}
            aria-pressed={appState.graphMergeInStyle === "featureSide"}
          >Feature-side</button><button
            type="button"
            class:active={appState.graphMergeInStyle === "symmetric"}
            disabled={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphMergeInStyle("symmetric")}
            aria-pressed={appState.graphMergeInStyle === "symmetric"}
          >Symmetric</button>
        </div>
      </div>

      <div class="seg-row">
        <span class="seg-label">Curviness</span>
        <div class="seg" role="group" aria-label="Graph curviness">
          <button
            type="button"
            class:active={appState.graphCurviness === 0.55}
            disabled={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphCurviness(0.55)}
            aria-pressed={appState.graphCurviness === 0.55}
          >Subtle</button><button
            type="button"
            class:active={appState.graphCurviness === 0.8}
            disabled={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphCurviness(0.8)}
            aria-pressed={appState.graphCurviness === 0.8}
          >Balanced</button><button
            type="button"
            class:active={appState.graphCurviness === 0.95}
            disabled={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphCurviness(0.95)}
            aria-pressed={appState.graphCurviness === 0.95}
          >Sweeping</button>
        </div>
      </div>
```

- [ ] **Step 10: Run the full gate + browser preview**

Run: `npm run check && npm test`
Expected: 0 errors; all vitest pass.
Preview: start the dev server, open Settings, toggle Merge-in curve (Hooked/Feature-side/Symmetric) and Curviness (Subtle/Balanced/Sweeping); confirm the graph re-renders and the controls disable under Angular. (`preview_screenshot` for proof.)

- [ ] **Step 11: Commit**

```bash
git add src/lib/store.svelte.ts src/lib/graph/index.ts src/lib/components/GraphGutter.svelte src/lib/components/GraphHistory.svelte src/lib/components/SettingsPanel.svelte
git commit -m "feat(graph): merge-in style + curviness settings (persisted, curved-only)"
```

---

## Task 4: Context-menu submenu support (shared infra)

**Why:** "Open With ▸" needs a nested flyout. Today `MenuItem` is single-level. Add an optional `submenu` and render it, with zero change to existing single-level menus.

**Files:**
- Modify: `git-it/src/lib/contextMenu.svelte.ts` (`MenuItem` type)
- Modify: `git-it/src/lib/components/ContextMenu.svelte` (nested flyout render + styles)

- [ ] **Step 1: Add `submenu` to the MenuItem type**

In `src/lib/contextMenu.svelte.ts`, extend the `MenuItem` type:

```ts
export type MenuItem = {
  label?: string;
  action?: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  submenu?: MenuItem[];
};
```

- [ ] **Step 2: Render the flyout in ContextMenu.svelte**

Replace the `{#each contextMenu.items …}` block in `src/lib/components/ContextMenu.svelte:40-54` with a version that renders a nested menu for items carrying `submenu`. Submenus open on hover; a parent has no `action`. Flip to the left when the menu was opened in the right ~40% of the viewport.

```svelte
    {#each contextMenu.items as item, i (i)}
      {#if item.separator}
        <div class="sep" role="separator"></div>
      {:else if item.submenu}
        <div class="sub-wrap" class:flip={flipSub}>
          <button class="item has-sub" class:danger={item.danger} role="menuitem" disabled={item.disabled}>
            <span>{item.label}</span><span class="chev" aria-hidden="true">›</span>
          </button>
          <div class="menu submenu" role="menu" aria-label={item.label}>
            {#each item.submenu as sub, j (j)}
              {#if sub.separator}
                <div class="sep" role="separator"></div>
              {:else}
                <button
                  class="item"
                  class:danger={sub.danger}
                  role="menuitem"
                  disabled={sub.disabled}
                  onclick={() => choose(sub)}
                >
                  {sub.label}
                </button>
              {/if}
            {/each}
          </div>
        </div>
      {:else}
        <button
          class="item"
          class:danger={item.danger}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => choose(item)}
        >
          {item.label}
        </button>
      {/if}
    {/each}
```

Add the `flipSub` derived value in the `<script>` (after the `choose` function):

```ts
  // Open submenus leftward when the menu itself is near the right edge, so a
  // right-hand flyout can't run off-screen.
  const flipSub = $derived(
    typeof window !== "undefined" && contextMenu.x > window.innerWidth * 0.6,
  );
```

- [ ] **Step 3: Add the flyout styles**

Append to the `<style>` in `src/lib/components/ContextMenu.svelte`:

```css
  .sub-wrap {
    position: relative;
  }
  .item.has-sub {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .chev {
    color: var(--text-muted, var(--text));
    opacity: 0.7;
  }
  .submenu {
    position: absolute;
    top: -4px;
    left: 100%;
    margin-left: 2px;
    display: none;
  }
  .sub-wrap.flip .submenu {
    left: auto;
    right: 100%;
    margin-left: 0;
    margin-right: 2px;
  }
  .sub-wrap:hover > .submenu,
  .submenu:hover {
    display: block;
  }
```

- [ ] **Step 4: Verify existing menus still work (type gate + preview)**

Run: `npm run check && npm test`
Expected: 0 errors; all pass (no logic tests here — the change is presentational and additive).
Preview: right-click a commit / branch and confirm existing single-level menus render unchanged.

- [ ] **Step 5: Commit**

```bash
git add src/lib/contextMenu.svelte.ts src/lib/components/ContextMenu.svelte
git commit -m "feat(menu): optional submenu flyout in the shared context menu"
```

---

## Task 5: `apps_for_file` backend command (macOS LaunchServices)

**Why:** The "Open With ▸" submenu needs the list of apps that can open a given file. This is a macOS/Cocoa concern, so it lives in the desktop shell, not `git-core`.

**Files:**
- Create: `git-it/src-tauri/src/openwith.rs`
- Modify: `git-it/src-tauri/src/lib.rs` (add `mod openwith;` + register the command)
- Modify: `git-it/src-tauri/Cargo.toml` (add `objc2-app-kit`, `objc2-foundation`)
- Modify: `git-it/src/lib/api.ts` (binding)

- [ ] **Step 1: Add the Cocoa crates**

In `src-tauri/Cargo.toml`, under `[dependencies]`, add (align the versions to whatever `tauri-plugin-opener` already resolves in `Cargo.lock` — it depends on `objc2-app-kit` and `objc2-foundation`; run `cargo tree -p tauri-plugin-opener | grep objc2` to read the exact minor and pin the same):

```toml
objc2-app-kit = "0.2"
objc2-foundation = "0.2"
```

- [ ] **Step 2: Create the command**

Create `src-tauri/src/openwith.rs`. This queries `NSWorkspace` for the apps that can open the file URL and returns display name + `.app` path. Method/selector names may differ slightly by `objc2-app-kit` version (e.g. `URLsForApplicationsToOpenURL` vs a snake variant) — the implementer confirms against the resolved crate and `cargo build`.

```rust
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSFileManager, NSString, NSURL};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Serialize)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
}

/// Apps that can open `path`, as (display name, .app bundle path). macOS only.
/// Returns an empty list for a missing file or when nothing handles it.
#[tauri::command]
pub fn apps_for_file(path: String) -> Result<Vec<AppEntry>, String> {
    if path.is_empty() || path.starts_with('-') {
        return Err("invalid path".into());
    }
    if !Path::new(&path).exists() {
        return Ok(Vec::new());
    }
    // Safety: NSWorkspace / NSURL / NSFileManager are main-thread-agnostic reads;
    // Tauri command threads carry an autorelease pool for the call.
    unsafe {
        let ns_path = NSString::from_str(&path);
        let url = NSURL::fileURLWithPath(&ns_path);
        let ws = NSWorkspace::sharedWorkspace();
        let urls = ws.URLsForApplicationsToOpenURL(&url);
        let fm = NSFileManager::defaultManager();

        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<AppEntry> = Vec::new();
        for app_url in urls.iter() {
            let Some(app_path) = app_url.path() else { continue };
            let app_path = app_path.to_string();
            if !Path::new(&app_path).exists() || !seen.insert(app_path.clone()) {
                continue;
            }
            let disp = fm.displayNameAtPath(&NSString::from_str(&app_path)).to_string();
            let name = disp.strip_suffix(".app").unwrap_or(&disp).to_string();
            out.push(AppEntry { name, path: app_path });
        }
        Ok(out)
    }
}
```

- [ ] **Step 3: Register the module + command**

In `src-tauri/src/lib.rs`: add `mod openwith;` near the other module declarations, and add `commands` is already imported; add `openwith::apps_for_file` to the `tauri::generate_handler![…]` list (next to `commands::fast_forward_branch`, keeping the trailing comma style):

```rust
            commands::fast_forward_branch,
            openwith::apps_for_file,
```

- [ ] **Step 4: Write a Rust smoke test for apps_for_file**

Add a test module at the bottom of `src-tauri/src/openwith.rs`. A `.txt` in the temp dir should be openable by at least one app (TextEdit ships with macOS); a missing file returns empty; a leading-dash path errors.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lists_apps_for_a_text_file() {
        let p = std::env::temp_dir().join(format!("gte-openwith-{}.txt", std::process::id()));
        fs::write(&p, "hello").unwrap();
        let apps = apps_for_file(p.to_str().unwrap().to_string()).unwrap();
        assert!(!apps.is_empty(), "a .txt should have at least one handler app");
        assert!(apps.iter().all(|a| !a.name.is_empty() && Path::new(&a.path).exists()));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_empty_and_dash_is_rejected() {
        assert!(apps_for_file("/no/such/file.xyz".into()).unwrap().is_empty());
        assert!(apps_for_file("-x".into()).is_err());
    }
}
```

- [ ] **Step 5: Build + test the backend**

Run: `cargo test -p git-it apps_for_file` and `cargo build -p git-it`
Expected: both tests PASS and the shell compiles. If `URLsForApplicationsToOpenURL` doesn't resolve, adjust to the resolved `objc2-app-kit` method name (check `cargo doc -p objc2-app-kit --open` or the crate source) and re-run.

- [ ] **Step 6: Add the TS binding**

In `src/lib/api.ts`, add near the other invoke bindings (after `fastForwardBranch`):

```ts
  appsForFile: (path: string) =>
    invoke<{ name: string; path: string }[]>("apps_for_file", { path }),
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/openwith.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src/lib/api.ts
git commit -m "feat(desktop): apps_for_file command (NSWorkspace handlers for Open With)"
```

---

## Task 6: Local Changes file context menu (Open / Open With ▸ / Show in Finder)

**Why:** Wire the file actions into the existing per-row menu, using the opener plugin (Open, Show in Finder), the submenu infra (Task 4), and `apps_for_file` (Task 5).

**Files:**
- Modify: `git-it/src/lib/components/WorkingCopyView.svelte` (imports; abs-path helper; `onRowContext` becomes async and prepends the file actions)

- [ ] **Step 1: Import the opener plugin**

In `src/lib/components/WorkingCopyView.svelte`, add near the top imports (the component already imports `api`, `contextMenu`, and `gitActions`):

```ts
  import { revealItemInDir, openPath } from "@tauri-apps/plugin-opener";
  import type { MenuItem } from "../contextMenu.svelte";
```

- [ ] **Step 2: Add an abs-path helper**

Add near `basename` (`:31`):

```ts
  // Working-tree absolute path for a repo-relative file (repo paths are absolute,
  // forward-slashed on macOS).
  const absPath = (rel: string) => `${appState.repo.replace(/\/$/, "")}/${rel}`;
```

- [ ] **Step 3: Build the shared file-action items and prepend them in `onRowContext`**

Replace `onRowContext` (`src/lib/components/WorkingCopyView.svelte:163-198`). It becomes `async`: it awaits `apps_for_file` (fast local call) to populate the submenu before opening the menu. File actions are Tauri-only; in the browser they're skipped. A staged deletion (status starting with `D`) disables Open/Open With.

```ts
  async function onRowContext(
    e: MouseEvent,
    f: WorkingFile,
    section: "staged" | "unstaged" | "untracked",
  ) {
    e.preventDefault();
    if (selectedFile !== f.path) appState.setSelectedFile(f.path);

    // File actions (Tauri only). A staged deletion has no on-disk file.
    const fileItems: MenuItem[] = [];
    if (isTauri()) {
      const abs = absPath(f.path);
      const gone = f.status.startsWith("D");
      fileItems.push({ label: "Open", disabled: gone, action: () => void openPath(abs) });
      if (!gone) {
        let apps: { name: string; path: string }[] = [];
        try {
          apps = await api.appsForFile(abs);
        } catch {
          apps = [];
        }
        if (apps.length > 0) {
          fileItems.push({
            label: "Open With",
            submenu: apps.map((a) => ({ label: a.name, action: () => void openPath(abs, a.path) })),
          });
        }
      }
      fileItems.push({ label: "Show in Finder", action: () => void revealItemInDir(abs) });
      fileItems.push({ separator: true });
    }

    // Section-specific stage/discard items (unchanged behavior).
    let sectionItems: MenuItem[];
    if (section === "staged") {
      sectionItems = [{ label: "Unstage", action: () => gitActions.unstage([f.path]) }];
    } else if (section === "untracked") {
      sectionItems = [
        { label: "Stage", action: () => gitActions.stage([f.path]) },
        { separator: true },
        { label: "Remove", danger: true, action: () => gitActions.clean([f.path]) },
      ];
    } else {
      const removeItem = f.untracked
        ? { label: "Remove", danger: true, action: () => gitActions.clean([f.path]) }
        : { label: "Discard changes", danger: true, action: () => gitActions.discard([f.path]) };
      sectionItems = [
        { label: "Stage", action: () => gitActions.stage([f.path]) },
        { separator: true },
        removeItem,
      ];
    }

    contextMenu.openAt(e.clientX, e.clientY, [...fileItems, ...sectionItems]);
  }
```

- [ ] **Step 4: Run the full gate**

Run: `npm run check && npm test`
Expected: 0 errors; all vitest pass.

- [ ] **Step 5: Preview (browser sanity) + note Tauri-only verification**

Preview: right-click a file row — in the browser the file actions are skipped (no Tauri), so confirm the existing Stage/Discard menu still renders. The Open / Open With ▸ / Show in Finder actions are verified in the user's `tauri dev` live test.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/WorkingCopyView.svelte
git commit -m "feat(working-copy): Open / Open With / Show in Finder on Local Changes rows"
```

---

## Final verification (whole batch)

- [ ] Run the full gate on the tip: `npm run check` (0 errors) + `npm test` + `cargo test`.
- [ ] `npm run build` (production frontend) succeeds.
- [ ] Browser preview: graph curve settings toggle live; existing context menus intact.
- [ ] Dispatch a final whole-batch adversarial code review; fix Critical/Important before stopping.
- [ ] **STOP before merge.** Hand off for the user's `tauri dev`/`build` live test:
  - FF a branch whose remote has a same-named tag (or any behind branch) → advances with a clear message; a diverged branch → "Can't fast-forward … diverged".
  - Right-click a Local Changes file → Open, Open With ▸ (real app list), Show in Finder all work; a staged-deleted file disables Open/Open With.
  - Settings → Merge-in curve (Hooked/Feature-side/Symmetric) + Curviness (Subtle/Balanced/Sweeping) change the graph and persist across restart; both disable under Angular.
- [ ] After user approval: ff-merge `desktop-batch9` → `main`, push (gh credential-helper pattern), delete the branch.

---

## Notes / non-goals
- Open With is Local Changes only (not commit-detail file lists).
- No keyboard nav for the submenu (pointer-driven, matching existing menus).
- No force fast-forward of a diverged branch.
- `angularEdgePath` and the Curved/Angular toggle are unchanged.
- Curviness presets are `0.55 / 0.8 / 0.95` (kept < 1 so the curve stays monotone / no kink).
