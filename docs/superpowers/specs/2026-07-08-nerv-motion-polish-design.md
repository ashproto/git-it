# NERV Motion Polish (round 2) — Design

**Goal:** Address the user's motion feedback on the NERV theme (PR #9): make the angular graph line visibly stepped; make the timeline reveal a single continuous bottom→top growth; make hover/live motion actually visible; theme + animate the toolbar buttons per the website; and add collapse, boot, and transient-UI (context-menu / settings) animations. All NERV-scoped, `data-motion`-gated, reduced-motion-safe, `transform`/`opacity`/registered-property only.

Folds into PR #9 on `nerv-theme`.

## Background (verified)

- Motion lives in `src/lib/theme/nerv-motion.css` (plain CSS, bare `:root[data-theme="nerv"]…`, **never `:global()`**), double-gated on `:root[data-theme="nerv"][data-motion="on"]` AND `@media (prefers-reduced-motion: no-preference)`. Static NERV decoration is in `nerv.css`; the token block is in `+page.svelte`.
- **Corner brackets** (`nerv.css` B3): a `.panel::after` / `.dialog::after` paints 4 L-marks from 8 gradient layers sized `var(--b) var(--w)` — `--b: 14px` (arm length), `--w: 1.5px`, `--c: var(--accent)`. **`--b` is a plain custom property → it cannot transition** (the current hover only does `filter: brightness(1.4)`, which is imperceptible).
- **Angular line** (`paths.ts:76`, fork/`else` case): stub `s = Math.min(6, rowHeight/3)` = 6px — too small to read against the 16px lane / 18px diagonal.
- **Timeline reveal** (Task 9): store `revealing`/`armReveal`/`revealSeq` state machine (armed once on repo-identity change; 5 no-replay tests in `store.reveal.test.ts`) toggles `.nerv-row-in` on each rendered row (`GraphHistory`) + `.nerv-edge-draw` on each gutter `<path>` (`GraphGutter`, with `pathLength="1"` + inline `animation-delay`). This per-entry/per-edge staggering is what the user does NOT want.
- **Website button reference** (`website/assets/css/styles.css:80-93`): `.btn` mono/uppercase/tracked; `.btn-ghost` bone text + void bg + line border with corner-bracket `::before`/`::after` (8px, orange, `opacity 0→1` on hover, `transition: opacity .18s, inset .18s`); `.btn-primary` accent bg + `:hover { transform: translateY(-1px) }`.
- **Toolbar buttons:** the Fetch/Pull/Push/manage/gear `<button>`s in `+page.svelte`'s `.app-header` are **not** NERV-themed today.
- **Live pulse:** only the op-time `.spinner` in `StatusBar.svelte:74` (rendered when `appState.busyOp`) — invisible in normal use.
- **Collapse:** `CollapsiblePanel.svelte` `toggle()` flips `collapsed`; `.panel.collapsed` becomes header-only.
- **Transient UI:** context menu is `.menu` (`ContextMenu.svelte`, a `position:fixed` popover, NOT `.panel`/`.dialog` — no NERV brackets today); Settings is a `.dialog` (`{#if settingsPanel.open}`), themed but with no entrance animation.

## The keystone — register `@property --b`

