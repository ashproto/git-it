// Pure prefill helpers for the create-PR wizard. Tauri-free (vitest-covered).

// "feature/fix-foo-bar" → "Fix foo bar"; "fix_thing" → "Fix thing"; "main" → "Main".
// Strips everything up to the last "/", replaces -/_ with spaces, collapses
// whitespace, and capitalizes the first letter only.
export function humanizeBranch(branch: string): string {
  const last = branch.slice(branch.lastIndexOf("/") + 1);
  const words = last.replace(/[-_]+/g, " ").replace(/\s+/g, " ").trim();
  if (!words) return "";
  return words.charAt(0).toUpperCase() + words.slice(1);
}

// Suggest a PR title + body from the branch's commit subjects (newest-first, as
// `git log` yields them):
//   0 subjects → humanized branch name, empty body
//   1 subject  → that subject as the title, empty body
//   n subjects → humanized branch name + a bullet list, reversed to oldest-first
export function prefillFromSubjects(
  subjects: string[],
  branch: string,
): { title: string; body: string } {
  if (subjects.length === 1) return { title: subjects[0], body: "" };
  const title = humanizeBranch(branch);
  if (subjects.length === 0) return { title, body: "" };
  const body = [...subjects]
    .reverse()
    .map((s) => `- ${s}`)
    .join("\n");
  return { title, body };
}
