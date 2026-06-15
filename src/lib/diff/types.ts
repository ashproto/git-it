export type DiffLineKind = "context" | "add" | "del";
export type DiffLine = { kind: DiffLineKind; text: string; oldNo: number | null; newNo: number | null };
export type DiffHunk = { header: string; lines: DiffLine[] };
export type DiffFile = { oldPath: string; newPath: string; language: string; binary: boolean; hunks: DiffHunk[] };
export type ParsedDiff = { files: DiffFile[] };
