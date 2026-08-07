# Diff hunk/block affordance + line discard — Design

**Goal:** Give the diff view a Fork/SourceTree-style hover affordance (ring + floating actions) at two nested
levels — hunk and change-block — add **Discard** for hunks and line ranges (which the app does not have at any
granularity below whole-file), and replace the line-selection visual that is effectively invisible in the NERV
theme. Unified **and** Split views. Working Copy (Local Changes) only; commit and PR diffs stay inert.

Interaction model was validated against a live prototype before this spec was written
(`.superpowers/brainstorm/*/content/interactive-prototype-v2.html`, gitignored).

## Background (verified)

### The NERV selection bug

`DiffView.svelte:889-896` is the entire selection visual:

```css
.diff-row.selected  { box-shadow: inset 2px 0 0 var(--accent);
                      background: color-mix(in srgb, var(--accent) 15%, transparent); }
.split-cell.selected { /* identical */ }
```

Both properties derive from `--accent`. In Classic that is `#2563eb` (blue) over a near-white row — legible.
In NERV (`+page.svelte:762-774`) `--accent` is `#F2542D` (orange) and the row underneath a deletion is already
`--diff-del-bg: rgba(255, 68, 56, 0.15)`. A 15% orange wash over a 15% red wash on a `#12171C` panel is very
close to no change at all. Contributing factors:

- `nerv.css:97-106` paints a fixed root `::after` (scanlines + a `color-mix(--accent 1.5%)` sheet) at
  `z-index: 9999` over the whole app, which further flattens low-alpha differences.
- `nerv.css:298-302` recolors `--accent` per scheme and nothing else, so **crimson** (`#E0445A`) collides with
  deletions and **phosphor** (`#46E88B`) collides with additions. Any accent-derived 15% wash is unfixable by
  tuning — it is the wrong mechanism.

### Existing staging machinery (all reused)

- `ops_worktree.rs:242` `split_hunks(diff) -> (header, Vec<hunk_text>)`.
- `ops_worktree.rs:264` `git_apply(repo, patch, reverse)` — **hardcodes `--cached`**, pipes the patch via stdin.
- `ops_worktree.rs:339` `build_partial_hunk(hunk, selected, reverse)` — `selected` holds **change-line ordinals**
  (0-based over the `+`/`-` lines of one hunk; context and `\ No newline` excluded). With `reverse: true` it emits
  a patch whose **new side matches the post-change image**: unselected `+` stay as context, unselected `-` are
  dropped. Recomputes the `@@` header, so any `-U{context}` works.
- `ops_worktree.rs:297/311/456/469` `stage_hunk` / `unstage_hunk` / `stage_lines` / `unstage_lines`.
- Chain: `gitActions.ts:865-872` → `api.ts:193-199` → `invoke` → `commands.rs:446-462` → `ops_worktree`.
  Commands registered at `lib.rs:199-202`. Context flows from `appState.effectiveDiffContext`.
