# NERV Motion Polish (round 2) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax. Many tasks are CSS/motion — the controller drives live browser QA and tunes exact values; subagents implement the structure/mechanism.

**Goal:** Ship the round-2 NERV motion feedback (spec `2026-07-08-nerv-motion-polish-design.md`): visible angular stub; one continuous bottom→top timeline reveal; hover/collapse/boot bracket motion; website-style toolbar buttons; status-bar live dot; themed+animated context menu & settings. Folds into PR #9 on `nerv-theme`.

**Architecture:** All NERV-scoped, double-gated `:root[data-theme="nerv"][data-motion="on"]` + `@media (prefers-reduced-motion: no-preference)`, in `nerv.css` (static) / `nerv-motion.css` (animation). Keystone: register `@property --b` (bracket arm length) so it interpolates — powers hover (C), collapse (F), boot (G).

**Tech Stack:** SvelteKit 5 (runes), plain CSS, Vitest. `@property` needs WebKit ≥ Safari 16.4 (macOS Ventura+) — consistent with the app's existing `color-mix` floor.

## Global Constraints

- **Green gate before "done":** `npm run check` 0 errors + `npm test` + `cargo test`.
- **Classic unchanged; NERV-scoped only.** Every rule keyed on `:root[data-theme="nerv"]…`.
- **Plain CSS in `nerv.css`/`nerv-motion.css` — bare selectors, NEVER `:global()`.**
- **All motion `data-motion`+reduced-motion gated**, `transform`/`opacity`/registered-`--b`/one-shot-clip only. Motion-off/reduced-motion ⇒ static.
- **The timeline reveal STATE MACHINE is untouched** (`revealing`/`armReveal`/`revealSeq` in `store.svelte.ts`; the 5 `store.reveal.test.ts` tests must stay green). Only the *visual* consumer changes (Task 2).
- **Commit subjects MUST start with a lowercase word** (commitlint `subject-case` — do not lead with "NERV").
- Definitive validation is **live browser QA** (`npm run dev` :1420); exact durations/sizes tuned there.

---

### Task 1: Angular stub — make it visibly stepped (A, TDD)

**Files:** `src/lib/graph/paths.ts`; `src/lib/graph/paths.test.ts`.

- [ ] **Step 1: Update the fork-case tests** in `paths.test.ts` for the larger stub. With `s = Math.min(rowHeight*0.42, rowHeight/2 - 1)` at `rowHeight 30` → `s = 12.6`; recompute the fork strings (e.g. fork `(0→1)`: `M12 0 L12 12.6 L28 17.4 L28 30`). Branch + same-lane cases unchanged. (Read the current tests to match `g`/edge shape; compute the exact numbers.)
- [ ] **Step 2: Run — verify fork fails.** `npx vitest run src/lib/graph/paths.test.ts` → FAIL.
- [ ] **Step 3: Implement** — change only the `else` return's stub in `angularEdgePath`:
```ts
  const s = Math.min(g.rowHeight * 0.42, g.rowHeight / 2 - 1); // clearly-visible straight stub
  return `M${x1} ${y1} L${x1} ${y1 + s} L${x2} ${y2 - s} L${x2} ${y2}`;
```
- [ ] **Step 4: Run — pass.** vitest paths → PASS; then `npm run check` + full `npm test`.
- [ ] **Step 5: Browser-QA note for the controller:** in NERV the fork lanes now show a clear straight section before angling; tune `0.42` if needed.
- [ ] **Step 6: Commit** — `git commit -m "feat(graph): enlarge angular stub so the step reads clearly"`

### Task 2: Timeline reveal → one continuous bottom→top wipe (B)

**Files:** `src/lib/components/GraphHistory.svelte`; `src/lib/components/GraphGutter.svelte`; `src/lib/theme/nerv-motion.css`. (`store.svelte.ts` only for `REVEAL_MS`.)

**Interfaces:** Consumes `appState.revealing` (unchanged). Produces a single wipe on the graph viewport; removes per-row/per-edge reveal.

- [ ] **Step 1: Remove per-entry reveal.**
  - `GraphHistory.svelte:532` — delete `class:nerv-row-in={revealing}` from the `.row`.
  - `GraphGutter.svelte` — delete `pathLength="1"` (:95), `class:nerv-edge-draw={revealing}` (:96), the `style animation-delay` (:97), the `revealing` prop (:25,:40) and any `revealDelay` helper. (Confirm `revealing` is no longer passed from GraphHistory.)
  - `nerv-motion.css` — delete the `.nerv-row-in` + `.nerv-edge-draw` rules and their `@keyframes nerv-row-in` / `nerv-edge-draw`.
