# NERV App Theme — Design

**Goal:** Ship a user-toggleable **NERV** theme for the Git It desktop app alongside the
existing **Classic** appearance, selectable in **Settings → Appearance**. NERV is the
git-it.app marketing console language (burnt-orange lead, phosphor-green status, warm-bone
type, Anton + IBM Plex, angular corners, HUD framing) brought back into the app as a
**re-skin of the same dense UI** — identical layout, density, and box model; only color,
type, shape (radius), elevation material, and decorative motif change. This is the **full
HUD treatment**: the token swap *plus* the decorative motifs (corner brackets, hazard-stripe
dividers, scanline/grid overlay, `● LIVE` status dots, mono readout styling).

The design language for both themes is specified in [`DESIGN.md`](../../../DESIGN.md); this
document is the **implementation design** for building it. It was hardened by a 5-lens
adversarial review against the real codebase; the "Verified code facts" callouts below are
the load-bearing ones.

## Background

Git It already ships a working "theme swap" it doesn't call a theme. All surface tokens live
in one block in `src/routes/+page.svelte:611–704`, written with `:global(...)` so the
component `<style>` doesn't scope them away:

- base **light** values on `:global(:root)` (`612–642`),
- a `@media (prefers-color-scheme: dark)` override (`644–669`),
- and a `:global(:root[data-tauri="true"])` **glass** override (`674–704`) that re-declares
  the *same* `--panel-bg`, `--border`, … as translucent `rgba()` so every component picks up
  glass **without any component CSS change**. `src/lib/tauriMode.ts` sets `data-tauri`
  synchronously, imported as a side-effect in `src/routes/+layout.ts:5` (which also sets
  `export const ssr = false`). **There is no `+layout.svelte`.**

Components consume these via `var(--panel-bg)` — never raw hex. NERV is architecturally the
*same trick* keyed off `data-theme="nerv"`.

**Verified code facts that shaped this design (do not re-derive — trust these):**

1. **`store.svelte.ts` has zero `$effect`s.** State is built by a plain factory
   `makeState()` (~line 490) invoked at **module scope** (`export const appState =
   makeState()`, ~line 1968). A bare `$effect` there throws `effect_orphan` at import →
   white screen. The 6 real `$effect`s in the app all live in `+page.svelte`.
2. **`loadSyncLineStyle()` returns a default under Tauri** (`if (isTauri()) return
   "curved"`, ~line 170) and **`persistLineStyle()` early-returns after writing the Tauri
   store, never touching localStorage under Tauri** (~672–687). localStorage is the
   *browser-only* fallback. Durable settings live in `settings.json` via
   `@tauri-apps/plugin-store`, hydrated a tick after launch.
3. **Fonts are not tokenized.** Body font is a **literal** `-apple-system,…` on
   `:global(:root)` at `+page.svelte:638` (not a var). Mono is hardcoded
   `font-family: ui-monospace, SFMono-Regular, Menlo, monospace` in **~40 component sites**
   (DiffView, GraphHistory, CommitDetail, every `lib/github/*` panel, …). There is **no**
   `--font-mono`/`--font-sans`/`--font-display` token. Anton has no application selector.
4. **Radius is not tokenized.** ~185 hardcoded `border-radius` declarations (6px ×~57, 8px
   ×~17, 10px ×~17, plus 5/7/4px and `999px` ×~27). No `--radius` token. Ref chips are
   `border-radius: 4px` (`GraphHistory.svelte:779`), **not** 999px pills.
5. **Diff colors are already `var()`-backed with a two-tier split.** `DiffView.svelte` uses
   `var(--diff-add-bg, rgba(46,160,67,.18))` in the base rule **and** a *separate*
   `var(--diff-add-bg-dark, …)` inside its own `@media (prefers-color-scheme: dark)` block,
   plus sigil colors `var(--diff-add-fg, #2da44e)` / `var(--diff-del-fg, #cf222e)`. **None
   of the six tokens are declared anywhere** (only fallbacks apply). The Shiki theme at
   `DiffView.svelte:42` already re-derives reactively from a `matchMedia` `$state`.
