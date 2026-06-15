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
  descend: LaneSlot[];
  forks: number[];
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