- [ ] **Step 2: Add the continuous wipe.** In `GraphHistory.svelte`, add `class:nerv-graph-reveal={revealing}` to the scroll viewport **`.wrap`** (:407, `bind:this={wrapEl}`). In `nerv-motion.css` (gated):
```css
@media (prefers-reduced-motion: no-preference) {
  :root[data-theme="nerv"][data-motion="on"] .nerv-graph-reveal {
    animation: nerv-graph-grow 700ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
}
@keyframes nerv-graph-grow {          /* reveal edge sweeps UP: bottom-only → full */
  from { clip-path: inset(92% 0 0 0); }
  to   { clip-path: inset(0 0 0 0); }
}
```
`.wrap` is the fixed-height viewport (overflow:auto), so `inset` % is viewport-relative → a perceptible upward sweep. **Controller QA note:** if the sticky column `.head-row` (inside `.wrap`) revealing last reads oddly, refine to a soft `mask` or exclude the header (apply to `.history` with a viewport-relative edge) — tune in-app.
- [ ] **Step 3: Retune `REVEAL_MS`** in `store.svelte.ts` to ~750ms (wipe duration + small buffer) so `revealing` clears after the wipe. Leave the arm logic + tests untouched.
- [ ] **Step 4: Verify.** `npm run check` + `npm test` (**the 5 reveal-state tests must still pass** — they test the store, not the CSS). Browser QA (controller): switching to the timeline shows ONE continuous line growing bottom→top (lanes + rows together), once; no per-entry stagger; no replay on scroll/refresh; instant when motion off.
- [ ] **Step 5: Commit** — `git commit -m "feat(graph): timeline reveal grows as one continuous bottom-to-top wipe"`

### Task 3: `@property --b` + hover brackets grow+brighten (C)

**Files:** `src/lib/theme/nerv.css` (register); `src/lib/theme/nerv-motion.css` (hover).

**Interfaces:** Produces an interpolatable `--b` — prerequisite for Tasks 4 & 7.

