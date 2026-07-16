# Git It — Landing Page Design Spec

- **Date:** 2026-07-06 · **Rev:** 2 (incorporates the three-lens adversarial review)
- **Status:** Draft for your sign-off
- **Owner:** Ash Shah
- **Deliverable:** A static marketing landing page for the Git It macOS app, hosted on GitHub Pages, in the spirit of the Resume-Designer site but with its own dark "NERV console" visual identity.

---

## 1. Goals & non-goals

**Goals**
- Present Git It as a fast, native macOS git client whose signature is **commit-time editing**.
- Give a clear, trustworthy **download** path (Apple Silicon + Intel `.dmg`).
- Provide **light guides** (install/prerequisites + a commit-time-editing walkthrough).
- Establish a distinctive identity: "clean, minimal, but a bit Neon-Genesis-Evangelion," landed at the **Balanced NERV console** level.
- Self-contained static site: no backend, no accounts, **no tracking/analytics of any kind**.

**Non-goals**
- No store, payments, waitlists, or auth. No live product embed / web app. No blog/CMS. Not a docs site (deep reference stays in the repo README).

**Success criteria**
- A visitor understands what Git It is within the first screen and reaches the correct `.dmg` in one click.
- **Every claim on the page matches the *shipped* build** (not the local ad-hoc build) — verified against the repo + release pipeline.
- Lighthouse ≈ 100 across Performance / Accessibility / Best-Practices / SEO.
- Zero third-party network requests at runtime (fonts self-hosted; no CDNs; no analytics).

---

## 2. Audience & the page's single job

**Audience:** macOS developers who use git daily and are dissatisfied with existing clients (Fork/Tower/SourceTree/GitKraken), plus people specifically after commit-time editing / history rewriting with a safety net.

**The one job:** convince a macOS developer Git It is worth downloading, and get them to the right `.dmg`. Everything else supports that. **Because the audience is defined by dissatisfaction with other clients, the page must state a reason to switch — not just list parity features** (see §7 ③).

---

## 3. Hosting & deployment

- **Repo:** new **public** repo `ashproto/git-it-site` (decoupled from the app repo).
- **Build/deploy:** plain static files, no build step. GitHub Pages from `main` (root). Optional Actions deploy only if a build step is later added.
- **Custom domain:** **`git-it.app`** via `CNAME` + DNS. (You own `git-it.app`, `git-it.dev`, `git-it.ai`, `getgitit.com` — switching is a one-line `CNAME`. `.app` is HSTS-preloaded, so HTTPS is mandatory; GitHub Pages provisions the cert.)
- **Launch model (decided):** **public releases go live first.** So download buttons resolve to real releases and the `LICENSE` is committed before the site claims any grant — no "coming soon" state needed as the default. (A minimal graceful fallback is still built in case `latest` is briefly absent.)
- **Download link strategy:** the release pipeline names DMGs with the embedded version (`Git-It_<ver>_aarch64.dmg` / `_x64.dmg`), so stable direct-asset URLs are **not** available by default. Two paths:
  - **(Default)** buttons deep-link to `https://github.com/ashproto/git-it/releases/latest` with clear per-arch labels ("Apple Silicon" / "Intel") and a one-line "opens the latest release — grab the build for your Mac."
  - **(Recommended enhancement, app-repo change)** add a rename step to `release.yml` producing arch-stable names (`Git-It-aarch64.dmg`, `Git-It-x64.dmg`) so `…/releases/latest/download/<name>` gives true **one-click** downloads. Flagged in §9.

---

## 4. Licensing & how the site describes it

**Decision:** **CC BY-NC-SA 4.0** (Attribution–NonCommercial–ShareAlike), mirroring Corridor Digital's *CorridorKey* precedent. It is the single well-known license that is *both* copyleft (ShareAlike) *and* strictly noncommercial — the closest match to "like AGPL, but noncommercial."

**Plain-English clarification (ships in the app repo README + linked from the site), Corridor-style:**
> Use Git It for any work you like — including your job and commercial projects. The noncommercial term is about **the app itself**: you may not resell it, repackage-and-sell it, or run it as a paid hosted service. Modify and share it freely; derivatives must stay under this same license (CC BY-NC-SA 4.0). Want a commercial arrangement the license doesn't cover? Get in touch.

