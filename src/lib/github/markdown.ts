import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: true });

// Open all links in a new tab + harden rel. (Global hook; added once on import.)
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noreferrer");
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
