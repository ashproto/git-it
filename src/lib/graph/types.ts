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
