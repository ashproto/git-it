<script lang="ts">
  import { parseDiff } from "../diff/parse";
  import { getHighlighter, LANGS } from "../diff/highlight";
  import { blocksOf, blockAt, ordinalsOf, ordsInRange, type Block } from "../diff/blocks";
  import {
    toSplitRows,
    isChangeRow,
    splitBlocksOf,
    splitBlockAt,
    splitBlockRanges,
    ordsInSplitRange,
    splitRangeSnappedToBlocks,
  } from "../diff/splitRows";
  import { appState } from "../store.svelte";
  import type { DiffFile, DiffHunk, DiffLine } from "../diff/types";
  import type { Highlighter, ThemedToken } from "shiki";

  // ─── Props ────────────────────────────────────────────────────────────────────

  interface Props {
    patch: string;
    language?: string;
    staged?: boolean;
    onStageHunk?: (i: number) => void;
    onUnstageHunk?: (i: number) => void;
    onStageLines?: (hunkIndex: number, selected: number[]) => void;
    onUnstageLines?: (hunkIndex: number, selected: number[]) => void;
    /** `changedLines` is how many +/- lines the hunk holds — the confirm dialog states it,
        because in Whole-file mode one hunk covers the entire file. */
    onDiscardHunk?: (i: number, changedLines: number) => void;
    onDiscardLines?: (hunkIndex: number, selected: number[]) => void;
    // PR-review comment affordance (GitHub line/side semantics: RIGHT = new-file
    // line number for context + added rows, LEFT = old-file line number for
    // deleted rows). Purely additive — when absent, nothing changes.
    onLineComment?: (line: number, side: "LEFT" | "RIGHT") => void;
    hasComment?: (line: number, side: "LEFT" | "RIGHT") => boolean;
  }

  let { patch, language, staged = false, onStageHunk, onUnstageHunk, onStageLines, onUnstageLines, onDiscardHunk, onDiscardLines, onLineComment, hasComment }: Props = $props();

  // The whole hover/selection affordance only exists where actions do. Commit diffs
  // (CommitFilesDiff) and PR review diffs (PrFilesTab) pass no callbacks and stay inert.
  const hasActions = $derived(
    !!(onStageHunk || onUnstageHunk || onStageLines || onUnstageLines || onDiscardHunk || onDiscardLines),
  );

  // Hovered unit. `block` is the index into blocksForView(hunk), or null on a context
  // row (which targets the whole hunk instead). `ri` is the row itself, kept so the
  // hunk-scope toolbar can anchor to a row the user can actually see.
  let hov = $state<{ fi: number; hi: number; ri: number; block: number | null } | null>(null);

  /**
   * Row indices mean different things per view: in unified they index `hunk.lines`,
   * in split they index the paired visual rows. Every hover/selection/ring value in
   * this component is in the CURRENT view's space; these two helpers are the only
   * places that translate.
   */
  function selectionOrds(h: DiffHunk, from: number, to: number): number[] {
    return appState.diffSplit
      ? ordsInSplitRange(h, toSplitRows(h), from, to)
      : ordsInRange(h, from, to);
  }

  function blocksForView(h: DiffHunk): Block[] {
    return appState.diffSplit ? splitBlocksOf(h, toSplitRows(h)) : blocksOf(h);
  }

  function hoverRow(fi: number, hi: number, ri: number) {
    if (!hasActions || sel) return; // locked: hover is dead
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return;
    const block = appState.diffSplit ? splitBlockAt(toSplitRows(h), ri) : blockAt(h, ri);
    hov = { fi, hi, ri, block };
  }

  function clearHover() {
    if (sel) return;
    hov = null;
  }

  /**
   * The row range the ring should cover for this hunk, or null for none. A locked
   * selection owns the ring outright; otherwise only a hovered BLOCK gets it and a
   * hovered context row gets the outer `<tbody>` outline instead (`hunk-hover`).
   */
  function ringRange(fi: number, hi: number): { from: number; to: number } | null {
    if (sel) {
      return sel.fi === fi && sel.hi === hi ? { from: sel.from, to: sel.to } : null;
    }
    if (!hov || hov.fi !== fi || hov.hi !== hi || hov.block === null) return null;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return null;
    const b = blocksForView(h)[hov.block];
    return b ? { from: b.rows[0], to: b.rows[b.rows.length - 1] } : null;
  }

  interface ActionTarget {
    fi: number;
    hi: number;
    scope: "sel" | "blk" | "hunk";
    /** null for a whole-hunk target — the hunk ops take no ordinals. */
    ords: number[] | null;
    /** Row the toolbar anchors to, in the current view's index space (see `selectionOrds`). */
    anchorRow: number;
  }

  /** What the visible toolbar acts on right now. A locked selection outranks hover. */
  function activeTarget(): ActionTarget | null {
    if (!hasActions) return null;
    if (sel) {
      const h = parsed.files[sel.fi]?.hunks[sel.hi];
      if (!h) return null;
      const ords = selectionOrds(h, sel.from, sel.to);
      return ords.length
        ? { fi: sel.fi, hi: sel.hi, scope: "sel", ords, anchorRow: sel.from }
        : null;
    }
    if (!hov) return null;
    const h = parsed.files[hov.fi]?.hunks[hov.hi];
    if (!h) return null;
    if (hov.block !== null) {
      const b = blocksForView(h)[hov.block];
      if (b) return { fi: hov.fi, hi: hov.hi, scope: "blk", ords: b.ords, anchorRow: b.rows[0] };
    }
    // Anchor to the hovered row, not row 0: a hunk taller than the pane would otherwise
    // put its anchor off-screen and `measureTool` would hide the toolbar entirely.
    return { fi: hov.fi, hi: hov.hi, scope: "hunk", ords: null, anchorRow: hov.ri };
  }

  function runAction(kind: "stage" | "unstage" | "discard") {
    const t = activeTarget();
    if (!t) return;
    if (t.scope === "hunk") {
      if (kind === "stage") onStageHunk?.(t.hi);
      else if (kind === "unstage") onUnstageHunk?.(t.hi);
      else {
        const h = parsed.files[t.fi]?.hunks[t.hi];
        const changed = h ? ordinalsOf(h).filter((o) => o !== null).length : 0;
        onDiscardHunk?.(t.hi, changed);
      }
    } else {
      const ords = t.ords ?? [];
      if (!ords.length) return;
      if (kind === "stage") onStageLines?.(t.hi, ords);
      else if (kind === "unstage") onUnstageLines?.(t.hi, ords);
      else onDiscardLines?.(t.hi, ords);
    }
    hov = null;
    sel = null;
  }

  // The toolbar lives OUTSIDE the horizontally-scrolling table wrap so `right` pins
  // it to the visible edge. Only its vertical offset needs measuring, and using
  // getBoundingClientRect differences means scrollTop needs no separate arithmetic.
  let wrapEls = $state<Record<number, HTMLDivElement | undefined>>({});
  let toolTop = $state(0);
  let toolVisible = $state(false);

  // Enough room for the toolbar's own height so clamping never parks it half out of view.
  const TOOLBAR_CLEARANCE = 30;

  function measureTool() {
    const t = activeTarget();
    const wrap = t ? wrapEls[t.fi] : undefined;
    if (!t || !wrap) {
      toolVisible = false;
      return;
    }
    const row = wrap.querySelector<HTMLElement>(
      `tr[data-h="${t.hi}"][data-i="${t.anchorRow}"]`,
    );
    if (!row) {
      toolVisible = false;
      return;
    }
    const raw = row.getBoundingClientRect().top - wrap.getBoundingClientRect().top;
    // Clamp rather than hide. The ring stays drawn whenever a target is live, so hiding
    // the toolbar would leave a visible target with no route to act on it — which happens
    // routinely for a block or selection taller than the pane, and whenever the user
    // scrolls with a selection locked.
    const maxTop = Math.max(0, wrap.clientHeight - TOOLBAR_CLEARANCE);
    toolTop = Math.min(Math.max(raw, 0), maxTop);
    toolVisible = true;
  }

  // Runs after the DOM settles, so the anchor row is guaranteed to exist.
  // diffSplit is tracked too: toggling Unified/Split unmounts the anchor row out
  // from under a toolbar that would otherwise keep floating at a stale offset.
  $effect(() => {
    hov;
    sel;
    appState.diffSplit;
    measureTool();
  });

  // ─── Theme detection ──────────────────────────────────────────────────────────

  function prefersDark(): boolean {
    return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches;
  }

  let darkMode = $state(prefersDark());

  // Re-evaluate on OS dark-mode changes (nice-to-have, as per spec).
  if (typeof window !== "undefined") {
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
      darkMode = e.matches;
    });
  }

  const activeTheme = $derived(
    appState.theme === "nerv" ? "nerv-" + appState.scheme : darkMode ? "github-dark" : "github-light",
  );

  // ─── Parsed diff ──────────────────────────────────────────────────────────────

  const parsed = $derived(parseDiff(patch));

  // Total additions / deletions across all parsed files (for the toolbar summary).
  const totals = $derived.by(() => {
    let add = 0;
    let del = 0;
    for (const file of parsed.files) {
      for (const hunk of file.hunks) {
        for (const line of hunk.lines) {
          if (line.kind === "add") add++;
          else if (line.kind === "del") del++;
        }
      }
    }
    return { add, del };
  });

  // ─── Highlighted token cache ──────────────────────────────────────────────────
  // Key: `${fileIndex}:${hunkIndex}:${theme}` → { beforeTokens, afterTokens } per line
  // null means not yet resolved; we render plain text until it arrives.

  type TokenLine = ThemedToken[][];
  interface HunkTokens {
    before: TokenLine; // context + del lines, in order
    after: TokenLine;  // context + add lines, in order
  }

  // Map from "fileIdx:hunkIdx:theme" → HunkTokens
  let tokenCache = $state<Map<string, HunkTokens>>(new Map());

  // Tracks the patch+theme combination for which the cache was last computed.
  // When either changes, invalidate and re-highlight.
  let lastHighlightKey = $state("");

  $effect(() => {
    const currentFiles = parsed.files;
    const theme = activeTheme;
    const cacheKey = patch + "::" + theme;

    if (cacheKey === lastHighlightKey) return;
    lastHighlightKey = cacheKey;

    // Clear the old cache so we immediately fall back to plain text while loading.
    tokenCache = new Map();

    if (currentFiles.length === 0) return;

    getHighlighter()
      .then((hl: Highlighter) => {
        // Re-check that this result is still for the current patch+theme
        // (the user may have navigated away during the async gap).
        if (patch + "::" + activeTheme !== cacheKey) return;

        const next = new Map<string, HunkTokens>();

        for (let fi = 0; fi < currentFiles.length; fi++) {
          const file = currentFiles[fi];
          if (file.binary) continue;

          // Resolve the language — fall back to "text" if Shiki doesn't know it.
          const fileLang = language ?? file.language;
          const resolvedLang: string = (LANGS as readonly string[]).includes(fileLang)
            ? fileLang
            : "text";

          for (let hi = 0; hi < file.hunks.length; hi++) {
            const hunk = file.hunks[hi];

            // Build before-text (context + del lines, in hunk order).
            // Build after-text (context + add lines, in hunk order).
            const beforeLines: string[] = [];
            const afterLines: string[] = [];

            for (const line of hunk.lines) {
              if (line.kind === "context") {
                beforeLines.push(line.text);
                afterLines.push(line.text);
              } else if (line.kind === "del") {
                beforeLines.push(line.text);
              } else {
                afterLines.push(line.text);
              }
            }

            // Highlight each block. If a language is "text" (not in Shiki's set),
            // codeToTokens still works but returns unstyled tokens — that's fine.
            let beforeTokens: TokenLine = [];
            let afterTokens: TokenLine = [];

            try {
              if (beforeLines.length > 0) {
                const res = hl.codeToTokens(beforeLines.join("\n"), {
                  lang: resolvedLang as never,
                  theme,
                });
                beforeTokens = res.tokens;
              }
              if (afterLines.length > 0) {
                const res = hl.codeToTokens(afterLines.join("\n"), {
                  lang: resolvedLang as never,
                  theme,
                });
                afterTokens = res.tokens;
              }
            } catch {
              // Shiki failure for a specific hunk — leave tokens empty (plain text fallback).
              beforeTokens = [];
              afterTokens = [];
            }

            next.set(`${fi}:${hi}:${theme}`, { before: beforeTokens, after: afterTokens });
          }
        }

        tokenCache = next;
      })
      .catch(() => {
        // Shiki highlighter init failed entirely — tokenCache stays empty,
        // rendering plain text. Never block or throw.
      });
  });

  // ─── Token lookup helpers ─────────────────────────────────────────────────────

  /** Return the highlighted tokens for a single diff line, or null for plain-text fallback. */
  function lineTokens(
    fi: number,
    hi: number,
    line: DiffLine,
    beforeIdx: number,
    afterIdx: number,
  ): ThemedToken[] | null {
    const key = `${fi}:${hi}:${activeTheme}`;
    const ht = tokenCache.get(key);
    if (!ht) return null;
    if (line.kind === "del") return ht.before[beforeIdx] ?? null;
    return ht.after[afterIdx] ?? null;
  }

  // ─── Per-hunk line index bookkeeping ─────────────────────────────────────────

  interface IndexedLine {
    line: DiffLine;
    beforeIdx: number; // index in the before-tokens array
    afterIdx: number;  // index in the after-tokens array
  }

  function indexHunkLines(hunk: DiffHunk): IndexedLine[] {
    let bi = 0;
    let ai = 0;
    return hunk.lines.map((line) => {
      const entry: IndexedLine = { line, beforeIdx: bi, afterIdx: ai };
      if (line.kind === "context") { bi++; ai++; }
      else if (line.kind === "del") { bi++; }
      else { ai++; }
      return entry;
    });
  }

  // ─── Line selection ───────────────────────────────────────────────────────────
  // A contiguous row range inside ONE hunk, in the current view's index space.
  // `anchor` is the row the range grew from, so extending down and then back up
  // pivots correctly. Ranges may span context rows; `selectionOrds` keeps only the
  // change lines.
  let sel = $state<{ fi: number; hi: number; anchor: number; from: number; to: number } | null>(null);

  // Roving tabindex: exactly ONE row per file is a tab stop, so the diff contributes a
  // single stop to the app's tab order and Tab from it reaches `.diff-tools` directly.
  // Unshifted arrows move between rows; every row stays programmatically focusable at -1.
  let focusedRow = $state<{ fi: number; hi: number; ri: number } | null>(null);

  function isTabStop(fi: number, hi: number, ri: number): boolean {
    if (!hasActions) return false;
    if (focusedRow) return focusedRow.fi === fi && focusedRow.hi === hi && focusedRow.ri === ri;
    return hi === 0 && ri === 0; // nothing focused yet: the file's first row
  }

  // A new patch re-indexes every hunk, and flipping Unified/Split re-indexes the rows
  // themselves — a range held across either is meaningless. `focusedRow` goes too: a
  // stale one matches no rendered row, which would leave the diff with NO tab stop.
  $effect(() => {
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    patch;
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    appState.diffSplit;
    sel = null;
    hov = null;
    focusedRow = null;
  });

  function lockSelection(fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return;
    if (appState.diffSplit) {
      // Split view snaps to whole blocks: a paired row is a display artifact, and a
      // partial selection of a mixed block yields non-contiguous ordinals, which
      // reorder the file when applied.
      const r = splitRangeSnappedToBlocks(toSplitRows(h), ri, ri);
      if (!r) return;
      sel = { fi, hi, anchor: ri, from: r.from, to: r.to };
    } else {
      if (h.lines[ri]?.kind === "context") return;
      sel = { fi, hi, anchor: ri, from: ri, to: ri };
    }
    hov = null;
    // A double-click is also the browser's select-word gesture, and `.diff-cell`
    // deliberately opts back into user-select so diff code stays copyable.
    window.getSelection()?.removeAllRanges();
  }

  function extendSelection(fi: number, hi: number, ri: number) {
    if (!sel || sel.fi !== fi || sel.hi !== hi) return;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return;
    let from: number;
    let to: number;
    if (appState.diffSplit) {
      const r = splitRangeSnappedToBlocks(toSplitRows(h), sel.anchor, ri);
      if (!r) return;
      from = r.from;
      to = r.to;
    } else {
      from = Math.min(sel.anchor, ri);
      to = Math.max(sel.anchor, ri);
    }
    if (!selectionOrds(h, from, to).length) return;
    sel = { fi, hi, anchor: sel.anchor, from, to };
    window.getSelection()?.removeAllRanges();
  }

  function clearSelection() {
    sel = null;
  }

  function onRowClick(e: MouseEvent, fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    if (e.shiftKey) {
      extendSelection(fi, hi, ri);
      return;
    }
    clearSelection();
  }

  /**
   * Keyboard parity for the mouse affordance: Enter/Space is the double-click
   * (lock), Shift+Arrow is the shift-click (extend). Focus itself is wired to
   * `hoverRow` on the rows, which is what raises the ring and toolbar.
   */
  function onRowKeydown(e: KeyboardEvent, fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      lockSelection(fi, hi, ri);
      return;
    }
    if (!e.shiftKey && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const wrap = wrapEls[fi];
      if (!wrap) return;
      // Querying the DOM keeps this correct across hunk boundaries without duplicating
      // the unified/split row-space logic — the rendered order IS the navigation order.
      const rows = [...wrap.querySelectorAll<HTMLElement>("tr[data-h][data-i]")];
      const cur = rows.findIndex(
        (el) => el.dataset.h === String(hi) && el.dataset.i === String(ri),
      );
      const next = rows[cur + (e.key === "ArrowDown" ? 1 : -1)];
      next?.focus();
      return;
    }
    if (e.shiftKey && (e.key === "ArrowDown" || e.key === "ArrowUp") && sel) {
      e.preventDefault();
      const delta = e.key === "ArrowDown" ? 1 : -1;
      const edge = sel.to === sel.anchor ? sel.from : sel.to;
      const h = parsed.files[fi]?.hunks[hi];
      if (!h) return;
      if (appState.diffSplit) {
        // Split selections snap to whole blocks, so stepping one ROW would always land
        // on the context between blocks and change nothing. Step one BLOCK instead.
        const ranges = splitBlockRanges(toSplitRows(h));
        const cur = ranges.findIndex((r) => edge >= r.from && edge <= r.to);
        const target = ranges[cur + delta];
        if (!target) return;
        extendSelection(fi, hi, target.from);
      } else {
        const next = Math.max(0, Math.min(h.lines.length - 1, edge + delta));
        extendSelection(fi, hi, next);
      }
      return;
    }
  }

  // ─── Rendering helper: tokens → HTML string (safe; tokens contain raw highlighted text) ──

  function renderTokens(tokens: ThemedToken[]): string {
    return tokens
      .map((t) => {
        const style = t.color ? `color:${t.color}` : "";
        const escaped = t.content
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;");
        return style ? `<span style="${style}">${escaped}</span>` : escaped;
      })
      .join("");
  }

  function esc(s: string): string {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
</script>

<!-- ─── Component markup ──────────────────────────────────────────────────────── -->

<!-- Line-comment affordance rendered inside a gutter cell: a hover-revealed ＋
     to start a comment, or a persistent 💬 marker when a draft comment already
     exists on that (line, side). Only rendered when onLineComment is provided. -->
{#snippet commentBtn(lineNo: number, side: "LEFT" | "RIGHT")}
  {@const marked = hasComment?.(lineNo, side) ?? false}
  <button
    type="button"
    class="cm-btn"
    class:marked
    aria-label="Comment on line {lineNo}"
    onclick={(e) => {
      e.stopPropagation();
      onLineComment!(lineNo, side);
    }}
  >{marked ? "💬" : "＋"}</button>
{/snippet}

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && sel) clearSelection();
  }}
