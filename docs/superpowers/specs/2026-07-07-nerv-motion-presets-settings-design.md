# NERV Motion, Color-Scheme Presets & Settings Tabs — Design

**Goal:** Extend the built NERV theme ([`2026-07-07-nerv-app-theme-design.md`](2026-07-07-nerv-app-theme-design.md), PR #9) with the *dynamic* half of the NERV language plus two adjacent appearance features:

1. **Color-scheme presets** — selectable NERV "ops" (Orange/default, Phosphor, Steel, Amber, Violet, Crimson) re-tinting the lead accent, graph-lane palette, and diff/Shiki accent while keeping NERV's structure.
2. **NERV motion** — boot-reveal + ambient + interaction, plus a **timeline graph reveal** (the commit graph builds itself top-to-bottom when it appears). Reduced-motion-safe, GPU-cheap, behind a Settings toggle.
3. **Settings tabs** — reorganize the Settings scroll into tabbed sections.
4. **Angular graph line** — a stub-then-angle fix to `angularEdgePath`; NERV defaults to angular.

Folds into PR #9 on `nerv-theme`. Hardened by a 3-lens adversarial review against the code — the "Verified" callouts below are load-bearing.

## Background (verified)

- **Theme mechanism:** `<html data-theme="nerv">` swaps tokens. NERV token block is `:global(:root[data-theme="nerv"])` in `src/routes/+page.svelte:718`; decoration is `src/lib/theme/nerv.css` (**plain CSS — bare `:root[data-theme="nerv"]`, never `:global()`** which is silently dropped there). `themeMode.ts` sets `data-theme` pre-paint; `store.svelte.ts` holds `theme` (`loadSyncTheme`/`applyThemeAttr`/`persistTheme`/`setTheme` + Tauri-store hydrate). **The store has ZERO `$effect`** (module-scope factory; explicit "NO $effect" note at `store.svelte.ts:195`) — a store `$effect` throws `effect_orphan`.
- **NERV orange:** only `--accent`/`--accent-hover` are true tokens. **Verified split of the ~13 orange literals** into two kinds:
  - **Decoration (follows the accent) — ~5:** `--row-hover`/`--row-selected`/`--row-selected-border` (token block); the scanline `rgba(242,84,45,.015)` and hazard `rgba(242,84,45,.35)` (`nerv.css`).
  - **Status/warning (must NOT follow the accent):** `--err`/`--status-mod` (token block, warning=amber); and in `nerv.css` the `.ref.detached` ×3 (detached-HEAD warning), `.banner` heading/border (PrereqBanner attention notice), `.glyph.s-mod` background (modified-file glyph, whose text already themes via `var(--status-mod)`).
  - `color-mix(in srgb, …)` is already used in `nerv.css` and elsewhere (GraphHistory ref chips) — supported.
- **Motion:** none beyond `● LIVE` blink. **Perf hard rule** (`+page.svelte:833-841`): never animate a full-viewport layer's `background-position`/`opacity` (per-frame repaint = the backdrop-filter wall). Animate `transform`/`opacity` on dedicated GPU-composited layers only. **CSS `mask`/`clip-path` animate via paint, not the compositor** — treat them as *not* free.
- **Settings:** `SettingsPanel.svelte` `.dialog`, one scroll: Appearance / Commit dates / Behavior / Updates; `.seg` + `.opt` idioms.
- **Graph lines:** `angularEdgePath` (`paths.ts:68`) pure + unit-tested. Fork/lane-shift edge (`:76`) diagonals *immediately* off the node (`M x1 y1 L x2 my L x2 y2`); merge-in edge (`:75`, kind `"branch"`) already stubs (`M x1 y1 L x1 my L x2 y2`). `graphLineStyle` (`"curved"|"angular"`, default `"curved"`) is the raw setting; **`GraphHistory.svelte:448` passes `lineStyle={appState.graphLineStyle}` to `GraphGutter` (the sole path-fn consumer); `GraphGutter.svelte:29,62-64` types the prop `"curved"|"angular"` and treats any non-`"angular"` as curved.** Validation sites: `loadSyncLineStyle` (`:173-181`, Tauri default `"curved"` at `:174`, accept `raw==="angular"` at `:178`), `$state` seed (`:675`), hydrate guard `saved==="curved"||saved==="angular"` (`:686`), getter/setter (`:1584-1590`). Merge-in/curviness disable on `graphLineStyle==="angular"` (SettingsPanel `:101/107/113`, `:126/132/138`); segment active/aria-pressed (`:83/85/88/90`).
- **Timeline virtualization:** `GraphHistory` renders rows `[winStart, winEnd)` + BUFFER=8 each side (`:33`), `rowHeight=30` (`:31`); `winStart/winEnd` are **component-local `$state`** — **scrolling never touches the store**, so a store-held reveal flag can't be re-armed by scroll. `GraphGutter` SVG is **full height** (`height = rows.length * rowHeight`, `:49`); only children are windowed.
- **Reveal trigger seams (verified):** `activeView` is set to `"timeline"` from `set repo` (`:1396`), `setCurrent(sha)` on *every* commit focus (`:1582`), and `setActiveView`/`setWorkingCopySelected` (`:1671/:1659`). The graph data enters via `setGraphCommits` (`:1805`, repo-switch reset — sets `graphCommitsRepo`), `applyGraphRefresh` (`:1820`) and `appendGraphCommits` (`:1848`) which **reassign `graphCommits` on every fswatch git event, window focus/visibilitychange, paging, and ~15 same-repo git ops** (`gitActions.ts:120`; `liveRefreshGraph` wired `+page.svelte:270-273`) — all keep `graphCommitsRepo === repo`. **A component-hosted `$effect` already exists at `+page.svelte:195` tracking `activeView`/`repo` with `changed`/`prevView`.**

## Design

### Part 1 — Color-scheme presets (`data-scheme` on NERV)

Second root attribute `data-scheme` layers on `data-theme="nerv"`, default `orange`. Classic ignores it. **~6 "ops"** (hexes tunable):

| Scheme | Lead accent | `--accent-hover` |
|---|---|---|
| `orange` (default) | `#F2542D` | `#ff6a44` |
| `phosphor` | `#46E88B` | `#6cf3a5` |
| `steel` | `#5AA9E6` | `#7cbef0` |
| `amber` | `#E8A33D` | `#f2b85c` |
| `violet` | `#9B7FE0` | `#b49bee` |
| `crimson` | `#E0445A` | `#ec6376` |

**Mechanism — accent as single source of truth, status kept separate:**
1. **Route only the ~5 *decoration* literals through the accent** (NOT the status/warning ones): `--row-hover`/`--row-selected`/`--row-selected-border` → `color-mix(in srgb, var(--accent) N%, transparent)` / `var(--accent)`; the scanline `rgba(242,84,45,.015)` → `color-mix(in srgb, var(--accent) 1.5%, transparent)`; the hazard stripe `rgba(242,84,45,.35)` → `color-mix(in srgb, var(--accent) 35%, transparent)`. Brackets already use `var(--accent)`.
2. **Status/warning literals go to the status tokens, NOT the accent:** in the token block set `--err`/`--status-mod` = **fixed amber `#D9922E`** (distinct from the *amber scheme's* accent `#E8A33D`, so warning ≠ accent in every scheme). In `nerv.css`, route `.ref.detached` ×3 and `.banner` heading/border → `var(--err)` (amber); `.glyph.s-mod` background → `color-mix(in srgb, var(--status-mod) 10%, transparent)` (matches its now-amber text). The `.banner .actions` CTA button stays `var(--accent)` (a primary action) — an accent button inside an amber notice is intentional. `--status-add` stays phosphor `#46E88B`, `--status-del`/hazard red `#FF4438` fixed. (Known, accepted overlaps: in the *Phosphor* scheme accent ≈ `--status-add` green — acceptable, accent styles chrome/brackets while success styles status glyphs; context disambiguates.)
3. **Per-scheme blocks live in `nerv.css` as BARE selectors** (NOT `+page.svelte` — a bare `:root[…]` in a Svelte `<style>` is scoped-away; the base block only works because it's `:global(...)` at `:718`). In `nerv.css`, `:root[data-theme="nerv"][data-scheme="phosphor"] { --accent:#46E88B; --accent-hover:#6cf3a5; }` — specificity (0,3,0) beats the base (0,2,0) block regardless of order. `orange` needs no block (it's the base). `--on-accent` stays `#0A0C0F` on **all** schemes — **verified AA**: dark-on-accent contrast is phosphor ≈12.3, amber ≈9.1, steel ≈7.7, violet ≈6.1, crimson ≈4.8 (all ≥4.5; white fails on crimson/violet, so dark is strictly correct). If crimson is retuned darker than `#E0445A`, re-check its ~4.8 margin.
4. **Lane palette per scheme (pure, zero-allocation):** add to `colors.ts` a **precomputed frozen** `NERV_SCHEME_PALETTES: Readonly<Record<Scheme, readonly string[]>>` (each an 8-hue array leading with the scheme accent, remaining a shared NERV secondary set), and `schemeLanePalette(scheme)` = an **O(1) lookup** returning the shared frozen array (no per-call allocation — `colorForIndex`/`colorForRef` run per lane/ref every render). Keep `laneColor` pure (palette injected). In the store, extend the two palette-selection sites (`colorForRef` `:1547`, `colorForIndex` `:1556`): `theme==="nerv" ? schemeLanePalette(scheme) : LANE_PALETTE`.
5. **Shiki per scheme:** convert `nervShikiTheme.ts` to a factory `nervShikiTheme(name, accentHex)`. **Edit `highlight.ts`** (`:2` import, `:41-42` `createHighlighter({themes:[…]})`): register all six `nerv-orange`…`nerv-crimson` variants in the `themes:` array. `DiffView.svelte:42-43` `activeTheme` → `appState.theme==="nerv" ? "nerv-"+appState.scheme : (darkMode?"github-dark":"github-light")`; the token cache is keyed by `activeTheme` (`:84-85`), so a scheme change re-highlights reactively once `scheme` is `$state`.

**Persistence + application:** store setting `scheme` (`"orange"|…|"crimson"`, default `"orange"`), mirroring `theme` (sync localStorage `gitit.scheme.v1` + Tauri-store hydrate + imperative `applySchemeAttr`; **two deviations preserved** — `loadSync` reads localStorage unconditionally; `persist` writes localStorage in BOTH branches). `themeMode.ts` reads `gitit.scheme.v1` and sets `data-scheme` pre-paint (guarded). **Selector:** an Appearance-tab row (NERV-only), a swatch row / `.seg`; `setScheme(x)` live.

### Part 2 — NERV motion (boot + ambient + interaction)

New `src/lib/theme/nerv-motion.css` (bare selectors, imported in `+layout.ts`). **Every** animation gated on BOTH `<html data-motion="on">` AND `@media (prefers-reduced-motion: no-preference)`. All `transform`/`opacity` on dedicated layers.

- **Boot reveal** (one-shot ~500ms) on NERV activation (launch-with-NERV / toggle Classic→NERV): a scanline sweep + bracket draw-in + shell fade/scale-up. Driven by a `data-boot` attribute pulsed on `<html>` for the reveal duration (set on activation, cleared by `setTimeout`).
- **Ambient:** split the current single overlay `::after` into (a) a **static** grid layer and (b) a **separate scanline layer animated with `transform: translateY()`** (compositor-only, no repaint); `● LIVE` pulse + status blink.
- **Interaction:** corner brackets brighten on `.panel:hover`/`.dialog:hover`; active-segment + row-selection short transitions. `transform`/`opacity`/`color` only.
- **Settings toggle:** "NERV motion" (Appearance tab, NERV-only).

**`motion` default-ON (attribute-present-by-default — inverted vs `theme`):** `theme` defaults to attribute-*absent*; `motion` defaults to attribute-*present*. So do NOT literal-mirror: `loadSyncMotion()` returns `true` on an absent key; `themeMode.ts` sets `data-motion="on"` **unless** localStorage is explicitly `"off"` (`value !== "off"`); the Tauri hydrate default is also on. (Keep the two theme deviations.)

### Part 3 — Timeline graph reveal (one-shot; armed on repo identity only)

When a **repo's graph first lands** (or a genuine changes→timeline view switch), the graph builds itself top-to-bottom. Part of NERV motion (same `data-motion` + reduced-motion gate; NERV-only). **The trigger is the load-bearing correctness point — pinned to seams, no store `$effect`:**

- **Repo-load reveal:** arm inside **`setGraphCommits` (`:1805`) ONLY when `graphCommitsRepo` transitions to a new repo identity** — set `revealing=true`, bump `revealSeq`, and schedule a `setTimeout` to clear `revealing`. **Do NOT arm in `applyGraphRefresh` (`:1820`) or `appendGraphCommits` (`:1848`)** — those fire on fswatch git events, window focus/visibilitychange, paging, and same-repo ops, and must NOT replay the reveal.
- **View-switch reveal:** reuse the **existing component `$effect` at `+page.svelte:195`** (already tracks `activeView`/`repo` with `changed`/`prevView`) — arm only on a genuine `prevView !== "timeline" → "timeline"` transition. No new store `$effect`.
- **Coalesce (avoid double-arm):** `set repo` forces `activeView="timeline"` *before* the new graph lands (the flicker fix keeps old rows until `setGraphCommits` swaps). So a repo switch must play the reveal **exactly once, on the new data** — key the single arm on the `graphCommitsRepo` transition (which lands with the data); the view-switch arm guards against a real changes→timeline switch on the *same* repo, deduped by `revealSeq`.
- **Duration derived, not fixed:** clear `revealing` after `renderedRowCount × step + rowDuration` (not a hard 600-900ms) — with BUFFER=8 and a tall window (~40-60 rendered rows) a fixed timeout would clear before the last row's delay elapses, snapping bottom rows in un-animated. Cap per-row delay under the total.
- **Lines grow (revised — no mask/clip-path):** a `mask`/`clip-path` % wipe on the **full-height** gutter SVG is geometrically wrong (the viewport-crossing segment completes in a sliver of one frame) and paints, not composites. Use the **per-visible-edge `stroke-dashoffset` "draw"** as the primary mechanism — bounded to the rendered window (`GraphGutter` `<path>` edges), using `pathLength`. Cheap and correct.
- **Entries come in:** each rendered row gets `animation: nerv-row-in` (short fade+`translateY`) with `animation-delay: calc((rowIndex − winStart) × step)` — the base `(rowIndex − winStart)` is always ≥0 and bounded by the window size regardless of scroll position. Applied only while `revealing`.

### Part 4 — Settings tabbed sections

Reorganize `SettingsPanel.svelte`'s scroll into a horizontal tab strip under the title; same controls, regrouped:
- **Appearance:** Theme; **Color scheme** (NERV-only); **NERV motion** (NERV-only); Diff view.
- **Graph:** Graph lines (Auto/Curved/Angular — Part 5); Merge-in curve; Curviness; Repository switcher.
- **Commits:** the 5 date options + PR activity order.
- **Behavior:** backup, pull-rebase, merge-untracked, output-debug.
- **Updates:** auto-check, channel, version.

`activeTab` = local `$state`, persisted `gitit.settingsTab.v1` (default `appearance`, localStorage-only). Active group renders, others hidden; NERV-only rows conditional on `appState.theme==="nerv"`. Scroll within the active tab if needed. NERV styles the strip via existing token/decoration rules.

### Part 5 — Angular line (stub-then-angle) + NERV defaults to angular

- **Stub fix — the immediate-angle (fork/lane-shift) case only.** In `angularEdgePath`, change the `else` branch (`paths.ts:76`, the one that diagonals immediately off the node) to a chamfer with straight stubs at both ends; **keep the merge-in `kind==="branch"` case (`:75`) unchanged** (it already comes straight out of the node, and this preserves the merge-in vs branch-off directional distinction that both themes rely on):
  ```
  const s = Math.min(6, g.rowHeight / 3);          // 2s ≤ rowHeight for all rowHeight → no crossing
  if (edge.kind === "branch") return `M${x1} ${y1} L${x1} ${my} L${x2} ${y2}`;   // unchanged
  return `M${x1} ${y1} L${x1} ${y1 + s} L${x2} ${y2 - s} L${x2} ${y2}`;          // stubbed
  ```
  Same-lane (`x1===x2`) still short-circuits to vertical (`:73`). **`paths.test.ts`:** update the non-branch/fork expectations (e.g. fork `(0→1)` at rowHeight 30 → `M12 0 L12 6 L28 24 L28 30`); branch-case and straight-case expectations unchanged.
- **NERV defaults to angular** via a **3-state** setting. Widen `graphLineStyle` to `"auto"|"curved"|"angular"`, default `"auto"`. **Enumerate the widening sites:** `loadSyncLineStyle` default→`"auto"` and its Tauri branch (`:174`)→`"auto"`, accept-list (`:178`) include `"auto"`; the `$state` type (`:675`); the hydrate accept-list (`:686`) include `"auto"`; getter/setter types (`:1584-1590`). Add a **`effectiveGraphLineStyle`** `$derived`: `graphLineStyle==="auto" ? (theme==="nerv" ? "angular" : "curved") : graphLineStyle` (reads `theme` `$state`, so it re-resolves live on theme toggle). **Wiring (raw vs effective — do not mix up):**
  - **Rendering:** `GraphHistory.svelte:448` passes `lineStyle={appState.effectiveGraphLineStyle}` (NOT raw) — keeps `GraphGutter`'s prop type `"curved"|"angular"` so the type gate enforces only a resolved value is passed.
  - **Disable state:** SettingsPanel merge-in/curviness `disabled=` (`:101-138`) → `effectiveGraphLineStyle==="angular"`.
  - **Selection state:** the Graph-lines segment `class:active`/`aria-pressed` (incl. the new **Auto** segment) stays on **raw** `graphLineStyle` (else Auto never shows selected).

## Components / files

- **Add:** `src/lib/theme/nerv-motion.css`; `NERV_SCHEME_PALETTES` + `schemeLanePalette` in `src/lib/graph/colors.ts` (pure, frozen).
- **Edit:** `src/routes/+page.svelte` (route the ~5 decoration literals → accent/color-mix; set `--err`/`--status-mod` = `#D9922E`; split the overlay into grid + drift layers; interaction/bracket-hover); `src/lib/theme/nerv.css` (route decoration vs warning literals per Part 1.1-1.2; **per-scheme `data-scheme` blocks (bare)**; motion may live here or in `nerv-motion.css`); `src/lib/theme/themeMode.ts` (set `data-scheme` + `data-motion` (present-unless-off) pre-paint); `src/routes/+layout.ts` (import `nerv-motion.css`); `src/lib/store.svelte.ts` (`scheme` + `motion` settings; widen `graphLineStyle` at all enumerated sites + `effectiveGraphLineStyle` derived; `revealing`/`revealSeq` armed in `setGraphCommits` on repo-identity change; scheme-aware palette in `colorForRef`/`colorForIndex`); `src/lib/diff/nervShikiTheme.ts` (factory) + **`src/lib/diff/highlight.ts`** (register 6 variants) + `DiffView.svelte` (select `nerv-<scheme>`); `src/lib/components/SettingsPanel.svelte` (tab shell; regroup; scheme/motion rows; Auto segment; effective-vs-raw bindings); `src/lib/components/GraphHistory.svelte` (pass `effectiveGraphLineStyle`; reveal classes/stagger while `revealing`); `src/lib/components/GraphGutter.svelte` (per-edge stroke-draw while revealing; prop type stays `"curved"|"angular"`); `src/routes/+page.svelte` `$effect@195` (view-switch reveal arm); `src/lib/graph/paths.ts` + `paths.test.ts` (stub).

## Data flow

- **Boot:** `themeMode.ts` sets `data-theme` + `data-scheme` + `data-motion` (present-unless-off) pre-paint; if NERV+motion, a `data-boot` pulse plays the reveal.
- **Scheme change:** `setScheme` → `applySchemeAttr` (live re-tint via `color-mix(var(--accent))` + scheme block) + palette recolor (store) + `nerv-<scheme>` Shiki → persist.
- **Motion toggle:** `setMotion` → `data-motion` add/remove → persist.
- **Reveal:** `graphCommitsRepo` new identity (in `setGraphCommits`) OR genuine changes→timeline (`+page.svelte:195` effect) → `revealing=true`+`revealSeq++` → rows/gutter animate staggered while true → derived-duration `setTimeout` clears it → fswatch/focus/paging/scroll afterward are static.
- **Line style:** graph reads `effectiveGraphLineStyle`; `angularEdgePath` draws the stubbed fork shape.

## Error handling / edge cases

- **Reduced motion / motion off:** all Part-2/3 animation disabled → NERV byte-identical to static; the timeline appears instantly. Verify with `data-motion` removed AND `prefers-reduced-motion: reduce`.
- **Reveal does NOT replay on:** scroll (component-local state), **fswatch git refresh, window focus/visibilitychange, same-repo git ops, or paging** — armed only on `graphCommitsRepo` identity change (+ genuine view switch), coalesced by `revealSeq`.
- **Perf:** no full-viewport `background-position`/`opacity`/`mask`/`clip-path` animation; scanline drift is a `transform` layer; the reveal is stroke-draw bounded to the window; boot/row-in are `transform`/`opacity`. Confirm framerate scrolling the graph during/after reveal.
- **Scheme legibility:** warning=`#D9922E` amber, success=`#46E88B`, hazard=`#FF4438` fixed and distinct from accents (except the accepted Phosphor accent≈success overlap); `--on-accent`=`#0A0C0F` AA on all six.
- **Classic unchanged EXCEPT the intended angular improvement:** `data-scheme`/`data-motion`/reveal/scheme rules are NERV-scoped; the fork-edge stub + 3-state line style DO affect Classic — Classic default stays curved (`auto`→curved), and only the fork angular edge changes (merge-in unchanged). Verify Classic curved/angular render and Classic default is curved.

## Testing

- **Green gate:** `npm run check` 0 errors + `npm test` + `cargo test`. (Note: passing raw `"auto"` to `GraphGutter` is a type error — the effective-wiring must land for the gate to pass.)
- **Unit:** `paths.test.ts` updated for the stubbed fork strings (branch/straight unchanged); `colors.test.ts` case for `schemeLanePalette` (pure, returns the shared frozen array); optionally `effectiveGraphLineStyle` resolution.
- **Browser QA (`npm run dev` sample mode):** 6 schemes recolor accent+lanes+diff+HUD, warnings stay amber (not the accent) in every scheme; motion on/off + reduced-motion gate cleanly; the timeline reveal plays once, does NOT replay on scroll or on a simulated refresh, is instant when off; boot reveal on toggle-to-NERV; Settings tabs group correctly with NERV-only rows conditional; angular fork edge shows the stub in both themes; NERV defaults to angular (Auto), Classic to curved. Classic verified unchanged except the fork-edge improvement.

## Out of scope
- Presets for Classic (NERV-only). Merging `next`→`main`. New graph line *styles* beyond curved/angular.

## Acceptance criteria
1. **~6 NERV schemes** (Appearance tab, NERV-only) live-recolor accent + lanes + diff/Shiki + HUD; **warnings/status stay legible and distinct from the accent** in every scheme; persisted, no-FOUC.
2. **NERV motion** (boot + ambient scanline-drift + `●LIVE`/blink + interaction) runs only under `data-motion="on"` AND `prefers-reduced-motion: no-preference`; **default ON**; a Settings toggle controls it; off/reduced ⇒ static NERV; no framerate regression.
3. **Timeline reveal** plays once when a repo's graph appears (lines draw + entries cascade), **does not replay on scroll, fswatch refresh, window focus, or same-repo ops**, and is instant under reduced-motion/off.
4. **Settings** organized into Appearance/Graph/Commits/Behavior/Updates tabs; all controls preserved; NERV-only rows conditional; last tab remembered.
5. **Angular fork edge** shows a straight stub out of the node before angling (both themes); merge-in edge unchanged; **NERV defaults to angular** (Auto), Classic to curved; users can pin either; the graph consumes `effectiveGraphLineStyle`.
6. Green gate passes; Classic unchanged except the intended fork-edge improvement; browser QA confirms the above.
