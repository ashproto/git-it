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
  if (edge.kind === "branch") return `M${x1} ${y1} L${x1} ${my} L${x2} ${y2}`;
  return `M${x1} ${y1} L${x2} ${my} L${x2} ${y2}`;
}