6. **`laneColor()` (`src/lib/graph/colors.ts`) is pure, zero-import, and unit-tested**
   (`colors.test.ts` asserts `laneColor(0,null,{}) === LANE_PALETTE[0]`). `store.svelte.ts`
   imports it (via `./graph`) at `colorForIndex`/`colorForRef` (~1489–1501); `GraphGutter`
   and `GraphHistory` resolve colors through `appState.colorForIndex`. The HEAD ring in
   `GraphHistory.svelte:124` uses HEAD's **lane** color (falls back to `--accent` only when
   null) — so in NERV it would frequently *not* be orange.
7. **~20+ components hardcode `color:#fff` on an accent-filled button/active-segment**
   (DiffView 639/641, RebaseTodo 359, WorkingCopyView 661, PrereqBanner 83, SettingsPanel
   `.seg button.active`, ≥6 `lib/github/*`). `#fff` on `#F2542D` ≈ **3.45:1 — below AA**.
8. **27 of 37 components use `overflow:hidden/auto`.** Absolutely-positioned corner-bracket
   `::before/::after` get **clipped** on scroll panels, and hosts are mostly
   `position:static` (making them `relative` can move existing abs-positioned descendants).
   There is **no shared `.panel`/`.dialog` class** — ~12 dialogs/panels use distinct local
   names (Modal, AmendDialog, RebaseTodo, ManageRepoModal, BranchColorDialog, SettingsPanel,
   ContextMenu, StashPanel, ReflogPanel, ApplyPanel, ConflictView, …) plus the ~25-file
   GitHub screen.

## Architecture (reviewer-approved)

NERV is a re-skin, not a rebuild. **Layout, density, spacing, and box model stay identical**
(text re-metrics — see below — but the box model does not). Two layers, both gated on
`<html data-theme="nerv">`:

1. **Token layer** — re-declare every themeable `--var` (color **and now font/radius**) with
   NERV values. Delivered by tokenizing the currently-hardcoded sites so the swap actually
   reaches them.
2. **Decoration layer** — the HUD motifs, in a global `nerv.css`, scoped under
   `:global(:root[data-theme="nerv"])`.

Two encoding decisions the review explicitly endorsed — **keep them**:

- **Attribute-only encoding** (`data-theme` × `data-tauri` × `@media`) is the sane model, not
  a fragile combinatorial cascade: NERV collapses to a single opaque dark appearance that
  **ignores `prefers-color-scheme`**, so only the NERV-over-glass corner needs an explicit
  combined selector, and NERV re-declaring the full token superset means nothing leaks
  through from the light/dark/glass blocks.
- **Append the NERV token block in `+page.svelte`; do NOT extract to `@layer`.** Wrapping the
  working light/dark/glass blocks in layers risks Classic for no functional gain. NERV wins
  by re-declaring a superset and, for the shared glass tokens, by **source order** (NERV
  block placed after both `[data-tauri]` blocks) — pinned with a `MUST remain after the
  glass blocks` comment. The one load-bearing background rule additionally uses the combined
  `(0,3,0)` selector so it is order-independent.

### `data-theme` contract

- absent / `"classic"` → **Classic**, unchanged: OS drives light/dark, glass under Tauri.
- `"nerv"` → **NERV**: dark-only, opaque, ignores `prefers-color-scheme`, overrides glass.

The theme setting is Classic ↔ NERV **only**. Within Classic, macOS still drives light/dark;
NERV has a single dark appearance. No separate light/dark control.

## Phasing (execution order)

The review's strongest structural recommendation: this is two deliverables of very different
size and certainty. Deliver the full HUD, but in ordered phases, each with its own green gate
and visual QA:

