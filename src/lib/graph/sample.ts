import type { GraphCommit } from "../types";

// A representative history (one merge, a merged feature branch, a tag, a separate
// unmerged branch, HEAD on main, a root) used when not running in Tauri so the
// browser preview can render the graph. Topologically ordered, children first.
export const SAMPLE_GRAPH: GraphCommit[] = [
  { sha: "a1b2c3d4e5f6", parents: ["b2c3d4e5f6a7"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T15:40:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T15:40:00Z", refs: [{ name: "main", kind: "local", is_head: true }], subject: "Render commit graph gutter" },
  { sha: "b2c3d4e5f6a7", parents: ["c3d4e5f6a7b8", "f6a7b8c9d0e1"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T14:10:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T14:10:00Z", refs: [], subject: "Merge branch 'feature/graph-view'" },
  { sha: "f6a7b8c9d0e1", parents: ["e5f6a7b8c9d0"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T13:05:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T13:05:00Z", refs: [{ name: "feature/graph-view", kind: "local", is_head: false }], subject: "Add lane assignment algorithm" },
  { sha: "c3d4e5f6a7b8", parents: ["e5f6a7b8c9d0"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-14T11:30:00Z", committer_name: "A. Shah", committer_date: "2026-06-14T11:30:00Z", refs: [{ name: "v1.2.0", kind: "tag", is_head: false }], subject: "Fetch parent SHAs + ref decoration" },
  { sha: "e5f6a7b8c9d0", parents: ["d0e1f2a3b4c5"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-13T17:00:00Z", committer_name: "A. Shah", committer_date: "2026-06-13T17:00:00Z", refs: [], subject: "Scaffold graph data layer" },
  { sha: "9a8b7c6d5e4f", parents: ["d0e1f2a3b4c5"], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-13T16:20:00Z", committer_name: "A. Shah", committer_date: "2026-06-13T16:20:00Z", refs: [{ name: "fix/dates", kind: "local", is_head: false }, { name: "origin/fix/dates", kind: "remote", is_head: false }], subject: "Fix Feb-30 date validation" },
  { sha: "d0e1f2a3b4c5", parents: [], author_name: "A. Shah", author_email: "a@x.dev", author_date: "2026-06-13T09:00:00Z", committer_name: "A. Shah", committer_date: "2026-06-13T09:00:00Z", refs: [], subject: "Round-2 audit hardening" },
];