- [ ] **Step 1: Register `@property --b`** at the top of `nerv.css` (above the bracket rules):
```css
@property --b { syntax: "<length-percentage>"; inherits: false; initial-value: 14px; }
```
- [ ] **Step 2: Hover grow+brighten** in `nerv-motion.css` (replace the current imperceptible `filter: brightness(1.4)` hover):
```css
@media (prefers-reduced-motion: no-preference) {
  :root[data-theme="nerv"][data-motion="on"] .panel::after,
  :root[data-theme="nerv"][data-motion="on"] .dialog::after {
    transition: --b 180ms ease, filter 180ms ease;
  }
  :root[data-theme="nerv"][data-motion="on"] .panel:hover::after,
  :root[data-theme="nerv"][data-motion="on"] .dialog:hover::after {
    --b: 22px; filter: brightness(1.5);
  }
}
```
- [ ] **Step 3: Verify + QA.** `npm run check`. Browser QA (controller): hovering a panel/dialog visibly grows + brightens its corner brackets; confirm `@property --b` interpolates on the target WebKit (if it snaps instead of animating, `@property` isn't registering — check syntax). Tune `22px`/curve.
- [ ] **Step 4: Commit** — `git commit -m "feat(theme): register @property --b + make panel-hover brackets grow and brighten"`

### Task 4: Collapse → full outline (F)

**Files:** `src/lib/theme/nerv.css` (collapsed state); `src/lib/theme/nerv-motion.css` (transition). Depends on Task 3's `@property --b`.

- [ ] **Step 1: Collapsed = full frame** in `nerv.css` (state style, always-on in NERV):
```css
:root[data-theme="nerv"] .panel.collapsed::after { --b: 51%; } /* arms meet → outline (51% closes the seam) */
```
- [ ] **Step 2: Gated transition** in `nerv-motion.css` so it animates on collapse/expand (the hover transition from Task 3 already covers `--b` on `.panel::after`; ensure the collapse also transitions — same `transition: --b …` applies since `.panel.collapsed::after` is still `.panel::after`). Confirm the transition duration reads well for collapse (~260ms); if hover (180ms) is too quick for the collapse sweep, scope a slightly longer transition for `.panel::after` generally or accept 180ms.
- [ ] **Step 3: Verify + QA.** `npm run check`. Browser QA: collapsing a sidebar panel animates the brackets extending into a complete outline around the header; expanding retracts to corners. Motion-off ⇒ collapsed frame still shows (snaps). Tune the `51%` until the seam fully closes.
- [ ] **Step 4: Commit** — `git commit -m "feat(theme): collapse extends NERV brackets into a full panel outline"`

### Task 5: Toolbar buttons — themed + motion per the website (D)

**Files:** `src/lib/theme/nerv.css` (static theme); `src/lib/theme/nerv-motion.css` (hover motion).

**Interfaces:** Themes `.fetch-btn` (+ `.push-main`/`.push-arrow`) and `.gear-btn` (Manage/Settings) — verified selectors in `+page.svelte`'s `.app-header`.

- [ ] **Step 1: Static NERV theme** in `nerv.css` — restyle `:root[data-theme="nerv"] .fetch-btn, :root[data-theme="nerv"] .gear-btn` as ghost console buttons (mirror website `.btn-ghost`, `styles.css:81-93`): mono font (`var(--font-mono)`), uppercase + `letter-spacing`, bone text, `var(--btn-bg)`/`var(--border)`, near-square; add corner-bracket `::before`/`::after` (small, `var(--accent)`, `opacity: 0`). Primary action (`.fetch-btn` that isn't Pull/Push? — or the whole group; controller decides which is "primary") may use `var(--accent)`/`var(--on-accent)`. Keep the toolbar's compact size (do NOT enlarge). Ensure the added `::before`/`::after` don't collide with existing pseudo-elements (check the current button CSS).
- [ ] **Step 2: Gated hover motion** in `nerv-motion.css`: the bracket pseudos `opacity: 0 → 1` on hover + `transform: translateY(-1px)` lift + `transition`. Gated.
- [ ] **Step 3: Verify + QA.** `npm run check`. Browser QA: the Fetch/Pull/Push/Manage/Settings buttons read as NERV console buttons; on hover the corner brackets light up + the button lifts; Classic toolbar unchanged. Tune sizing/tracking so the dense toolbar stays tidy.
- [ ] **Step 4: Commit** — `git commit -m "feat(theme): style NERV toolbar buttons as website-style console buttons"`

### Task 6: Status-bar live dot (E)

**Files:** `src/lib/components/StatusBar.svelte`; `src/lib/theme/nerv.css` (static dot); `src/lib/theme/nerv-motion.css` (pulse).

- [ ] **Step 1: Add a persistent dot.** In `StatusBar.svelte`'s `.section.right` (before `.status-text`), add a `<span class="live-dot" class:busy={!!appState.busyOp} aria-hidden="true"></span>`. (Keep the existing op `.spinner` or fold it in — controller decides; simplest: keep spinner for the op label, add the always-on dot as the "live" indicator.)
- [ ] **Step 2: Static NERV style** in `nerv.css`: `:root[data-theme="nerv"] .live-dot { … }` — a small circle, `background: var(--status-add)` (phosphor); `.live-dot.busy { background: var(--accent); }`. Classic: hide it or leave unstyled (NERV-only indicator — `.live-dot` renders in both but only NERV styles it; give it `display:none` by default and `display:inline-block` under NERV, so Classic is unaffected).
- [ ] **Step 3: Gated pulse** in `nerv-motion.css`: idle slow pulse (`opacity`/`scale`, ~2s); `.live-dot.busy` faster (~0.8s). Motion-off ⇒ static dot.
- [ ] **Step 4: Verify + QA.** `npm run check`. Browser QA: a phosphor dot pulses gently in the NERV status bar at idle; (simulate a busy op if possible) it goes accent + faster; static when motion off; absent/unstyled in Classic.
- [ ] **Step 5: Commit** — `git commit -m "feat(theme): add pulsing live dot to the NERV status bar"`

### Task 7: Boot rework — panel cascade + bracket draw-on (G)

**Files:** `src/lib/theme/themeMode.ts` + `src/lib/store.svelte.ts` (the two `data-boot` pulse sites — add `--boot-i`); `src/lib/theme/nerv-motion.css` (cascade + draw-on). Depends on Task 3's `@property --b`.

- [ ] **Step 1: Index panels during boot.** In BOTH boot-pulse sites (`themeMode.ts:71` launch; `store.svelte.ts:223` toggle), after setting `dataset.boot`, assign an ascending index to each panel and clear it when `data-boot` is removed:
```ts
const panels = document.querySelectorAll(".panel");
panels.forEach((el, i) => (el as HTMLElement).style.setProperty("--boot-i", String(i)));
// in the same setTimeout that deletes data-boot:
panels.forEach((el) => (el as HTMLElement).style.removeProperty("--boot-i"));
```
(Guard for SSR/`typeof document`, matching the existing code. Extract a small shared helper if the two sites duplicate meaningfully.)
- [ ] **Step 2: Per-panel cascade** in `nerv-motion.css` — replace the single `body` shell-fade with a per-panel entrance keyed on `--boot-i`:
```css
@media (prefers-reduced-motion: no-preference) {
  :root[data-theme="nerv"][data-motion="on"][data-boot] .panel {
    animation: nerv-boot-panel 380ms ease-out both;
    animation-delay: calc(var(--boot-i, 0) * 70ms);
  }
}
@keyframes nerv-boot-panel { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
```
- [ ] **Step 3: Bracket draw-on** in `nerv-motion.css` — replace the scale-pop with `--b` growing `0 → 14px` (arms extend out), staggered a touch after each panel:
```css
@media (prefers-reduced-motion: no-preference) {
  :root[data-theme="nerv"][data-motion="on"][data-boot] .panel::after,
  :root[data-theme="nerv"][data-motion="on"][data-boot] .dialog::after {
    animation: nerv-boot-draw 320ms ease-out both;
    animation-delay: calc(var(--boot-i, 0) * 70ms + 120ms);
  }
}
@keyframes nerv-boot-draw { from { --b: 0px; opacity: 0; } to { --b: 14px; opacity: 1; } }
```
(Keep the scanline sweep. Remove the old `nerv-boot-brackets` scale-pop + `nerv-boot-shell` body-fade keyframes/rules they replace.)
- [ ] **Step 4: Verify + QA.** `npm run check`. Browser QA: toggling Classic→NERV cascades the panels in one-by-one with the brackets drawing on; motion-off/reduced-motion ⇒ instant. Tune the 70ms stagger + durations.
- [ ] **Step 5: Commit** — `git commit -m "feat(theme): boot reveal cascades panels in and draws brackets on"`

### Task 8: Context menu + Settings entrance (H)

**Files:** `src/lib/theme/nerv.css` (menu theme + brackets); `src/lib/theme/nerv-motion.css` (entrance animations).

- [ ] **Step 1: Theme the context menu** in `nerv.css`: `:root[data-theme="nerv"] .menu` — ensure it reads as a NERV console popover (tokens already apply via `--popover-bg`/`--border`; add corner-bracket `::after` like `.panel`/`.dialog` — flush-inset, `var(--accent)`; verify `.menu` doesn't already use `::after`). (`.submenu` is also `.menu` — the rule covers it.)
- [ ] **Step 2: Menu entrance** in `nerv-motion.css` (gated): a short fade + scale/translate on `.menu` appearance (~140ms) — `@keyframes nerv-menu-in { from { opacity:0; transform: scale(0.97) translateY(-2px); } to { opacity:1; transform: none; } }` applied to `:root[data-theme="nerv"][data-motion="on"] .menu`.
- [ ] **Step 3: Settings dialog entrance** in `nerv-motion.css` (gated): a fade + slight scale-up on the `.dialog`/`.overlay` when it mounts (~180ms) — `@keyframes nerv-dialog-in { from { opacity:0; transform: scale(0.98); } to { opacity:1; transform: none; } }` on `:root[data-theme="nerv"][data-motion="on"] .dialog`.
- [ ] **Step 4: Verify + QA.** `npm run check`. Browser QA: right-click pops a themed, animated console menu (with brackets); opening Settings animates in; motion-off ⇒ instant; Classic unchanged.
- [ ] **Step 5: Commit** — `git commit -m "feat(theme): theme and animate the context menu and settings dialog"`

### Task 9: Final gate + full visual QA

**Files:** none (verification); commit any QA tuning.

- [ ] **Step 1: Full gate.** `npm run check` (0) + `npm test` (incl. reveal + paths) + `cargo test` → green.
- [ ] **Step 2: Full browser QA** (both OS light/dark; motion on, motion off, and — via CSS inspection if OS emulation is unavailable — reduced-motion): all 8 items (angular stub, continuous bottom→top reveal, hover brackets, toolbar buttons, live dot, collapse outline, boot cascade, menu/settings entrance). For each confirm gated fallback to static + **Classic unaffected**. Confirm the reveal still plays once (no replay on scroll/refresh).
- [ ] **Step 3:** Commit tuning; the work rides on PR #9.

## Self-review notes
- **Spec coverage:** A→T1, B→T2, C→T3, F→T4, D→T5, E→T6, G→T7, H→T8, gate→T9. Acceptance 1–9 map.
- **Ordering:** T3 (`@property --b`) precedes T4 (collapse) and T7 (boot) which depend on it. T1/T2 (graph) are independent and first.
- **State machine preserved:** T2 removes only the *visual* reveal consumers; `revealing`/`armReveal`/`revealSeq` + the 5 tests are untouched. This is the one place to be careful.
- **Cleanup:** T2 and T7 remove now-dead keyframes/rules (`nerv-row-in`, `nerv-edge-draw`, `nerv-boot-brackets`, `nerv-boot-shell`) — don't leave orphans.
- **Non-TDD:** only T1 (paths) is unit-tested; the rest are CSS/motion verified by the green gate + live browser QA (the reveal state machine keeps its existing tests).
