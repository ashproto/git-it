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

/** How far (as a fraction of the row height) each bezier control point sits from
 * its own endpoint. 0.5 put both controls at mid-row — a gentle, nearly diagonal
 * S that read as "hard curves" (user feedback). 0.8 pulls the tangents vertical
 * at both ends so the edge leaves and arrives straight, sweeping across in a
 * full round S with no elbow (approved "option C"). */
const CURVE_TENSION = 0.8;

/** Cubic-bezier connector (Fork style). `topY` is the y of the band's top row dot. */
export function curvedEdgePath(edge: Edge, topY: number, g: GeomConfig): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const a = g.rowHeight * CURVE_TENSION;
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
  return `M${x1} ${y1} L${x2} ${my} L${x2} ${y2}`;
}
