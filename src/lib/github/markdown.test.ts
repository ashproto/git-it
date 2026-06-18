// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { mdToSafeHtml } from "./markdown";

describe("mdToSafeHtml", () => {
  it("renders basic markdown", () => {
    const html = mdToSafeHtml("# Hi\n\n- a\n- b");
    expect(html).toContain("<h1");
    expect(html).toContain("<li>a</li>");
  });
  it("strips <script> tags", () => {
    expect(mdToSafeHtml("hi <script>alert(1)</script>")).not.toContain("<script");
  });
  it("strips event handlers and javascript: urls", () => {
    const html = mdToSafeHtml('<a href="javascript:alert(1)" onclick="x()">x</a>');
    expect(html).not.toContain("javascript:");
    expect(html.toLowerCase()).not.toContain("onclick");
  });
  it("opens links in a new tab", () => {
    expect(mdToSafeHtml("[x](https://example.com)")).toContain('target="_blank"');
  });
});
