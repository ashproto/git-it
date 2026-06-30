import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: true });

// Harden links + images. (Global hook; added once on module import.)
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noopener noreferrer");
  }
  // Defense-in-depth: DOMPurify's DATA_URI_TAGS default permits `data:` on
  // <img src> even though ALLOWED_URI_REGEXP excludes it. We don't need data:
  // images, so drop them outright.
  if (node.tagName === "IMG") {
    const src = (node.getAttribute("src") ?? "").trim().toLowerCase();
    if (src.startsWith("data:")) node.removeAttribute("src");
  }
});

const CONFIG = {
  ALLOWED_TAGS: [
    "h1", "h2", "h3", "h4", "h5", "h6", "p", "br", "hr", "ul", "ol", "li", "a",
    "code", "pre", "blockquote", "strong", "em", "del", "img", "table", "thead",
    "tbody", "tr", "th", "td", "input", "span", "kbd", "sup", "sub",
  ],
  ALLOWED_ATTR: ["href", "src", "alt", "title", "class", "type", "checked", "disabled", "align"],
  ALLOWED_URI_REGEXP: /^(?:https?:|mailto:|#)/i,
};

/** Render GitHub-flavored markdown to SANITIZED HTML (third-party content). */
export function mdToSafeHtml(src: string): string {
  const raw = marked.parse(src ?? "", { async: false }) as string;
  return DOMPurify.sanitize(raw, CONFIG);
}

/** Rewrite relative img[src] and a[href] in already-parsed HTML to absolute
 *  GitHub URLs so images/links in a README render correctly in a WKWebView. */
function absolutizeRelativeUrls(
  html: string,
  owner: string,
  repo: string,
  branch: string,
): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const rawBase = `https://raw.githubusercontent.com/${owner}/${repo}/${branch}/`;
  const blobBase = `https://github.com/${owner}/${repo}/blob/${branch}/`;
  // relative = not scheme:, not protocol-relative (//), not in-page anchor (#)
  const isRel = (u: string) => !!u && !/^(?:[a-z][a-z0-9+.-]*:|\/\/|#)/i.test(u);
  // README-relative paths resolve against the repo root, so drop any leading
  // ./ or ../ segments (we can't go above the root) and a leading /.
  const strip = (u: string) => u.replace(/^(?:\.\.?\/)+/, "").replace(/^\//, "");
  doc.querySelectorAll("img[src]").forEach((el) => {
    const v = el.getAttribute("src") ?? "";
    if (isRel(v)) el.setAttribute("src", rawBase + strip(v));
  });
  doc.querySelectorAll("a[href]").forEach((el) => {
    const v = el.getAttribute("href") ?? "";
    if (isRel(v)) el.setAttribute("href", blobBase + strip(v));
  });
  return doc.body.innerHTML;
}

/** Render a repo README to SANITIZED HTML, resolving relative image/link URLs
 *  against the repo's default branch so logos/screenshots/links work. */
export function readmeMdToSafeHtml(
  src: string,
  ctx: { owner: string; repo: string; branch: string },
): string {
  const raw = marked.parse(src ?? "", { async: false }) as string;
  const abs = absolutizeRelativeUrls(raw, ctx.owner, ctx.repo, ctx.branch);
  return DOMPurify.sanitize(abs, CONFIG);
}