**Site copy framing:**
- The site says **"Free · source-available · noncommercial (CC BY-NC-SA 4.0)."**
- It **must not** use "open source" — a noncommercial field-of-use restriction fails the OSI definition (OSD §6). This reasoning is correct and deliberate.
- "Built in the open" is fine **only when paired with the license line**. Footnote wherever license appears: *"Source-available under CC BY-NC-SA 4.0 — not an OSI open-source license."*

**Caveats & obligations (baked into the plan):**
- CC 4.0 isn't drafted for software (no explicit patent/source-vs-object provisions); acceptable for this project and consistent with the referenced precedent. The README clarification handles the software-vs-output ambiguity (and Git It's "outputs" are your own commits anyway).
- **Third-party:** the bundled `git-filter-repo` is **MIT** (`src-tauri/resources/git-filter-repo/COPYING.mit`). Its notice must ship in a **third-party-licenses / acknowledgements** section of the app. Git It's CC BY-NC-SA license applies only to Git It's own code.

**Pre-launch action (app repo — separate from this site, tracked in §9):** commit the `LICENSE` (CC BY-NC-SA 4.0) + the README licensing note. This **overrides the current `CLAUDE.md` "do not add a LICENSE" instruction** and flips `package.json` `"license"` off `UNLICENSED`. Because releases go live first, this happens before the site makes any grant claim — the sequencing risk the review flagged is resolved.

---

## 5. Visual system

Dark, technical, editorial — a NERV terminal crossed with an EVA episode title-card. Orange leads; green is data-only; red is a single hazard signal; warm bone off-white is the title-card contrast.

### 5.1 Color tokens
| Token | Hex | Role |
|---|---|---|
| `--void` | `#0A0C0F` | primary ground (cool near-black) |
| `--void2` | `#0E1216` | raised ground / cells |
| `--panel` | `#12171C` | HUD panels |
| `--line` | `#232A31` | hairlines, grid |
| `--line2` | `#2E3742` | stronger borders |
| `--bone` | `#EAE6DA` | headlines, title-card, primary text (chosen warm neutral) |
| `--haze` | `#8A94A0` | secondary/mono text (steel-blue) |
| `--haze-dim` | `#5A636E` | tertiary/labels |
| `--orange` | `#F2542D` | **dominant accent** — CTAs, marks, hazard |
| `--orange-deep` | `#C23A18` | pressed/borders |
| `--phosphor` | `#46E88B` | **data/status only** — live/OK/added |
| `--alert` | `#FF4438` | single hazard/conflict signal, sparingly |

Accent discipline: orange is the only "brand" accent; green/red are **semantic** (status / hazard), never decorative — the deliberate move away from the "near-black + one green pop" AI-design cliché.

### 5.2 Typography (self-hosted woff2, subsetted)
- **Display** (title-card mega-type + operation headers): **Anton** — condensed, heavy, all-caps, poster-grade; used sparingly.
- **Body** (running copy, UI): **IBM Plex Sans** — engineering-flavored grotesque; deliberately *not* Inter/Space Grotesk (flagged generic defaults).
- **Utility/data** (eyebrows, labels, readouts, code): **IBM Plex Mono**.
- Rules: running text ≈ 65ch; fixed `clamp()` scale; `text-wrap: balance` on headings; letter-spacing on uppercase labels; `font-variant-numeric: tabular-nums` on aligned digits.

> The current hero mockup uses Mac-local **Futura Condensed** as a stand-in; the shipped site self-hosts **Anton + IBM Plex Sans/Mono** so it renders identically for everyone and makes zero external font requests. The mockup will be re-rendered with the real fonts (and the copy fixes below) so we approve the true result.

