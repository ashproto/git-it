# NERV Motion, Presets & Settings-Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Extend the built NERV theme (PR #9, branch `nerv-theme`) with: an angular graph-line stub + NERV-defaults-to-angular; NERV color-scheme presets (`data-scheme`); NERV motion (boot + ambient + interaction) + a timeline graph reveal; and a tabbed Settings panel.

**Architecture:** Everything hangs off root attributes on `<html>`: existing `data-theme="nerv"`, plus new `data-scheme` (preset) and `data-motion="on"` (motion). Presets recolor by making `var(--accent)` the single source of truth (decoration derives from it via `color-mix`; status colors are decoupled to fixed amber/green/red). Motion is CSS gated on `data-motion` AND `prefers-reduced-motion`. The timeline reveal is a store `revealing` flag armed only on a repo-identity change. Line style becomes 3-state (`auto`/`curved`/`angular`) with a theme-aware `effectiveGraphLineStyle`.

**Tech Stack:** Tauri 2 + SvelteKit 5 (runes), Shiki, Vitest.

## Global Constraints

- **Green gate before any task is "done":** `npm run check` (0 errors) + `npm test` + `cargo test`.
- **Classic renders unchanged** EXCEPT the one intended cross-theme improvement (the angular fork-edge stub). Every scheme/motion/reveal rule is `data-theme="nerv"`-scoped.
- **`nerv.css` / `nerv-motion.css` are PLAIN CSS** — bare `:root[data-theme="nerv"] …`, **never `:global()`** (Svelte-only; silently dropped in plain CSS). Inside `+page.svelte`'s `<style>` the inverse holds: tokens use `:global(:root…)`.
- **No `$effect` in `store.svelte.ts`** (module-scope factory → `effect_orphan`). Reveal state uses a plain `setTimeout` in a setter; the view-switch arm reuses the existing `+page.svelte:195` component `$effect`.
- **Keep `src/lib/graph/*.ts` pure** (no store/DOM imports); unit-tested.
- **Perf:** never animate a full-viewport layer's `background-position`/`opacity`/`mask`/`clip-path` (per-frame repaint — the `backdrop-filter` wall at `+page.svelte:833-841`). Motion is `transform`/`opacity` on dedicated layers; the reveal is per-visible-edge `stroke-dashoffset`.
- **NERV accent discipline:** orange (or the scheme accent) leads; success green, warning amber `#D9922E`, hazard red `#FF4438` are fixed and accent-independent. `--on-accent` = `#0A0C0F` on all schemes (dark-on-accent, AA-verified).
- Definitive visual verification is **browser QA in the real app** (`npm run dev` :1420, sample mode) — CSS/motion tasks aren't unit-testable.

---

### Task 1: Angular fork-edge stub (pure, TDD)

**Files:** Modify `src/lib/graph/paths.ts:68-77`; Modify `src/lib/graph/paths.test.ts`.

**Interfaces:** Produces the stubbed `angularEdgePath`; consumed by `GraphGutter`.

- [ ] **Step 1: Update the failing tests** in `paths.test.ts` — the non-branch/fork expectations change; branch + straight cases unchanged. (Read the file for the exact existing cases; the fork case at rowHeight 30, fromLane 0→toLane 1 becomes `M12 0 L12 6 L28 24 L28 30`.) Add/adjust assertions:

