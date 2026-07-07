import { createHighlighter, type Highlighter } from "shiki";
import { nervShikiTheme } from "./nervShikiTheme";

// Curated language list — bounds the bundle size by not loading every grammar.
// Must match the `language` values produced by parse.ts's EXT_LANG map + "text".
export const LANGS = [
  "typescript",
  "tsx",
  "javascript",
  "jsx",
  "rust",
  "python",
  "json",
  "html",
  "css",
  "svelte",
  "markdown",
  "bash",
  "yaml",
  "toml",
  "go",
  "c",
  "cpp",
  "java",
  "ruby",
  "php",
  "sql",
  "diff",
  // "text" is not a shiki grammar — fall back to raw text rendering in DiffView
] as const;

export type SupportedLang = (typeof LANGS)[number];

// Memoised promise — created once, shared across all callers.
// Shiki's grammars and themes are bundled by Vite as dynamic-import chunks:
// offline, no CDN, no network required at runtime.
let hp: Promise<Highlighter> | null = null;

export function getHighlighter(): Promise<Highlighter> {
  if (!hp) {
    hp = createHighlighter({
      themes: ["github-light", "github-dark", nervShikiTheme],
      langs: [...LANGS],
    }).catch((e) => {
      // If Shiki fails to initialise (e.g. in SSR / test env), reset so a
      // subsequent call retries rather than serving a permanently-rejected promise.
      hp = null;
      return Promise.reject(e);
    });
  }
  return hp;
}
