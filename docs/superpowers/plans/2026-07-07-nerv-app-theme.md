# NERV App Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a user-toggleable **NERV** theme (Settings → Appearance) alongside the current **Classic** appearance, re-skinning the same dense UI in the git-it.app console language (orange/bone/phosphor, Anton + IBM Plex, angular corners, HUD framing) — colors, type, shape, and decoration swap; layout/density/box-model do not.

**Architecture:** Everything hangs off `<html data-theme="nerv">`. A **token layer** re-declares themeable CSS custom properties (color **and** new font/radius/on-accent/diff tokens) with NERV values; consumers already read `var(--…)`, so the swap reaches them once the currently-hardcoded sites are tokenized. A **decoration layer** (`nerv.css`, `data-theme`-scoped) adds the HUD motifs. Theme choice persists (Tauri store + localStorage) and is applied **before paint** to avoid a flash. Executed in three phases: **Phase 0** spike (de-risk the HUD), **Phase A** token theme (colors+type+shape, independently shippable), **Phase B** HUD decoration.

**Tech Stack:** Tauri 2 (Rust) + SvelteKit 5 (runes), `@tauri-apps/plugin-store`, Shiki (diff highlighting), self-hosted woff2 fonts, Vitest.

## Global Constraints

- **Full green gate before any task is "done":** `npm run check` (0 errors) + `npm test` + `cargo test` (copied from `CLAUDE.md`). No Rust changes are expected; keep the workspace green.
- **Classic must render identically.** Every new CSS selector is scoped `:global(:root[data-theme="nerv"]) …`; every new token's **Classic** value equals the literal it replaced (verify per task).
- **Keep `src/lib/graph/colors.ts` pure and Tauri-free** (`CLAUDE.md` rule) — no store/DOM imports; it stays unit-tested.
- **No `$effect` in `store.svelte.ts`** — it is a module-scope factory; a bare `$effect` throws `effect_orphan` at boot. Reflect `data-theme` imperatively.
- **License/copy:** the app is **not** "open source" (CC BY-NC-SA 4.0); bundled fonts keep their SIL **OFL** notices.
- **macOS-only, Tauri-window truth:** the definitive visual QA runs under `npm run tauri dev`; Vitest/browser cannot exercise `invoke` paths.
- **NERV accent discipline:** orange leads; green = status/added only; red = hazard/conflict/deleted only; bone for headings. The one sanctioned exception is the GitHub screen's PR/issue state badges (Task A11).

---

## Phase 0 — Vertical spike (de-risk the HUD)

### Task 0: HUD feasibility spike

**Files:**
- Create (throwaway branch/commit, or a scratch commit reverted after): temporary edits to `src/lib/theme/nerv.css` and ONE panel + ONE dialog + the status bar.
- Create: `docs/superpowers/plans/2026-07-07-nerv-spike-findings.md` (the deliverable).

**Interfaces:**
- Produces: a findings doc that feeds Phase B — the enumerated corner-bracket host list, the chosen bracket delivery mechanism (shared wrapper hook vs per-host pseudo), and a go/no-go on the fixed scanline overlay's per-frame cost.

- [ ] **Step 1: Stand up a minimal NERV attribute + overlay to experiment with.** In a scratch working state, temporarily set `document.documentElement.dataset.theme = "nerv"` (via the browser console or a hardcoded line) and add to a scratch `nerv.css`:

```css
:global(:root[data-theme="nerv"])::after {
  content: ""; position: fixed; inset: 0; pointer-events: none; z-index: 9999;
  background:
    repeating-linear-gradient(0deg, rgba(255,255,255,.02) 0 1px, transparent 1px 3px),
    linear-gradient(rgba(242,84,45,.015), rgba(242,84,45,.015));
}
@media (prefers-reduced-motion: reduce) { :global(:root[data-theme="nerv"])::after { animation: none; } }
```

- [ ] **Step 2: Measure the overlay's cost.** Run `npm run tauri dev`, scroll the commit graph hard, and confirm framerate is unaffected (the app previously hit a `backdrop-filter` per-frame repaint wall — `+page.svelte:777-785`). Record the result. If it regresses, note that the overlay must be a static painted layer, not animated.

- [ ] **Step 3: Try corner brackets on a SCROLL panel and a dialog.** Pick the commit list (a scroll container, `overflow:auto`) and one dialog (e.g. `AmendDialog`). Attempt bracket `::before/::after`. Observe: (a) does `overflow` clip them? (b) does making the host `position: relative` move any existing absolutely-positioned descendant? Record which hosts need a non-clipping wrapper.

- [ ] **Step 4: Try a mono header bar** (`◇ LABEL … ● LIVE`) on one panel — determine whether it needs additive markup and, if so, how minimal.

- [ ] **Step 5: Write the findings doc.** In `docs/superpowers/plans/2026-07-07-nerv-spike-findings.md` record: overlay go/no-go, the enumerated list of components that need a bracket wrapper vs work as pure pseudo, the chosen shared-hook approach (e.g. a single `data-nerv-frame` wrapper class added once per host), and the measured edit count. **This replaces the placeholders in Phase B.**

- [ ] **Step 6: Revert all scratch edits.** `git checkout -- .` (or discard the scratch commit). The spike ships **only** the findings doc.

```bash
git add docs/superpowers/plans/2026-07-07-nerv-spike-findings.md
git commit -m "docs(theme): NERV HUD spike findings (bracket hosts + overlay cost)"
```

---

## Phase A — Token theme (colors + type + shape)

### Task A1: Bundle the NERV fonts

**Files:**
- Create: `static/fonts/anton-latin-400-normal.woff2`, `ibm-plex-sans-latin-400-normal.woff2`, `ibm-plex-sans-latin-500-normal.woff2` (NEW weight), `ibm-plex-sans-latin-600-normal.woff2`, `ibm-plex-mono-latin-400-normal.woff2`, `ibm-plex-mono-latin-500-normal.woff2`
- Create: `static/fonts/Anton-OFL.txt`, `static/fonts/IBMPlex-OFL.txt`

