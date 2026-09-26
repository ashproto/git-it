// Turn whatever an operation rejected with into something a person can read: the sentence
// git actually complained with, git's own advice, and the raw output for anyone who wants
// the whole story. Pure (Tauri-free) so it is unit-tested with vitest.

export type GitErrorSummary = {
  /** git's complaint: the first `fatal:`/`error:` line plus its continuation lines. */
  headline: string;
  /** git's `hint:` advice without the prefix; "" when there is none. */
  hints: string;
  /** The text exactly as received. */
  output: string;
};

const NO_MESSAGE = "The operation failed without an error message.";

// Full error text. GitHub commands reject with a serialized GithubError OBJECT
// ({kind, message?}) — String() on that yields "[object Object]", so unwrap it first.
export function errorText(e: unknown): string {
  if (e == null) return "";
  if (typeof e === "object" && "kind" in e) {
    const g = e as { kind: string; message?: string };
    return String(g.kind === "Other" ? (g.message ?? g.kind) : g.kind);
  }
  if (e instanceof Error) return e.message;
  return String(e);
}

const HEADLINE = /^(?:fatal|error): ?/;
const PREFIXED = /^(?:fatal|error|warning|hint|remote):/;
const HINT = /^hint:(?: |$)/;

// Lines git writes to STDOUT while merging. Git It reads stdout and stderr on separate threads,
// so these can land anywhere — even between an `error:` line and its file list.
function isInterleavedStdout(line: string): boolean {
  return /^(?:Updating [0-9a-f]+\.\.[0-9a-f]+|Auto-merging .+|CONFLICT \(.+)$/.test(line);
}

// git's transport report: a `From <url>` / `To <url>` header or a ref-status line
// ("   a1b2..c3d4  main -> origin/main", " ! [rejected] …"). Not part of any complaint — it
// ends an error block, and it is never a headline. Advice such as "To push the history…"
// has more than one word after "To", so it never matches.
function isTransportLine(line: string): boolean {
  return /^(?:From|To) (?:\S+|\/.*)$/.test(line) || /^ [ !*+=t-] .+ -> /.test(line);
}

function isMeaningful(line: string): boolean {
  return (
    line.trim() !== "" &&
    !line.startsWith(" ") && // indented commands and ref-status lines
    !line.startsWith("> ") && // the npm/pnpm script banner that leads hook output
    !isTransportLine(line) &&
    !isInterleavedStdout(line) &&
    !HINT.test(line) &&
    !line.startsWith("remote:") &&
    line !== "Aborting"
  );
}

function pickHeadline(lines: string[]): string | null {
  const start = lines.findIndex((line) => HEADLINE.test(line));
  if (start >= 0) {
    const block = [lines[start].replace(HEADLINE, "")];
    for (const line of lines.slice(start + 1)) {
      // The complaint runs to git's next prefixed line, its closing "Aborting", or its
      // transport report. Blank and indented lines stay: git's advice often reads
      // "…, use\n\n    git <command>".
      if (PREFIXED.test(line) || line === "Aborting" || isTransportLine(line)) break;
      if (isInterleavedStdout(line)) continue;
      block.push(line);
    }
    while (block.length > 1 && block[block.length - 1].trim() === "") block.pop();
    return block.join("\n");
  }
  // Nothing prefixed: a non-git message, git's unprefixed advice, hook output, or git under a
  // translated locale. Multi-paragraph advice (a pull with no upstream, or in detached HEAD)
  // leads with the complaint, so take its first paragraph that says something.
  const paragraphs: string[][] = [[]];
  for (const line of lines) {
    if (line.trim() !== "") paragraphs[paragraphs.length - 1].push(line);
    else if (paragraphs[paragraphs.length - 1].length) paragraphs.push([]);
  }
  if (paragraphs.filter((p) => p.length).length > 1) {
    const first = paragraphs.find((p) => p.some(isMeaningful));
    if (first) return first.filter(isMeaningful).map((line) => line.trim()).join("\n");
  }
  // Otherwise the last line that says something — extended back over one sentence git wrapped
  // across lines ("…merge with the ref 'refs/heads/gone'\nfrom the remote, but no such ref was
  // fetched."): a lowercase line continues the one above unless that one ended a sentence.
  const meaningful = lines.filter(isMeaningful);
  if (!meaningful.length) return null;
  let from = meaningful.length - 1;
  while (
    from > 0 &&
    /^[a-z]/.test(meaningful[from]) &&
    !/[.!?]$/.test(meaningful[from - 1].trimEnd())
  ) {
    from--;
  }
  return meaningful.slice(from).map((line) => line.trim()).join("\n");
}

function collectHints(lines: string[]): string {
  const hints = lines
    .filter((line) => HINT.test(line))
    .map((line) => line.replace(HINT, ""))
    // CLI advice about silencing the hint means nothing in a GUI.
    .filter((line) => !line.startsWith("Disable this message with"));
  while (hints.length && hints[0].trim() === "") hints.shift();
  while (hints.length && hints[hints.length - 1].trim() === "") hints.pop();
  return hints.join("\n");
}

export function summarizeGitOutput(text: string): GitErrorSummary {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const headline = pickHeadline(lines);
  return {
    headline: headline && headline.trim() ? headline : NO_MESSAGE,
    hints: collectHints(lines),
    output: text,
  };
}
