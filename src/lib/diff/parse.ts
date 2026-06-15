import type { DiffFile, DiffHunk, DiffLine, DiffLineKind, ParsedDiff } from "./types";

// ─── language detection ───────────────────────────────────────────────────────

const EXT_LANG: Record<string, string> = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  rs: "rust",
  py: "python",
  json: "json",
  html: "html",
  htm: "html",
  css: "css",
  svelte: "svelte",
  md: "markdown",
  mdx: "markdown",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  yml: "yaml",
  yaml: "yaml",
  toml: "toml",
  go: "go",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  java: "java",
  rb: "ruby",
  php: "php",
  sql: "sql",
  diff: "diff",
};

function langFromPath(path: string): string {
  const base = path.split("/").pop() ?? path;
  const dot = base.lastIndexOf(".");
  if (dot === -1) return "text";
  const ext = base.slice(dot + 1).toLowerCase();
  return EXT_LANG[ext] ?? "text";
}

// ─── path extraction ──────────────────────────────────────────────────────────

/** Strip the `a/` or `b/` prefix that git adds, unless it's `/dev/null`. */
function stripGitPrefix(p: string): string {
  if (p === "/dev/null") return p;
  // git uses exactly `a/` / `b/` prefixes by default
  if (p.startsWith("a/") || p.startsWith("b/")) return p.slice(2);
  return p;
}

// ─── hunk header parsing ──────────────────────────────────────────────────────

/** Parse `@@ -a[,b] +c[,d] @@` returning [oldStart, newStart] (1-based). */
function parseHunkHeader(header: string): [number, number] {
  const m = header.match(/^@@\s+-(\d+)(?:,\d+)?\s+\+(\d+)(?:,\d+)?\s+@@/);
  if (!m) return [1, 1];
  return [parseInt(m[1], 10), parseInt(m[2], 10)];
}

// ─── main parser ──────────────────────────────────────────────────────────────

/**
 * Parse a raw unified diff (as returned by `git diff` / `git show`) into a
 * structured `ParsedDiff`. Never throws — malformed or empty input yields an
 * empty `files` array (or files with empty hunks).
 */
export function parseDiff(patch: string): ParsedDiff {
  if (!patch || !patch.trim()) return { files: [] };

  // Split on `diff --git` boundary. The first element before the first
  // `diff --git` line is typically empty — skip it.
  const blocks = patch.split(/(?=^diff --git )/m);
  const files: DiffFile[] = [];

  for (const block of blocks) {
    if (!block.trim()) continue;

    // Must start with `diff --git` (the split guarantees this for non-first blocks)
    if (!block.startsWith("diff --git ")) continue;

    // --- derive old/new paths ---
    // Seed from the `diff --git a/<old> b/<new>` header line so that pure-rename
    // (100% similarity) and mode-only diffs — which have no `---`/`+++` lines —
    // still yield non-empty paths. `---`/`+++` override if present.
    let oldPath = "";
    let newPath = "";
    let binary = false;
    const hunks: DiffHunk[] = [];

    const lines = block.split("\n");
    let i = 0;

    // Extract seed paths from the `diff --git a/... b/...` header.
    // The header has the form: diff --git a/<oldPath> b/<newPath>
    // Because paths may contain spaces, we cannot simply split on space; however,
    // since both sides are prefixed we find the ` b/` boundary from the right.
    const diffGitHeader = lines[0]; // guaranteed to start with "diff --git "
    const headerRest = diffGitHeader.slice("diff --git ".length); // "a/<old> b/<new>"
    // Walk right-to-left: find the last " b/" that splits the string correctly.
    let splitIdx = -1;
    for (let k = headerRest.length - 3; k >= 2; k--) {
      if (headerRest[k] === " " && headerRest[k + 1] === "b" && headerRest[k + 2] === "/") {
        splitIdx = k;
        break;
      }
    }
    if (splitIdx !== -1) {
      const seedOld = headerRest.slice(0, splitIdx); // "a/<old>"
      const seedNew = headerRest.slice(splitIdx + 1); // "b/<new>"
      oldPath = stripGitPrefix(seedOld);
      newPath = stripGitPrefix(seedNew);
    }

    // Parse file header lines (before the first `@@`), overriding the seed paths
    // with authoritative `---`/`+++`, `rename from`/`rename to`, or `Binary files`.
    while (i < lines.length && !lines[i].startsWith("@@")) {
      const line = lines[i];

      if (line.startsWith("--- ")) {
        oldPath = stripGitPrefix(line.slice(4).trim());
      } else if (line.startsWith("+++ ")) {
        newPath = stripGitPrefix(line.slice(4).trim());
      } else if (line.startsWith("rename from ")) {
        // Prefer explicit rename metadata over the header seed.
        oldPath = line.slice("rename from ".length).trim();
      } else if (line.startsWith("rename to ")) {
        newPath = line.slice("rename to ".length).trim();
      } else if (line.startsWith("Binary files ")) {
        // "Binary files a/foo and b/foo differ"
        binary = true;
        // Extract paths from the binary announcement if ---/+++ not set yet
        if (!oldPath && !newPath) {
          const bm = line.match(/^Binary files (.+) and (.+) differ/);
          if (bm) {
            oldPath = stripGitPrefix(bm[1].trim());
            newPath = stripGitPrefix(bm[2].trim());
          }
        }
      }
      i++;
    }

    // Determine language: prefer new path (unless it is /dev/null → use old path)
    const langPath = newPath && newPath !== "/dev/null" ? newPath : oldPath;
    const language = langFromPath(langPath);

    // --- parse hunks ---
    while (i < lines.length) {
      const hdrLine = lines[i];
      if (!hdrLine.startsWith("@@")) {
        i++;
        continue;
      }

      const header = hdrLine;
      const [oldStart, newStart] = parseHunkHeader(header);
      let oldNo = oldStart;
      let newNo = newStart;

      const hunkLines: DiffLine[] = [];
      i++; // move past the `@@` line

      while (i < lines.length && !lines[i].startsWith("@@") && !lines[i].startsWith("diff --git ")) {
        const raw = lines[i];
        i++;

        // Skip "\ No newline at end of file" marker
        if (raw.startsWith("\\ ")) continue;

        // Skip empty trailing line produced by the split at block end
        if (raw === "" && i >= lines.length) continue;

        const sigil = raw[0];
        const text = raw.slice(1);

        let kind: DiffLineKind;
        let lineOldNo: number | null = null;
        let lineNewNo: number | null = null;

        if (sigil === "+") {
          kind = "add";
          lineNewNo = newNo++;
        } else if (sigil === "-") {
          kind = "del";
          lineOldNo = oldNo++;
        } else {
          // context (space or empty — some diffs omit the space on blank context lines)
          kind = "context";
          lineOldNo = oldNo++;
          lineNewNo = newNo++;
        }

        hunkLines.push({ kind, text, oldNo: lineOldNo, newNo: lineNewNo });
      }

      hunks.push({ header, lines: hunkLines });
    }

    files.push({ oldPath, newPath, language, binary, hunks });
  }

  return { files };
}
