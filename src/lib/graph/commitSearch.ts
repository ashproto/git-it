import type { GraphCommit } from "../types";

// A query is "hex-like" (eligible for SHA-prefix matching) only at 4+ hex chars:
// shorter prefixes match far too many commits to be a useful jump target.
const HEX_QUERY = /^[0-9a-f]{4,}$/i;

/**
 * Indices (ascending, deduped) of the loaded commits matching `query`:
 * case-insensitive substring on subject + author name, plus SHA prefix
 * when the query looks hex-like. Empty/whitespace query matches nothing.
 */
export function matchCommits(commits: GraphCommit[], query: string): number[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const tryShaPrefix = HEX_QUERY.test(q);
  const out: number[] = [];
  for (let i = 0; i < commits.length; i++) {
    const c = commits[i];
    if (
      c.subject.toLowerCase().includes(q) ||
      c.author_name.toLowerCase().includes(q) ||
      (tryShaPrefix && c.sha.toLowerCase().startsWith(q))
    ) {
      out.push(i);
    }
  }
  return out;
}
