# NERV HUD spike — findings & Phase B blueprint

> Task 0 of the NERV app-theme project. **Static analysis only** — the app was not run and no
> `.svelte`/`.ts`/`.css`/config file was changed. This document replaces the deferred placeholders in
> Phase B (B1–B4) of `2026-07-07-nerv-app-theme.md` with an enumerated, measured plan.

## TL;DR

- **Overlay: GO, static.** A single root `::after` painting static repeating-gradients composites once
  and never re-samples the backdrop per frame, so it does **not** reintroduce the `backdrop-filter` wall
  (`src/routes/+page.svelte:777-785`). No animation on it. (Runtime FPS confirmation is deferred to the
  existing B1-Step-2 verify.)
- **Brackets: pure pseudo, ZERO markup edits.** The app already has two *de-facto shared DOM classes* —
  `.panel` and `.dialog` — and Svelte does not rename DOM classes, so global `nerv.css` rules match them.
  Brackets ship as one `::after` corner-mark overlay per shared class, drawn **flush-inside** the padding
  box (never outset) so the pervasive `overflow:hidden` never clips them. **No wrapper elements, no
  attributes, no component edits are required for any host.**
- **Header bars / hazard dividers / display type: also mostly zero-edit**, because `CollapsiblePanel`'s
  `.panel-header`+`<h2>` is the single header for every sidebar/graph/detail panel, and 3 of 6 dialogs
  already have a `.header` flex bar.
- **GitHub screen: tokens-only.** Its modal box is `.modal` (not `.dialog`) and its sub-panel `.panel`
  lives under `.gh`; a one-line `.gh` exclusion keeps HUD framing out of it. ~25 files, no per-file work.

The net Phase B component-markup edit count is **0 required** (all motifs are pure `nerv.css`), with a
small optional-additive allowance only if QA wants a right-aligned `● LIVE` token on the bare-`<h3>`
dialogs.

---

## The structural discovery that makes this cheap

The spike brief warned there is "no shared `.panel`/`.dialog` class today." That is true at the
*Svelte-scoped-CSS* level (each component hashes its own rules), but **not** at the DOM level, and the
DOM level is exactly what a `:global(:root[data-theme="nerv"]) .thatClass` rule keys off:

1. **`CollapsiblePanel.svelte` is the shared panel frame.** Its root is `<section class="panel">`
   (`CollapsiblePanel.svelte:172`) with `overflow:hidden` (`:204`), `border-radius:10px` (`:202`), a
   `<header class="panel-header">` (`:178`) containing `<h2>` (`:181`) + an optional `.actions` slot.
   **Ten call-sites render through it**, so they all share the identical `.panel` root, `.panel-header`,
   and `<h2>`: Sidebar Local/Remotes/Tags (`Sidebar.svelte:274,294,300`), `RemotePanel:65`,
   `ReflogPanel:68`, `BackupsPanel:114`, `StashPanel:71`, `LogPanel:21` (Output), `EditTabs:295`,
   `CommitDetail:100` (Commit), and **`GraphHistory:386` — the commit-list/graph frame itself.**

2. **The `.panel` DOM class is used verbatim by four more components:** `ApplyPanel.svelte:68`
   (`class:panel={!bare}`), `WorkingCopyView.svelte:334` (`class="wc-view panel"`),
   `ConflictView.svelte:46` (`class="conflict panel"`), and `github/ReviewBar.svelte:96`.

3. **The `.dialog` DOM class is used verbatim by all six modal dialogs:** `Modal.svelte` (×7 instances:
   `108,135,160,192,236,294,355`), `AmendDialog:43`, `RebaseTodo:115`, `ManageRepoModal:66`,
   `BranchColorDialog:63`, `SettingsPanel:46`. (The GitHub screen's modal is `.modal`, not `.dialog` —
   `github/GithubActionModal.svelte:38` — so it is naturally excluded.)

