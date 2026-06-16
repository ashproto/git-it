import { parseDiff } from "./parse";

export type FileStatus = "added" | "deleted" | "modified" | "renamed";

export interface DiffFileEntry {
  /** Display path (new path, or old path for a deletion). */
  path: string;
  oldPath: string;
  newPath: string;
  status: FileStatus;
  binary: boolean;
  /** The raw unified-diff text for just this file — feed straight to DiffView. */
  patch: string;
}

/**
 * Split a multi-file unified diff into one entry per file. Each entry keeps the
 * file's own raw patch text (so it can be rendered by DiffView independently) plus
 * a derived status. Splitting on `diff --git` mirrors how parseDiff delimits files,
 * so the per-chunk re-parse always yields exactly one file. Pure + deterministic.
 */
export function splitDiffFiles(patch: string): DiffFileEntry[] {
  if (!patch) return [];

  const lines = patch.split("\n");
  const chunks: string[] = [];
  let current: string[] | null = null;
  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      if (current) chunks.push(current.join("\n"));
      current = [line];
    } else if (current) {
      current.push(line);
    }
    // Any preamble before the first `diff --git` (commit message etc.) is dropped;
    // commitDiff/diff return a pure diff, so this is normally a no-op.
  }
  if (current) chunks.push(current.join("\n"));

  const entries: DiffFileEntry[] = [];
  for (const text of chunks) {
    const file = parseDiff(text).files[0];
    if (!file) continue;
    const { oldPath, newPath, binary } = file;
    let status: FileStatus;
    if (newPath === "/dev/null") status = "deleted";
    else if (oldPath === "/dev/null" || oldPath === "") status = "added";
    else if (oldPath !== newPath) status = "renamed";
    else status = "modified";
    const path = status === "deleted" ? oldPath : newPath || oldPath;
    entries.push({ path, oldPath, newPath, status, binary, patch: text });
  }
  return entries;
}