- **Phase 0 — Vertical spike (de-risk).** Implement the *complete* treatment end-to-end on
  **one scroll panel (the commit list) + one dialog + the status bar**: token swap, corner
  brackets (on a clipping panel), a mono header bar, a hazard divider, mono readout, and the
  fixed scanline/grid overlay. Goals: (a) measure the *real* per-component edit count for
  brackets; (b) confirm the overflow/`position:relative`/pseudo-collision reality; (c) verify
  the `position:fixed` full-viewport overlay does **not** reintroduce the per-frame repaint
  pathology the team already hit with `backdrop-filter` (`+page.svelte:777–785`). The spike's
  findings **feed back into Phase B's plan**.
- **Phase A — Token theme (colors + type + shape).** The complete, shippable "NERV colors and
  typography and angular corners" theme: the token block, font application, radius squaring,
  persistence + no-FOUC, the Settings control, theme-aware graph lanes, diff tokens + Shiki,
  and the hardcoded-color/font audit. Satisfies acceptance #1–4, #6–7. **Independently
  correct and demoable without any HUD decoration.**
- **Phase B — HUD decoration.** Corner brackets, hazard dividers, scanline/grid overlay,
  `● LIVE`/blink dots, mono readout styling, Anton mapping — per the spike's measured plan.
  Satisfies acceptance #5.

## Phase A — token theme

### The NERV token block (`+page.svelte`, appended after the glass blocks)

Written `:global(:root[data-theme="nerv"])`. Re-declares the full themeable set. Colors are
DESIGN.md's contract (verified faithful). **New token families** (font, radius, on-accent,
diff) are added to **both** themes so Classic keeps its exact current values and NERV
overrides:

```
:global(:root[data-theme="nerv"]) {
  /* surfaces / text / accent / status — DESIGN.md contract */
  --bg:#0A0C0F; --panel-bg:#12171C; --popover-bg:#0E1216;
  --header-bg:#0E1216; --input-bg:#0E1216; --btn-bg:#0E1216; --btn-hover:#14181d;
  --border:#2E3742; --border-subtle:#232A31;
  --text:#EAE6DA; --text-muted:#8A94A0;          /* meta uses --text-muted (AA ~6:1) */
  --row-hover:rgba(242,84,45,.06); --row-selected:rgba(242,84,45,.10);
  --row-selected-border:#F2542D;
  --accent:#F2542D; --accent-hover:#ff6a44; --on-accent:#0A0C0F;   /* dark text on orange */
  --danger:#FF4438; --danger-hover:#ff5a4f; --err:#F2542D;         /* NERV warning = orange */
  --status-add:#46E88B; --status-mod:#F2542D; --status-del:#FF4438;
  /* diff — BOTH tiers (NERV ignores prefers-color-scheme, but @media dark still matches) */
  --diff-add-bg:rgba(70,232,139,.15);  --diff-add-bg-dark:rgba(70,232,139,.15);
  --diff-del-bg:rgba(255,68,56,.15);   --diff-del-bg-dark:rgba(255,68,56,.15);
  --diff-add-fg:#46E88B; --diff-del-fg:#FF4438;
  /* type */
  --font-sans:'IBM Plex Sans', system-ui, -apple-system, sans-serif;
  --font-mono:'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, monospace;
  --font-display:'Anton', 'Arial Narrow', Impact, sans-serif;
  /* shape */
  --radius-sm:0px; --radius-md:2px; --radius-lg:2px;
  color-scheme: dark;
  font-family: var(--font-sans);   /* overrides the literal SF stack at :root */
}
/* opaque-over-glass — cover BOTH root and body (the existing transparent reset targets body) */
:global(:root[data-theme="nerv"][data-tauri="true"]),
:global(:root[data-theme="nerv"][data-tauri="true"] body) { background: var(--bg); }
```