### 5.3 Motifs & structure
- **HUD framing** (corner brackets, steel hairlines, mono header/footer bars with status ticks).
- **Hazard stripes** as major dividers, each with a mono chip (`// WHAT'S DIFFERENT`, `// OPERATIONS`, `// DOWNLOAD`).
- **AT-field hexagons** as faint ambient decoration.
- **Operation codenames, not `01/02/03`** — features tagged `OP // GRAPH`, `OP // TIME`… encoding category, not a false sequence.
- Subtle scanlines + faint grid overlay (very low opacity), fixed.

### 5.4 Motion (all `prefers-reduced-motion`-gated)
- **One orchestrated boot-reveal** on load: eyebrow → headline (masked slide-up, staggered) → subhead → CTAs → readouts → console draws in.
- **Scroll-reveal** for panels via `IntersectionObserver` (once).
- **Ambient:** slow scanline drift; a slowly-rotating AT-field hexagon behind the hero console.
- **Micro-interactions:** CTA corner-brackets expand on hover; nav caret; blinking "LIVE" dot.
- Reduced-motion resolves everything to its final state immediately.

### 5.5 Product imagery = **themed illustrations** (decided) + honesty guardrails
No app screenshots. Every product concept is drawn in the site's own NERV/HUD language (SVG-first; Canvas only where motion warrants). To keep the "is this a real app?" trust signal (the review's flagged risk) **without reversing the decision**:
- Present illustrations honestly as **diagrams that explain a concept**, never as fake app chrome.
- A mono microline near the hero: **"Real app, shipping — see it on GitHub ↗."**
- Let the strongest proof carry it: a genuinely **notarized** download, and the GitHub repo (which has real screenshots) one click away.

---

## 6. Wordmark & logo (site-specific)
- A **site-only** mark, independent of the placeholder app icon.
- **Glyph:** a hexagon (AT-field) enclosing a 3-node commit graph forming a subtle checkmark (orange/green/bone nodes).
- **Wordmark:** "GIT IT" in Anton for hero/brand lockups; mono "Git It" in nav.
- **Icons/meta:** hexagon glyph as `favicon.svg` + `.ico`; `apple-touch-icon.png`; minimal `site.webmanifest`; a dedicated 1200×630 Open Graph card (hero-styled).

---

## 7. Site map & section specs

Single long-scroll `index.html` + a separate `guides.html`.

### ① Nav (sticky)
Glyph + "Git It" (mono) · `Features · Download · Guides · GitHub` · orange bracketed **`↓ Download`**. Collapses to GitHub + Download on small screens.

### ② Hero  *(mocked ✓ — copy to be updated per below)*
- Eyebrow (**elevated to bone**, doing real work): `◇ native macOS git client · Apple Silicon + Intel`.
- Headline: **"Bend the / timeline."** (bone, Anton). No hard-coded version tag.
- Annotation (adjacent to headline, visible above the fold at 375px): `Commit-time editing — shift, set, or compress any range. Preview, then undo.`
- Subhead: "A fast, native git client for macOS — commit graph, branches, merges, per-hunk diffs, remotes, and a built-in GitHub dashboard. Local-first: no account, no analytics, no tracking."
- Microline: `Real app, shipping — see it on GitHub ↗`.
- CTAs: **`↓ Download for macOS`** (primary) · `View on GitHub ↗` (ghost).
- Readouts: `No account · No tracking · Native — no Electron · ● Auto-updates`.
- Visual: HUD **commit-graph console** with the `REWRITE ⟳` timestamp-shift annotation.

### ③ What's different  *(NEW — the reason-to-switch strip, above the feature grid)*
Three bullets, hazard-chip header `// WHY GIT IT`:
- **Commit-time editing** — reshape *when* history happened, right in the client. A rarity in any git GUI.
- **A safety net for every rewrite** — automatic backup (a git bundle) + one-click undo before any destructive op, plus a reflog browser.
- **Local-first & free** — no account, no analytics; native (no Electron); source-available for noncommercial use.

### ④ Spec band  *(mocked ✓)*
Four mono cells: `Platform / macOS 12.3+` · `Architecture / Apple Silicon · Intel` · `Engine / Rust · Tauri 2 · Svelte 5 (no Electron)` · `Price / Free · Source-available`.