**Interfaces:**
- Produces: woff2 assets at `/fonts/*.woff2` (SvelteKit static adapter serves `static/` at the app root), referenced by Task A2's `@font-face`.

- [ ] **Step 1: Copy the existing woff2 + OFL notices from the website into `static/fonts/`.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
mkdir -p static/fonts
cp website/assets/fonts/anton-latin-400-normal.woff2 static/fonts/
cp website/assets/fonts/ibm-plex-sans-latin-400-normal.woff2 static/fonts/
cp website/assets/fonts/ibm-plex-sans-latin-600-normal.woff2 static/fonts/
cp website/assets/fonts/ibm-plex-mono-latin-400-normal.woff2 static/fonts/
cp website/assets/fonts/ibm-plex-mono-latin-500-normal.woff2 static/fonts/
cp website/assets/fonts/Anton-OFL.txt website/assets/fonts/IBMPlex-OFL.txt static/fonts/
ls static/fonts/
```
Expected: the five copied woff2 + two OFL txt files listed.

- [ ] **Step 2: Add the missing IBM Plex Sans 500 weight.** Download `ibm-plex-sans-latin-500-normal.woff2` from the same @fontsource source the other faces came from (jsdelivr: `https://cdn.jsdelivr.net/fontsource/fonts/ibm-plex-sans@latest/latin-500-normal.woff2`) into `static/fonts/`.

```bash
curl -fsSL "https://cdn.jsdelivr.net/fontsource/fonts/ibm-plex-sans@latest/latin-500-normal.woff2" -o static/fonts/ibm-plex-sans-latin-500-normal.woff2
ls -la static/fonts/ibm-plex-sans-latin-500-normal.woff2
```
Expected: a non-empty woff2 (~30-40KB). If the network is unavailable, note it and proceed — NERV body weight-500 will faux-synthesize until this is added (acceptable fallback).

- [ ] **Step 3: Commit.**

```bash
git add static/fonts/
git commit -m "feat(theme): bundle NERV woff2 fonts (Anton, IBM Plex Sans 400/500/600, Mono 400/500)"
```

### Task A2: Global `nerv.css` skeleton + `@font-face` + wire it in

**Files:**
- Create: `src/lib/theme/nerv.css`
- Modify: `src/routes/+layout.ts` (add the import beside `$lib/tauriMode`)

**Interfaces:**
- Produces: a globally-imported, `data-theme`-scoped stylesheet (empty of decoration for now) with all `@font-face` declarations. Phase B fills the decoration.

- [ ] **Step 1: Create `src/lib/theme/nerv.css`** with the `@font-face` block (adapted from `website/assets/fonts/fonts.css`, paths pointing at `/fonts/…`, plus the new Sans 500):

```css
/* NERV theme — global, data-theme-scoped. @font-face declarations are unconditional
   but cost nothing until NERV actually renders these families. SIL OFL notices ship
   in static/fonts/{Anton-OFL,IBMPlex-OFL}.txt. */
@font-face { font-family: "Anton"; font-style: normal; font-weight: 400; font-display: swap;
  src: url("/fonts/anton-latin-400-normal.woff2") format("woff2"); }
@font-face { font-family: "IBM Plex Sans"; font-style: normal; font-weight: 400; font-display: swap;
  src: url("/fonts/ibm-plex-sans-latin-400-normal.woff2") format("woff2"); }
@font-face { font-family: "IBM Plex Sans"; font-style: normal; font-weight: 500; font-display: swap;
  src: url("/fonts/ibm-plex-sans-latin-500-normal.woff2") format("woff2"); }
@font-face { font-family: "IBM Plex Sans"; font-style: normal; font-weight: 600; font-display: swap;
  src: url("/fonts/ibm-plex-sans-latin-600-normal.woff2") format("woff2"); }
@font-face { font-family: "IBM Plex Mono"; font-style: normal; font-weight: 400; font-display: swap;
  src: url("/fonts/ibm-plex-mono-latin-400-normal.woff2") format("woff2"); }
@font-face { font-family: "IBM Plex Mono"; font-style: normal; font-weight: 500; font-display: swap;
  src: url("/fonts/ibm-plex-mono-latin-500-normal.woff2") format("woff2"); }

/* ── Phase B decoration goes below (all rules :global(:root[data-theme="nerv"]) …) ── */
```

- [ ] **Step 2: Import it (and reserve the themeMode import slot) in `src/routes/+layout.ts`.** After line 5 (`import "$lib/tauriMode";`):

```ts
import "$lib/tauriMode";
import "$lib/theme/themeMode";   // sets data-theme before paint (Task A3)
import "$lib/theme/nerv.css";    // global NERV stylesheet
```

- [ ] **Step 3: Create a stub `src/lib/theme/themeMode.ts`** so the import resolves (real logic in A3):

```ts
// Placeholder — real no-FOUC logic lands in Task A3.
export {};
```

- [ ] **Step 4: Verify the app still builds and Classic is unaffected.**

Run: `npm run check`
Expected: 0 errors. (Fonts declared but unused → Classic unchanged.)

- [ ] **Step 5: Commit.**

```bash
git add src/lib/theme/nerv.css src/lib/theme/themeMode.ts src/routes/+layout.ts
git commit -m "feat(theme): add global nerv.css (@font-face) wired via +layout.ts"
```

### Task A3: No-FOUC — set `data-theme` before paint

**Files:**
- Modify (replace stub): `src/lib/theme/themeMode.ts`

