// Pure helpers for the PR "Files" tab: split a raw `gh pr diff` unified diff
// into per-file chunks with +/− counts. Dependency-free (mirrors the shape of
// src/lib/diff/split.ts but avoids pulling the full diff parser into tests).

export type PrDiffFile = {
  /** Display path — the new ("b/") side; the old side for deletions. */
  path: string;
  /** Set only when the file was renamed. */
  oldPath: string | null;
  /** The file's full patch text, including its own `diff --git` header. */
  patch: string;
  additions: number;
  deletions: number;
};

/** Strip a leading `a/` or `b/` git prefix from a diff header path. */
function stripPrefix(p: string): string {
  return p.replace(/^[ab]\//, "");
}

/** Best-effort parse of a `diff --git a/X b/Y` header line into [old, new]. */
function pathsFromHeader(header: string): { oldP: string; newP: string } {
  const rest = header.slice("diff --git ".length);
  // Paths with spaces are naturally bounded by " b/" — split on the LAST " b/".
  const i = rest.lastIndexOf(" b/");
  if (i >= 0) {
    return { oldP: stripPrefix(rest.slice(0, i)), newP: rest.slice(i + 3) };
  }
  return { oldP: stripPrefix(rest), newP: stripPrefix(rest) };
}

/**
 * Split a multi-file unified diff (`gh pr diff` output) into one entry per
 * file. Line-based scan: a new record starts on every `diff --git ` line.
 */
export function splitPatchByFile(patch: string): PrDiffFile[] {
  if (!patch || !patch.trim()) return [];

  const lines = patch.split("\n");
  const chunks: string[][] = [];
  let current: string[] | null = null;
  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      if (current) chunks.push(current);
      current = [line];
    } else if (current) {
      current.push(line);
    }
    // Preamble before the first `diff --git` is dropped (gh emits none).
  }
  if (current) chunks.push(current);

  const files: PrDiffFile[] = [];
  for (const chunk of chunks) {
    const header = pathsFromHeader(chunk[0]);
    let renameFrom: string | null = null;
    let renameTo: string | null = null;
    let minusPath: string | null = null; // from "--- " line
    let plusPath: string | null = null; // from "+++ " line
    let additions = 0;
    let deletions = 0;
    let inBody = false; // only count +/− after the first hunk header

    for (let i = 1; i < chunk.length; i++) {
      const line = chunk[i];
      if (line.startsWith("@@")) {
        inBody = true;
      } else if (!inBody) {
        if (line.startsWith("rename from ")) renameFrom = line.slice("rename from ".length);
        else if (line.startsWith("rename to ")) renameTo = line.slice("rename to ".length);
        else if (line.startsWith("--- ")) minusPath = line.slice(4);
        else if (line.startsWith("+++ ")) plusPath = line.slice(4);
        continue;
      }
      if (inBody) {
        if (line.startsWith("+") && !line.startsWith("+++")) additions++;
        else if (line.startsWith("-") && !line.startsWith("---")) deletions++;
      }
    }

    let path: string;
    let oldPath: string | null = null;
    if (renameFrom !== null && renameTo !== null) {
      // `rename from`/`rename to` carry the paths verbatim (no a/ b/ prefix).
      path = renameTo;
      oldPath = renameFrom;
    } else if (plusPath !== null || minusPath !== null) {
      const newSide = plusPath && plusPath !== "/dev/null" ? stripPrefix(plusPath) : null;
      const oldSide = minusPath && minusPath !== "/dev/null" ? stripPrefix(minusPath) : null;
      // New side wins; a deletion (+++ /dev/null) falls back to the old side.
      path = newSide ?? oldSide ?? header.newP;
    } else {
      // Binary stubs have no ---/+++ lines — fall back to the diff --git header.
      path = header.newP;
    }

    files.push({ path, oldPath, patch: chunk.join("\n"), additions, deletions });
  }
  return files;
}