- `gitActions.ts:932` `discard(paths)` — the file-level danger-confirm pattern to match ("This cannot be undone.
  (Stash instead to keep them.)"), routed through `runWorktree`.

### DiffView structure

- `DiffView.svelte:435` — a **single `<tbody>`** wraps every hunk of every file.
- `DiffView.svelte:422` `<table class="diff-table" class:split>`; split adds a `<colgroup>` (`:428`).
- `DiffView.svelte:260` `toSplitRows(hunk)` pairs a contiguous run of `del`/`add` lines into side-by-side rows;
  context lines span both sides. Selection in split view lives on the `<td>`, not the `<tr>`.
- `DiffView.svelte:210` `selected = $state<Map<string, Set<number>>>` keyed `"fileIdx:hunkIdx"`; `:230` `toggleLine`
  toggles on single click; `:213` an `$effect` clears the selection whenever `patch` changes.
- `DiffView.svelte:450-478` renders "Stage N line(s)" buttons inside the hunk-header row.
- `DiffView.svelte:721-733` `.diff-table-wrap { overflow: auto }` is the single scroll container; the table is
  `width: max-content`, so the view scrolls **horizontally** as one unit.
- Three hosts: `WorkingCopyView.svelte:549` (passes staging callbacks), `CommitFilesDiff.svelte:94` and
  `github/PrFilesTab.svelte:174` (pass none).
- `+page.svelte:819` opts `.diff-cell` back into `user-select` (the body baseline is `user-select: none`) so diff
  code can be copied.

## Interaction model

One ring visual, used by both hover and selection. They are **mutually exclusive**, so they cannot be confused
and do not need to look different.

| Gesture | Result |
|---|---|
| Hover a changed line | **Block ring** (contiguous run of `+`/`-` lines) + block actions |
| Hover a context line | **Hunk ring** (outline on the hunk) + hunk actions |
| Double-click a changed line | **Lock** a one-line selection; hover goes inert |
| Shift+click another line, same hunk | Extend the selection to a contiguous range |
| `Esc`, or any plain click | Clear the selection; hover resumes |

While a selection is locked: no hunk ring, no block ring, no hover response of any kind. The only ring on screen
is the selection, and its toolbar is persistent rather than hover-transient.

Nesting: hovering a changed line draws **both** rings — the hunk ring (outer, subtle) and the block ring (inner,
solid) — but only the innermost unit carries buttons. Never two toolbars.

**Actions by pane:**

| Pane | Buttons |
|---|---|
| Unstaged | `Stage <n>` · `Discard <n>` (or `Stage hunk` · `Discard hunk`) |
| Staged | `Unstage <n>` (or `Unstage hunk`) |

Discard is deliberately **unstaged-only**. On the staged side it would have to mean unstage-and-revert — a
two-step destruction whose blast radius is not visible from the pane you are looking at, sitting one hover away
from a routine action. Discarding staged work stays a deliberate two-step: unstage, then discard.

**Ranges may span context lines.** A range anchored on a change line and extended past unchanged rows stays
visually contiguous; `ordsInRange` collects only the change lines inside it, so `Stage 5` means five real
changes even when the ring covers seven rows.

**Where the affordance does not appear:** untracked files (no index entry — they render via
`git diff --no-index` and have no hunk-level staging today), and any host that supplies no action callbacks
(`CommitFilesDiff`, `PrFilesTab`). Gate on prop presence, matching how `onLineComment` already gates the
PR-review comment button.

## Frontend design

### `src/lib/diff/blocks.ts` (new, pure, vitest-covered)

Per the convention that logic which can be pure lives outside `.svelte`:

```ts
export interface Block { rows: number[]; ords: number[] }   // row indices within the hunk, change ordinals
export function blocksOf(hunk: DiffHunk): Block[]           // contiguous runs of non-context lines
export function ordinalsOf(hunk: DiffHunk): (number | null)[]  // per row: change ordinal, or null for context
export function ordsInRange(hunk: DiffHunk, from: number, to: number): number[]
```

`blocksOf` and `ordinalsOf` replace the inline `hunkOrdinals` / `indexHunkLines` helpers
(`DiffView.svelte:195-225`), which stay as thin callers or are removed.

### `DiffView.svelte`

**DOM:**
- Emit **one `<tbody class="hunk">` per hunk** instead of the single wrapping `<tbody>` at `:435`. This is the
  whole reason the hunk ring is free: a `<tbody>` is a real element and takes a CSS `outline`. A run of `<tr>`s
  is not, which is why the block ring cannot use the same mechanism.
- **The block/selection ring needs no JS.** `box-shadow` does not merge across CSS rules — a later declaration
  replaces an earlier one wholesale — but *custom properties* do compose. Every `<td>` carries
  `box-shadow: var(--rt,…), var(--rb,…), var(--rl,…), var(--rr,…)`, and four separate selectors
  (`tr.ring-first td`, `tr.ring-last td`, `tr.ring td:first-child`, `tr.ring td:last-child`) each set one edge.
  A ring around a run of `<tr>`s falls out with no measurement, no overlay, and no scroll listener.
- **Only the toolbar needs JS**, and only for its vertical offset.

**State:**
```ts
let sel = $state<{ fi: number; hi: number; anchor: number; from: number; to: number } | null>(null);
let hov = $state<{ fi: number; hi: number; block: number | null } | null>(null);
```
`sel` replaces `Map<string, Set<number>>` — contiguity makes a set unnecessary. `anchor` is the row the range
grew from, kept distinct from `from`/`to` so extending down and then back up pivots on the original
double-clicked row rather than on whichever edge moved last. The existing `$effect` that clears selection on
`patch` change (`:213`) keeps working and stays.

**Toolbar positioning:** the toolbar is absolutely positioned inside a **new non-scrolling**
`.diff-table-outer` that wraps `.diff-table-wrap`. This matters: an absolutely-positioned child of an
`overflow: auto` element scrolls with its content, so a toolbar inside the wrap would drift sideways on a
horizontally scrolling diff. From outside it, `right: 14px` stays pinned. Its vertical offset is measured as a
`getBoundingClientRect()` difference against the wrap — already relative to the visible box, so `scrollTop`
needs no separate arithmetic — and is recomputed on hover/selection change and on scroll. When the anchor row
scrolls out of view the toolbar hides rather than floating.

**Split view:** the ring spans the **full row width**, covering both columns. A paired row's change ordinals are
the same set as in Unified, so `stage_lines` / `discard_lines` receive identical arguments in both modes and no
backend branching is needed. Accepted cost: a paired row selects as a unit, so Split loses "stage the addition
but not its deletion" precision. Unified retains it and is one toolbar click away.

**Removed:** `.diff-row.selected` / `.split-cell.selected` CSS (`:889-896`), the `toggleLine` single-click
handler and its `onkeydown` twin, and the hunk-header selection buttons (`:450-457`, `:471-478`). The original
NERV bug is fixed by **deleting** the rule that caused it, not by restyling it.

**Also removed:** the hunk-header `Stage hunk` / `Unstage hunk` buttons (`:444-448`, `:465-469`). The hover
toolbar supersedes them; leaving both would mean two live paths to the same action. The hunk header keeps its
`@@` range label.

**After any action** the selection clears and hover resumes. This falls out of the existing `$effect` at `:213`
— every op triggers a working-copy refresh, the `patch` prop changes, and the hunk indices the selection was
expressed in are stale by definition.

### Keyboard

Focus behaves as hover, so the feature is not mouse-only (today's rows are focusable and handle Enter/Space —
this preserves that rather than regressing it):

- Focusing a changed row raises its block ring + toolbar; focusing a context row raises the hunk ring.
- `Enter` / `Space` on a focused changed row locks a one-line selection there.
- `Shift`+`↑`/`↓` extends the locked range within the hunk.
- `Esc` clears.
- Toolbar buttons are real `<button>`s, reachable by Tab whenever the toolbar is visible.

### Theming

Introduce **`--diff-ring`**, defaulting to `var(--accent)` in the Classic token block. The ring is a solid
1.5px edge rather than a low-alpha fill, which is a far stronger signal than the 15%-on-15% wash that failed —
it is expected to hold across all six NERV schemes. The token exists so `nerv.css` can retune crimson/phosphor
later **without touching component markup**, which is that file's standing contract. Per `CLAUDE.md`, any such
rule must use bare `:root[data-theme="nerv"]` — never `:global()`, which is silently dropped in plain CSS.

## Backend design — `crates/git-core/src/ops_worktree.rs`

Thread a flag through the existing helper:

```rust
fn git_apply(repo: &Path, patch: &str, reverse: bool, cached: bool) -> Result<(), String>
```
`cached: true` keeps `--cached` (index); `cached: false` omits it, so the patch applies to the **working tree
only**. Existing callers pass `true`.

```rust
pub fn discard_hunk(repo: &Path, path: &str, hunk_index: usize, context: u32) -> Result<(), String>
pub fn discard_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize], context: u32)
    -> Result<(), String>
```

Both read the **unstaged** diff (`diff(repo, Some(path), false, context)`), reuse `split_hunks` /
`build_partial_hunk(.., reverse: true)`, and call `git_apply(.., reverse: true, cached: false)`.

`reverse: true` is already exactly correct here and needs no new logic: it emits a patch whose new side matches
the working file (unselected `+` become context because they *are* in the worktree; unselected `-` are dropped
because they are not). The unstaged diff's old side is the **index**, so a reverse-apply reverts the selected
lines to their staged state — **not** to HEAD. Staged work on the same file survives untouched. This is what
makes Discard safe to place beside Stage.

Hunk indices and ordinals refer to the **current** diff; any successful op re-indexes the remainder, so the
frontend must re-fetch before issuing another. The existing `runWorktree` refresh already does this.

## Plumbing

| Layer | Addition |
|---|---|
| `src-tauri/src/commands.rs` | `discard_hunk`, `discard_lines` — sync, matching the existing staging commands (local `git apply` is fast; the async rule targets `gh`/network `git`) |
| `src-tauri/src/lib.rs:199` | register both |
| `src/lib/api.ts` | `discardHunk`, `discardLines` — mirror `stageHunk` / `stageLines` signatures incl. `context` |
| `src/lib/gitActions.ts` | `discardHunk`, `discardLines` — danger-confirm via `dialogs.confirm` (same copy as `discard`, including "Stash instead to keep them"), then `runWorktree` |
| `WorkingCopyView.svelte:549` | pass `onDiscardHunk` / `onDiscardLines` only when `!selectedIsStaged && !selectedIsUntracked` |

Confirm-before-discard is **on**, matching file-level discard. No undo bundle: uncommitted work is not in the
reflog and a `--all` bundle would not capture it — the same reasoning already documented at `gitActions.ts:928`.

## Testing

**vitest — `src/lib/diff/blocks.test.ts`:**
- `blocksOf`: adjacent `del`/`add` runs form one block; context splits blocks; a block at the first and last row
  of a hunk; a hunk that is entirely context yields none.
- `ordinalsOf`: context rows are `null`; ordinals count only `+`/`-`, in order, from 0.
- `ordsInRange`: a range spanning context lines returns only the change ordinals; a single-row range; a range
  covering a whole hunk equals every ordinal.

**cargo — `ops_worktree.rs` tests:**
- `discard_hunk_reverts_only_that_hunk` — two hunks, discard one, assert the other survives in the worktree.
- `discard_lines_reverts_only_selected` — partial-selection revert, both an addition and a deletion.
- `discard_lines_leaves_staged_changes_intact` — **load-bearing**: stage some lines, discard others in the same
  file, assert the staged content is still staged and only the worktree changed. This is the test that proves
  the `cached: false` semantic.
- `git_apply` existing callers unaffected (staging round-trip tests already cover this).

**Manual (`npm run tauri dev` — Tauri-only, not reachable from vitest or the browser preview):** hover both ring
levels; double-click + Shift+click across context lines; Split view; the staged pane offering only Unstage; all
six NERV schemes plus Classic light and dark; discard with and without staged changes on the same file.

**Gate before done:** `npm run check` (0 errors) + `npm test` + `cargo test`.

## Risks

- **Double-click collides with text selection.** `.diff-cell` deliberately opts back into `user-select`
  (`+page.svelte:819`) so diff code can be copied, and double-click is the browser's select-word gesture.
  Mitigation: `getSelection().removeAllRanges()` on `dblclick` (validated in the prototype; a brief
  word-highlight flicker is possible). If it proves annoying in the real app, the fallback is drag-to-select
  instead of double-click — the range model is identical either way, so only the gesture binding changes.
- **Ring tracking on horizontal scroll** — the table is wider than its container by design. Covered by the
  `scroll` listener; verify with a long-line diff.
- **Scheme collision on the ring edge.** Expected to hold, but unverified in the real app across all six
  schemes. `--diff-ring` is the escape hatch if it does not.
- **`<tbody>`-per-hunk is a structural change** to a table that also renders split mode with a `<colgroup>`.
  Verify column sizing is unaffected in both modes (the `<colgroup>` sits outside `<tbody>` and should not care).
