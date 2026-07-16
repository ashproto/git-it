# Git It Landing Page — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the static "NERV console" marketing landing page for the Git It macOS git client, deployed to GitHub Pages at `git-it.app`.

**Architecture:** A hand-authored static site — `index.html` (long-scroll), `guides.html`, `404.html`, one `styles.css`, one small `app.js`, self-hosted fonts, inline SVG illustrations. No framework, no build step, no runtime third-party requests. Verification is browser-based via the preview tools (serve → screenshot/inspect/snapshot/console), not unit tests.

**Tech Stack:** HTML5, CSS3 (custom properties, grid/flex, `clamp()`), vanilla JS (`IntersectionObserver`), self-hosted woff2 (Anton, IBM Plex Sans, IBM Plex Mono), GitHub Pages.

**Canonical design reference:** `design/hero-mockup-v1.html` in this repo is the approved implementation of the visual system, nav, hero, spec band, and hazard divider. Reuse its CSS/markup as the starting point; apply the deltas each task specifies (font swap + copy corrections). Full spec: `docs/superpowers/specs/2026-07-06-git-it-landing-page-design.md`.

## Global Constraints

*Every task's requirements implicitly include this section. Values are verbatim from the spec.*

- **Platform copy:** target is **macOS 12.3+**, **Apple Silicon + Intel** native builds, **no Electron**.
- **Requirements copy:** users need **`git` + `python3` on `PATH`** (`xcode-select --install`). `git-filter-repo` is **bundled** — never tell users to install it.
- **Download trust line:** the release is **"Signed & notarized by Apple — opens with a normal double-click."** NEVER tell download users to right-click→Open or bypass Gatekeeper (that applies only to a self-built ad-hoc copy, mentioned only in a build-from-source aside if at all).
- **Privacy copy (precise):** "Local-first. No account, no analytics, no tracking. Repos and credentials stay on your Mac; the only network calls are the git/GitHub operations you trigger, plus a version check for updates." Footer site-scoped line: **"This site collects nothing — no analytics, no cookies, no third-party requests."** Never claim "100% local / everything runs locally."
- **License copy:** **"Free · source-available · noncommercial (CC BY-NC-SA 4.0)."** NEVER use the words **"open source."** Pair "built in the open" with the license line. Footnote: *"Source-available under CC BY-NC-SA 4.0 — not an OSI open-source license."*
- **No hard-coded version** anywhere (repo is at 0.1.0; don't print a `REV x.y.z`).
- **Download links** resolve to `https://github.com/ashproto/git-it/releases/latest` unless arch-stable asset names exist (see Task 9).
- **No runtime third-party requests:** fonts self-hosted; no CDNs, analytics, cookies, or web-font URLs.
- **Domain:** `git-it.app`. **GitHub repo:** `https://github.com/ashproto/git-it`.
- **Accent discipline:** orange (`--orange #F2542D`) is the only brand accent; green (`--phosphor #46E88B`) = status/data only; red (`--alert #FF4438`) = single hazard only.
- **Fonts:** display **Anton**; body **IBM Plex Sans**; mono **IBM Plex Mono** (self-hosted woff2).
- **Motion:** every animation gated behind `@media (prefers-reduced-motion: no-preference)`; reduced-motion shows final state.
- **A11y:** semantic landmarks, visible focus, descriptive `alt`/`aria` on illustrations, AA contrast.

---

## File Structure

```
git-it-site/
├── index.html            # long-scroll landing (nav, hero, what's-different, spec, features, download, guides, faq, community, footer)
├── guides.html           # two light guides (getting-started, commit-time-editing)
├── 404.html              # "// SIGNAL LOST"
├── CNAME                 # git-it.app
├── robots.txt
├── sitemap.xml
├── site.webmanifest
├── assets/
│   ├── css/styles.css    # reset + tokens + all component CSS
│   ├── js/app.js         # boot reveal + IntersectionObserver scroll-reveal + nav
│   ├── fonts/            # anton, ibm-plex-sans, ibm-plex-mono woff2 + fonts.css (@font-face)
│   └── img/              # favicon.svg, favicon.ico, apple-touch-icon.png, og.png (1200×630)
└── .claude/launch.json   # static server for preview tools
```

Responsibilities: `styles.css` holds the token system + every component's styles (organized by clearly-commented section). `app.js` holds only progressive-enhancement JS (page works without it). Illustrations are **inline SVG** in the HTML so they theme via CSS variables and animate via CSS. `guides.html` and `404.html` reuse `styles.css`.

---

## Task 1: Scaffold, tokens, fonts, meta, preview server

**Files:**
- Create: `assets/css/styles.css`, `assets/fonts/fonts.css`, `assets/js/app.js`, `index.html`, `CNAME`, `robots.txt`, `sitemap.xml`, `site.webmanifest`, `.claude/launch.json`
- Create: `assets/fonts/*.woff2` (Anton, IBM Plex Sans 400/600, IBM Plex Mono 400/500)
- Create: `assets/img/favicon.svg` (hexagon+commit-graph glyph)

**Interfaces:**
- Produces (consumed by all later tasks): CSS custom properties from spec §5.1 (`--void`, `--void2`, `--panel`, `--line`, `--line2`, `--bone`, `--haze`, `--haze-dim`, `--orange`, `--orange-deep`, `--phosphor`, `--alert`), font vars `--font-display` (Anton), `--font-body` (IBM Plex Sans), `--font-mono` (IBM Plex Mono), the `.wrap` container (max-width 1240px, responsive padding), and the `.page` ground/scanline/grid background.

- [ ] **Step 1: Create the design-token + reset CSS.** In `assets/css/styles.css`, port the `:root` tokens, the `*{box-sizing}` reset, `.page` background (radial glows + scanlines + grid overlay), and `.wrap` container from `design/hero-mockup-v1.html`'s `<style>`. Change the font vars to the self-hosted stacks:

```css
:root{
  --font-display:"Anton","Arial Narrow",Impact,sans-serif;
  --font-body:"IBM Plex Sans",system-ui,-apple-system,sans-serif;
  --font-mono:"IBM Plex Mono",ui-monospace,"SF Mono",Menlo,monospace;
  /* + all color tokens from spec §5.1, verbatim */
}
```

- [ ] **Step 2: Acquire + wire the fonts.** Download woff2 for Anton (OFL), IBM Plex Sans (400, 600) and IBM Plex Mono (400, 500) from their official open-source releases into `assets/fonts/`. Create `assets/fonts/fonts.css` with `@font-face` blocks (`font-display:swap`, correct `src url("../fonts/…woff2") format("woff2")`). If network access blocks the download, note it and fall back to the system stacks above (build proceeds; swap fonts in later). Link `fonts.css` before `styles.css` in the head.

- [ ] **Step 3: Create `index.html` skeleton with full head.** Include: `<meta charset>`, viewport, `<title>Git It — a native macOS git client</title>`, description meta, canonical `https://git-it.app/`, `theme-color #0A0C0F`, Open Graph (`og:title/description/image=https://git-it.app/assets/img/og.png/type=website/url`), Twitter `summary_large_image`, favicon links (`favicon.svg`, `favicon.ico`, `apple-touch-icon`), `site.webmanifest`, and a `SoftwareApplication` JSON-LD block:

```html
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"SoftwareApplication","name":"Git It",
 "applicationCategory":"DeveloperApplication","operatingSystem":"macOS 12.3+",
 "offers":{"@type":"Offer","price":"0","priceCurrency":"USD"},
 "url":"https://git-it.app/","downloadUrl":"https://github.com/ashproto/git-it/releases/latest"}
</script>
```
Body: `<div class="page">` wrapping empty `<nav>`, `<main>`, `<footer>` landmarks. Link `assets/css/styles.css` and `<script defer src="assets/js/app.js">`.

- [ ] **Step 4: Create static files.** `CNAME` → `git-it.app`. `robots.txt` → allow all + `Sitemap: https://git-it.app/sitemap.xml`. `sitemap.xml` listing `/` and `/guides.html`. `site.webmanifest` (name "Git It", `theme_color`/`background_color` `#0A0C0F`, icons). `assets/js/app.js` → empty IIFE stub. `assets/img/favicon.svg` → the hexagon+3-node-commit-graph glyph (adapt the nav `<svg>` from the mockup, standalone).

- [ ] **Step 5: Configure the preview server.** `.claude/launch.json`:

```json
{"version":"0.0.1","configurations":[{"name":"git-it-site","runtimeExecutable":"python3","runtimeArgs":["-m","http.server","4321"],"port":4321}]}
```

- [ ] **Step 6: Serve & verify.** `preview_start` name `git-it-site`. Then `preview_console_logs` (expect no errors), `preview_inspect body` → confirm `font-family` resolves to IBM Plex Sans (or documented fallback) and `background-color` is the void ground. `preview_snapshot` → confirm page structure (nav/main/footer landmarks present). Fix any 404s on fonts/CSS via `preview_network` (filter failed).

- [ ] **Step 7: Commit.**
```bash
git add -A && git commit -m "chore: scaffold site — tokens, fonts, meta, preview server"
```

---

## Task 2: Sticky nav + progressive-enhancement JS

**Files:** Modify `index.html` (nav), `assets/css/styles.css` (nav styles), `assets/js/app.js`
**Interfaces:** Consumes tokens/`.wrap` from Task 1. Produces: `.reveal`/`.d1..d6` boot-reveal classes and a `data-reveal` scroll-reveal hook (consumed by every later section), and the `.btn`/`.btn-primary`/`.btn-ghost` button components.

- [ ] **Step 1: Build the nav.** Port `<nav>` + `.nav*` CSS from the mockup. Links: `Features` (`#features`), `Download` (`#download`), `Guides` (`guides.html`), `GitHub` (`https://github.com/ashproto/git-it`), plus the orange `↓ Download` (`#download`). Glyph SVG + mono "Git It" wordmark on the left. Collapse to GitHub + Download under 720px.
- [ ] **Step 2: Port button + boot-reveal + reduced-motion CSS** (`.btn*`, `.reveal`, `.d1..d6`, `@keyframes rise/slideup`, `@media (prefers-reduced-motion…)`) from the mockup into `styles.css`.
- [ ] **Step 3: Write `app.js`.** An IIFE that (a) adds a `.is-scrolled` class to `<nav>` after 8px scroll (for a subtle border), and (b) an `IntersectionObserver` that adds `.in-view` to any `[data-reveal]` element once it enters the viewport — but only when `matchMedia('(prefers-reduced-motion: no-preference)').matches`; otherwise mark them all `.in-view` immediately.
- [ ] **Step 4: Add the scroll-reveal CSS.** `[data-reveal]{opacity:0;transform:translateY(16px)}` inside the no-preference media query only; `[data-reveal].in-view{opacity:1;transform:none;transition:.6s cubic-bezier(.2,.8,.2,1)}`. Outside the query, `[data-reveal]{opacity:1;transform:none}`.
- [ ] **Step 5: Verify.** Reload. `preview_snapshot` → nav links present with correct hrefs. `preview_inspect .nav-dl` → orange border. `preview_console_logs` → no errors. `preview_resize` preset `mobile` → confirm collapse. `preview_resize colorScheme` unaffected. Emulate reduced motion (`preview_resize` won't; instead verify in code that reduced-motion path sets `.in-view`). Screenshot desktop nav.
- [ ] **Step 6: Commit.** `git commit -am "feat: sticky nav + boot/scroll-reveal JS"`

---

## Task 3: Hero section

**Files:** Modify `index.html` (hero), `styles.css` (hero styles)
**Interfaces:** Consumes tokens, `.wrap`, `.btn*`, `.reveal`/`.d*`. Produces the `.console`/`.brk`/`.con-*` HUD-panel classes (reused by feature illustrations) and the `.eyebrow`, `.readout`, `.hazard` classes.

- [ ] **Step 1: Port the hero** from the mockup, then apply these exact copy/structure corrections (from spec §7②):
  - Eyebrow (elevate to bone, not haze): `◇ native macOS git client · Apple Silicon + Intel`.
  - Headline: `Bend the` / `timeline.` — **remove** the `REV 0.2.0` tag entirely.
  - Annotation (keep directly under headline): `Commit-time editing — shift, set, or compress any range. Preview, then undo.`
  - Subhead: `A fast, native git client for macOS — commit graph, branches, merges, per-hunk diffs, remotes, and a built-in GitHub dashboard. Local-first: no account, no analytics, no tracking.`
  - Add a mono microline after the subhead: `Real app, shipping — see it on GitHub ↗` (links to the repo).
  - CTAs: primary `↓ Download for macOS` → `#download`; ghost `View on GitHub ↗` → repo.
  - Readouts (replace mockup's): `No account · No tracking · Native — no Electron · ● Auto-updates` (the `●` is `--phosphor`).
- [ ] **Step 2: Keep the HUD commit-graph console** (the right-column inline SVG with the `REWRITE ⟳` `14:22 → 09:05` annotation) as-is from the mockup — it is the illustration template for later tasks. Ensure `aria-hidden="true"` and a visually-hidden text alternative describing it ("Diagram: a commit graph with one commit's timestamp being rewritten from 14:22 to 09:05").
- [ ] **Step 3: Verify first-screen comprehension.** `preview_resize` preset `mobile` (375). `preview_screenshot`. **Confirm the eyebrow text "native macOS git client" is visible without scrolling** (spec success criterion). `preview_inspect .eyebrow` → color is `--bone`. `preview_snapshot` → headline + subhead + both CTAs present, CTA hrefs correct.
- [ ] **Step 4: Verify desktop + motion.** `preview_resize` preset `desktop`, screenshot. `preview_console_logs` → clean. Confirm no horizontal scroll: `preview_eval` `document.documentElement.scrollWidth <= window.innerWidth`.
- [ ] **Step 5: Commit.** `git commit -am "feat: hero section (corrected copy, notarized-safe)"`

---

## Task 4: "What's different" strip

**Files:** Modify `index.html`, `styles.css`
**Interfaces:** Consumes tokens, `.wrap`, hazard-chip pattern, `[data-reveal]`.

- [ ] **Step 1: Build the strip** below the hero with a hazard-chip header `// WHY GIT IT` and three items (icon/`OP`-mono label + bold line + supporting sentence), verbatim from spec §7③:
  - **Commit-time editing** — "reshape *when* history happened, right in the client. A rarity in any git GUI."
  - **A safety net for every rewrite** — "automatic backup (a git bundle) + one-click undo before any destructive op, plus a reflog browser."
  - **Local-first & free** — "no account, no analytics; native (no Electron); source-available for noncommercial use."
- [ ] **Step 2: Style** as a 3-col grid (stacks to 1-col under 760px), each item a corner-bracket mini-panel; add `data-reveal` with staggered nth-child transition-delays (inside the no-preference query).
- [ ] **Step 3: Verify.** Reload, `preview_snapshot` → three items with exact text. `preview_resize mobile` → stacks, screenshot. No horizontal scroll (`preview_eval`).
- [ ] **Step 4: Commit.** `git commit -am "feat: what's-different reason-to-switch strip"`

---

## Task 5: Spec band

**Files:** Modify `index.html`, `styles.css`
**Interfaces:** Consumes tokens, `.wrap`.

- [ ] **Step 1: Port the spec band** from the mockup, updating the Engine + Price cells: `Platform / macOS 12.3+` · `Architecture / Apple Silicon · Intel` · `Engine / Rust · Tauri 2 · Svelte 5 (no Electron)` · `Price / Free · Source-available`.
- [ ] **Step 2: Verify.** `preview_inspect .spec` grid; `preview_resize` at 760 → confirm 4→2 columns; screenshot. `preview_snapshot` → all four labels/values.
- [ ] **Step 3: Commit.** `git commit -am "feat: spec band"`

---

## Task 6: Feature-panel component + `OP // GRAPH` and `OP // TIME`

**Files:** Modify `index.html` (add `<section id="features">`), `styles.css`
**Interfaces:** Consumes `.console`/`.con-*`/`.brk` from Task 3. Produces the `.op-panel` component (2-col: copy + illustration; `.op-panel.reverse` flips), `.op-code` mono codename tag, and the illustration conventions (inline SVG, tokens for colors, `viewBox` ~360×300, mono `<text>`, corner-bracket frame, `data-reveal`).

- [ ] **Step 1: Build `.op-panel`.** A responsive 2-col grid (copy | HUD illustration), `.reverse` swaps order; stacks to 1-col under 900px. Copy column = `.op-code` (e.g. `OP // GRAPH`, orange mono) + `<h2>` (Anton, sentence headline) + `<p>` body + a mono sub-point list. Illustration column = a `.console` HUD frame (reuse Task 3 classes) containing an inline SVG diagram. Each panel gets `data-reveal`.
- [ ] **Step 2: `OP // GRAPH` — "Read history at a glance."** Body + sub-points verbatim from spec §7⑤(1). Illustration: an enlarged commit graph (adapt the hero console SVG) — 5–6 nodes across 2 lanes, curved+angular edges, ref-badge chips (`main`, `origin/main`, `v1.0`, `HEAD`) as small bordered mono labels, one selected row highlighted. `aria` describes it.
- [ ] **Step 3: `OP // TIME` — "Commit-time editing." (largest panel).** Body verbatim from spec §7⑤(2). Illustration (the signature — make it the richest): a vertical timeline of nodes with mono timestamps; show a selected range bracketed `SELECTED`; a `REWRITE ⟳` control with three chips `OFFSET · EXACT · COMPRESS`; two nodes visibly morphing time (`14:22 → 09:05`, struck-through old in `--haze-dim`, new in `--phosphor`); a `PREVIEW / UNDO` footer strip. Provide it as inline SVG following the console conventions. Give this panel more vertical room (full-bleed-ish, not `.reverse`).
- [ ] **Step 4: Verify.** Reload. `preview_snapshot` → both panels, exact headlines/bodies. `preview_inspect .op-code` → orange mono. `preview_resize` mobile → panels stack, illustration below copy; screenshot both panels. No horizontal scroll (`preview_eval`). `preview_console_logs` clean.
- [ ] **Step 5: Commit.** `git commit -am "feat: feature-panel component + GRAPH & TIME"`

---

## Task 7: `OP // MERGE` and `OP // REWRITE`

**Files:** Modify `index.html`, `styles.css`
**Interfaces:** Consumes `.op-panel` + illustration conventions from Task 6.

- [ ] **Step 1: `OP // MERGE` — "Integrate, and resolve."** Body verbatim from spec §7⑤(3). Illustration: two lanes converging into a merge node; a small conflict panel with `USE OURS` / `USE THEIRS` toggle chips and one `CONFLICT` marker in `--alert` (the single hazard use). `.reverse` layout.
- [ ] **Step 2: `OP // REWRITE` — "Rewrite history — safely."** Body verbatim from spec §7⑤(4). Illustration: an interactive-rebase list — rows with drag handles and `pick / squash / drop / reword` mono chips (one row `squash`, one `drop` struck-through) + a `BACKUP CREATED ✓` readout in `--phosphor` and an `UNDO` control.
- [ ] **Step 3: Verify.** `preview_snapshot` → both panels exact text; confirm `--alert` used only on the conflict marker (`preview_inspect`). Mobile screenshot. No h-scroll.
- [ ] **Step 4: Commit.** `git commit -am "feat: MERGE & REWRITE panels"`

---

## Task 8: `OP // DIFF`, `OP // REMOTE`, multi-repo line

**Files:** Modify `index.html`, `styles.css`
**Interfaces:** Consumes `.op-panel` + conventions.

- [ ] **Step 1: `OP // DIFF` — "Stage exactly what you mean."** Body verbatim from spec §7⑤(5). Illustration: a split diff — two columns with line-number gutters, added lines with `+` in `--phosphor`, removed with `-` in `--alert`, and per-hunk `STAGE` toggle chips; one hunk shown staged.
- [ ] **Step 2: `OP // REMOTE` — "Push, pull, and GitHub — in one place."** Body verbatim from spec §7⑤(6) (include "credentials prompted on demand and never stored — a transient `0600` file, deleted immediately after"). Illustration: a streaming push/pull progress readout (mono lines + a progress bar) with an `↑2 ↓1` ahead/behind counter, beside a mini GitHub panel with `PR`, `ISSUE`, `CI ✓` chips. `.reverse`.
- [ ] **Step 3: Multi-repo line.** Under the feature grid, one compact centered mono line: `Also — open several repos at once, as tabs or a sidebar list, your choice.`
- [ ] **Step 4: Verify.** `preview_snapshot` → both panels + multi-repo line exact text. Mobile screenshots. No h-scroll. Console clean.
- [ ] **Step 5: Commit.** `git commit -am "feat: DIFF & REMOTE panels + multi-repo line"`

---

## Task 9: Download section

**Files:** Modify `index.html` (`<section id="download">`), `styles.css`
**Interfaces:** Consumes hazard divider, `.btn*`, mono readout.

- [ ] **Step 1: Build the section** with a hazard divider chip `// DOWNLOAD` above. Headline "Download Git It." Sub (verbatim spec §7⑥). Two large arch buttons: `Apple Silicon (.dmg)` and `Intel (.dmg)`, both → `https://github.com/ashproto/git-it/releases/latest`, with a mono note "each runs fully native; in-app updates track the right build."
- [ ] **Step 2: Trust + requirements lines (exact).** Trust: **"Signed & notarized by Apple — opens with a normal double-click."** Requirements readout (mono): `macOS 12.3+ · needs git + python3 on PATH (xcode-select --install)`. **Do not** include any Gatekeeper/right-click language here.
- [ ] **Step 3: (Optional, app-repo dependency) direct links.** If arch-stable asset names are confirmed available (see spec §9.6), point each button at `…/releases/latest/download/Git-It-aarch64.dmg` / `…-x64.dmg` instead. Otherwise keep `/releases/latest`. Add an HTML comment noting the swap point.
- [ ] **Step 4: Verify.** `preview_snapshot` → headline, both buttons + hrefs, trust line, requirements line. **Grep-check no forbidden copy:** `preview_eval` returns `document.body.innerText.includes('right-click') || document.body.innerText.toLowerCase().includes('gatekeeper')` → must be `false`. Screenshot desktop + mobile.
- [ ] **Step 5: Commit.** `git commit -am "feat: download section (notarized, git+python3)"`

---

## Task 10: Guides — in-page section + `guides.html`

**Files:** Modify `index.html` (`<section id="guides">`), Create `guides.html`, Modify `styles.css`
**Interfaces:** Consumes tokens, nav, footer (footer added in Task 12 — for now include the nav + a minimal footer stub, reconciled in Task 12).

- [ ] **Step 1: In-page guides section.** Two cards linking to `guides.html#getting-started` and `guides.html#commit-time-editing`, each with a mono title + one-line description, styled as "MAGI briefing" panels.
- [ ] **Step 2: Build `guides.html`.** Same `<head>` pattern (title "Git It — Guides"), nav, and a `.wrap` body with two guides using mono step numbers:
  - `#getting-started`: install (`xcode-select --install` for git + python3), download the notarized `.dmg`, drag to Applications, open your first repo.
  - `#commit-time-editing`: select a commit range → choose Offset / Exact / Compress → Preview → Apply → Undo if needed. Note the automatic backup.
  Reuse `styles.css`; add minimal `.guide`/`.step` styles.
- [ ] **Step 3: Verify.** `preview_snapshot` of `index.html` guides section (two cards, correct hrefs). Navigate `preview_eval` `location.href='http://localhost:4321/guides.html'`; `preview_snapshot` → both guides render in-system; `preview_console_logs` clean. Screenshot guides page.
- [ ] **Step 4: Commit.** `git commit -am "feat: guides section + guides.html"`

---

## Task 11: FAQ section

**Files:** Modify `index.html` (`<section id="faq">`), `styles.css`
**Interfaces:** Consumes tokens.

- [ ] **Step 1: Build the FAQ** as semantic `<details>/<summary>` items (accessible, no JS), using the **exact** eight Q&As from spec §7⑧ (privacy precise; "Do I have to bypass Gatekeeper? No — signed & notarized…"; why-macOS; safe-rewrites; what-you-need git+python3; updates; arch; license = CC BY-NC-SA 4.0 with the Corridor-style clarification, never "open source").
- [ ] **Step 2: Verify.** `preview_snapshot` → all questions present. **Forbidden-copy check:** `preview_eval` `document.body.innerText.toLowerCase().includes('open source')` → the ONLY allowed occurrence is inside "not an OSI open-source license"; confirm no bare "open source" claim. `preview_click` a `<summary>` → `preview_snapshot` shows the answer. Mobile screenshot.
- [ ] **Step 3: Commit.** `git commit -am "feat: FAQ (accessible details/summary)"`

---

## Task 12: Community + footer

**Files:** Modify `index.html` (community section + final footer), `guides.html`/`404.html` footers, `styles.css`
**Interfaces:** Consumes tokens, `.wrap`. Finalizes the footer stub from Task 10.

- [ ] **Step 1: Community section.** "Built in the open." Links: Source (repo), Issues (`/issues`). **Only include a Discussions link if confirmed enabled** (spec §9.7) — otherwise omit. Pair with the license line + the OSI footnote. "Contributions welcome" only alongside a one-line note: "contributions accepted under the project's CC BY-NC-SA 4.0 license."
- [ ] **Step 2: Footer.** Three columns (Product / Source / Legal). Legal → License (link to CC BY-NC-SA 4.0 deed + the repo LICENSE), Privacy. **Site-scoped privacy line (exact):** "This site collects nothing — no analytics, no cookies, no third-party requests." Add a third-party acknowledgements line: "Bundles git-filter-repo (MIT)." Wordmark, "Made for macOS", `© 2026`, `git-it.app`.
- [ ] **Step 3: Verify.** `preview_snapshot` → footer columns + exact privacy line + license link (no bare "open source"). Confirm Discussions link absent unless enabled. Screenshot.
- [ ] **Step 4: Commit.** `git commit -am "feat: community + footer"`

---

## Task 13: 404 page + OG image + favicon set

**Files:** Create `404.html`, `assets/img/og.png`, `assets/img/favicon.ico`, `assets/img/apple-touch-icon.png` (favicon.svg exists from Task 1)
**Interfaces:** Consumes `styles.css`, glyph.

- [ ] **Step 1: `404.html`.** In-system page: hazard chip `// SIGNAL LOST`, big Anton "404", a mono line "This route isn't in the MAGI system.", and a `Return to base ↩` link home. Reuse `styles.css` + nav.
- [ ] **Step 2: OG card.** Create `assets/img/og.png` (1200×630): void ground, the glyph, "Git It" (Anton), tagline "Bend the timeline.", and "native macOS git client" mono. (Author an SVG at 1200×630 and rasterize, or hand-compose; must be a real PNG at the path the head references.)
- [ ] **Step 3: Favicons.** Generate `favicon.ico` (multi-size) and `apple-touch-icon.png` (180×180, void bg, glyph) from `favicon.svg`.
- [ ] **Step 4: Verify.** `preview_eval` navigate to a bad path → `preview_snapshot` shows 404 content (note: python http.server returns its own 404 body; verify `404.html` renders correctly when opened directly at `/404.html`, since GitHub Pages — not the local server — serves it for unknown routes). Confirm `assets/img/og.png`, `favicon.ico`, `apple-touch-icon.png` load (`preview_network` no failures). Screenshot 404.
- [ ] **Step 5: Commit.** `git commit -am "feat: 404 page, OG card, favicon set"`

---

## Task 14: Final QA — a11y, performance, no-tracking, responsive, link audit

**Files:** Possibly small fixes across `index.html`, `guides.html`, `404.html`, `styles.css`, `app.js`
**Interfaces:** None new — this is the acceptance gate against spec §1 success criteria and §8.

- [ ] **Step 1: No third-party requests.** `preview_network` (filter all) → confirm every request is same-origin (localhost). Zero external hosts (fonts self-hosted). Record the list.
- [ ] **Step 2: No horizontal scroll at all breakpoints.** For each of 320, 375, 768, 1280: `preview_resize` then `preview_eval` `document.documentElement.scrollWidth <= window.innerWidth` → must be `true`. Screenshot each.
- [ ] **Step 3: Reduced motion.** Confirm in code that all animations/transitions are inside `@media (prefers-reduced-motion: no-preference)` and `[data-reveal]` elements are visible (`.in-view`) under reduced motion. `preview_resize colorScheme:dark` sanity screenshot (site is dark-native).
- [ ] **Step 4: Contrast + focus.** `preview_inspect` body text (`--bone`/`--haze` on `--void`) → verify AA (≥4.5:1 for body, ≥3:1 for large). Tab through interactively (`preview_eval` focus) → every link/button/`summary` has a visible focus ring.
- [ ] **Step 5: Copy-guard sweep.** `preview_eval` over `document.body.innerText` on index + guides: assert it does NOT contain "100% local", "right-click", "gatekeeper" (except build-from-source aside), or a bare "open source" (only inside "not an OSI open-source license"); assert it DOES contain "Signed & notarized", "git + python3", "CC BY-NC-SA 4.0".
- [ ] **Step 6: Link audit.** `preview_snapshot` → collect all `href`s; confirm external links point to `github.com/ashproto/git-it…` and internal anchors (`#features/#download/#guides/#faq`) resolve to real ids. No dead links.
- [ ] **Step 7: Lighthouse (if available).** Run a Lighthouse pass (chrome-devtools `lighthouse_audit` if available, else note manual check) targeting ~100 Perf/A11y/Best-Practices/SEO; fix any low-hanging findings (missing `lang`, alt text, meta).
- [ ] **Step 8: Commit.** `git commit -am "chore: final QA — a11y, no-tracking, responsive, copy-guard"`

---

## Deployment (post-implementation, requires user)

Not code tasks — checklist for going live (spec §3, §9):
1. Push `git-it-site` to a new **public** `ashproto/git-it-site` repo; enable GitHub Pages (branch `main`, root); confirm the `CNAME`/`git-it.app` DNS + HTTPS cert.
2. **App-repo pre-launch (spec §9.5–9.7):** commit `LICENSE` (CC BY-NC-SA 4.0) + README licensing note + third-party-licenses (git-filter-repo MIT) to `ashproto/git-it`; optionally add the release-pipeline rename step for one-click asset URLs (then flip Task 9 Step 3); confirm/disable the Discussions link.
3. Verify the live site's download buttons resolve to a real public release.

---

## Self-Review

- **Spec coverage:** nav ✓(T2) hero ✓(T3) what's-different ✓(T4) spec band ✓(T5) 6 features + multi-repo ✓(T6–8) download ✓(T9) guides ✓(T10) FAQ ✓(T11) community+footer ✓(T12) 404/OG/favicons ✓(T13) tokens/fonts/meta/JSON-LD/manifest/sitemap/robots ✓(T1) a11y/perf/no-tracking/responsive ✓(T14). License framing, notarized copy, privacy precision, no-version, no-external-requests → encoded in Global Constraints + per-task copy-guard checks.
- **Placeholder scan:** copy is verbatim from spec; illustrations specified as concrete build briefs following the Task 3 console template (a worked example) — no "TBD".
- **Type consistency:** shared class names (`.wrap`, `.btn*`, `.reveal`/`.d*`, `[data-reveal]`, `.console`/`.con-*`/`.brk`, `.op-panel`/`.op-code`, `.hazard`, `.eyebrow`, `.readout`) are defined once (T1–T3, T6) and consumed by name thereafter.