/>

<div class="diff-view">
  <!-- Header: split/unified toggle + totals + context controls -->
  <div class="diff-toolbar">
    <span class="diff-mode-label">View:</span>
    <button
      class="mode-btn"
      class:active={!appState.diffSplit}
      onclick={() => appState.setDiffSplit(false)}
    >Unified</button>
    <button
      class="mode-btn"
      class:active={appState.diffSplit}
      onclick={() => appState.setDiffSplit(true)}
    >Split</button>

    <span class="toolbar-totals">
      <span class="tot-add">+{totals.add}</span>
      <span class="tot-del">−{totals.del}</span>
    </span>

    <span class="toolbar-spacer"></span>

    <span class="diff-mode-label">Context:</span>
    <button
      class="mode-btn"
      aria-label="Fewer context lines"
      disabled={appState.diffWholeFile}
      onclick={() => appState.setDiffContext(Math.max(0, appState.diffContext - 3))}
    >−</button>
    <span class="context-val">{appState.diffContext}</span>
    <button
      class="mode-btn"
      aria-label="More context lines"
      disabled={appState.diffWholeFile}
      onclick={() => appState.setDiffContext(appState.diffContext + 3)}
    >+</button>

    <button
      class="mode-btn"
      class:active={appState.diffWholeFile}
      onclick={() => appState.setDiffWholeFile(!appState.diffWholeFile)}
    >Whole file</button>
  </div>

  {#if !patch || !patch.trim()}
    <p class="empty-msg">No changes.</p>
  {:else if parsed.files.length === 0}
    <p class="empty-msg">No changes.</p>
  {:else}
    {#each parsed.files as file, fi (fi)}
      <!-- File header -->
      <div class="file-header">
        <span class="file-path mono">
          {#if file.oldPath === "/dev/null"}
            <span class="added-label">added</span> {file.newPath}
          {:else if file.newPath === "/dev/null"}
            <span class="deleted-label">deleted</span> {file.oldPath}
          {:else if file.oldPath !== file.newPath}
            {file.oldPath} → {file.newPath}
          {:else}
            {file.newPath}
          {/if}
        </span>
      </div>

      {#if file.binary}
        <div class="binary-notice">Binary file — no preview.</div>
      {:else if file.hunks.length === 0}
        <div class="empty-hunk">No textual changes.</div>
      {:else}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="diff-table-outer" onmouseleave={clearHover}>
        <div
          class="diff-table-wrap"
          bind:this={wrapEls[fi]}
          onscroll={measureTool}
        >
          <table class="diff-table mono" class:split={appState.diffSplit}>
            {#if appState.diffSplit}
              <!-- Fixed column widths. Without this, the colspan hunk-header row is the
                   width-defining first row under table-layout:fixed and the old/deletion
                   side collapses to ~28px. The two 44px gutters + two auto side columns
                   split the remaining width evenly (50/50). -->
              <colgroup>
                <col class="dt-gutter" />
                <col class="dt-side" />
                <col class="dt-gutter" />
                <col class="dt-side" />
              </colgroup>
            {/if}
            {#each file.hunks as hunk, hi (hi)}
              {@const ring = ringRange(fi, hi)}
              <tbody
                class="hunk"
                class:hunk-hover={hasActions && !sel && hov?.fi === fi && hov?.hi === hi}
              >
                <!-- Hunk header row -->
                <tr class="hunk-header-row">
                  {#if appState.diffSplit}
                    <td class="hunk-header-cell" colspan="4">
                      <span class="hunk-range">{hunk.header}</span>
                    </td>
                  {:else}
                    <td class="gutter" colspan="2"></td>
                    <td class="hunk-header-cell">
                      <span class="hunk-range">{hunk.header}</span>
                    </td>
                  {/if}
                </tr>

                {#if !appState.diffSplit}
                  <!-- ── UNIFIED view ── -->
                  {#each indexHunkLines(hunk) as { line, beforeIdx, afterIdx }, ri (ri)}
                    {@const toks = lineTokens(fi, hi, line, beforeIdx, afterIdx)}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <tr
                      data-h={hi}
                      data-i={ri}
                      class="diff-row {line.kind}"
                      class:ring={ring !== null && ri >= ring.from && ri <= ring.to}
                      class:ring-first={ring !== null && ri === ring.from}
                      class:ring-last={ring !== null && ri === ring.to}
                      onmouseenter={() => hoverRow(fi, hi, ri)}
                      style={hasActions && line.kind !== "context" ? "cursor: pointer" : ""}
                      tabindex={hasActions ? (isTabStop(fi, hi, ri) ? 0 : -1) : undefined}
                      onfocus={() => {
                        focusedRow = { fi, hi, ri };
                        hoverRow(fi, hi, ri);
                      }}
                      onkeydown={(e) => onRowKeydown(e, fi, hi, ri)}
                      onclick={(e) => onRowClick(e, fi, hi, ri)}
                      ondblclick={() => lockSelection(fi, hi, ri)}
                    >
                      <td class="gutter old-gutter" class:commentable={!!onLineComment}>
                        {line.oldNo ?? ""}
                        {#if onLineComment && line.kind === "del" && line.oldNo != null}
                          {@render commentBtn(line.oldNo, "LEFT")}
                        {/if}
                      </td>
                      <td class="gutter new-gutter" class:commentable={!!onLineComment}>
                        {line.newNo ?? ""}
                        {#if onLineComment && line.kind !== "del" && line.newNo != null}
                          {@render commentBtn(line.newNo, "RIGHT")}
                        {/if}
                      </td>
                      <td class="diff-cell">
                        <span class="sigil">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span>
                        {#if toks}
                          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                          {@html renderTokens(toks)}
                        {:else}
                          {esc(line.text)}
                        {/if}
                      </td>
                    </tr>
                  {/each}
                {:else}
                  <!-- ── SPLIT view ── -->
                  {#each toSplitRows(hunk) as row, ri (ri)}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <tr
                      class="diff-row split-row"
                      class:ring={ring !== null && ri >= ring.from && ri <= ring.to}
                      class:ring-first={ring !== null && ri === ring.from}
                      class:ring-last={ring !== null && ri === ring.to}
                      data-h={hi}
                      data-i={ri}
                      style={hasActions && isChangeRow(row) ? "cursor: pointer" : ""}
                      onmouseenter={() => hoverRow(fi, hi, ri)}
                      tabindex={hasActions ? (isTabStop(fi, hi, ri) ? 0 : -1) : undefined}
                      onfocus={() => {
                        focusedRow = { fi, hi, ri };
                        hoverRow(fi, hi, ri);
                      }}
                      onkeydown={(e) => onRowKeydown(e, fi, hi, ri)}
                      onclick={(e) => onRowClick(e, fi, hi, ri)}
                      ondblclick={() => lockSelection(fi, hi, ri)}
                    >
                      <!-- Old side -->
                      <td class="gutter old-gutter" class:commentable={!!onLineComment}>
                        {row.oldLine?.oldNo ?? ""}
                        {#if onLineComment && row.oldLine?.kind === "del" && row.oldLine.oldNo != null}
                          {@render commentBtn(row.oldLine.oldNo, "LEFT")}
                        {/if}
                      </td>
                      <td
                        class="diff-cell split-cell"
                        class:del={row.oldLine?.kind === "del"}
                        class:context={row.oldLine?.kind === "context"}
                      >
                        {#if row.oldLine}
                          {@const toks = row.oldLine.kind === "del"
                            ? (tokenCache.get(`${fi}:${hi}:${activeTheme}`)?.before[row.oldBeforeIdx] ?? null)
                            : (tokenCache.get(`${fi}:${hi}:${activeTheme}`)?.after[row.oldAfterIdx] ?? null)}
                          <span class="sigil">{row.oldLine.kind === "del" ? "−" : " "}</span>
                          {#if toks}
                            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                            {@html renderTokens(toks)}
                          {:else}
                            {esc(row.oldLine.text)}
                          {/if}
                        {/if}
                      </td>

                      <!-- New side: context + added rows comment on RIGHT/newNo -->
                      <td class="gutter new-gutter" class:commentable={!!onLineComment}>
                        {row.newLine?.newNo ?? ""}
                        {#if onLineComment && row.newLine && row.newLine.newNo != null}
                          {@render commentBtn(row.newLine.newNo, "RIGHT")}
                        {/if}
                      </td>
                      <td
                        class="diff-cell split-cell"
                        class:add={row.newLine?.kind === "add"}
                        class:context={row.newLine?.kind === "context"}
                      >
                        {#if row.newLine}
                          {@const toks = row.newLine.kind === "add"
                            ? (tokenCache.get(`${fi}:${hi}:${activeTheme}`)?.after[row.newAfterIdx] ?? null)
                            : (tokenCache.get(`${fi}:${hi}:${activeTheme}`)?.after[row.oldAfterIdx] ?? null)}
                          <span class="sigil">{row.newLine.kind === "add" ? "+" : " "}</span>
                          {#if toks}
                            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                            {@html renderTokens(toks)}
                          {:else}
                            {esc(row.newLine.text)}
                          {/if}
                        {/if}
                      </td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            {/each}
          </table>
        </div>
        {#if hasActions && toolVisible}
          {@const t = activeTarget()}
          {#if t && t.fi === fi}
            {@const label = t.scope === "hunk" ? "hunk" : String((t.ords ?? []).length)}
            <div class="diff-tools" style="top: {toolTop}px">
              {#if !staged && onStageHunk}
                <button onclick={() => runAction("stage")}>Stage {label}</button>
              {/if}
              {#if !staged && onDiscardHunk}
                <button class="danger" onclick={() => runAction("discard")}>Discard {label}</button>
              {/if}
              {#if staged && onUnstageHunk}
                <button onclick={() => runAction("unstage")}>Unstage {label}</button>
              {/if}
            </div>
          {/if}
        {/if}
        </div>
      {/if}
    {/each}
  {/if}
</div>

<style>
  /* ── Layout ────────────────────────────────────────────────────────────────── */
  .diff-view {
    display: flex;
    flex-direction: column;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    overflow: auto;
  }

  .diff-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--header-bg);
    flex-shrink: 0;
  }

  .diff-mode-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-right: 2px;
  }

  .mode-btn {
    padding: 2px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
    transition: background 0.1s;
  }
  .mode-btn:hover {
    background: var(--btn-hover);
  }
  .mode-btn.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .mode-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .mode-btn:disabled:hover {
    background: var(--btn-bg);
  }

  .toolbar-spacer {
    flex: 1 1 auto;
  }

  .toolbar-totals {
    display: inline-flex;
    gap: 6px;
    margin-left: 8px;
    font-size: 11px;
    font-family: var(--font-mono);
  }
  .tot-add {
    color: var(--diff-add-fg, #2da44e);
  }
  .tot-del {
    color: var(--diff-del-fg, #cf222e);
  }

  .context-val {
    min-width: 1.4ch;
    text-align: center;
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--text);
  }

  /* ── File header ────────────────────────────────────────────────────────────── */
  .file-header {
    display: flex;
    align-items: center;
    padding: 5px 10px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--header-bg);
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    flex-shrink: 0;
  }

  .file-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .added-label {
    color: var(--diff-add-fg, #2da44e);
    font-weight: 600;
    margin-right: 4px;
  }

  .deleted-label {
    color: var(--diff-del-fg, #cf222e);
    font-weight: 600;
    margin-right: 4px;
  }

  /* ── Binary / empty notices ─────────────────────────────────────────────────── */
  .binary-notice,
  .empty-hunk,
  .empty-msg {
    padding: 8px 12px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  /* ── Diff table ─────────────────────────────────────────────────────────────── */

  /* Positioning context for the floating toolbar. It must NOT be the scroll
     container: an absolutely-positioned child of a scroller is laid out against
     the content, so `right` would drift as the diff scrolls sideways. */
  .diff-table-outer {
    position: relative;
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .diff-table-wrap {
    overflow: auto;
    flex: 1 1 auto;
    min-width: 0;
  }

  .diff-tools {
    position: absolute;
    right: 14px;
    z-index: 6;
    display: flex;
    gap: 4px;
    padding: 3px;
    background: var(--header-bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .diff-tools button {
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent);
    background: var(--btn-bg);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    white-space: nowrap;
    cursor: pointer;
  }
  .diff-tools button:hover {
    background: var(--accent);
    color: var(--on-accent);
  }
  .diff-tools button.danger {
    border-color: var(--danger);
    color: var(--danger);
  }
  .diff-tools button.danger:hover {
    background: var(--danger);
    color: var(--on-accent);
  }

  .diff-table {
    border-collapse: collapse;
    /* Grow to the widest line so the WHOLE diff scrolls horizontally as one unit
       (.diff-table-wrap is the single scroll container) — instead of each cell
       scrolling on its own. min-width keeps short diffs filling the pane. */
    width: max-content;
    min-width: 100%;
  }

  /* Split-view column sizing (see the <colgroup> in the markup): the gutters are
     fixed, the two text sides share the rest evenly so deletions aren't clipped. */
  .diff-table > colgroup .dt-gutter {
    width: 44px;
  }
  .diff-table > colgroup .dt-side {
    width: auto;
  }

  /* ── Gutters ─────────────────────────────────────────────────────────────────── */
  .gutter {
    width: 44px;
    min-width: 32px;
    padding: 0 6px;
    text-align: right;
    font-size: 11px;
    color: var(--text-muted);
    user-select: none;
    white-space: nowrap;
    vertical-align: top;
    border-right: 1px solid var(--border-subtle);
    background: var(--header-bg);
  }

  /* ── Hover / selection ring ──────────────────────────────────────────────────
     A hunk is a <tbody> and takes a plain outline. A BLOCK is only a run of <tr>s
     with no wrapping element, so its ring is built from four box-shadow insets
     spread across the run's cells. They must be four SEPARATE custom properties
     feeding one box-shadow: written as four rules on `box-shadow` directly, the
     last would simply win. As custom properties they compose. */
  .diff-table td {
    box-shadow: var(--rt, 0 0 transparent), var(--rb, 0 0 transparent),
                var(--rl, 0 0 transparent), var(--rr, 0 0 transparent);
  }
  tr.ring-first td { --rt: inset 0 1.5px 0 var(--diff-ring); }
  tr.ring-last td { --rb: inset 0 -1.5px 0 var(--diff-ring); }
  tr.ring td:first-child { --rl: inset 1.5px 0 0 var(--diff-ring); }
  tr.ring td:last-child { --rr: inset -1.5px 0 0 var(--diff-ring); }

  /* Outer ring — subtler than the block ring, and drawn on the hunk's own element. */
  tbody.hunk.hunk-hover {
    outline: 1px solid color-mix(in srgb, var(--diff-ring) 34%, transparent);
    outline-offset: -1px;
  }

  /* ── Diff rows ─────────────────────────────────────────────────────────────── */

  .diff-cell {
    padding: 0 4px 0 2px;
    white-space: pre;
    word-break: keep-all;
    vertical-align: top;
  }

  /* Unified row backgrounds */
  .diff-row.add {
    background: var(--diff-add-bg, rgba(46, 160, 67, 0.18));
  }
  .diff-row.del {
    background: var(--diff-del-bg, rgba(210, 35, 35, 0.18));
  }

  @media (prefers-color-scheme: dark) {
    .diff-row.add {
      background: var(--diff-add-bg-dark, rgba(46, 160, 67, 0.24));
    }
    .diff-row.del {
      background: var(--diff-del-bg-dark, rgba(210, 35, 35, 0.24));
    }
  }

  /* Sigil column */
  .sigil {
    display: inline-block;
    width: 1ch;
    margin-right: 4px;
    user-select: none;
    color: var(--text-muted);
  }
  .diff-row.add .sigil {
    color: var(--diff-add-fg, #2da44e);
  }
  .diff-row.del .sigil {
    color: var(--diff-del-fg, #cf222e);
  }

  /* ── Split view cells ───────────────────────────────────────────────────────── */
  .split-cell {
    width: 50%;
  }

  .split-cell.add {
    background: var(--diff-add-bg, rgba(46, 160, 67, 0.18));
  }
  .split-cell.del {
    background: var(--diff-del-bg, rgba(210, 35, 35, 0.18));
  }
  .split-cell.context {
    background: transparent;
  }

  @media (prefers-color-scheme: dark) {
    .split-cell.add {
      background: var(--diff-add-bg-dark, rgba(46, 160, 67, 0.24));
    }
    .split-cell.del {
      background: var(--diff-del-bg-dark, rgba(210, 35, 35, 0.24));
    }
  }

  /* Split add/del sigil colours (scoped to split-cell rather than the row) */
  .split-cell.add .sigil {
    color: var(--diff-add-fg, #2da44e);
  }
  .split-cell.del .sigil {
    color: var(--diff-del-fg, #cf222e);
  }

  /* ── Hunk header row ─────────────────────────────────────────────────────────── */
  .hunk-header-row {
    background: var(--header-bg);
  }

  .hunk-header-row .gutter {
    border-right: 1px solid var(--border-subtle);
  }

  .hunk-header-cell {
    padding: 2px 8px;
    color: var(--text-muted);
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid var(--border-subtle);
    white-space: nowrap;
  }

  /* td cannot be flex by default — use a wrapper instead */
  td.hunk-header-cell {
    display: table-cell;
  }

  .hunk-range {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent);
    margin-right: auto;
  }

  /* ── Line-comment affordance (only present when onLineComment is passed) ────── */
  /* Anchor for the absolutely-positioned button; position:relative on a td with
     no offsets is layout-neutral, and the class only exists in comment mode. */
  .gutter.commentable {
    position: relative;
  }
  .cm-btn {
    position: absolute;
    left: 2px;
    top: 50%;
    transform: translateY(-50%);
    width: 15px;
    height: 15px;
    padding: 0;
    border: none;
    border-radius: 3px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 11px;
    line-height: 15px;
    text-align: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.08s;
  }
  tr:hover .cm-btn {
    opacity: 1;
  }
  .cm-btn:focus-visible {
    opacity: 1;
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  /* Persistent marker when a draft comment exists on the line (click re-opens it). */
  .cm-btn.marked {
    opacity: 1;
    background: transparent;
    font-size: 10px;
  }

  /* ── Mono utility ───────────────────────────────────────────────────────────── */
  .mono {
    font-family: var(--font-mono);
  }
</style>