**Interfaces:**
- Consumes: `localStorage["gitit.theme.v1"]` (written by Task A4's `persistTheme`).
- Produces: `document.documentElement`'s `data-theme` attribute set synchronously at import, before first paint.

- [ ] **Step 1: Replace `themeMode.ts` with the real, guarded side-effect** (mirrors `tauriMode.ts`):

```ts
// Sets data-theme="nerv" on <html> BEFORE first paint when the saved theme is NERV,
// so launch never flashes Classic. Mirrors tauriMode.ts: a synchronous module
// side-effect, guarded for SSR/build/test (no document/localStorage). The durable
// source of truth is the Tauri store (hydrated a tick later by store.svelte.ts); we
// also persist to localStorage (persistTheme) so this synchronous read is current.
const THEME_KEY = "gitit.theme.v1";

function applySavedTheme(): void {
  if (typeof document === "undefined") return;
  try {
    if (typeof localStorage === "undefined") return;
    if (localStorage.getItem(THEME_KEY) === "nerv") {
      document.documentElement.setAttribute("data-theme", "nerv");
    }
    // "classic" / null → attribute absent (Classic is the default)
  } catch {
    // ignore — Classic is a safe default
  }
}

applySavedTheme();
```

- [ ] **Step 2: Verify build + that a manually-seeded localStorage value paints NERV pre-hydrate.**

Run: `npm run check` → 0 errors. Then under `npm run tauri dev`, in devtools run `localStorage.setItem("gitit.theme.v1","nerv"); location.reload()` and confirm `<html>` has `data-theme="nerv"` immediately on load (even before Task A6's tokens make it visible).

- [ ] **Step 3: Commit.**

```bash
git add src/lib/theme/themeMode.ts
git commit -m "feat(theme): set data-theme before paint (no-FOUC) via themeMode.ts"
```

### Task A4: `theme` state + persistence in the store

**Files:**
- Modify: `src/lib/store.svelte.ts` — add keys near line 61; `loadSyncTheme` + `applyThemeAttr` near the other `loadSync*`; state + hydrate + `persistTheme` inside `makeState()` near the `graphLineStyle` block (~654-687); `get theme` + `setTheme` in the returned object near `setGraphLineStyle` (~1527-1534).

**Interfaces:**
- Consumes: existing `getStore()`, `isTauri()`.
- Produces: `appState.theme: "classic" | "nerv"` and `appState.setTheme(v)`. Task A5 (Settings), A7 (palette selection), A8 (Shiki) read `appState.theme`.

- [ ] **Step 1: Add the storage keys** after line 62 (`const LINESTYLE_STORE_KEY = "graphLineStyle";`):

```ts
const THEME_KEY = "gitit.theme.v1";       // localStorage (also read synchronously by themeMode.ts)
const THEME_STORE_KEY = "theme";          // Tauri store (durable)
```

- [ ] **Step 2: Add the sync loader + imperative attribute helper** near `loadSyncLineStyle` (~line 170). **DEVIATION from the mirror is intentional and required** (see comments):

```ts
// DEVIATION vs loadSyncLineStyle: reads localStorage UNCONDITIONALLY (incl. under
// Tauri) so the store seed matches what themeMode.ts already painted pre-paint.
function loadSyncTheme(): "classic" | "nerv" {
  try {
    if (typeof localStorage === "undefined") return "classic";
    return localStorage.getItem(THEME_KEY) === "nerv" ? "nerv" : "classic";
  } catch {
    return "classic";
  }
}

// Reflect the theme onto <html> imperatively — NO $effect (module-scope factory).
function applyThemeAttr(t: "classic" | "nerv"): void {
  if (typeof document === "undefined") return;
  if (t === "nerv") document.documentElement.setAttribute("data-theme", "nerv");
  else document.documentElement.removeAttribute("data-theme");
}
```

- [ ] **Step 3: Add the state, hydrate, and persist inside `makeState()`** next to the `graphLineStyle` block (after ~line 687). Mirrors the pattern with the required localStorage deviation in `persistTheme`:

```ts
let theme = $state<"classic" | "nerv">(loadSyncTheme());
let themeTouched = false;

const themeHydrate = getStore();
if (themeHydrate) {
  themeHydrate
    .then((store) => store.get<string>(THEME_STORE_KEY))
    .then((saved) => {
      if ((saved === "classic" || saved === "nerv") && !themeTouched) {
        theme = saved;
        applyThemeAttr(saved);
      }
    })
    .catch((e) => console.warn("[gte] could not load theme", e));
}

function persistTheme() {
  const snapshot = theme;
  // DEVIATION vs persistLineStyle: write localStorage in BOTH branches so
  // themeMode.ts's synchronous pre-paint read is always current.
  try {
    if (typeof localStorage !== "undefined") localStorage.setItem(THEME_KEY, snapshot);
  } catch (e) {
    console.warn("[gte] could not persist theme (localStorage)", e);
  }
  const sp = getStore();
  if (sp) {
    sp.then(async (store) => {
      await store.set(THEME_STORE_KEY, snapshot);
      await store.save();
    }).catch((e) => console.warn("[gte] could not persist theme (store)", e));
  }
}
```

- [ ] **Step 4: Expose `theme` + `setTheme`** in the returned object near `setGraphLineStyle` (~1534):

```ts
get theme() {
  return theme;
},
setTheme(v: "classic" | "nerv") {
  themeTouched = true;
  theme = v;
  applyThemeAttr(v);
  persistTheme();
},
```

- [ ] **Step 5: Verify.**

Run: `npm run check` → 0 errors. `npm test` → green (no regressions).

- [ ] **Step 6: Commit.**

```bash
git add src/lib/store.svelte.ts
git commit -m "feat(theme): add persisted theme state (classic/nerv) to the store"
```

### Task A5: Settings → Appearance "Theme" control

**Files:**
- Modify: `src/lib/components/SettingsPanel.svelte` — add a `seg-row` as the FIRST row of the Appearance group (before "Graph lines" at line 61).

**Interfaces:**
- Consumes: `appState.theme`, `appState.setTheme`.

- [ ] **Step 1: Insert the Theme segmented control** immediately after `<p class="group-label">Appearance</p>` (line 59), cloning the existing "Graph lines" `.seg` idiom (lines 61-76):

```svelte
      <div class="seg-row">
        <span class="seg-label">Theme</span>
        <div class="seg" role="group" aria-label="App theme">
          <button
            type="button"
            class:active={appState.theme === "classic"}
            onclick={() => appState.setTheme("classic")}
            aria-pressed={appState.theme === "classic"}
          >Classic</button><button
            type="button"
            class:active={appState.theme === "nerv"}
            onclick={() => appState.setTheme("nerv")}
            aria-pressed={appState.theme === "nerv"}
          >NERV</button>
        </div>
      </div>
```

- [ ] **Step 2: Verify the toggle flips `data-theme` live.**

Run: `npm run tauri dev`; open Settings → Appearance; click NERV, then Classic. In devtools confirm `<html data-theme="nerv">` appears/disappears. (No visible re-skin yet — that's Task A6.)

- [ ] **Step 3: Commit.**

```bash
git add src/lib/components/SettingsPanel.svelte
git commit -m "feat(theme): add Classic/NERV toggle to Settings → Appearance"
```

### Task A6: The NERV token block + Classic-side token declarations

**Files:**
- Modify: `src/routes/+page.svelte` — add new token declarations to `:global(:root)` (~612-642) and the `@media dark` block (~644-669); append the NERV block **after** the glass blocks (after ~704).

**Interfaces:**
- Produces: every themeable `--var` resolved for NERV. Later tasks (A7 lanes, A8 diff, A9 mono, A10 radius, A11 on-accent, A12 display) reference these tokens.

- [ ] **Step 1: Declare the NEW token families in the Classic `:global(:root)` block** (each equal to today's literal, so Classic is unchanged), after the existing `--err` line (~630):

```css
    /* type + shape + on-accent + diff tokens — Classic values (identical to today) */
    --on-accent: #fff;
    --font-sans: -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
    --font-mono: ui-monospace, SFMono-Regular, Menlo, monospace;
    --font-display: var(--font-sans);
    --radius-sm: 4px; --radius-md: 6px; --radius-lg: 10px;
    --diff-add-bg: rgba(46,160,67,.18);  --diff-del-bg: rgba(210,35,35,.18);
    --diff-add-bg-dark: rgba(46,160,67,.24); --diff-del-bg-dark: rgba(210,35,35,.24);
    --diff-add-fg: #2da44e; --diff-del-fg: #cf222e;
```
Also change the root `font-family:` (line ~638) to reference the token:
```css
    font-family: var(--font-sans);
```

- [ ] **Step 2: Override the diff `-fg` (and, if desired, the `-dark` bg) in the `@media dark` block** so Classic dark keeps its current github-dark diff colors. In the `@media (prefers-color-scheme: dark) :global(:root)` block (~644-669) add:

```css
      --diff-add-fg: #3fb950; --diff-del-fg: #f85149;
```
(The `-bg`/`-bg-dark` Classic values already match DiffView's fallbacks; leave them.)

- [ ] **Step 3: Append the NERV token block AFTER both `[data-tauri]` glass blocks** (after line ~704). Keep the placement comment:

```css
  /* ===== NERV theme tokens — MUST remain AFTER both [data-tauri] glass blocks
     (equal specificity → source order wins). NERV is dark-only + opaque and
     ignores prefers-color-scheme. ===== */
  :global(:root[data-theme="nerv"]) {
    --bg: #0A0C0F;
    --panel-bg: #12171C;
    --popover-bg: #0E1216;
    --header-bg: #0E1216;
    --input-bg: #0E1216;
    --btn-bg: #0E1216;
    --btn-hover: #14181d;
    --border: #2E3742;
    --border-subtle: #232A31;
    --text: #EAE6DA;
    --text-muted: #8A94A0;
    --row-hover: rgba(242,84,45,.06);
    --row-selected: rgba(242,84,45,.10);
    --row-selected-border: #F2542D;
    --accent: #F2542D;
    --accent-hover: #ff6a44;
    --on-accent: #0A0C0F;                 /* dark text on orange (AA) */
    --danger: #FF4438;
    --danger-hover: #ff5a4f;
    --err: #F2542D;                        /* NERV warning = orange; red reserved for hazard */
    --status-add: #46E88B;
    --status-mod: #F2542D;
    --status-del: #FF4438;
    /* diff — BOTH tiers (NERV ignores prefers-color-scheme, but @media dark still matches) */
    --diff-add-bg: rgba(70,232,139,.15);  --diff-add-bg-dark: rgba(70,232,139,.15);
    --diff-del-bg: rgba(255,68,56,.15);   --diff-del-bg-dark: rgba(255,68,56,.15);
    --diff-add-fg: #46E88B;               --diff-del-fg: #FF4438;
    /* type */
    --font-sans: "IBM Plex Sans", system-ui, -apple-system, sans-serif;
    --font-mono: "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
    --font-display: "Anton", "Arial Narrow", Impact, sans-serif;
    /* shape — near-sharp */
    --radius-sm: 0px; --radius-md: 2px; --radius-lg: 2px;
    color-scheme: dark;
    font-family: var(--font-sans);
  }
  /* opaque-over-glass: cover BOTH root and body (the transparent reset at ~715-718
     targets body too). (0,3,0)/(0,3,1) → order-independent for this load-bearing rule. */
  :global(:root[data-theme="nerv"][data-tauri="true"]),
  :global(:root[data-theme="nerv"][data-tauri="true"] body) {
    background: var(--bg);
  }
```

- [ ] **Step 4: Verify NERV re-skins and Classic is unchanged.**

Run: `npm run tauri dev`. Toggle NERV: the whole app should go near-black with orange accents (fonts/radius/lanes/diff come in later tasks). Toggle Classic: pixel-identical to before. Also run `npm run check` → 0 errors (watch for "unused CSS selector" — the `:global()` wrappers prevent scoping-away).

- [ ] **Step 5: Commit.**

```bash
git add src/routes/+page.svelte
git commit -m "feat(theme): add NERV token block + tokenize font/radius/on-accent/diff"
```

### Task A7: Theme-aware commit-graph lane palette

**Files:**
- Modify: `src/lib/graph/colors.ts` (add `NERV_LANE_PALETTE`, add `palette` param to `laneColor`)
- Modify: `src/lib/graph/colors.test.ts` (add cases)
- Modify: `src/lib/graph/index.ts` (re-export `NERV_LANE_PALETTE` if it barrels `colors`)
- Modify: `src/lib/store.svelte.ts` (`colorForRef`/`colorForIndex` select the palette by `theme`)

**Interfaces:**
- Consumes: `appState.theme`.
- Produces: `laneColor(idx, branch, overrides, palette?)`, `NERV_LANE_PALETTE`. All graph colors already route through `appState.colorForIndex`/`colorForRef`, so no component edits are needed; the HEAD ring is already `var(--accent)`.

- [ ] **Step 1 (TDD): Add failing tests** to `src/lib/graph/colors.test.ts` — import `NERV_LANE_PALETTE` and assert palette injection:

```ts
import { LANE_PALETTE, NERV_LANE_PALETTE, laneColor } from "./colors";

  it("uses a provided palette when passed", () => {
    expect(laneColor(0, null, {}, NERV_LANE_PALETTE)).toBe(NERV_LANE_PALETTE[0]);
    expect(laneColor(NERV_LANE_PALETTE.length, null, {}, NERV_LANE_PALETTE)).toBe(NERV_LANE_PALETTE[0]);
  });
  it("defaults to the classic palette when no palette is passed", () => {
    expect(laneColor(3, null, {})).toBe(LANE_PALETTE[3]);
  });
```

- [ ] **Step 2: Run — verify it fails.**

Run: `npx vitest run src/lib/graph/colors.test.ts`
Expected: FAIL (`NERV_LANE_PALETTE` is not exported; `laneColor` ignores a 4th arg).

- [ ] **Step 3: Implement in `src/lib/graph/colors.ts`** — add the palette and the parameter (default preserves all existing behavior):

```ts
export const NERV_LANE_PALETTE: string[] = [
  "#F2542D", // orange (lead)
  "#46E88B", // phosphor
  "#5AA9E6", // steel-cyan
  "#9B7FE0", // violet
  "#E8A33D", // amber
  "#E0608A", // pink
  "#EAE6DA", // bone
  "#8A94A0", // haze
];

export function laneColor(
  colorIndex: number,
  branchName: string | null,
  overrides: Record<string, string>,
  palette: string[] = LANE_PALETTE,
): string {
  if (branchName && overrides[branchName]) return overrides[branchName];
  const i = ((colorIndex % palette.length) + palette.length) % palette.length;
  return palette[i];
}
```

- [ ] **Step 4: Run — verify pass.**

Run: `npx vitest run src/lib/graph/colors.test.ts`
Expected: PASS (all old + new cases).

- [ ] **Step 5: Re-export `NERV_LANE_PALETTE`.** Check `src/lib/graph/index.ts`; if it re-exports from `./colors` (e.g. `export * from "./colors"` or a named list), ensure `NERV_LANE_PALETTE` is included so `store.svelte.ts` can import it the same way it imports `laneColor`.

- [ ] **Step 6: Select the palette in the store.** In `src/lib/store.svelte.ts`, import `NERV_LANE_PALETTE` alongside `laneColor`, and update the two methods (~1489-1501):

```ts
    colorForRef(name: string, sha: string): string {
      const ov = branchColors[repo]?.[name];
      if (ov) return ov;
      const palette = theme === "nerv" ? NERV_LANE_PALETTE : LANE_PALETTE;
      const idx = colorBySha.get(sha);
      if (idx === undefined) return laneColor(0, null, {}, palette);
      return overrideByIndex.get(idx) ?? laneColor(idx, null, {}, palette);
    },
    colorForIndex(idx: number): string {
      const palette = theme === "nerv" ? NERV_LANE_PALETTE : LANE_PALETTE;
      return overrideByIndex.get(idx) ?? laneColor(idx, null, {}, palette);
    },
```
(If `LANE_PALETTE` isn't already imported in the store, add it to the `./graph` import.)

- [ ] **Step 7: Verify live recolor + gate.**

Run: `npm run tauri dev` → toggle NERV → the commit graph lanes/dots/ref chips recolor to the NERV palette instantly; HEAD ring is orange. Then `npm run check` + `npm test` → green.

- [ ] **Step 8: Commit.**

```bash
git add src/lib/graph/colors.ts src/lib/graph/colors.test.ts src/lib/graph/index.ts src/lib/store.svelte.ts
git commit -m "feat(theme): theme-aware commit-graph lane palette (NERV) via pure laneColor"
```

### Task A8: NERV Shiki diff theme

**Files:**
- Create: `src/lib/diff/nervShikiTheme.ts`
- Modify: `src/lib/diff/highlight.ts` (register the theme)
- Modify: `src/lib/components/DiffView.svelte` (select it when `appState.theme === "nerv"`)

**Interfaces:**
- Consumes: `appState.theme`; the six `--diff-*` tokens (Task A6) already re-skin the row backgrounds/sigils with no DiffView CSS change.
- Produces: NERV-tuned syntax highlighting on the void ground.

- [ ] **Step 1: Author the theme object** in `src/lib/diff/nervShikiTheme.ts`:

```ts
import type { ThemeRegistrationRaw } from "shiki";

// Minimal NERV-tuned dark theme: bone text on the void, orange keywords, phosphor
// strings, restrained accents. A raw object → no extra bundled-theme chunk, offline.
export const nervShikiTheme: ThemeRegistrationRaw = {
  name: "nerv",
  type: "dark",
  colors: { "editor.background": "#0A0C0F", "editor.foreground": "#EAE6DA" },
  settings: [
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "#7C8794", fontStyle: "italic" } },
    { scope: ["string", "string.quoted", "constant.other.symbol"], settings: { foreground: "#46E88B" } },
    { scope: ["keyword", "storage", "storage.type", "keyword.control"], settings: { foreground: "#F2542D" } },
    { scope: ["entity.name.function", "support.function", "meta.function-call"], settings: { foreground: "#E8A33D" } },
    { scope: ["entity.name.type", "support.type", "support.class", "entity.name.class"], settings: { foreground: "#5AA9E6" } },
    { scope: ["constant.numeric", "constant.language"], settings: { foreground: "#9B7FE0" } },
    { scope: ["variable", "meta.definition.variable"], settings: { foreground: "#EAE6DA" } },
    { scope: ["entity.name.tag"], settings: { foreground: "#F2542D" } },
    { scope: ["entity.other.attribute-name"], settings: { foreground: "#E0608A" } },
  ],
};
```

- [ ] **Step 2: Register it** in `src/lib/diff/highlight.ts` — import and add to the `createHighlighter` themes:

```ts
import { nervShikiTheme } from "./nervShikiTheme";
// …
    hp = createHighlighter({
      themes: ["github-light", "github-dark", nervShikiTheme],
      langs: [...LANGS],
    })
```

- [ ] **Step 3: Select it in `DiffView.svelte`.** At the `activeTheme` derivation (line 42), prefer the app theme (ensure `appState` is imported — it is used elsewhere; if not, add the import):

```ts
  const activeTheme = $derived(
    appState.theme === "nerv" ? "nerv" : darkMode ? "github-dark" : "github-light",
  );
```

- [ ] **Step 4: Verify diffs on a dark-mode Mac specifically.** Run `npm run tauri dev` with macOS in **Dark** mode, open a diff, toggle NERV: syntax highlighting uses the NERV theme; added/deleted row backgrounds are phosphor/alert (NOT github-green — this is the OS-dark-leak check). Then `npm run check` → 0 errors.

- [ ] **Step 5: Commit.**

```bash
git add src/lib/diff/nervShikiTheme.ts src/lib/diff/highlight.ts src/lib/components/DiffView.svelte
git commit -m "feat(theme): NERV Shiki diff theme + app-theme-aware selection"
```

### Task A9: Mono font-family audit (route to `var(--font-mono)`)

**Files (26, mechanical — replace each `font-family: ui-monospace, …` with `var(--font-mono)`):**
`BranchColorDialog.svelte:269`, `ReflogPanel.svelte:148`, `BackupsPanel.svelte:296`, `AmendDialog.svelte:116`, `RemotePanel.svelte:145`, `GraphHistory.svelte:791`, `PrereqBanner.svelte:99`, `CommitFilesDiff.svelte:178`, `LogPanel.svelte:64`, `EditTabs.svelte:375,387`, `RemoteProgress.svelte:70`, `RebaseTodo.svelte:231,298`, `StashPanel.svelte:149`, `WorkingCopyView.svelte:776,824`, `DiffView.svelte:603,660,673,856,938`, `CommitDetail.svelte:327`, `ConflictView.svelte:164`, `github/PrCommitsTab.svelte:84`, `github/GithubActions.svelte:105`, `github/GithubDetail.svelte:471`, `github/GithubOverview.svelte:156`, `github/PrFilesTab.svelte:315,362`, `github/Markdown.svelte:35`, `github/PrTimeline.svelte:609`, `github/GithubPulls.svelte:211`, `github/GithubReleases.svelte:338`.

**Interfaces:** Consumes `--font-mono` (Task A6). No unit test (CSS) — verified by grep + Classic-identical reasoning (token's Classic value = the exact literal; the two `"Cascadia Code"` variants at `DiffView.svelte:603,938` drop a font that never resolves on macOS anyway).

- [ ] **Step 1: Replace each site.** For every line above, change `font-family: ui-monospace, SFMono-Regular, Menlo[, "Cascadia Code"], monospace;` → `font-family: var(--font-mono);`.

- [ ] **Step 2: Verify none remain.**

Run: `grep -rn "ui-monospace" src/`
Expected: **no matches** (all now go through the token).

- [ ] **Step 3: Verify Classic identical + NERV mono renders.** `npm run check` → 0 errors. `npm run tauri dev`: Classic mono unchanged; NERV → IBM Plex Mono in diffs/SHAs/readouts.

- [ ] **Step 4: Commit.**

```bash
git add src/lib/components
git commit -m "refactor(theme): route hardcoded mono font-family through --font-mono"
```

### Task A10: Radius audit — square the rectangular surfaces

**Files:** the `border-radius: 6px` (×57), `8px` (×17), and `10px` (×17) sites — the friendly-macOS rounding on buttons/panels/dialogs/inputs/cards/menus/segments. **Leave `999px` (functional pills: toggles, avatars, status dots), `50%` (circles), and the rare `5px/7px/4px/3px` one-offs** — they are either intentionally round or negligible.

**Interfaces:** Consumes `--radius-sm/md/lg` (Task A6, Classic 4/6/10 = current; NERV 0/2/2).

- [ ] **Step 1: Enumerate the target sites.**

```bash
grep -rn "border-radius:[[:space:]]*6px" src/ ; grep -rn "border-radius:[[:space:]]*8px" src/ ; grep -rn "border-radius:[[:space:]]*10px" src/
```

- [ ] **Step 2: Replace by mapping** — `6px` → `var(--radius-md)`, `8px` → `var(--radius-md)` (dialogs collapse to 2px in NERV; 8px→6px in Classic is a 2px visual change on a few dialogs — **acceptable**, or use a dedicated value; if strict Classic-identical is required for those, instead map `8px` → `var(--radius-lg8, 8px)` and add `--radius-lg8: 8px`/NERV `2px`). Default: `8px`→`var(--radius-md)` and accept the tiny Classic dialog change, OR keep it exact with the extra token. `10px` → `var(--radius-lg)`.

  Recommended exact-Classic mapping to avoid ANY Classic change: add one more token pair in Task A6's Classic + NERV blocks — `--radius-dialog: 8px` (Classic) / `2px` (NERV) — and map the `8px` sites to it. Keeps Classic pixel-identical.

- [ ] **Step 3: Verify.** `grep -rn "border-radius:[[:space:]]*6px\|border-radius:[[:space:]]*10px" src/` → no matches (8px handled per chosen mapping). `npm run check` → 0 errors. `npm run tauri dev`: Classic corners unchanged; NERV buttons/panels/dialogs are near-square. Note any still-visibly-rounded element for a follow-up.

- [ ] **Step 4: Commit.**

```bash
git add src/lib src/routes
git commit -m "refactor(theme): tokenize rectangular border-radius (angular in NERV)"
```

### Task A11: On-accent text + stuck-in-Classic color audit

**Files:** the ~38 `color:#fff`/`white` sites (grep list) **classified per background**, plus specific stuck-in-Classic components.

**Interfaces:** Consumes `--on-accent` (Task A6), existing status tokens.

- [ ] **Step 1: Classify each `color:#fff`/`white` site by its element's background.** Rule:
  - **On `var(--accent)` fill** (primary buttons, `.seg button.active`) → `color: var(--on-accent);`. Includes e.g. `DiffView.svelte:641`, `RebaseTodo.svelte:359`, `WorkingCopyView.svelte:661`, `PrereqBanner.svelte:83`, `SettingsPanel.svelte:425`, `CommitComposer.svelte:299`, `AmendDialog.svelte:183`, `ManageRepoModal.svelte:218`, `CommitDetail.svelte:349`, `CommitFilesDiff.svelte:129`, `CommitMessageEdit.svelte:162`, `BranchColorDialog.svelte:243`, `Modal.svelte:465`, `ConflictView.svelte:195`, `github/GithubDetail.svelte:451`, and the `github/*` accent buttons.
  - **On `var(--danger)` fill** (destructive buttons) → also `color: var(--on-accent);` (NERV danger red needs dark text too; Classic `--on-accent` = `#fff`, unchanged). Verify each such button's bg first.
  - **On a genuinely dark/opaque non-accent surface** (white text meant to stay white) → **leave as `#fff`**. Inspect before changing.
  - `+page.svelte:1026` and `UndoBar.svelte:46` → inspect individually.

- [ ] **Step 2: Route the stuck-in-Classic status colors** so they don't stay Classic under NERV:
  - `Sidebar` detached-HEAD `#d97706` → `var(--err)` (orange in NERV, amber-ish in Classic — verify Classic value; if it must stay exact, add `--warn: #d97706`/NERV `#F2542D`).
  - `WorkingCopyView` status-row bg tints (`rgba(46,160,67,.1)`/amber/red) → route through `--status-add/-mod/-del` (as `color-mix` or rgba of the token).
  - `PrereqBanner` amber (`#fde68a`/`#b45309`) → keep legible on the void; theme-scope if needed.

- [ ] **Step 3: GitHub screen state badges (DECISION — user-approved default).** `lib/github/itemState.ts` OPEN=green / MERGED=purple / CLOSED=red / DRAFT=gray mirror GitHub's own external convention — **keep them semantically** under NERV; only bump lightness if any fails AA on `#0A0C0F`. Do **not** remap to the NERV palette. (Flagged for veto in the spec.)

- [ ] **Step 4: Verify.** `npm run tauri dev`: in NERV, primary/destructive buttons show **dark** text on orange/red (legible); no white-on-orange remains; status tints read on the void; GitHub badges still green/purple/red. Classic unchanged. `npm run check` → 0 errors.

- [ ] **Step 5: Commit.**

```bash
git add src/lib
git commit -m "feat(theme): --on-accent text + route stuck-in-Classic status colors for NERV"
```

### Task A12: Anton display type on headers/labels

**Files:** the section/panel header + `group-label`/`seg-label`/title selectors across SettingsPanel, dialogs, and panel headers.

**Interfaces:** Consumes `--font-display` (Classic = SF stack → no Classic change; NERV = Anton).

- [ ] **Step 1: Enumerate heading/label selectors** to receive the display face — e.g. `SettingsPanel .group-label`/`h3`, dialog titles (`Modal`, `AmendDialog`, `ManageRepoModal` `h2/h3`), panel section headers, and any `OP //`-style labels. Grep for the header classes:

```bash
grep -rn "group-label\|dialog-title\|panel-header\|\.title\b\|<h2\|<h3" src/lib/components | head -60
```

- [ ] **Step 2: Apply `font-family: var(--font-display)`** to those selectors. For selectors where editing the component is undesirable, add theme-scoped rules to `nerv.css` instead (`:global(:root[data-theme="nerv"]) .group-label { font-family: var(--font-display); letter-spacing: .02em; }`). Prefer component-level `var(--font-display)` where a header class already exists (Classic value = SF → identical).

- [ ] **Step 3: Verify.** `npm run tauri dev`: NERV headers/section labels render in Anton (condensed, heavy); body stays IBM Plex Sans; Classic headers unchanged. Check tight labels don't overflow (Anton is condensed → usually narrower, safe). `npm run check` → 0 errors.

- [ ] **Step 4: Commit.**

```bash
git add src/lib src/routes
git commit -m "feat(theme): apply Anton display face to NERV headers/labels"
```

### Task A13: Phase A green gate + visual QA

**Files:** none (verification only).

- [ ] **Step 1: Full gate.**

Run: `npm run check` (0 errors) + `npm test` + `cargo test`
Expected: all green.

- [ ] **Step 2: "Classic renders identically" audit.**
  - `grep -rn "data-theme" src/` — confirm every NERV rule is `:global(:root[data-theme="nerv"])`-scoped.
  - Spot-check the new tokens' Classic values equal the literals they replaced (`--on-accent:#fff`, `--font-mono` = `ui-monospace, SFMono-Regular, Menlo, monospace`, `--radius-md:6px`, diff fallbacks).

- [ ] **Step 3: NERV visual QA under `npm run tauri dev`** (both OS light AND dark mode). Verify: commit graph (NERV lanes, orange HEAD ring, lane dots), a diff on a **dark-mode Mac** (phosphor/alert bg + NERV Shiki), Settings, dialogs/menus, status bar, ref chips (near-square), sidebar selection (orange edge), the GitHub screen (semantic badges intact). Toggle Classic↔NERV repeatedly: **no FOUC on relaunch, no box-model shift**. QA tight fixed-width chrome (24px status bar, sidebar labels) for text overflow (type re-metrics — that's expected; truncation is not).

- [ ] **Step 4: Commit** any QA fixes, then tag the phase.

```bash
git commit -am "fix(theme): Phase A QA fixes" --allow-empty
```

---

## Phase B — HUD decoration (spike-informed)

> Use `docs/superpowers/plans/2026-07-07-nerv-spike-findings.md` (Task 0) for the enumerated bracket-host list and the chosen delivery mechanism. All rules are `:global(:root[data-theme="nerv"]) …` in `nerv.css`, `pointer-events:none`/additive, degrading under `prefers-reduced-motion`.

### Task B1: Scanline + grid overlay

- [ ] **Step 1:** Add the fixed root `::after` overlay to `nerv.css` (final, tuned version of the spike's Step-1 CSS; static if the spike found any per-frame cost).
- [ ] **Step 2:** Verify no framerate regression while scrolling the graph (`npm run tauri dev`); reduced-motion → static.
- [ ] **Step 3:** Commit — `feat(theme): NERV scanline/grid overlay`.

### Task B2: Mono readout styling + `● LIVE` status dots

- [ ] **Step 1:** Theme-scoped uppercase/tracked restyle of status-bar/section-label classes; a `● LIVE`/blink pseudo-element on the existing live/status indicators (blink gated on reduced-motion).
- [ ] **Step 2:** Verify status bar reads as a mono console; labels don't overflow.
- [ ] **Step 3:** Commit — `feat(theme): NERV mono readout + live status dots`.

### Task B3: Corner brackets

- [ ] **Step 1:** Implement the shared hook the spike chose (e.g. a single `data-nerv-frame` wrapper class), or per-host `::before/::after` for hosts that aren't clipped. Apply to the enumerated hosts (dialogs: Modal, AmendDialog, RebaseTodo, ManageRepoModal, BranchColorDialog, SettingsPanel; panels: StashPanel, ReflogPanel, ApplyPanel, ConflictView; + the commit-list/graph frame). Add the minimal Classic-hidden wrapper markup only where the spike proved it necessary.
- [ ] **Step 2:** Verify brackets frame each host, are not clipped, and cause no layout shift (Classic hidden). 
- [ ] **Step 3:** Commit — `feat(theme): NERV corner-bracket framing on panels/dialogs`.

### Task B4: Hazard-stripe dividers + mono header bars

- [ ] **Step 1:** Hazard `repeating-linear-gradient` dividers on section headers; the `◇ LABEL … ● LIVE` mono header bar on the hosts the spike identified (additive, Classic-hidden).
- [ ] **Step 2:** Verify placement/contrast; Classic hidden.
- [ ] **Step 3:** Commit — `feat(theme): NERV hazard dividers + console header bars`.

### Task B5: Phase B green gate + full visual QA

- [ ] **Step 1:** `npm run check` + `npm test` + `cargo test` → green.
- [ ] **Step 2:** Full NERV visual QA (all surfaces + the GitHub screen), Classic-unchanged check, reduced-motion check, relaunch no-FOUC check.
- [ ] **Step 3:** Commit any fixes — `fix(theme): Phase B QA fixes`.

---

## Self-review notes

- **Spec coverage:** token block (A6), fonts applied (A1/A2/A9/A12), radius (A10), on-accent (A11), diff six-token + Shiki (A6/A8), persistence + no-FOUC (A3/A4), settings control (A5), theme-aware lanes with pure `laneColor` (A7), audit (A9-A11), GitHub decision (A11), HUD (B1-B4), phased gates (A13/B5), spike (Task 0). Acceptance #1-7 all map.
- **Correction vs spec:** the HEAD ring is already `var(--accent)` (`GraphGutter:93`) and all graph colors route through `appState.colorForIndex`/`colorForRef`, so **no `GraphHistory`/`GraphGutter` edits** are needed for lanes/HEAD — only the two store methods + palette (A7). The spec's "add GraphHistory to the edit list / force HEAD dot to accent" is superseded: HEAD dot stays lane-colored per DESIGN.md.
- **Type consistency:** `theme: "classic" | "nerv"`, `setTheme`, `NERV_LANE_PALETTE`, `laneColor(…, palette?)`, `nervShikiTheme`, tokens `--on-accent`/`--font-sans|mono|display`/`--radius-sm|md|lg`/`--diff-*` are used consistently across tasks.
- **Not TDD-shaped:** CSS/theme tasks are verified by grep + `npm run check` + `tauri dev` visual QA rather than unit tests; only `colors.ts` (A7) is genuinely unit-testable and is done TDD-first.
