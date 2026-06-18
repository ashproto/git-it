const SEGMENT = /^[A-Za-z0-9._-]+$/;
const PREFIXES = [
  "https://github.com/",
  "http://github.com/",
  "git@github.com:",
  "ssh://git@github.com/",
];

/** A valid owner/repo segment: charset only, and not a `.`/`..` traversal or a
 *  leading-dash (flag-like) value. Mirrors the Rust `is_valid_segment`. */
function validSegment(s: string): boolean {
  return s !== "" && s !== "." && s !== ".." && !s.startsWith("-") && SEGMENT.test(s);
}

/** Pure TS mirror of the Rust `parse_github_remote` — used for cheap nav
 *  visibility (no `gh` call needed to know a repo has a github.com remote). */
export function parseGithubRemote(url: string): { owner: string; repo: string } | null {
  const u = (url ?? "").trim();
  const prefix = PREFIXES.find((p) => u.startsWith(p));
  if (!prefix) return null;
  let rest = u.slice(prefix.length);
  if (rest.endsWith(".git")) rest = rest.slice(0, -4);
  const slash = rest.indexOf("/");
  if (slash < 0) return null;
  const owner = rest.slice(0, slash).trim();
  const repo = rest.slice(slash + 1).trim();
  if (repo.includes("/") || !validSegment(owner) || !validSegment(repo)) return null;
  return { owner, repo };
}