### ⑤ Features — six `OP //` panels, alternating left/right; each = codename + headline + 1–2 sentences + sub-points + themed SVG illustration
1. **`OP // GRAPH` — "Read history at a glance."** Multi-lane commit graph with curved or angular edges, ref badges (branches, tags, remotes, `HEAD`), infinite-scroll loading, right-click checkout / branch / tag / fetch.
2. **`OP // TIME` — "Commit-time editing."** *(largest panel — the signature)* Select a range and shift by an offset, set an exact time, or compress the range proportionally into a new window. Preview before it rewrites; undo in one click. Powered by a **bundled** `git-filter-repo`.
3. **`OP // MERGE` — "Integrate, and resolve."** Merge (plain or `--no-ff`), cherry-pick, and revert — with an in-app conflict resolver: use-ours, use-theirs, continue, or abort, file by file.
4. **`OP // REWRITE` — "Rewrite history — safely."** Soft/mixed/hard reset, amend, rebase-onto, and interactive rebase (reorder / squash / drop / reword). Every destructive op takes a configurable auto-backup (a git bundle) first, with one-click undo and a reflog browser.
5. **`OP // DIFF` — "Stage exactly what you mean."** Working-copy file list with whole-file and **per-hunk** stage / unstage / discard, syntax-highlighted diffs (unified or split, highlighted offline via bundled Shiki), a commit composer, and stash.
6. **`OP // REMOTE` — "Push, pull, and GitHub — in one place."** Streamed pull (merge or rebase) and push (`--force-with-lease`, `--set-upstream`) with live progress, cancel, and an ahead/behind indicator. Plus a built-in GitHub dashboard: pull requests, issues, releases, and CI runs. Credentials are prompted on demand and never stored (a transient `0600` file, deleted immediately after the op).

*Also (one compact line under the grid):* **"Open several repos at once — as tabs or a sidebar list, your choice."** (multi-repo).

### ⑥ Download  *(hazard-divider `// DOWNLOAD` above; the key CTA)*
- Headline: "Download Git It." Sub: "Open the `.dmg`, drag Git It into Applications, launch. It keeps itself up to date after that — with an optional beta channel in Settings → Updates."
- **`Apple Silicon (.dmg)`** · **`Intel (.dmg)`** → GitHub Releases (`/releases/latest`; or direct one-click if the pipeline-rename enhancement lands). Mono line: "each runs fully native; in-app updates track the right build."
- Trust line (**corrected — the release is notarized**): **"Signed & notarized by Apple — opens with a normal double-click."**
- Requirements readout (mono): `macOS 12.3+  ·  needs git + python3 on PATH (xcode-select --install)`.
- (Any Gatekeeper right-click caveat appears **only** in the "Build from source" note, which describes an ad-hoc local build — not the download.)

### ⑦ Guides (light)
In-page section, two cards → `guides.html`: **"Getting started"** (install, prerequisites, first repo) and **"Commit-time editing"** (select range → offset / exact / compress → preview → apply → undo), styled as a "MAGI briefing." `guides.html` reuses the system; short, scannable, mono step numbers.

### ⑧ FAQ (real, corrected)
- **Is it really private?** No account, no backend, no analytics. Your repos and credentials stay on your Mac. The only network calls are the git/GitHub operations you trigger, plus a version check for updates. Credentials are prompted on demand and never written to disk beyond a transient `0600` file that's deleted right after.
- **Do I have to bypass Gatekeeper?** No — the release is **signed and notarized by Apple**; it opens with a normal double-click. (Only self-built ad-hoc copies trigger the Gatekeeper prompt.)
- **Why macOS only?** Native vibrancy and title-bar handling are macOS-specific. Apple Silicon and Intel builds are both fully native (no Electron).
- **Is rewriting history safe?** Destructive ops take an automatic git-bundle backup first, force-push is always `--force-with-lease`, and there's one-click undo plus a reflog browser.
- **What do I need installed?** `git` and `python3` — both from the Xcode Command Line Tools (`xcode-select --install`). `git-filter-repo` is bundled.
- **How do updates work?** In-app auto-updates, with an optional beta channel in Settings → Updates.
- **Apple Silicon or Intel?** Download the `.dmg` that matches your Mac; each runs fully native. Updates then track the right build.
- **What's the license?** Free and **source-available under CC BY-NC-SA 4.0** — use it for any work (including at your job); you just can't resell or repackage the app itself, and forks stay under the same license. Not an OSI open-source license.