Register the bracket arm-length as a typed custom property so it can **interpolate** (transitions + animations). One declaration (in `nerv.css` or `nerv-motion.css`, global):
```css
@property --b { syntax: "<length-percentage>"; inherits: false; initial-value: 14px; }
```
This single lever powers the hover-grow (C), the collapse full-outline (F), and the boot draw-on (G). (WebKit supports `@property` from Safari 16.4 / macOS Ventura — same modern-macOS floor the app's existing `color-mix` already assumes; document it.)

## Design (by item)

### A. Angular line — make the stub obvious
In `angularEdgePath` (`paths.ts`), increase the fork/`else` stub so a clearly-straight vertical section leaves the node before the diagonal:
```
const s = Math.min(g.rowHeight * 0.42, g.rowHeight / 2 - 1); // ~12-13px at rowHeight 30
```
(merge-in `kind==="branch"` and same-lane unchanged). Update `paths.test.ts` expectations to the new `s`. Final value tuned in the live preview until the step reads clearly (both themes; NERV defaults to angular).

### B. Timeline reveal — ONE continuous bottom→top growth
**Replace** the per-row/per-edge staggering with a single continuous wipe:
- **Remove** `.nerv-row-in` application (+ its inline `animation-delay`) from `GraphHistory` and `.nerv-edge-draw` (+ `pathLength="1"` / dasharray / inline delay) from `GraphGutter`, and their keyframes.
- **Add** a single reveal-wipe on the **graph scroll viewport** (the fixed-height element whose box == the visible graph area — the implementer identifies it in `GraphHistory`). While `appState.revealing`, add a class (e.g. `.nerv-graph-reveal`) whose gated animation sweeps a **`clip-path: inset(T 0 0 0)`** (or a soft `mask`) from "only the bottom sliver visible" to "fully revealed" — `inset(~92% 0 0 0)` → `inset(0 0 0 0)` — over ~700ms, so the lanes + rows are uncovered together, bottom → top, as one edge rising toward HEAD. Edge softness (hard clip vs. gradient mask) tuned in-app.
- **State machine unchanged:** `revealing`/`armReveal`/`revealSeq` and the 5 no-replay tests stay exactly as they are (only the *visual* consumer changes). `REVEAL_MS` set to the wipe duration.
- **Perf:** clip/mask animation paints, but this is a **one-shot ~700ms transition** (not steady-state) on the graph area — acceptable, unlike a continuous full-viewport animation. Applied to the viewport (not the full-height SVG) so the sweep is perceptible.
- Gated + Classic/motion-off/reduced-motion ⇒ instant graph, no wipe.

### C. Hover — brackets react
On panel/dialog hover, the brackets **grow + brighten** (via registered `--b`):
```css
/* gated */ .panel::after, .dialog::after { transition: --b 180ms ease, filter 180ms ease; }
.panel:hover::after, .dialog:hover::after { --b: 22px; filter: brightness(1.5); }
```
Also applies the hover to the toolbar buttons' brackets (D). Value/curve tuned in-app.

### D. Toolbar buttons — themed + animated per the website
NERV-scope (in `nerv.css` + gated motion in `nerv-motion.css`) the header buttons (`+page.svelte` `.app-header` Fetch/Pull/Push/manage/gear — implementer confirms the selector). Match the website's `.btn-ghost`/`.btn-primary`:
- Ghost style: bone text, `--btn-bg` void, hairline `--border`, mono, slightly tracked; **corner-bracket `::before`/`::after`** (small, accent) that fade+seat in on hover; `:hover { transform: translateY(-1px) }` lift; the primary action (Fetch) on `var(--accent)` + `var(--on-accent)` text. Keep the toolbar's existing compact sizing (adapt the marketing treatment to toolbar scale — don't enlarge).
- The lift/bracket transitions are motion-gated; the static themed look (colors/border/mono) is always-on in NERV. Classic toolbar untouched.

### E. Live pulse — status-bar console dot
In `StatusBar.svelte`, add a persistent NERV-only `●` + state text at the start of the status strip (or repurpose the existing status region). NERV styling in `nerv.css`; the pulse animation in `nerv-motion.css` (gated):
- **Idle:** a small **phosphor** (`--status-add`) dot, slow gentle pulse (`opacity`/`scale`, ~2s).
- **During an op** (`appState.busyOp` truthy): the dot goes **accent** + faster pulse (drives off the existing busy state; the op `.spinner` can be folded into or coexist with this).
- Motion-off/reduced-motion ⇒ a static dot (no pulse). Classic ⇒ the current status bar, unchanged.

### F. Collapse → full outline
When a panel collapses, the brackets **extend into a complete frame**; expand retracts to corners — via registered `--b`:
```css
/* NERV, always-on state */ .panel.collapsed::after { --b: 50%; } /* arms meet → full outline */
/* gated transition */ .panel::after { transition: --b 260ms ease; }
```
`--b: 50%` makes each corner's two arms span half the box so opposite arms meet, forming a continuous border around the collapsed header. Motion-off ⇒ the collapsed frame still shows (state style) but snaps (no transition). Tune the exact collapsed `--b` (may need slightly >50% to fully close the seam) in-app.

### G. Boot reveal — panels cascade + brackets draw on
Rework the boot (`data-boot` window):
- **Per-panel cascade:** during the boot pulse, JS assigns each `.panel` an ascending `--boot-i` in DOM order (`querySelectorAll(".panel").forEach((el,i)=>el.style.setProperty("--boot-i", String(i)))`, cleared when `data-boot` is removed — added to the existing boot-pulse code in `themeMode.ts` + `store.svelte.ts setTheme`). CSS: each panel fades/rises in with `animation-delay: calc(var(--boot-i,0) * 70ms)`. Replaces the single `body` shell-fade with a real consecutive entrance.
- **Bracket draw-on:** the corner brackets **extend from zero** — `--b` animates `0 → 14px` (arms grow out of each corner) instead of the current scale-pop — synced a touch after each panel's entrance. Uses registered `--b`.
- Keep the scanline sweep. All gated; motion-off/reduced-motion ⇒ instant (no boot).