Classic-side additions (in the existing `:root` / dark blocks), each equal to the **current
literal** so Classic is provably identical: `--on-accent:#fff`; `--font-sans` = the current
SF stack; `--font-mono:ui-monospace,SFMono-Regular,Menlo,monospace`; `--font-display` = SF
stack (Classic has no distinct display face); `--radius-sm:4px --radius-md:6px
--radius-lg:10px`; and the six `--diff-*` values = the current fallbacks (add
`rgba(46,160,67,.18)`/`.24`-dark, del `rgba(210,35,35,.18)`/`.24`-dark, fg `#2da44e`/`#cf222e`
— and their dark-mode `github`-dark equivalents in the `@media dark` block; hoisting
DiffView's dark values up here also removes the only OS-dark leak).

### Applying the new tokens (the audit — a first-class Phase A task)

`@font-face` and a token *declaration* render nothing until the hardcoded sites reference the
token. Strategy: **tokenize with a Classic-identical fallback** so each edit provably
preserves Classic and is compile-safe (not a fragile reach-around):

- **Mono:** replace the ~40 `font-family: ui-monospace, SFMono-Regular, Menlo, monospace`
  with `font-family: var(--font-mono)` (token's Classic value = that exact literal).
- **Body/UI:** the root `font-family: var(--font-sans)` covers proportional text via
  inheritance (form controls already `font-family: inherit`).
- **Display (Anton):** enumerate the header/section-label/`OP //`-label selectors and set
  `font-family: var(--font-display)` (Classic value = SF, so no Classic change).
- **Radius:** rewire the dominant shared control/surface/dialog/input/segment/chip classes to
  `--radius-sm/md/lg`. Rare bespoke one-off radii (a few 5/7px) may remain; flag any that
  read as visibly rounded under NERV during QA. (Chips: 4px → `--radius-lg` = 2px in NERV.)
- **On-accent text:** replace the ~20 `color:#fff`/`white` on accent-filled buttons and
  `.seg button.active` with `color: var(--on-accent)` (Classic `#fff`, NERV `#0A0C0F`).
- **Stuck-in-Classic component colors:** route the hardcoded status/brand colors that would
  otherwise stay Classic under NERV — Sidebar detached-HEAD `#d97706`, WorkingCopyView
  status-row tints, PrereqBanner amber — through existing status tokens or theme-scoped
  overrides.
- **GitHub screen state-colors (decision):** `lib/github/itemState.ts` uses semantic
  green/purple/red/gray for PR/issue **OPEN/MERGED/CLOSED/DRAFT**. These mirror GitHub's own
  external convention (not decorative chrome), so **keep them semantically** under NERV but
  tune for legibility on the void ground (bump lightness for AA if needed). This is the one
  sanctioned exception to NERV's "no decorative green/red" — flagged for the user to veto.

The audit's concrete file list comes from a hex/`rgba`/`font-family`/`border-radius` grep at
plan time (~15+ files for color, ~40 for mono, plus the radius set).

### Persistence + no-FOUC (the corrected mechanism)

- **`src/lib/theme/themeMode.ts`** — sibling of `tauriMode.ts`. Module side-effect, **guarded**
  (`typeof document/localStorage === "undefined"` + try/catch, matching tauriMode/loadSync*):
  synchronously read `localStorage["gitit.theme.v1"]` and set
  `document.documentElement.dataset.theme` **before paint**. Imported as a JS side-effect in
  **`src/routes/+layout.ts`** beside `import "$lib/tauriMode"` (the earliest global hook — not
  `+page.svelte`, which runs later).
- **`theme` state in `store.svelte.ts`** — mirrors `graphLineStyle`'s hydrate/touched-guard/
  setter shape, with **two deliberate deviations from the exact mirror** (both required for
  no-FOUC — see Verified facts #1, #2):
  1. **`loadSyncTheme()` reads localStorage UNCONDITIONALLY, including under Tauri** (validated,
     default `"classic"`), so the store seed matches what `themeMode.ts` already painted.
  2. **`persistTheme()` writes localStorage in BOTH branches** — under Tauri it writes the
     durable store **and** localStorage (no early-return before `localStorage.setItem`); in the
     browser, localStorage only.
- **No `$effect`.** Set `document.documentElement.dataset.theme` (or `removeAttribute` for
  Classic) **imperatively** inside `setTheme()` and inside the async Tauri-store hydrate
  `.then()` callback — exactly how `tauriMode.ts` and `persistLineStyle()` work. This boots
  cleanly and never fights `themeMode.ts` (both read the same localStorage key).
- Exposed as `appState.theme` + `appState.setTheme("classic"|"nerv")`.

### Settings → Appearance control

A **Theme** segmented control (`Classic` | `NERV`) as the first row of the Appearance group in
`SettingsPanel.svelte`, matching the existing `.seg` idiom. Bound to `appState.theme` /
`setTheme(...)`.

### Graph lanes (pure `laneColor`, palette selected in the store)

Keep `colors.ts` pure and its test green (Verified fact #6 — a theme/DOM read there is a
circular import and breaks the unit test):

- Add `export const NERV_LANE_PALETTE` (DESIGN.md's 8, tuned for the void):
  `#F2542D #46E88B #5AA9E6 #9B7FE0 #E8A33D #E0608A #EAE6DA #8A94A0`.
- Add an optional `palette: string[] = LANE_PALETTE` parameter to `laneColor()` (existing
  tests unaffected; add one NERV-palette test).
- **Select the palette in the store callers that already hold `appState.theme`**
  (`colorForIndex`/`colorForRef`, ~1489–1501). `GraphGutter`/`GraphHistory` receive colors
  through those, so a toggle recolors live.
- **HEAD ring:** add `GraphHistory.svelte` (and check `GraphGutter.svelte`) to the edit list —
  under NERV force the HEAD **ring and HEAD dot** to `var(--accent)`; **non-HEAD dots stay in
  their lane color** (do not flatten all dots to orange).
- Ref badges keep tinting from the lane color; ref **kind** stays the RefIcon glyph.

### Diff (Shiki + the six tokens)

- The six `--diff-*` tokens above re-skin backgrounds and sigils with **no DiffView CSS edit**
  (they were already `var()`-backed — Verified fact #5): declaring both the base **and**
  `-dark` tiers in each theme block is sufficient for correctness on both OS appearances.
  **Optional cleanup** (only if those `@media dark` blocks contain *only* diff-color rules —
  verify at impl time): hoist DiffView's `@media dark` diff values into `+page.svelte`'s
  `@media dark :root` block and drop the now-redundant DiffView declarations, so a single
  token pair is cascade-driven. The two-tier token approach is the safe default; the hoist is
  a tidiness win, not required.
- **Custom NERV Shiki theme:** author a small theme *object* (bone base text; orange/phosphor/
  haze token colors on `#0A0C0F`), register it in `highlight.ts` beside `github-light/dark`
  (offline, no extra bundled chunk). `DiffView.svelte:42` selects it when the **active app
  theme** is `nerv`, else keeps the existing `prefers-color-scheme` logic — read from
  `appState.theme` (the `matchMedia` `$state` already proves reactive re-highlight works).

### Fonts (bundled, applied, offline)

- Copy the site's woff2 into `static/fonts/` and reuse `website/assets/fonts/fonts.css`
  **verbatim** (adjust `url()` paths) rather than re-authoring `@font-face`. **Add IBM Plex
  Sans 500** (the site ships 400/600 only; DESIGN.md body weights are 400/500/600, and the app
  uses `font-weight:500` — otherwise 500 faux-synthesizes). Ship the SIL **OFL** notices.
- `@font-face` globally/unconditionally in `nerv.css` (declarations don't fetch until a family
  renders → zero cost for Classic; loaded only when NERV references them). `font-display: swap`;
  fallback stacks keep text legible on load failure. SvelteKit static adapter serves
  `static/` at the app root; bundled in the `.app`; no runtime network.

## Phase B — HUD decoration layer

`src/lib/theme/nerv.css`, imported once in `+layout.ts`; only `:global(:root[data-theme="nerv"])`
rules. **Informed by the Phase 0 spike.** Split by honesty about what pure decoration can do:

- **Genuinely pure (no markup):** the fixed `position:fixed` scanline/grid overlay (one root
  `::after`, `pointer-events:none`, `prefers-reduced-motion` → static); mono readout restyling
  (uppercase/tracked) on existing label classes; square chips (already tokenized in Phase A);
  `● LIVE`/blink dots on existing status indicators (blink gated on reduced-motion).
- **Requires bounded, additive markup (behind `[data-theme]`, `display:none` in Classic):**
  corner brackets on scroll/clipping panels and the mono `◇ LABEL … ● LIVE` header bars. The
  spike produces the **enumerated target list**; prefer introducing **one shared theming hook**
  (a common wrapper/`data-nerv-frame` attribute added once per panel) so brackets are a single
  rule, not N, and so a later component refactor can't silently break the theme.

**Acceptance #5 is reframed** to accept a bounded, enumerated set of additive-markup edits —
not "zero component edits."

## Components / files

- **Add:** `src/lib/theme/themeMode.ts`; `src/lib/theme/nerv.css`; the NERV Shiki theme object
  (e.g. `src/lib/diff/nervShikiTheme.ts`); `static/fonts/*` (Anton 400; IBM Plex Sans 400/500/
  600; IBM Plex Mono 400/500) + OFL notices + copied `fonts.css`.
- **Edit:** `src/routes/+page.svelte` (NERV token block + new Classic-side token declarations +
  hoist dark diff values); `src/routes/+layout.ts` (import `themeMode` + `nerv.css`);
  `src/lib/store.svelte.ts` (`theme` state, `loadSyncTheme`, hydrate, `persistTheme`,
  `setTheme`, palette selection in `colorForIndex`/`colorForRef`); `src/lib/components/
  SettingsPanel.svelte` (Theme control + on-accent for `.seg.active`); `src/lib/graph/colors.ts`
  (`NERV_LANE_PALETTE` + `palette` param); `src/lib/graph/colors.test.ts` (NERV case);
  `src/lib/components/GraphHistory.svelte` (+ `GraphGutter.svelte`) (NERV HEAD ring/dot);
  `src/lib/diff/highlight.ts` + `DiffView.svelte` (NERV Shiki select; remove `@media dark`
  diff blocks); the **audit set** — ~40 mono sites, the radius sites, ~20 on-accent sites, and
  the stuck-in-Classic color components (Sidebar, WorkingCopyView, PrereqBanner, `lib/github/*`).
- **Phase B additionally edits** the enumerated bracket/header-bar host components (bounded,
  additive, Classic-hidden).

## Data flow

- **Boot:** `+layout.ts` runs `themeMode.ts` (sync localStorage read → `data-theme` before
  paint) and `tauriMode.ts` (`data-tauri`). First paint is already NERV. `nerv.css` global.
- **Hydrate (Tauri):** store loads `settings.json` a tick later; if durable `theme` differs and
  untouched, apply it and imperatively re-set `data-theme`.
- **Toggle:** `SettingsPanel` → `setTheme(x)` → imperative `data-theme` write (live re-skin, no
  reload) → `persistTheme()` writes Tauri store **+ localStorage**.
- **Render:** `var(--…)` consumers re-skin (color/type/radius); `colorForIndex` returns NERV
  lane colors; DiffView renders NERV Shiki; `nerv.css` adds HUD decoration.

## Error handling / edge cases

- **Missing/garbage persisted value** → default `"classic"` (`loadSyncTheme` validates).
- **Browser dev (no Tauri store)** → localStorage is the source of truth; NERV fully
  previewable (`?translucent` also exercises the glass-interplay path).
- **FOUC narrow window (accepted):** macOS WKWebView flushes localStorage lazily (the reason
  the durable store exists). A crash/force-quit shortly after a toggle can leave localStorage
  stale → one flash on next launch, then the hydrate corrects it and rewrites localStorage
  (self-heals). Acceptance #2 is "no flash **in the normal case**", not unconditional.
- **Reduced motion** → scanline/blink degrade to static.
- **AA** → NERV meta uses `--text-muted` (#8A94A0 ≈ 6:1); `--on-accent` fixes the
  white-on-orange failure; keep the GitHub state-colors AA on the void.
- **Font load failure** → `font-display:swap` + fallback stacks keep text legible.

## Testing / verification

- **Green gate (per phase, required before "done"):** `npm run check` (0 errors) + `npm test`
  + `cargo test`.
- **Unit:** `colors.test.ts` gains a NERV-palette case; existing cases stay green (proves
  `laneColor` still pure).
- **"Classic renders identically" (replaces "byte-for-byte"):** verify (a) every selector in
  `nerv.css` and the NERV token block is `data-theme="nerv"`-scoped so it cannot match Classic,
  and (b) each **new token's Classic value equals the literal it replaced** (e.g.
  `--on-accent:#fff`, `--font-mono` = the exact prior stack, `--radius-md:6px`). Optional
  Classic before/after screenshot diff.
- **Definitive visual QA under `npm run tauri dev`** (theme is a Tauri-window experience):
  toggle Classic ↔ NERV and verify **Classic unchanged** and **NERV correct** across: commit
  graph (NERV lanes, **orange HEAD ring**, lane-colored dots), a diff **on a dark-mode Mac
  specifically** (phosphor/alert bg + NERV Shiki), Settings, dialogs/menus (brackets, no blur,
  2px), status bar (mono readout), ref chips (square), sidebar (orange selection), the GitHub
  screen, and the scanline overlay. Confirm **no FOUC** on relaunch, **no box-model shift**
  between themes, and — because type changes — **QA tight fixed-width chrome (24px status bar,
  sidebar labels) for text overflow/truncation** (text re-metrics; that is expected, overflow
  is not).
- **Browser pass** (`?translucent`) for the token + decoration layers not needing native git.
- **Phase 0 spike** validates bracket edit-count, overflow/position reality, and the fixed
  overlay's per-frame cost **before** Phase B is planned in full.
- Executed **subagent-driven with adversarial review** per standing preference.

## Out of scope (explicit)

- **Merging `next` → `main`** + enabling Pages / the custom domain — *after* this theme work,
  so site + theme ship together.
- **A real app icon** (placeholder today) — related brand work, separate.
- A NERV **boot/reveal animation** beyond the static scanline/grid — optional follow-up.
- Any change to layout, density, spacing, or information architecture.

## Acceptance criteria

1. A **Theme (Classic | NERV)** control in Settings → Appearance toggles the whole app **live**
   (no reload, no box-model shift).
2. The choice **persists** across relaunch (durable Tauri store) with **no theme flash in the
   normal case** (crash-before-flush edge accepted, self-healing).
3. **Classic renders identically** — verified by (a) all NERV CSS being `data-theme`-scoped and
   (b) every new token's Classic value equaling the literal it replaced.
4. NERV renders the **full token contract**: orange/bone/phosphor colors, **IBM Plex Sans body
   + IBM Plex Mono (applied, offline)**, **angular 0–2px corners** (buttons/panels/dialogs/
   chips), NERV graph lanes + **orange HEAD ring**, NERV Shiki diff with phosphor/alert
   backgrounds correct **on both OS light and dark**, `--on-accent` (AA-legible on orange),
   opaque (no vibrancy).
5. NERV shows the **full HUD** — corner-bracket framing, hazard-stripe dividers, scanline/grid
   overlay, `● LIVE`/mono readout — delivered via pure decoration where possible and a
   **bounded, enumerated set of additive, Classic-hidden markup edits** where not; degrading
   under reduced motion.
6. **No hardcoded color, font, or radius** remains stuck in Classic values under NERV (audit
   complete, including on-accent text and the GitHub screen).
7. The green gate passes each phase; `tauri dev` visual QA confirms the above (incl. a
   dark-mode-Mac diff check and tight-label overflow check).
