// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { mdToSafeHtml, readmeMdToSafeHtml } from "./markdown";

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
  it("opens links in a new tab with noopener", () => {
    const html = mdToSafeHtml("[x](https://example.com)");
    expect(html).toContain('target="_blank"');
    expect(html).toContain("noopener");
  });
  it("strips onerror from an img", () => {
    expect(mdToSafeHtml('<img src="x" onerror="alert(1)">').toLowerCase()).not.toContain("onerror");
  });
  it("drops data: image sources", () => {
    const html = mdToSafeHtml('<img src="data:image/svg+xml,<svg onload=alert(1)>">');
    expect(html).not.toContain("data:");
  });
});

describe("readmeMdToSafeHtml", () => {
  const ctx = { owner: "o", repo: "r", branch: "main" };

  it("absolutizes a relative image to raw.githubusercontent", () => {
    const html = readmeMdToSafeHtml("![logo](assets/logo.png)", ctx);
    expect(html).toContain("https://raw.githubusercontent.com/o/r/main/assets/logo.png");
  });

  it("absolutizes a relative image with leading ./ prefix", () => {
    const html = readmeMdToSafeHtml("![logo](./assets/logo.png)", ctx);
    expect(html).toContain("https://raw.githubusercontent.com/o/r/main/assets/logo.png");
  });

  it("absolutizes a relative link to github.com/blob", () => {
    const html = readmeMdToSafeHtml("[docs](docs/guide.md)", ctx);
    expect(html).toContain("https://github.com/o/r/blob/main/docs/guide.md");
  });

  it("leaves absolute https urls untouched", () => {
    const html = readmeMdToSafeHtml("![x](https://img.shields.io/x.svg)", ctx);
    expect(html).toContain("https://img.shields.io/x.svg");
    expect(html).not.toContain("raw.githubusercontent.com/o/r/main/https");
  });

  it("leaves in-page anchors (#) untouched", () => {
    const html = readmeMdToSafeHtml("[section](#section)", ctx);
    expect(html).toContain('href="#section"');
    expect(html).not.toContain("blob/main/#");
  });

  it("leaves protocol-relative urls untouched", () => {
    const html = readmeMdToSafeHtml("![x](//example.com/img.png)", ctx);
    expect(html).not.toContain("raw.githubusercontent.com/o/r/main//");
  });

  it("sanitizes script tags as well as absolutizing", () => {
    const html = readmeMdToSafeHtml("hi <script>alert(1)</script>", ctx);
    expect(html).not.toContain("<script");
  });

  it("absolutizes a bare relative path with leading /", () => {
    const html = readmeMdToSafeHtml("![x](/img/logo.png)", ctx);
    expect(html).toContain("https://raw.githubusercontent.com/o/r/main/img/logo.png");
  });
});
