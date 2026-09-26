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

// Lines git writes to STDOUT during a pull or push. Git It reads stdout and stderr on separate
// threads, so these can land anywhere — even between an `error:` line and its file list.
function isStdoutNoise(line: string): boolean {
  return (
    /^Updating [0-9a-f]+\.\.[0-9a-f]+$/.test(line) || // merge preamble
    /^(?:From|To) \S/.test(line) || // fetch / push header
    line.startsWith(" ") // ref updates: "   a1b2..c3d4  main -> origin/main", " ! [rejected] …"
  );
}

function pickHeadline(lines: string[]): string | null {
  const start = lines.findIndex((line) => HEADLINE.test(line));
  if (start >= 0) {
    const block = [lines[start].replace(HEADLINE, "")];
    for (const line of lines.slice(start + 1)) {
      // The complaint ends at git's next prefixed line, its closing "Aborting", or a blank line.
      if (PREFIXED.test(line) || line === "Aborting" || line.trim() === "") break;
      if (isStdoutNoise(line)) continue;
      block.push(line); // tab-indented file lists and unprefixed follow-up sentences
    }
    return block.join("\n");
  }
  // Nothing prefixed (a non-git message, or git under a translated locale): the last line
  // that says something is the best guess.
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i];
    if (
      line.trim() &&
      !isStdoutNoise(line) &&
      !HINT.test(line) &&
      !line.startsWith("remote:") &&
      line !== "Aborting"
    ) {
      return line.trim();
    }
  }
  return null;
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
