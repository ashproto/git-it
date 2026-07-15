---
version: alpha
name: Git It
description: >-
  Design language for the Git It macOS git client. Two themes share ONE
  semantic-token contract so users can toggle between them at runtime:
  "Classic" — the native-macOS vibrancy UI shipping today — and "NERV" —
  the dark editorial console language from the git-it.app marketing site.
  Components reference semantic tokens by intent; switching theme swaps the
  values behind those tokens, never the component CSS.
# NOTE: the DESIGN.md standard is single-theme-per-file. This file extends it
# with a `themes` map so both appearances live in one contract, which is what
# the upcoming theming-system work needs. Values below are the CANONICAL
# (dark) values; Classic's full light / dark / glass triple and the complete
# graph-lane palettes are in the prose "Colors" section.

spacing: { xs: 4px, sm: 6px, md: 8px, lg: 12px, xl: 16px, xxl: 24px }

themes:
  classic:
    colors:
      bg-app:            "#0f1115"
      surface:           "#1b1f26"
      surface-raised:    "#181b20"   # popovers / menus / sticky headers
      surface-header:    "#1a1e24"   # toolbars / section headers / gutters
      surface-input:     "#1a1e24"
      control:           "#2a313b"
      control-hover:     "#353d48"
      border:            "#353c46"
      border-subtle:     "#20252c"
      text-primary:      "#e5e7eb"
      text-secondary:    "#9ca3af"
      text-tertiary:     "#9ca3af"   # Classic has two text levels; tertiary collapses to secondary
      row-hover:         "#1c2027"
      row-selected:      "#1e40af66"
      border-selected:   "#60a5fa"
      accent:            "#3b82f6"
      accent-hover:      "#2563eb"
      danger:            "#ef4444"
      warning:           "#fbbf24"   # detached HEAD / status-modified gold
      success:           "#3fb950"   # status-added
      diff-add:          "#3fb950"
      diff-del:          "#f85149"
    typography:
      display: { fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif", fontWeight: 600 }
      body:    { fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif", fontSize: 14px, fontWeight: 400 }
      mono:    { fontFamily: "ui-monospace, SFMono-Regular, Menlo, 'Cascadia Code', monospace", fontSize: 12px }
    rounded: { sm: 4px, md: 6px, lg: 10px, pill: 999px }
  nerv:
    colors:
      bg-app:            "#0A0C0F"   # --void
      surface:           "#12171C"   # --panel
      surface-raised:    "#0E1216"   # --void2
      surface-header:    "#0E1216"
      surface-input:     "#0E1216"
      control:           "#0E1216"
      control-hover:     "#14181d"
      border:            "#2E3742"   # --line2
      border-subtle:     "#232A31"   # --line
      text-primary:      "#EAE6DA"   # --bone
      text-secondary:    "#8A94A0"   # --haze
      text-tertiary:     "#7C8794"   # --haze-dim (AA on --void)
      row-hover:         "rgba(242,84,45,.06)"
      row-selected:      "rgba(242,84,45,.10)"
      border-selected:   "#F2542D"
      accent:            "#F2542D"   # --orange (the lead accent)
      accent-hover:      "#ff6a44"
      danger:            "#FF4438"   # --alert
      warning:           "#F2542D"   # NERV uses orange for warning; red reserved for hazard/conflict
      success:           "#46E88B"   # --phosphor (status / OK / added)
      diff-add:          "#46E88B"
      diff-del:          "#FF4438"
    typography:
      display: { fontFamily: "Anton, 'Arial Narrow', Impact, sans-serif", fontWeight: 400, letterSpacing: 0.01em }
      body:    { fontFamily: "'IBM Plex Sans', system-ui, -apple-system, sans-serif", fontSize: 14px, fontWeight: 400 }
      mono:    { fontFamily: "'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, monospace", fontSize: 12px }
    rounded: { sm: 0px, md: 2px, lg: 2px, pill: 999px }   # near-sharp; framing via corner brackets, not radius

components:
  button-primary:
    classic: { backgroundColor: "{classic.colors.accent}", textColor: "#ffffff", rounded: "{classic.rounded.md}" }
    nerv:    { backgroundColor: "{nerv.colors.accent}",    textColor: "{nerv.colors.bg-app}", rounded: "{nerv.rounded.md}" }
  panel:
    classic: { backgroundColor: "{classic.colors.surface}", rounded: "{classic.rounded.md}" }   # + macOS vibrancy in glass mode
    nerv:    { backgroundColor: "{nerv.colors.surface}",    rounded: "{nerv.rounded.md}" }       # + HUD corner brackets, no vibrancy
---

# Git It — Design Language

## Overview

Git It ships **one product with two skins**. This document defines the shared
**semantic-token contract** both skins implement, then describes each skin's
design language and how components render in it. A Classic↔NERV toggle in
**Settings → Appearance** ships this (`src/lib/components/SettingsPanel.svelte`).

- **Classic** — what ships today. A native-macOS, information-dense git client
  in the **Fork / Tower / SourceTree** lineage: monochrome-neutral chrome, a
  single **blue** accent, and its signature material — **real macOS vibrancy**
  (the desktop wallpaper frosts through translucent panels). Calm, professional,
  gets out of the way.
- **NERV** — the design language of the **git-it.app** marketing site brought
  back into the app. A dark **"NERV / EVA console"**: near-black ground, a lead
  **burnt-orange** accent, **phosphor-green** reserved for status, condensed
  display type (**Anton**) + **IBM Plex** body/mono, and HUD motifs (corner
  brackets, hazard stripes, hexagons, mono readouts). Opinionated, technical,
  memorable.

**The load-bearing idea for the theming phase:** NERV-as-an-app-theme is a
**re-skin of the same dense app**, not the marketing site's spacious editorial
layout. **Layout, density, spacing, and component structure stay identical
across themes.** What changes is *color, type, shape (radius), elevation
material, and decorative motif*. A user toggling to NERV gets the same
30px-row commit graph and the same panels — dressed in the console language.

### Token architecture (primitive → semantic → component)

Following design-token best practice, tokens are named **by intent, not
appearance** and resolved in three tiers:

1. **Primitive** — raw values (a hex, a px). Per-theme (`#3b82f6`, `#F2542D`).
2. **Semantic** — intent aliases that components actually use
   (`accent`, `surface`, `text-secondary`, `diff-add`). **The key set is
   identical across themes**; only the values differ. This is the contract.
3. **Component** — a component may map a semantic token to a local role
   (`button-primary.backgroundColor → {accent}`).

Because component CSS references **semantic** tokens only, switching a theme is
purely swapping the primitive→semantic value layer — no component change. Git It
**already proves this pattern**: today a `[data-tauri="true"]` attribute on
`<html>` re-declares the surface tokens as translucent `rgba()` to turn on glass
*without touching any component*. The theme toggle extends the exact same
mechanism with `[data-theme="nerv"]`.

---

## Colors

Both themes provide values for the **same semantic keys**. Classic is a
three-mode system (macOS chooses light/dark; glass activates in-app); NERV is
dark-only.

### Semantic color contract

| Semantic token | Role | Classic — light / dark / glass | NERV |
|---|---|---|---|
| `bg-app` | App background | `#eceef1` / `#0f1115` / `transparent`¹ | `#0A0C0F` |
| `surface` | Panels, cards, rows | `#ffffff` / `#1b1f26` / `rgba(255,255,255,.16)` · `rgba(28,32,39,.24)` | `#12171C` |
| `surface-raised` | Popovers, menus, sticky headers | `rgba(250,250,252,.85)` · `rgba(24,27,32,.88)` (glass) | `#0E1216` |
| `surface-header` | Toolbars, section headers, gutters | `#f1f3f5` / `#1a1e24` / translucent | `#0E1216` |
| `surface-input` | Form fields | `#f7f8fa` / `#1a1e24` / translucent | `#0E1216` |
| `control` | Button fill | `#e9ecef` / `#2a313b` / `rgba(…, .62–.66)`² | `#0E1216` (ghost + orange border) |
| `control-hover` | Button hover | `#dfe3e8` / `#353d48` / translucent | `#14181d` |
| `border` | Structural dividers | `#d6d9de` / `#353c46` / `rgba(255,255,255,.4)` · `rgba(120,130,142,.34)` | `#2E3742` |
| `border-subtle` | Fine row separators | `#eef0f3` / `#20252c` / translucent | `#232A31` |
| `text-primary` | Primary text | `#1a1d20` / `#e5e7eb` | `#EAE6DA` (warm bone) |
| `text-secondary` | Muted / secondary | `#6b7280` / `#9ca3af` | `#8A94A0` |
| `text-tertiary` | Labels, meta (AA-min) | (use `text-secondary`) | `#7C8794` |
| `row-hover` | Row hover | `#f5f7fa` / `#1c2027` | `rgba(242,84,45,.06)` |
| `row-selected` | Selected row fill | `#bfdbfe` / `#1e40af66` | `rgba(242,84,45,.10)` |
| `border-selected` | Selected row / focus edge | `#2563eb` / `#60a5fa` | `#F2542D` |
| `accent` | Lead accent / brand | `#2563eb` / `#3b82f6` (blue) | `#F2542D` (burnt orange) |
| `accent-hover` | Accent hover | `#1d4ed8` / `#2563eb` | `#ff6a44` |
| `success` | Added / OK / live | `#2da44e` / `#3fb950` | `#46E88B` (phosphor) |
| `warning` | Modified / detached HEAD | `#bf8700`·`#b45309` / `#e3b341`·`#fbbf24` | `#F2542D`³ |
| `danger` | Destructive / deleted | `#dc2626`·`#cf222e` / `#ef4444`·`#f85149` | `#FF4438` |
| `diff-add` | Diff added | `#2da44e` · bg `rgba(46,160,67,.18–.24)` | `#46E88B` · bg `rgba(70,232,139,.15)` |
| `diff-del` | Diff removed | `#cf222e` · bg `rgba(210,35,35,.18–.24)` | `#FF4438` · bg `rgba(255,68,56,.15)` |

¹ In glass mode the app background is `transparent` so the native `NSVisualEffect`
material (the desktop wallpaper, frosted) shows through. NERV has **no vibrancy** —
it is an opaque near-black ground.
² Button fill is deliberately *more opaque* than other surfaces in glass mode for
legibility.
³ NERV reserves **red (`danger`) for hazard/conflict only**; ordinary warnings use
the orange accent. Accent discipline: **orange leads, green is status-only.**

### Accent discipline (differs sharply between themes)
- **Classic:** one calm blue accent; color is used sparingly and semantically
  (status add/mod/del, ref lanes). Neutral-forward.
- **NERV:** **orange is the only brand accent**; **green (`success`) appears
  only for status/OK/added**; **red (`danger`) only for hazard/conflict**; a
  warm **bone** for headings. This restraint is what keeps NERV from reading as
  a generic dark theme — do not introduce decorative green or red.

### Commit-graph lane palette (semantic key `graph.lane.1…8`)
The graph cycles 8 lane colors, per-branch overridable. This is a **first-class
themeable token** — the graph is the centre of the app.

- **Classic** (`src/lib/graph/colors.ts`): `#378ADD` `#1D9E75` `#D85A30`
  `#7F77DD` `#BA7517` `#D4537E` `#639922` `#5F5E5A`.
- **NERV** (proposed — tune during implementation, keep 8 distinguishable hues
  in the NERV register): `#F2542D` (orange) `#46E88B` (phosphor) `#5AA9E6`
  (steel-cyan) `#9B7FE0` (violet) `#E8A33D` (amber) `#E0608A` (pink) `#EAE6DA`
  (bone) `#8A94A0` (haze). Ref badges tint from the lane color in both themes;
  ref *kind* is shown by the RefIcon glyph, not color — keep that.

**Diff syntax highlighting** is Shiki. Classic uses `github-light` / `github-dark`.
NERV should ship a **dark, orange/green-leaning Shiki theme** (or reuse a dark
theme tuned to the NERV palette) so code reads on the void ground.

---

## Typography

| Role | Classic | NERV |
|---|---|---|
| **Display** (titles, section labels) | System **San Francisco**, weight 600 (no heavy display face) | **Anton** — condensed, heavy, uppercase; the EVA title-card voice. Use sparingly (headers, `OP //` labels). |
| **Body / UI** | System SF stack, **14px** base, weights 400/500/600 | **IBM Plex Sans** at the same sizes/weights |
| **Mono** (diffs, SHAs, hashes, readouts) | **SF Mono** (`ui-monospace, SFMono-Regular, Menlo`) | **IBM Plex Mono** |

- **Scale is shared and dense** (a professional tool): base 14px; UI text
  11–13px; mono 11–12px; a few 15–18px headings. **Do not enlarge type for
  NERV** — keep the app's density; NERV expresses itself through *family +
  uppercase mono labels*, not bigger text.
- NERV leans **mono-forward** for labels/readouts/status (uppercase, tracked
  `0.08–0.2em`), matching the site. Classic uses proportional SF for labels.
- Fonts are **self-hosted** for NERV (`website/assets/fonts/` — Anton + IBM Plex
  woff2); the app currently loads **no** custom faces. The theming phase must
  bundle the NERV woff2 into the app and `@font-face` them (no network at
  runtime).

---

## Layout (Layout & Spacing)

**Layout and density are theme-independent** — both skins render the identical
information architecture:

- Compact, panel-based shell: resizable sidebar (default 240px), commit graph +
  details pane, status bar (24px), optional repo tabs (~28px).
- **Commit rows are 30px** (`GraphHistory.svelte:31`); this and all spacing stay
  fixed across themes.
- Spacing rhythm: an informal **4 / 6 / 8 / 10 / 12 / 16 / 18px** scale
  (tokenized above as `spacing.xs…xxl`); panel padding ~`16px 18px`; column gaps
  ~12px.

What themes may adjust: **decorative framing** (NERV adds thin corner-bracket
frames + hazard-stripe section dividers + faint hex/scanline texture; Classic is
plain hairline-bordered panels) — but not the box model / density.

---

## Elevation & Depth

This is one of the biggest felt differences between the themes.

- **Classic** — depth comes from the **OS vibrancy material**, not shadows.
  Always-on panels are flat (1px `border` + a translucent tint over the
  `NSVisualEffect` "popover/acrylic" material; `tauri.conf.json` `transparent:true`
  + `windowEffects`). **No CSS `backdrop-filter` on always-on surfaces** — a
  second CSS blur over the native material tanked framerate
  (`+page.svelte:777–785`). Shadows + `blur(20px) saturate(140%)` appear **only
  on transient overlays** (dialogs `0 12px 32px rgba(0,0,0,.28)`, menus
  `0 8px 24px`). Selection uses **inset box-shadow** (`inset 2px 0 0 accent`) so
  flex layout never shifts.
- **NERV** — **flat and opaque**; no vibrancy, no soft shadows. Depth reads
  through the **HUD language**: thin **corner brackets** framing panels/consoles,
  **hazard-stripe dividers** (diagonal orange/void) with a mono chip label,
  hairline `border` on the void ground, and a faint fixed **scanline + grid**
  overlay. Keep transient overlays legible (an opaque `surface-raised` on the
  void ground) rather than blurred.

---

## Shapes

| | Classic | NERV |
|---|---|---|
| Controls (buttons, segs) | `6px` radius | near-sharp (`0–2px`); "framed" via corner brackets |
| Panels / cards | `6px` | `2px` (essentially square) |
| Dialogs | `8–10px` | `2px` + corner brackets |
| Chips / pills / badges | fully round `999px` | keep `999px` for ref/branch chips **or** square them to `2px` (decide once, apply consistently) |
| Dots (graph) | `50%` | `50%` (unchanged) |

NERV's identity is **angular** — favour sharp corners + bracket accents over
rounded rectangles. Classic is the friendly macOS `6px`-everywhere rounding.

---

## Components

How key surfaces render per theme. Components reference the semantic tokens
above; only these *treatments* differ.

| Component | Classic | NERV |
|---|---|---|
| **Primary button** | `accent` (blue) fill, white text, `6px` | `accent` (orange) fill, `bg-app` text, near-square; mono uppercase label |
| **Ghost / secondary button** | `control` fill, `border`, `6px` | `surface-raised` + `border`, corner brackets that light up on hover |
| **Panel / console** | `surface` tint over vibrancy, 1px `border`, flat | `surface` opaque, 1px `border`, **corner brackets**, optional mono header bar (`◇ LABEL … ● LIVE`) |
| **Toolbar / overlay header** | translucent `surface-header`, native traffic lights kept, window-centered title | opaque `surface-header`, mono wordmark, hazard underline |
| **Sidebar / tabs** | neutral, `row-hover`/`row-selected` blues, resize handle thickens to `accent` on hover | same layout; orange selection edge, mono section labels, resize handle → orange |
| **Commit graph** | 8 blue-family+earthy lanes, `accent` HEAD ring, dots in lane color | 8 NERV-register lanes (orange/phosphor-led), orange HEAD ring; same 30px rows, same curved/angular option |
| **Ref badge** | outlined pill tinted by lane color; kind = glyph | same, tinted by NERV lane color; consider squared chip |
| **Diff** | green `#2da44e` / red `#cf222e`, Shiki github-*; per-hunk stage | phosphor `#46E88B` / alert `#FF4438`, dark Shiki; identical hunk UI |
| **Status bar** | 24px, `surface-header`, muted proportional text | 24px, opaque, **mono** readout style |
| **Dialog / menu** | `surface-raised`, `10px`, soft shadow + CSS blur | `surface-raised`, `2px`, corner brackets, no blur, hazard header chip |
| **Segmented control** | the primary picker idiom — keep in both | keep; NERV uses mono labels + orange active segment |
| **Spinners / status** | fast functional spinners | add a mono/`● LIVE`-style blinking status dot idiom |

---

## Do's and Don'ts

**Do**
- **Reference semantic tokens** (`var(--accent)`, `var(--surface)`) in component
  CSS — never raw hex. This is what makes a one-attribute theme swap possible.
- **Keep the semantic key set identical** across themes; add a key to *both*
  themes or neither.
- **Extend the existing `[data-tauri]` pattern**: set `[data-theme="nerv"]` on
  `<html>` and re-declare the token block; leave component CSS untouched.
  Classic keeps its light/dark/glass logic; NERV is a third top-level branch
  (dark-only, opaque, no vibrancy).
- **Treat the commit-graph lane palette + diff colors as themeable tokens** —
  they carry meaning and must read on each theme's ground.
- **Respect `prefers-reduced-motion`** in both themes (already the app's rule);
  NERV's boot/scanline flourishes must degrade to static.
- **Bundle NERV's fonts** (Anton + IBM Plex woff2) into the app and `@font-face`
  them locally — no runtime network.
- **Preserve accent discipline in NERV**: orange leads, green = status only, red
  = hazard/conflict only, bone for headings.

**Don't**
- Don't change **layout, density, spacing, or component structure** per theme —
  NERV is a re-skin, not the marketing layout.
- Don't hardcode a theme's colors/fonts in a component.
- Don't put a CSS `backdrop-filter` on always-on panels (kills framerate over
  the native material) — that constraint holds for Classic; NERV is opaque so it
  simply doesn't need blur.
- Don't let NERV drop below **AA contrast** (the site already tunes `#7C8794`
  for this) — an app is read all day.
- Don't enlarge type or add whitespace to "make it feel like the site" — keep
  the professional density.

---

## Implementation notes for the theming phase

- **Current source of truth (Classic):** all tokens live in one
  `:global(:root)` block, `src/routes/+page.svelte:611–704` — light `612–642`,
  dark `@media prefers-color-scheme` `644–669`, glass `[data-tauri="true"]`
  `674–704`. Toggle logic: `src/lib/tauriMode.ts`. Vibrancy config:
  `src-tauri/tauri.conf.json` (transparent + Overlay titlebar + `windowEffects
  popover/acrylic`). Lane palette: `src/lib/graph/colors.ts`. Diff colors:
  `DiffView.svelte`. Shiki themes: `src/lib/diff/highlight.ts`.
- **Current source of truth (NERV):** the shipped marketing site —
  `website/assets/css/styles.css` (`:root` token block) and
  `website/assets/fonts/` (Anton + IBM Plex woff2).
- **Suggested mechanism:** promote the token block to a small dedicated
  stylesheet with three cascading layers — base (light), `@media` dark,
  `[data-tauri]` glass — and add a parallel `:root[data-theme="nerv"]` block
  (dark-only, opaque) plus `:root[data-theme="nerv"][data-tauri="true"]` if any
  glass interplay is wanted (likely NERV ignores vibrancy). A user setting in
  **Settings → Appearance** writes `data-theme` to `<html>` and persists it
  (mirror the existing appearance settings + `tauriMode.ts`).
- **Resolved during implementation:** per-scheme NERV lane palettes live in
  `src/lib/graph/colors.ts` (`NERV_SCHEME_PALETTES`); the NERV Shiki syntax theme
  is `src/lib/diff/nervShikiTheme.ts`; Classic's automatic light/dark stays
  independent of the Classic/NERV toggle (theme = Classic/NERV; within Classic,
  macOS still drives light/dark).
- **A real app icon** shipped (`src-tauri/icons/icon-source.svg` — a hexagon emblem
  enclosing the tri-color commit graph, in the NERV palette). The web favicon
  (`static/favicon.png`) is still the stock SvelteKit mark — unrelated brand work,
  still outstanding.

---

## Sources

- DESIGN.md format: the [DESIGN.md spec](https://github.com/google-labs-code/design.md)
  (Google) and [Atlassian's DESIGN.md write-up](https://www.atlassian.com/blog/how-we-build/atlassians-design-md-is-here-what-we-learned-testing-portable-design-context-in-practice).
- Design-token tiering (primitive → semantic → component) and intent-based
  naming follow common design-system practice
  ([UXPin](https://www.uxpin.com/studio/blog/what-are-design-tokens/),
  [Contentful](https://www.contentful.com/blog/design-token-system/)).
- Classic values extracted from this repo (anchors above); NERV values from
  `website/` (this PR).
