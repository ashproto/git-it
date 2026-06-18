// Commit message bodies are conventionally hard-wrapped at ~72 columns. Rendering them
// with `white-space: pre-wrap` faithfully preserves every hard line break, so the text
// never reflows to fill a wide details pane — leaving a lot of empty space on the right.
//
// reflowCommitBody re-flows the body into paragraph "blocks": consecutive prose lines are
// joined back into one paragraph (the hard wraps become soft, so the paragraph fills the
// available width and re-wraps to the container), while the structure that actually
// matters is preserved:
//   • blank lines separate paragraphs (a new, non-"tight" block),
//   • list / enumeration items (-, *, +, •, "1.", "2)") each start their own block, and
//     their wrapped continuation lines fold back into that item,
//   • trailer lines ("Co-Authored-By: …", "Signed-off-by: …") each start their own block
//     so a trailer block stays one-per-line instead of being run together.
//
// `tight` marks a block that followed the previous one with NO blank line between them
// (consecutive list items / trailers), so the renderer can use a smaller gap there than
// between true paragraphs.

export type CommitBodyBlock = { text: string; tight: boolean };

// A line that begins a list / enumeration item: a bullet (-, *, +, •) or "N." / "N)".
const LIST_RE = /^\s*([-*+•]|\d+[.)])\s+/;
// A git trailer-style line: a token of letters/digits/hyphens immediately followed by
// ": " (e.g. "Co-Authored-By: x", "Signed-off-by: y", "Fixes: #123").
const TRAILER_RE = /^[A-Za-z][A-Za-z0-9-]*:\s/;

export function reflowCommitBody(body: string): CommitBodyBlock[] {
  const lines = (body ?? "").replace(/\r\n?/g, "\n").split("\n");
  const blocks: CommitBodyBlock[] = [];
  let parts: string[] = [];
  let blankBefore = true; // start-of-body acts like a paragraph boundary (→ loose)
  let pendingBlank = true; // a blank line has been seen since the last block was flushed

  const flush = () => {
    if (!parts.length) return;
    const text = parts.join(" ").replace(/[ \t]+/g, " ").trim();
    if (text) blocks.push({ text, tight: !blankBefore });
    parts = [];
  };

  for (const raw of lines) {
    if (raw.trim() === "") {
      flush();
      pendingBlank = true;
      continue;
    }
    // A list item or trailer line begins a new block even without a blank line before it.
    if (parts.length > 0 && (LIST_RE.test(raw) || TRAILER_RE.test(raw))) {
      flush();
      blankBefore = false; // adjacent to the previous block → tight
    } else if (parts.length === 0) {
      blankBefore = pendingBlank; // first line of a block: loose iff a blank preceded it
    }
    pendingBlank = false;
    parts.push(raw.trim());
  }
  flush();
  return blocks;
}
