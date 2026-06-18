import { describe, it, expect } from "vitest";
import { reflowCommitBody } from "./commitBody";

describe("reflowCommitBody", () => {
  it("returns no blocks for empty / whitespace bodies", () => {
    expect(reflowCommitBody("")).toEqual([]);
    expect(reflowCommitBody("   \n  \n")).toEqual([]);
  });

  it("joins a single hard-wrapped paragraph into one block", () => {
    const body = "Draws the lane SVG behind the commit list as a separate,\nabsolutely-positioned layer so it can be windowed\nindependently of the row virtualization.";
    const blocks = reflowCommitBody(body);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].text).toBe(
      "Draws the lane SVG behind the commit list as a separate, absolutely-positioned layer so it can be windowed independently of the row virtualization.",
    );
    expect(blocks[0].tight).toBe(false);
    // No embedded newlines remain — the paragraph can reflow to any width.
    expect(blocks[0].text).not.toContain("\n");
  });

  it("splits paragraphs on a blank line, both loose", () => {
    const blocks = reflowCommitBody("First paragraph line one\nline two.\n\nSecond paragraph.");
    expect(blocks.map((b) => b.text)).toEqual(["First paragraph line one line two.", "Second paragraph."]);
    expect(blocks.every((b) => !b.tight)).toBe(true);
  });

  it("collapses multiple blank lines to a single paragraph boundary", () => {
    const blocks = reflowCommitBody("One.\n\n\n\nTwo.");
    expect(blocks.map((b) => b.text)).toEqual(["One.", "Two."]);
  });

  it("keeps numbered list items as separate blocks and folds their wrapped continuations in", () => {
    const body = [
      "Two improvements:",
      "",
      "1. Details pane now fills the freed space when the graph is",
      "   collapsed (previously only the reverse worked).",
      "",
      "2. CollapsiblePanel bodies slide open and closed so the",
      "   sidebar sections animate.",
    ].join("\n");
    const blocks = reflowCommitBody(body);
    expect(blocks.map((b) => b.text)).toEqual([
      "Two improvements:",
      "1. Details pane now fills the freed space when the graph is collapsed (previously only the reverse worked).",
      "2. CollapsiblePanel bodies slide open and closed so the sidebar sections animate.",
    ]);
    // Each numbered item is preceded by a blank line here → all loose.
    expect(blocks.every((b) => !b.tight)).toBe(true);
  });

  it("marks consecutive (no blank line) list items as tight", () => {
    const blocks = reflowCommitBody("- first item\n- second item\n- third item");
    expect(blocks.map((b) => b.text)).toEqual(["- first item", "- second item", "- third item"]);
    expect(blocks.map((b) => b.tight)).toEqual([false, true, true]);
  });

  it("keeps a trailer block one-per-line", () => {
    const body = "Real fix here.\n\nCo-Authored-By: A <a@x.dev>\nSigned-off-by: B <b@x.dev>";
    const blocks = reflowCommitBody(body);
    expect(blocks.map((b) => b.text)).toEqual([
      "Real fix here.",
      "Co-Authored-By: A <a@x.dev>",
      "Signed-off-by: B <b@x.dev>",
    ]);
    // First trailer is loose (blank before it); the second is tight (adjacent).
    expect(blocks.map((b) => b.tight)).toEqual([false, false, true]);
  });

  it("normalizes CRLF line endings", () => {
    const blocks = reflowCommitBody("line one\r\nline two\r\n\r\nnext para");
    expect(blocks.map((b) => b.text)).toEqual(["line one line two", "next para"]);
  });

  it("does not split a mid-paragraph continuation that merely contains a colon later", () => {
    // "the" doesn't start with a Word: trailer, so the wrapped line folds in normally.
    const blocks = reflowCommitBody("We fixed the bug where\nthe value: foo was wrong.");
    expect(blocks).toHaveLength(1);
    expect(blocks[0].text).toBe("We fixed the bug where the value: foo was wrong.");
  });
});