4. **Nesting is already solved by the `bare` prop.** When a panel is embedded in another panel it is
   rendered `bare`, which drops the `.panel` chrome: `CollapsiblePanel` renders `.cp-bare` instead of
   `.panel` (`:167-168`), and `ApplyPanel`'s `class:panel={!bare}` drops the class. `CommitDetail`
   embeds `<EditTabs bare />` and `<ApplyPanel bare />` (`CommitDetail.svelte:120-121`), so **no
   `.panel` is ever nested inside another `.panel`** → no double-bracketing.

Consequence: **two global rules (`.panel`, `.dialog`) frame every host with zero markup edits**, and
ancestor scoping (`.gh`, `.side-col`) — both classes live in files we're already free to reference from
`nerv.css` without editing components — lets us subtract the surfaces we don't want framed.

---

## Per-component catalog

`overflow`/`position` are for the **root element a global rule would target** (inner scroll regions are
noted separately). "Verdict" = can corner brackets be pure pseudo-element decoration.

| Component | Root DOM class (target) | overflow (root) | position (root) | Header element | `::before/::after` on root already? | Bracket verdict |
|---|---|---|---|---|---|---|
| **CollapsiblePanel** (frames Stash/Reflog/Remote/Backups/Log/Edit/CommitDetail/GraphHistory/Sidebar×3) | `.panel` (`:172`) | `hidden` (`:204`) | static | `.panel-header`>`<h2>` (`:178,181`) | no | **Pure pseudo — flush-inset** |
| Modal (×7 variants) | `.dialog` (`:108…`) | visible | static (in `.overlay` fixed) | bare `<h3>` (`:109…`) | no | Pure pseudo |
| AmendDialog | `.dialog` (`:43`) | visible | static | bare `<h3>` (`:44`) | no | Pure pseudo |
| RebaseTodo | `.dialog` (`:115`) | **`hidden`** (`:218`) | static | bare `<h3>` | no | Pure pseudo — flush-inset |
| ManageRepoModal | `.dialog` (`:66`) | **`hidden`** (`:131`) | static | `.header`>`<h3>` (`:67`) | no | Pure pseudo — flush-inset |
| BranchColorDialog | `.dialog` (`:63`) | visible | static | `.header`>`<h3>` (`:64`) | no | Pure pseudo |
| SettingsPanel | `.dialog` (`:46`) | **`auto`** (self-scrolls, `:318`) | static | `.header`>`<h3>` (`:47`) | no | Pure pseudo — flush-inset |
| ApplyPanel (standalone only) | `.apply-row.panel` (`:68`, only when `!bare`) | visible | static | none | no | Pure pseudo (via `.panel`) |
| ConflictView | `.conflict.panel` (`:46`) | visible (`.files` inner `auto` `:138`) | static | `header.ch`>`.title` (`:47,48`); footer `.cf` | no | Pure pseudo (via `.panel`) |
| WorkingCopyView | `.wc-view.panel` (`:334`) | `hidden` (`:581`) | static | `.section-header` (sticky, `:703`) | root: no (`.files-resize-handle::before` `:678`, not root) | Pure pseudo — flush-inset; **`position:relative` safe** (see note) |
| StatusBar | `.status-bar` (`:51`) | `hidden` (`:92`), 24px fixed | static | none (sections + `.spinner` `:74`) | no | **Not a bracket host** → mono-readout motif (B2) |
| ContextMenu | `.menu` (`:41`) | `auto` (`:149`) | **fixed** (`:89`) | none | no | Popover — **tokens/optional**, not in B3 set |
| github/ReviewBar | `.panel` (`:96`) | — | — | — | no | Inside `.gh` → **suppress** (tokens-only) |
| github/GithubActionModal | `.modal` (`:38`) | — | — | `.sub` | no | `.modal` ≠ `.dialog` → **naturally excluded** |

**WorkingCopyView `position:relative` safety:** its root `.wc-view` is `static`; the only absolutely
positioned descendant is `.files-resize-handle::before` (`:680`) whose containing block is
`.files-resize-handle` (`position:relative` at `:673`), and `.section-header` is `position:sticky`
(`:709`, unaffected by an ancestor `relative`). So making `.wc-view` relative under NERV shifts nothing.
The same holds for every other host: their roots have **no** absolutely-positioned descendant that
resolves to the root, so `[data-theme="nerv"] .panel/.dialog { position: relative }` is layout-neutral.