### H. Context menu + Settings entrance
- **Context menu (`.menu`):** give it NERV token styling + corner brackets (like `.panel`/`.dialog`, scoped to `.menu` in `nerv.css`) and a gated **entrance** animation (short fade + scale/translate from the pointer, ~140ms) in `nerv-motion.css`. Right-click should now pop a themed, animated console menu.
- **Settings dialog:** a gated **open** animation on the `.dialog`/`.overlay` (fade + slight scale-up, ~180ms) when `settingsPanel.open` mounts it. (A themed close is nice-to-have; entrance is the priority.)
- Motion-off/reduced-motion ⇒ instant appear; Classic ⇒ unchanged.

## Components / files

- **`src/lib/graph/paths.ts`** + `paths.test.ts` — angular stub (A).
- **`src/lib/theme/nerv.css`** — `@property --b`; toolbar-button theming (D static); `.menu` NERV styling + brackets (H); `.panel.collapsed::after { --b }` state (F); status-dot static styling (E); collapsed full-outline.
- **`src/lib/theme/nerv-motion.css`** — hover `--b` grow (C); toolbar button lift/bracket transitions (D); status-dot pulse (E); collapse `--b` transition (F); reworked boot (per-panel cascade + bracket draw-on) (G); menu/settings entrance (H); the graph reveal wipe (B). **Remove** the old `.nerv-row-in` / `.nerv-edge-draw` keyframes+rules.
- **`src/lib/components/GraphHistory.svelte`** — remove `.nerv-row-in` + inline delay; add the `.nerv-graph-reveal` class on the scroll viewport while `revealing` (B).
- **`src/lib/components/GraphGutter.svelte`** — remove `.nerv-edge-draw` / `pathLength` / dasharray / inline delay (B).
- **`src/lib/components/StatusBar.svelte`** — the persistent live dot + state text (E).
- **`src/lib/theme/themeMode.ts`** + **`src/lib/store.svelte.ts`** — boot-pulse also sets/clears `--boot-i` on panels (G); `REVEAL_MS` retune (B). State machine (`revealing`/`armReveal`) otherwise unchanged.
- **`src/routes/+page.svelte`** — only if a toolbar-button class/hook is needed (prefer styling existing selectors from `nerv.css`; add a minimal hook only if unavoidable).

## Testing

- **Green gate:** `npm run check` 0 errors + `npm test` (the 5 reveal-state tests + `paths.test.ts` updated stub) + `cargo test`.
- **Unit:** `paths.test.ts` updated for the larger stub. The reveal **state-machine** tests are unchanged and must still pass (the visual swap doesn't touch the store).
- **Browser QA (`npm run dev`, the real validator for all of this):** angular stub clearly stepped; the timeline reveal is ONE continuous bottom→top growth (not per-entry); panel-hover brackets grow+brighten; toolbar buttons themed with bracket-hover + lift; status-bar live dot pulses (idle phosphor, op accent); collapse extends brackets to a full outline and expand retracts; boot cascades panels + draws brackets on; right-click menu + settings open themed and animated. For each: verify **motion-off and reduced-motion fall back to static** and **Classic is unaffected**. `@property --b` interpolation confirmed working on the target WebKit.

## Out of scope
- Non-NERV (Classic) motion. Merging `next`→`main`. New features beyond the listed polish.

## Acceptance criteria
1. **Angular** fork edge shows a clearly-visible straight stub before angling (both themes).
2. **Timeline reveal** is a single continuous line growing bottom→top (lanes + entries together), one-shot, still no-replay (state machine + 5 tests intact); instant under motion-off/reduced-motion.
3. **Hover** on panels/dialogs visibly grows+brightens the corner brackets.
4. **Toolbar buttons** are NERV-themed with website-style bracket-hover + lift; Classic toolbar unchanged.
5. **Status-bar live dot** persistently pulses (idle phosphor, op accent), static when motion off.
6. **Collapse** animates brackets into a full outline; **expand** retracts to corners.
7. **Boot** cascades panels in consecutively and draws the corner brackets on.
8. **Context menu** and **Settings** are NERV-themed and animate in.
9. All of 2–8 are `data-motion`+reduced-motion gated and NERV-scoped; green gate passes; Classic unchanged.
