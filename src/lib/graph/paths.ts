import type { Edge } from "./types";

// A same-lane edge (x1 === x2) always renders as a straight vertical segment
// regardless of its `kind`; `kind` only chooses the bend direction when the lane
// actually changes.

export interface GeomConfig {
  laneWidth: number;
  rowHeight: number;
  offsetX: number;
}

/** Lane geometry shared by the gutter SVG (GraphGutter) and the commit-row
 * layout (GraphHistory's gutter width). Single source of truth so the SVG lanes
 * and the table's left padding can never drift apart. */
export const LANE_WIDTH = 16;
export const OFFSET_X = 12;

export function laneX(lane: number, g: GeomConfig): number {
  return g.offsetX + lane * g.laneWidth;
}

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
    // b is the complement of a (h - a, not h*(1-tension)) to avoid float noise
    // like 30*(1-0.8) === 5.999999999999998.
    const b = h - a;
    if (opts.mergeInStyle === "hooked") {
      // Feature lane (x2) runs straight; hook into the node (x1) at the top.
      return `M${x1} ${y1} C${x2} ${y1} ${x2} ${y2 - b} ${x2} ${y2}`;
    }
    // featureSide: node lane (x1) runs straight; bend into the feature lane near the bottom.
    return `M${x1} ${y1} C${x1} ${y1 + b} ${x1} ${y2} ${x2} ${y2}`;
  }
  return `M${x1} ${y1} C${x1} ${y1 + a} ${x2} ${y2 - a} ${x2} ${y2}`;
}

/** Right-angle connector (SourceTree style). */
export function angularEdgePath(edge: Edge, topY: number, g: GeomConfig): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const my = y1 + g.rowHeight / 2;
  if (edge.kind === "branch") return `M${x1} ${y1} L${x1} ${my} L${x2} ${y2}`;
  const s = Math.min(6, g.rowHeight / 3); // stub; 2s ≤ rowHeight ⇒ y1+s ≤ y2-s (no crossing)
  return `M${x1} ${y1} L${x1} ${y1 + s} L${x2} ${y2 - s} L${x2} ${y2}`;
}