**Overflow reality (refines the brief's "27 of 37"):** measured across all 61 component files, 38 use
`overflow:hidden|auto`. The decisive fact for brackets is narrower and confirmed: **the shared `.panel`
root, plus 3 of 6 `.dialog` roots, clip** (`overflow:hidden`/`auto`). That single fact — not per-host
counting — is why brackets must be flush-inset rather than outset.

---

## Decisions (binding for Phase B)

### 1. Corner-bracket delivery mechanism → **per shared DOM class, flush-inset `::after`, ZERO markup edits**

**Chosen:** target the two existing shared DOM classes in `nerv.css`; do **not** add a `data-nerv-frame`
attribute or wrapper to any component.

```css
:global(:root[data-theme="nerv"]) .panel,
:global(:root[data-theme="nerv"]) .dialog { position: relative; }

:global(:root[data-theme="nerv"]) .panel::after,
:global(:root[data-theme="nerv"]) .dialog::after {
  content: ""; position: absolute; inset: 0; pointer-events: none; z-index: 1;
  /* four L-brackets painted as corner gradients — one pseudo, all four corners,
     flush INSIDE the padding box so overflow:hidden never clips them */
  --b: 14px; --w: 1.5px; --c: var(--accent);
  background:
    linear-gradient(var(--c),var(--c)) 0 0/var(--b) var(--w),
    linear-gradient(var(--c),var(--c)) 0 0/var(--w) var(--b),
    linear-gradient(var(--c),var(--c)) 100% 0/var(--b) var(--w),
    linear-gradient(var(--c),var(--c)) 100% 0/var(--w) var(--b),
    linear-gradient(var(--c),var(--c)) 0 100%/var(--b) var(--w),
    linear-gradient(var(--c),var(--c)) 0 100%/var(--w) var(--b),
    linear-gradient(var(--c),var(--c)) 100% 100%/var(--b) var(--w),
    linear-gradient(var(--c),var(--c)) 100% 100%/var(--w) var(--b);
  background-repeat: no-repeat;
}
```

**Why this over a shared `data-nerv-frame` hook (option a) or per-host pseudo (option b):**
- Svelte keeps the plain DOM class, so `.panel`/`.dialog` are *already* the shared hook the brief was
  looking for — adding an attribute would be redundant markup for the same reach.
- One pseudo (`::after` only) with 8 gradient stops draws all four corners, leaving `::before` free and
  needing only `position:relative` on the host (verified layout-neutral above).
- **Flush-inset (`inset:0`, brackets at the padding-box corners) is the universal solvent for the
  overflow problem:** an `inset:0` absolute pseudo of a scroll/clip container is pinned to the padding
  box and is *not* clipped and does *not* scroll with content — so `overflow:hidden` (`.panel`,
  RebaseTodo, ManageRepoModal, WorkingCopyView) and `overflow:auto` (SettingsPanel) hosts all work
  identically. Outset brackets (negative offsets sitting on the border) are the only thing that would
  clip, so **B3 must not outset.** NERV squares the corners (radius 0–2px, Task A10), so flush brackets
  read crisp.
- No wrapper is ever needed. The brief's "additive Classic-hidden wrapper" allowance is **unused.**

**Over-framing control (also zero component edits):** a blanket `.panel` rule frames every module,
including the three stacked Sidebar sub-panels and the GitHub `ReviewBar`. Subtract with ancestor
selectors that live in files `nerv.css` may freely reference:
```css
/* GitHub screen is tokens-only */
:global(:root[data-theme="nerv"]) .gh .panel::after { content: none; }
/* If sidebar sub-panels read noisy in QA, drop their brackets (keep the graph/detail/main ones): */
:global(:root[data-theme="nerv"]) .side-col .panel::after { content: none; }
```
`.gh` is `GithubView.svelte:56`; `.side-col` is `+page.svelte:506`. Recommend shipping the `.gh`
exclusion always, and applying the `.side-col` exclusion only if QA finds three bracketed sidebar
panels too busy (my recommendation: **keep** them — uniform module framing is on-brand for the console).

**Enumerated bracket hosts (all pure pseudo, all zero-edit):**
`.dialog` → Modal, AmendDialog, RebaseTodo, ManageRepoModal, BranchColorDialog, SettingsPanel.
`.panel` → GraphHistory (Commits/graph frame), CommitDetail, WorkingCopyView, ConflictView, standalone
ApplyPanel, RemotePanel, ReflogPanel, BackupsPanel, StashPanel, LogPanel, EditTabs(standalone),
Sidebar Local/Remotes/Tags. Excluded: `.gh .panel` (ReviewBar), any `.cp-bare`/`bare` nested panel.

### 2. Scanline / grid overlay → **single static root `::after`; GO**

```css
:global(:root[data-theme="nerv"])::after {
  content: ""; position: fixed; inset: 0; pointer-events: none; z-index: 9999;
  background:
    repeating-linear-gradient(0deg, rgba(255,255,255,.02) 0 1px, transparent 1px 3px),
    linear-gradient(rgba(242,84,45,.015), rgba(242,84,45,.015));
}
/* no animation is declared; reduced-motion is therefore a no-op but kept explicit for intent */
@media (prefers-reduced-motion: reduce) { :global(:root[data-theme="nerv"])::after { animation: none; } }
```

**Why it does not reintroduce the `backdrop-filter` wall (`+page.svelte:777-785`):** `backdrop-filter`
capped the framerate because it is *not* GPU-composited over a transparent WKWebView and forced a full
**re-blur of the backdrop every frame** — it re-samples pixels behind it continuously. A painted
gradient layer has no backdrop input: it rasterizes **once** into its own compositor layer and is then
just composited (a cheap blend), with **no per-frame repaint and no backdrop sampling**. Scrolling the
graph moves other layers underneath; this fixed layer is untouched. So it is safe by construction.

**Hard rules for B1:** (a) **static only** — no `background-position`/opacity animation (that would
repaint the full-viewport layer every frame). (b) If animated scanline motion is ever wanted, it must be
a `transform: translateY()` on a *separate, cheap, GPU-composited* layer (transforms don't repaint), and
gated behind `prefers-reduced-motion`; otherwise **drop it.** (c) `z-index`: keep it above content but
the overlay is `pointer-events:none`, and modal `.overlay`s are `z-index:3000–4000` — the `9999` scanline
sits above them, which is the intended "HUD glass over everything" look; if it ever muddies dialog text,
lower it to `z-index:1` and let modals paint above. (d) Keep the alphas tiny (≤.02) so it reads as
texture, not a scrim, on the `#0A0C0F` void.

### 3. Mono console header bars → **restyle existing headers; keep to high-impact surfaces**

Two existing header shapes already provide a two-slot flex bar (label left / actions right) with **zero
markup edits**, which is exactly what `◇ LABEL … ● LIVE` needs:
- **`CollapsiblePanel .panel-header` + `<h2>` (`:178,181`)** — one restyle reaches *every* panel
  (Commits graph, Commit detail, sidebar sections, Output, etc.). `◇` via `h2::before`; the optional
  `● LIVE`/actions already live in `.panel-header .actions`.
- **`.header` + `<h3>` on ManageRepoModal / BranchColorDialog / SettingsPanel (`:67/:64/:47`)** — the
  `.close-btn` already occupies the right slot; add `◇` via `h3::before`.

The other three dialogs (**Modal, AmendDialog, RebaseTodo**) use a **bare `<h3>`** with no right-hand
slot. Recommendation: restyle the `<h3>` (Anton/tracked + `◇` sigil) but **do not** force a full
`… ● LIVE` bar there — a right-aligned `● LIVE` is the *one* place a minimal Classic-hidden additive
span would be justified, and only if QA wants it. Default: **skip it**; keep header bars to the shared
`.panel-header` and the three `.header` dialogs (highest impact, zero edits).

### 4. Hazard-stripe dividers → **target existing header/section borders**

Existing divider selectors to restyle (all zero-edit, `background-image` hazard `repeating-linear-gradient`
replacing or overlaying the `border-bottom`):
- **`CollapsiblePanel .panel:not(.collapsed) .panel-header` border-bottom (`:226-228`)** — the primary,
  highest-reach divider (under every panel title).
- `WorkingCopyView .section-header` border-bottom (`:708`) and `.file-section` border-bottom (`:695`).
- `ConflictView .conflict` uses `border: var(--err)` (`:100`) — already hazard-colored; a
  `.cf` (footer, `:171`) top-border hazard stripe is the natural accent.
- Dialog `.header` bottom edges (ManageRepoModal/BranchColorDialog/SettingsPanel) via `.header::after`.

Keep hazard stripes to **section dividers and the conflict/danger surfaces** (accent discipline: red is
hazard-only) — do not stripe every border.

### 5. GitHub screen (`lib/github/`, ~25 files) → **tokens-only, no HUD framing**

Rationale: (a) it already carries a *sanctioned* semantic-color exception (PR/issue state badges,
Task A11) that the HUD language would fight; (b) its surfaces are `.gh`-scoped and its modal is `.modal`
(not `.dialog`), so it is **excluded from brackets/console-bars by construction** except the single
`ReviewBar .panel`, killed by the one-line `.gh .panel::after{content:none}` above; (c) framing ~25
dense sub-panels (tabs, timelines, file lists, stat tiles) would be visual overload for little gain.
It inherits the full **token** re-skin (color/type/radius/diff) from Phase A automatically, which is
sufficient. Net Phase B work in `lib/github/`: **one exclusion line**, no per-file edits.

---

## Phase B task adjustments (hand-off to implementers)

All rules land in `src/lib/theme/nerv.css`, all `:global(:root[data-theme="nerv"]) …`-scoped,
`pointer-events:none`/additive, Classic untouched. **Required component-markup edits: 0.**

- **B1 — Scanline/grid overlay:** ship the static root `::after` from Decision 2 verbatim. No animation.
  Keep the existing B1-Step-2 FPS check (scroll the graph) as the empirical confirm of the GO call.
- **B2 — Mono readout + `● LIVE`:** restyle `StatusBar .status-bar`/`.section`/`.status-text` as an
  uppercase, tracked, `var(--font-mono)` console strip; convert the existing `.spinner`
  (`StatusBar.svelte:74,169`; also `RemoteProgress.svelte:16,51`) into the `● LIVE` blink dot (blink gated
  on `prefers-reduced-motion`). No new markup.
- **B3 — Corner brackets:** implement Decision 1 exactly — one `::after` corner-mark on `.panel` and
  `.dialog`, `position:relative` on those roots, **flush-inset (never outset)**, plus the `.gh .panel`
  exclusion (and optional `.side-col .panel` exclusion). Host list is the enumerated set in Decision 1.
  **No wrapper elements, no attributes.**
- **B4 — Hazard dividers + console header bars:** hazard `repeating-linear-gradient` on the Decision-4
  divider selectors (lead with `.panel-header` border-bottom); `◇ LABEL … ● LIVE` restyle on
  `.panel-header`+`<h2>` and the three `.header`+`<h3>` dialogs (Decision 3). The bare-`<h3>` dialogs get
  the sigil/type only. Any `● LIVE` on a bare-`<h3>` dialog is the sole optional additive span, Classic
  `display:none` — ship only if QA asks.

### What changed vs the plan's Phase B placeholders
- "shared `data-nerv-frame` wrapper class added once per host" → **not needed**; use the existing
  `.panel`/`.dialog` DOM classes directly (Svelte doesn't rename them). Edit count for framing: **0**.
- "Add the minimal Classic-hidden wrapper markup only where the spike proved it necessary" → the spike
  proved it necessary **nowhere**; flush-inset pseudo clears every overflow host.
- The commit-list/graph "frame" is not a bespoke container — it is `GraphHistory`'s `CollapsiblePanel`
  `.panel` (`GraphHistory.svelte:386`), so it's covered by the same `.panel` rule as the other panels.