### ⑨ Open source / community
"Built in the open." Links: source on GitHub, issues. **Discussions link only if Discussions is actually enabled on the repo** (else drop it). Framing per §4 (never "open source"). "Contributions welcome" is paired with a one-line `CONTRIBUTING`/DCO note ("contributions accepted under the project's CC BY-NC-SA 4.0 license") so framing doesn't outrun the legal setup.

### ⑩ Footer
- Columns: Product (Features, Download, Guides) · Source (GitHub, Releases, Issues) · Legal (License, Privacy).
- **Privacy line, correctly scoped:** "**This site collects nothing** — no analytics, no cookies, no third-party requests." (Product privacy detail lives in the FAQ.)
- Third-party acknowledgements link (git-filter-repo · MIT). Wordmark, "Made for macOS," `© 2026`, domain.

---

## 8. Technical approach

- **Stack:** hand-authored `index.html`, `guides.html`, `404.html`; one `styles.css`; one small `app.js` (boot reveal, nav, `IntersectionObserver` scroll-reveal). No framework, no build step.
- **Assets:** SVG for illustrations/icons; self-hosted subsetted **woff2** (Anton + IBM Plex Sans/Mono) via `@font-face`, `font-display: swap`.
- **Performance:** single CSS file (critical CSS inlined); `defer` `app.js`; lazy-render below-fold illustrations; no CLS; target ~100 Lighthouse.
- **Accessibility:** semantic landmarks; visible focus; `prefers-reduced-motion` honored; descriptive `alt`/`aria` on diagrams; verified contrast (bone/haze on void meet AA; orange/green for accents/large text, not small body copy).
- **Privacy/no-tracking:** zero analytics, zero third-party requests, no cookies.
- **SEO/meta:** `<title>`, description, canonical, Open Graph + Twitter card (custom 1200×630 image), **`SoftwareApplication` JSON-LD** (name, `operatingSystem: macOS`, `applicationCategory: DeveloperApplication`, `offers` = free), `theme-color: #0A0C0F`, favicon set, `apple-touch-icon`, `site.webmanifest`, `sitemap.xml`, `robots.txt`.
- **404:** `404.html` styled in-system (`// SIGNAL LOST`, link home) — GitHub Pages serves it automatically.
- **Responsive:** mobile-first; hero stacks (console below copy); panels stack; spec band 4→2 cols; **verify at 375px that "native macOS git client" is visible without scrolling**; no horizontal body scroll (wide content scrolls in its own container).

---

## 9. Open decisions & pre-launch checklist

**Non-blocking design decisions (answer at review; I have recommendations):**
1. **Domain:** `git-it.app` recommended — OK, or `git-it.dev` / `git-it.ai` / `getgitit.com`?
2. **Fonts:** Anton + IBM Plex Sans/Mono — approve, or see alternatives first?
3. **Feature count:** six `OP //` panels — keep six, or fold to five and give `OP // TIME` more room?
4. **Guides depth:** two short guides at launch — enough, or add more?

**Pre-launch actions (outside this repo; required before the site claims grants — sequencing is fine since releases go live first):**
5. **App repo:** commit `LICENSE` (CC BY-NC-SA 4.0) + README licensing note; add a **third-party-licenses** section including git-filter-repo (MIT). Overrides `CLAUDE.md`'s "do not add a LICENSE."
6. **App repo (recommended):** add the release-pipeline **rename step** for arch-stable DMG names → true one-click downloads.
7. **Confirm** GitHub **Discussions** is enabled (or the community link is dropped).

---

## 10. Out of scope / future
- Real app icon / brand system beyond the site mark.
- Windows/Linux (app is macOS-only).
- Localization, blog, changelog page, in-page search.