```ts
// fork / lane-shift (kind !== "branch"): straight stub out of the node, diagonal, stub in
expect(angularEdgePath({ fromLane: 0, toLane: 1, colorIndex: 0, kind: "merge" }, 0, g))
  .toBe("M12 0 L12 6 L28 24 L28 30");
// merge-in (kind "branch") UNCHANGED
expect(angularEdgePath({ fromLane: 0, toLane: 1, colorIndex: 0, kind: "branch" }, 0, g))
  .toBe("M12 0 L12 15 L28 30");
// same-lane UNCHANGED (vertical)
expect(angularEdgePath({ fromLane: 1, toLane: 1, colorIndex: 0, kind: "merge" }, 0, g))
  .toBe("M28 0 L28 30");
```
(`g = { laneWidth: 16, rowHeight: 30, offsetX: 12 }`. Match the existing test's `g`/edge shape — read it first.)

- [ ] **Step 2: Run — verify fork case fails.** `npx vitest run src/lib/graph/paths.test.ts` → FAIL on the fork expectation.

- [ ] **Step 3: Implement** — change only the `else` (non-branch) return in `angularEdgePath` (keep the `x1===x2` short-circuit and the `kind==="branch"` line):

```ts
export function angularEdgePath(edge: Edge, topY: number, g: GeomConfig): string {
  const x1 = laneX(edge.fromLane, g);
  const x2 = laneX(edge.toLane, g);
  const y1 = topY;
  const y2 = topY + g.rowHeight;
  if (x1 === x2) return `M${x1} ${y1} L${x2} ${y2}`;
  const my = y1 + g.rowHeight / 2;
  if (edge.kind === "branch") return `M${x1} ${y1} L${x1} ${my} L${x2} ${y2}`;
  const s = Math.min(6, g.rowHeight / 3); // stub; 2s ≤ rowHeight ⇒ y1+s ≤ y2-s (no crossing)
  return `M${x1} ${y1} L${x1} ${y1 + s} L${x2} ${y2 - s} L${x2} ${y2}`;
}
```

- [ ] **Step 4: Run — verify pass.** `npx vitest run src/lib/graph/paths.test.ts` → PASS. Then `npm run check` + full `npm test`.

- [ ] **Step 5: Commit.** `git commit -m "feat(graph): angular fork edge stubs out of the node before angling"`

### Task 2: 3-state line style + `effectiveGraphLineStyle` + graph wiring

**Files:** Modify `src/lib/store.svelte.ts` (enumerated sites); `src/lib/components/GraphHistory.svelte:448`. (SettingsPanel's Auto segment lands in Task 3.)

**Interfaces:** Produces `appState.graphLineStyle: "auto"|"curved"|"angular"` (raw) + `appState.effectiveGraphLineStyle: "curved"|"angular"` (derived). GraphHistory passes the **effective** value to `GraphGutter` (whose prop type stays `"curved"|"angular"`).

- [ ] **Step 1: Widen the enum at every validation/type site in `store.svelte.ts`** (locate by the quoted code, line numbers approx):
  - `loadSyncLineStyle` (~173-181): Tauri branch return `"auto"`; accept-list `raw === "auto" || raw === "angular" || raw === "curved" ? raw : "auto"`; default `"auto"`.
  - `$state<"curved" | "angular">` seed (~675) → `$state<"auto" | "curved" | "angular">(loadSyncLineStyle())`.
  - hydrate guard (~686): `saved === "auto" || saved === "curved" || saved === "angular"`.
  - getter/setter types (~1584-1590): `"auto" | "curved" | "angular"`.

- [ ] **Step 2: Add `effectiveGraphLineStyle` derived getter** to the returned object (near `graphLineStyle`), reading the local `theme` + `graphLineStyle` `$state` so it re-resolves on theme toggle:

```ts
    get effectiveGraphLineStyle(): "curved" | "angular" {
      if (graphLineStyle === "curved" || graphLineStyle === "angular") return graphLineStyle;
      return theme === "nerv" ? "angular" : "curved"; // "auto"
    },
```

- [ ] **Step 3: Point the graph at the effective value.** `GraphHistory.svelte:448` (the `<GraphGutter … lineStyle=… />` prop) → `lineStyle={appState.effectiveGraphLineStyle}`. Leave `GraphGutter`'s prop type `"curved" | "angular"` (the type gate then guarantees only a resolved value is passed).

- [ ] **Step 4: Verify.** `npm run check` → 0 errors (this is where a leftover raw `"auto"` to GraphGutter would error). `npm test` green. Browser QA: in NERV the graph is angular by default; toggling to Classic makes it curved; no explicit setting needed yet.

- [ ] **Step 5: Commit.** `git commit -m "feat(graph): 3-state line style (auto) + theme-aware effectiveGraphLineStyle"`

### Task 3: Settings → tabbed sections (+ Auto segment)

**Files:** Modify `src/lib/components/SettingsPanel.svelte` (tab shell + regroup all controls); add `settingsTab` persistence (localStorage-only) either inline or in `store.svelte.ts`.

**Interfaces:** Produces the tabbed shell with groups Appearance / Graph / Commits / Behavior / Updates; the Graph-lines control gains an **Auto** segment.

- [ ] **Step 1: Add a tab strip + `activeTab` state.** Read the current `SettingsPanel.svelte`. Introduce `let activeTab = $state<'appearance'|'graph'|'commits'|'behavior'|'updates'>(loadTab())` where `loadTab` reads `localStorage['gitit.settingsTab.v1']` (guarded, default `'appearance'`); a small `setTab(t)` persists it. Render a `.seg`-style tab row under the dialog `.header`, then render each group's existing markup inside `{#if activeTab === '…'}` blocks. **Move the existing controls into groups** (no control changes yet beyond the Auto segment):
  - Appearance: Theme (existing) — scheme + motion rows are added in Tasks 6/7. Also Diff view.
  - Graph: Graph lines, Merge-in curve, Curviness, Repository switcher.
  - Commits: the 5 date `.opt`s + PR activity order.
  - Behavior: backup / pull-rebase / merge-untracked / output-debug.
  - Updates: auto-check / channel / version.

- [ ] **Step 2: Add the Auto segment + fix the effective/raw bindings** in the Graph-lines control (now in the Graph tab):
  - Add a leading `<button>` for `"auto"` (before Curved/Angular), `class:active={appState.graphLineStyle === "auto"}`, `onclick={() => appState.setGraphLineStyle("auto")}`, `aria-pressed` on raw. Curved/Angular buttons keep **raw** `graphLineStyle` for `class:active`/`aria-pressed`.
  - Change the merge-in + curviness `disabled={appState.graphLineStyle === "angular"}` (6 sites) → `disabled={appState.effectiveGraphLineStyle === "angular"}`.

- [ ] **Step 3: Verify.** `npm run check` 0 errors. Browser QA: Settings opens on the Appearance tab; tabs switch and show the right groups; last tab persists across reopen; Graph-lines shows Auto selected by default and merge-in/curviness disable correctly when effective-angular (NERV default).

- [ ] **Step 4: Commit.** `git commit -m "feat(settings): tabbed sections + Auto graph-line segment"`

### Task 4: Preset palettes + `scheme` store setting + palette/theme wiring

**Files:** Modify `src/lib/graph/colors.ts` (add `Scheme`, `NERV_SCHEME_PALETTES`, `schemeLanePalette`); `src/lib/graph/colors.test.ts`; `src/lib/graph/index.ts` (re-export); `src/lib/theme/themeMode.ts` (set `data-scheme`); `src/lib/store.svelte.ts` (`scheme` setting + palette selection).

**Interfaces:** Produces `appState.scheme` + `setScheme`; `data-scheme` on `<html>`; scheme-aware graph lanes.

- [ ] **Step 1 (TDD): `colors.ts` scheme palettes (pure, frozen, O(1)).** Add:

```ts
export type Scheme = "orange" | "phosphor" | "steel" | "amber" | "violet" | "crimson";
// Each leads with the scheme accent; the rest are shared NERV secondaries (tuned in QA).
export const NERV_SCHEME_PALETTES: Readonly<Record<Scheme, readonly string[]>> = Object.freeze({
  orange:   NERV_LANE_PALETTE, // = existing ["#F2542D","#46E88B","#5AA9E6","#9B7FE0","#E8A33D","#E0608A","#EAE6DA","#8A94A0"]
  phosphor: Object.freeze(["#46E88B","#F2542D","#5AA9E6","#9B7FE0","#E8A33D","#E0608A","#EAE6DA","#8A94A0"]),
  steel:    Object.freeze(["#5AA9E6","#46E88B","#F2542D","#9B7FE0","#E8A33D","#E0608A","#EAE6DA","#8A94A0"]),
  amber:    Object.freeze(["#E8A33D","#46E88B","#5AA9E6","#9B7FE0","#F2542D","#E0608A","#EAE6DA","#8A94A0"]),
  violet:   Object.freeze(["#9B7FE0","#46E88B","#5AA9E6","#E8A33D","#F2542D","#E0608A","#EAE6DA","#8A94A0"]),
  crimson:  Object.freeze(["#E0445A","#46E88B","#5AA9E6","#9B7FE0","#E8A33D","#EAE6DA","#8A94A0","#F2542D"]),
});
export function schemeLanePalette(scheme: Scheme): readonly string[] {
  return NERV_SCHEME_PALETTES[scheme] ?? NERV_LANE_PALETTE;
}
```
Add a `colors.test.ts` case: `schemeLanePalette("phosphor")[0] === "#46E88B"` and `schemeLanePalette("orange") === NERV_LANE_PALETTE` (same frozen ref, no allocation). Run RED→GREEN.

- [ ] **Step 2: Re-export** `Scheme`, `NERV_SCHEME_PALETTES`, `schemeLanePalette` from `src/lib/graph/index.ts` (like `NERV_LANE_PALETTE`).

- [ ] **Step 3: `scheme` store setting** in `store.svelte.ts` — mirror the `theme` pattern EXACTLY (keys `gitit.scheme.v1` / store `scheme`; `loadSyncScheme` reads localStorage **unconditionally**, validates against the 6 values, default `"orange"`; `applySchemeAttr(s)` sets `document.documentElement.dataset.scheme = s` (always set — `orange` too, harmless); hydrate w/ touched-guard; `persistScheme` writes **both** localStorage and the Tauri store; `get scheme` + `setScheme(v)` calling `applySchemeAttr`+`persistScheme`). Import `Scheme`/`schemeLanePalette`/`NERV_SCHEME_PALETTES` from `./graph`.

- [ ] **Step 4: Scheme-aware palette** — in `colorForRef` (~1547) and `colorForIndex` (~1556) change `const palette = theme === "nerv" ? NERV_LANE_PALETTE : LANE_PALETTE;` → `theme === "nerv" ? schemeLanePalette(scheme) : LANE_PALETTE;` (reads the local `scheme` `$state`).

- [ ] **Step 5: `themeMode.ts`** — read `gitit.scheme.v1` synchronously (guarded) and set `document.documentElement.dataset.scheme` pre-paint (only when NERV would be active is fine, but setting it always is harmless).

- [ ] **Step 6: Verify.** `npm run check` + `npm test` (incl. new colors case) green. (No visible change yet — no scheme selector or CSS blocks; `data-scheme` is set but `orange` = base.)

- [ ] **Step 7: Commit.** `git commit -m "feat(theme): scheme setting + per-scheme NERV lane palettes (pure)"`

### Task 5: Accent-as-source-of-truth refactor + per-scheme CSS + Shiki variants

**Files:** Modify `src/routes/+page.svelte` (NERV token block); `src/lib/theme/nerv.css`; `src/lib/diff/nervShikiTheme.ts` (→ factory); `src/lib/diff/highlight.ts`; `src/lib/components/DiffView.svelte`.

**Interfaces:** Makes NERV recolor from `var(--accent)`; adds the 5 non-orange scheme blocks + 6 Shiki variants.

- [ ] **Step 1: Decouple warning to amber (token block, `+page.svelte`).** In the `:global(:root[data-theme="nerv"])` block set `--err: #D9922E;` and `--status-mod: #D9922E;` (was `#F2542D`). Route the **decoration** tints to the accent: `--row-hover: color-mix(in srgb, var(--accent) 6%, transparent);` `--row-selected: color-mix(in srgb, var(--accent) 10%, transparent);` `--row-selected-border: var(--accent);`. Leave `--accent`/`--accent-hover` (orange base) and all other tokens.

- [ ] **Step 2: Route `nerv.css` literals** (per their MEANING):
  - **Decoration → accent:** scanline `rgba(242,84,45,.015)` → `color-mix(in srgb, var(--accent) 1.5%, transparent)`; hazard stripe `rgba(242,84,45,.35)` → `color-mix(in srgb, var(--accent) 35%, transparent)`.
  - **Warning → amber:** `.ref.detached` bg/`.rn`/`.warn-icon` (the 3 `#F2542D`/color-mix uses) → `var(--err)` (or `color-mix(in srgb, var(--err) …)` for the tint); `.banner` heading `strong` + `border-color` → `var(--err)`. Keep `.banner .actions button { background: var(--accent); color: var(--on-accent); }` (a primary CTA — intentionally accent).
  - **Modified glyph → status-mod:** `.glyph.s-mod { background: color-mix(in srgb, var(--status-mod) 10%, transparent); }` (was `rgba(242,84,45,.1)`).

- [ ] **Step 3: Per-scheme blocks in `nerv.css`** (bare selectors — specificity 0,3,0 beats the base 0,2,0):

```css
:root[data-theme="nerv"][data-scheme="phosphor"] { --accent: #46E88B; --accent-hover: #6cf3a5; }
:root[data-theme="nerv"][data-scheme="steel"]    { --accent: #5AA9E6; --accent-hover: #7cbef0; }
:root[data-theme="nerv"][data-scheme="amber"]    { --accent: #E8A33D; --accent-hover: #f2b85c; }
:root[data-theme="nerv"][data-scheme="violet"]   { --accent: #9B7FE0; --accent-hover: #b49bee; }
:root[data-theme="nerv"][data-scheme="crimson"]  { --accent: #E0445A; --accent-hover: #ec6376; }
/* orange = base, no block */
```

- [ ] **Step 4: Shiki factory + 6 variants.** Convert `nervShikiTheme.ts` to `export function nervShikiTheme(name: string, accent: string): ThemeRegistrationRaw` (the keyword `["keyword","storage",…]` + tag `["entity.name.tag"]` foregrounds use `accent`; the rest stay fixed). In `highlight.ts`: import the factory; build the 6 variants and register them:

```ts
const NERV_SHIKI = [
  ["nerv-orange","#F2542D"],["nerv-phosphor","#46E88B"],["nerv-steel","#5AA9E6"],
  ["nerv-amber","#E8A33D"],["nerv-violet","#9B7FE0"],["nerv-crimson","#E0445A"],
].map(([n,a]) => nervShikiTheme(n, a));
// createHighlighter({ themes: ["github-light","github-dark", ...NERV_SHIKI], langs: […] })
```
In `DiffView.svelte:42-43`: `const activeTheme = $derived(appState.theme === "nerv" ? "nerv-" + appState.scheme : (darkMode ? "github-dark" : "github-light"));` (the token cache is already keyed by `activeTheme`).

- [ ] **Step 5: Verify.** `npm run check` + `npm test` green. **Browser QA:** with `data-scheme` toggled via devtools (`document.documentElement.dataset.scheme='phosphor'`), the whole NERV UI (accent, brackets, row hover/selected, scanline, hazard, HEAD ring, diff syntax) recolors; **warnings/detached-HEAD/modified stay amber** (not the accent) in every scheme; Classic unaffected. Verify each of the 6 schemes.

- [ ] **Step 6: Commit.** `git commit -m "feat(theme): route NERV decoration through --accent + 6 scheme presets + Shiki variants"`

### Task 6: Scheme selector row (Appearance tab)

**Files:** Modify `src/lib/components/SettingsPanel.svelte`.

- [ ] **Step 1:** In the Appearance tab (after the Theme row), add a NERV-only **Color scheme** row (`{#if appState.theme === "nerv"}`) — a swatch/`.seg` row of the 6 schemes; each button `class:active={appState.scheme === s}` / `onclick={() => appState.setScheme(s)}` / an accessible label; render a color chip per scheme.

- [ ] **Step 2: Verify.** Browser QA: in NERV the Color-scheme row appears; clicking a scheme live-recolors the app + persists (reload keeps it, no FOUC); in Classic the row is hidden. `npm run check` 0 errors.

- [ ] **Step 3: Commit.** `git commit -m "feat(settings): NERV color-scheme selector (Appearance tab)"`

### Task 7: `motion` setting + gating + ambient split + interaction

**Files:** Add `src/lib/theme/nerv-motion.css`; Modify `src/routes/+layout.ts` (import it); `src/lib/theme/themeMode.ts` (set `data-motion`); `src/lib/store.svelte.ts` (`motion` setting); `src/lib/theme/nerv.css` (split the overlay); `src/lib/components/SettingsPanel.svelte` (motion row).

**Interfaces:** `appState.motion` (bool, default true) → `data-motion="on"`; all motion CSS gated.

- [ ] **Step 1: `motion` store setting — default ON, ATTRIBUTE-PRESENT-BY-DEFAULT (inverted vs theme).** `loadSyncMotion()` returns `true` unless localStorage `gitit.motion.v1 === "off"` (absent ⇒ true). `applyMotionAttr(on)` sets `data-motion="on"` when on, removes it when off. Hydrate default true. `persistMotion` writes `"on"|"off"` to **both** stores. `get motion` + `setMotion(b)`.

- [ ] **Step 2: `themeMode.ts`** — set `data-motion="on"` pre-paint UNLESS `localStorage['gitit.motion.v1'] === "off"` (guarded).

- [ ] **Step 3: Split the ambient overlay** in `nerv.css`. The current single root `::after` scanline (static grid + faint accent wash) stays STATIC. Add, in `nerv-motion.css`, a **separate** thin scanline layer as a second fixed `pointer-events:none` element/pseudo animated with `transform: translateY(...)` only, gated:

```css
@media (prefers-reduced-motion: no-preference) {
  :root[data-theme="nerv"][data-motion="on"] .some-drift-layer { animation: nerv-scanline-drift 8s linear infinite; }
}
@keyframes nerv-scanline-drift { to { transform: translateY(3px); } } /* transform only */
```
(Exact layer element + values tuned in QA. Do NOT animate `background-position` on the full-viewport layer.)

- [ ] **Step 4: Interaction motion** in `nerv-motion.css` (gated): corner brackets brighten on `:root[data-theme="nerv"][data-motion="on"] .panel:hover::after` / `.dialog:hover::after` (transition `--c`/opacity); active-segment + row-selection short `transition`. `● LIVE`/blink already exists — move under the `data-motion` gate.

- [ ] **Step 5: Motion toggle row** in SettingsPanel Appearance tab (NERV-only), `.opt` checkbox bound to `appState.motion` / `setMotion`.

- [ ] **Step 6: Verify.** `npm run check` green. Browser QA: with motion on, brackets brighten on hover + the scanline drifts subtly + no framerate drop scrolling the graph; toggling motion off (and with `prefers-reduced-motion: reduce` via `preview_resize colorScheme`/emulation) freezes all motion → static NERV. `data-motion` absent in Classic.

- [ ] **Step 7: Commit.** `git commit -m "feat(theme): NERV motion setting + ambient drift + interaction (gated)"`

### Task 8: Boot reveal

**Files:** Modify `src/lib/store.svelte.ts` (or a small helper) to pulse `data-boot`; `src/lib/theme/themeMode.ts` (boot on launch); `src/lib/theme/nerv-motion.css` (boot keyframes).

- [ ] **Step 1: `data-boot` pulse.** On NERV activation — (a) launch with NERV saved: in `themeMode.ts`, after setting `data-theme="nerv"`, set `document.documentElement.dataset.boot=""` and `setTimeout(() => delete …dataset.boot, 600)`; (b) toggle Classic→NERV: in `setTheme("nerv")` do the same pulse. Only pulse when motion is on (skip if `data-motion` absent). Guarded for SSR.

- [ ] **Step 2: Boot keyframes** in `nerv-motion.css` (gated `[data-motion="on"]` + reduced-motion): while `:root[data-theme="nerv"][data-boot]`, play a one-shot scanline sweep + bracket draw-in + shell `opacity/scale` fade-up (`transform`/`opacity` only, ~500ms).

- [ ] **Step 3: Verify.** Browser QA: toggling Classic→NERV plays a brief reveal; reload with NERV saved plays it on launch; motion-off / reduced-motion ⇒ no reveal (instant). `npm run check` green.

- [ ] **Step 4: Commit.** `git commit -m "feat(theme): NERV boot reveal on activation (gated)"`

### Task 9: Timeline graph reveal (armed on repo identity)

**Files:** Modify `src/lib/store.svelte.ts` (`revealing`/`revealSeq` + `armReveal` + arm in `setGraphCommits`); `src/routes/+page.svelte:195` effect (view-switch arm); `src/lib/components/GraphHistory.svelte` (row-in stagger while revealing); `src/lib/components/GraphGutter.svelte` (per-edge stroke-draw while revealing); `src/lib/theme/nerv-motion.css` (keyframes).

**Interfaces:** `appState.revealing` (bool) + `appState.armReveal()`; consumed by GraphHistory/GraphGutter.

- [ ] **Step 1: Store reveal state (no `$effect`).**

```ts
let revealing = $state(false);
let revealSeq = 0;
const REVEAL_MS = 1100; // ≥ (max staggered rows × step) + rowDuration; per-row delay is capped in the component
// in the returned object:
get revealing() { return revealing; },
armReveal() {
  revealSeq++; revealing = true;
  const seq = revealSeq;
  setTimeout(() => { if (revealSeq === seq) revealing = false; }, REVEAL_MS);
},
```

- [ ] **Step 2: Arm on repo-identity change** — in `setGraphCommits` (`:1805`) capture BEFORE reassigning and arm only on a genuine repo transition; do NOT touch `applyGraphRefresh`/`appendGraphCommits`:

```ts
setGraphCommits(gc: GraphCommit[]) {
  const repoChanged = graphCommitsRepo !== repo; // identity transition (incl. first load)
  graphCommits = gc;
  graphCommitsRepo = repo;
  // …existing resets…
  if (repoChanged) this.armReveal(); // or call a local armReveal() fn
},
```
(If `this` isn't available in the object-literal method, extract `armReveal` to a `makeState`-local function and call it from both here and the exposed method.)

- [ ] **Step 3: View-switch arm (same-repo only), reusing the existing effect** at `+page.svelte:195`. Where it computes `changed` (using the OLD `prevView`/`prevRepo` before reassignment), also compute and, after the existing animate block, fire:

```ts
const revealOnViewSwitch = view === "timeline" && prevView !== "timeline" && repo === prevRepo;
// …after the existing el.animate(...)…
if (revealOnViewSwitch && !window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches) {
  appState.armReveal();
}
```
(Repo switches are handled by Step 2; the `repo === prevRepo` guard prevents a double-arm.)

- [ ] **Step 4: Row-in stagger** in `GraphHistory.svelte` (gated by `appState.revealing`). Apply a `nerv-row-in` class + `style="animation-delay: {(i - winStart) * STEP}ms"` (STEP ≈ 15ms) to each rendered row while `appState.revealing`. Cap the effective index (e.g. `Math.min(i - winStart, 40)`) so the max delay stays under `REVEAL_MS`. The keyframes live in `nerv-motion.css` (gated on `data-theme="nerv"][data-motion="on"]` + reduced-motion), so Classic / motion-off get no animation.

- [ ] **Step 5: Per-edge stroke-draw** in `GraphGutter.svelte` (gated by a `revealing` prop passed from GraphHistory, or read `appState.revealing`). While revealing, add a class to the edge `<path>`s that animates `stroke-dashoffset` from `pathLength` to 0 (set `pathLength="1"` + `stroke-dasharray:1; stroke-dashoffset:1` and animate to 0), staggered to match the row cascade. Bounded to the rendered window. Keyframes in `nerv-motion.css`, gated.

- [ ] **Step 6: Verify (the critical one).** Browser QA in the app:
  - Open a repo / switch to the timeline → the graph builds top-to-bottom once (lines draw + rows cascade).
  - **Scroll during & after the reveal → NO replay.**
  - **Simulate a refresh** (`appState.applyGraphRefresh(appState.graphCommits)` via `preview_eval`, or trigger a live refresh) → NO replay.
  - Switch Local Changes ↔ Timeline (same repo) → plays once per switch (acceptable) — confirm not doubled on a repo switch.
  - Motion off / reduced-motion → the graph appears instantly (no reveal).
  - `npm run check` + `npm test` green.

- [ ] **Step 7: Commit.** `git commit -m "feat(graph): NERV timeline reveal (armed on repo identity, one-shot)"`

### Task 10: Final gate + full visual QA

**Files:** none (verification); commit any QA fixes.

- [ ] **Step 1: Full gate.** `npm run check` (0) + `npm test` + `cargo test` → all green.
- [ ] **Step 2: Full browser QA** (`npm run dev`, both OS light/dark): all 6 schemes recolor everything with warnings staying amber; motion on/off + reduced-motion gate cleanly (boot, ambient drift, interaction, timeline reveal); reveal plays once and does NOT replay on scroll/refresh/focus; Settings tabs group correctly with NERV-only rows conditional and last-tab remembered; angular fork stub shows in both themes; NERV defaults to angular (Auto), Classic to curved. **Classic verified unchanged** except the fork-edge stub (check curved + angular Classic, no `data-scheme`/`data-motion`/reveal leakage).
- [ ] **Step 3:** Commit QA fixes; the work rides on PR #9.

## Self-review notes

- **Spec coverage:** angular stub (T1), 3-state + effective (T2/T3), tabs (T3), scheme palettes+store (T4), scheme CSS+Shiki (T5), scheme selector (T6), motion+ambient+interaction (T7), boot (T8), timeline reveal (T9), gate/QA (T10). Acceptance #1-6 map.
- **Ordering:** tabs (T3) precede the new scheme/motion rows (T6/T7) to avoid double-editing SettingsPanel; angular/line-style (T1/T2) are independent and first.
- **Non-TDD:** CSS/motion tasks verified by green gate + browser QA (only `paths.ts` T1 and `schemeLanePalette` T4 are unit-testable).
- **Type-consistency:** `Scheme`, `schemeLanePalette`, `effectiveGraphLineStyle` (`"curved"|"angular"`), `graphLineStyle` (`"auto"|"curved"|"angular"`), `revealing`/`armReveal`, `data-scheme`/`data-motion`/`data-boot` used consistently. GraphGutter's `lineStyle` prop type stays `"curved"|"angular"` (fed the effective value).
