<script lang="ts">
  import { parseDiff } from "../diff/parse";
  import { getHighlighter, LANGS } from "../diff/highlight";
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
  }

  let { patch, language, staged = false, onStageHunk, onUnstageHunk, onStageLines, onUnstageLines }: Props = $props();

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

  const activeTheme = $derived(darkMode ? "github-dark" : "github-light");

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

  // ─── Line-level selection ─────────────────────────────────────────────────────

  // Map keyed by "${fi}:${hi}" → Set of change-line ordinals (0-based among +/- lines).
  let selected = $state<Map<string, Set<number>>>(new Map());

  // Clear selection whenever the patch changes (file/diff switch).
  $effect(() => {
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    patch;
    selected = new Map();
  });

  /** Map each +/- line of a hunk to its 0-based change ordinal (context lines excluded). */
  function hunkOrdinals(hunk: DiffHunk): Map<DiffLine, number> {
    const m = new Map<DiffLine, number>();
    let ord = 0;
    for (const line of hunk.lines) if (line.kind !== "context") m.set(line, ord++);
    return m;
  }

  function keyOf(fi: number, hi: number) { return `${fi}:${hi}`; }
  function isSelected(fi: number, hi: number, ord: number) { return selected.get(keyOf(fi, hi))?.has(ord) ?? false; }

  function toggleLine(fi: number, hi: number, ord: number) {
    const k = keyOf(fi, hi);
    const next = new Map(selected);
    const s = new Set(next.get(k) ?? []);
    if (s.has(ord)) s.delete(ord); else s.add(ord);
    if (s.size) next.set(k, s); else next.delete(k);
    selected = next;
  }

  function selCount(fi: number, hi: number) { return selected.get(keyOf(fi, hi))?.size ?? 0; }

  function applyLines(fi: number, hi: number) {
    const ords = [...(selected.get(keyOf(fi, hi)) ?? [])].sort((a, b) => a - b);
    if (!ords.length) return;
    if (staged) onUnstageLines?.(hi, ords); else onStageLines?.(hi, ords);
    const next = new Map(selected); next.delete(keyOf(fi, hi)); selected = next;
  }

  // ─── Split view helpers ───────────────────────────────────────────────────────

  interface SplitRow {
    oldLine: DiffLine | null;
    newLine: DiffLine | null;
    oldBeforeIdx: number;
    oldAfterIdx: number;
    newBeforeIdx: number;
    newAfterIdx: number;
  }

  /** Pair up del/add lines into side-by-side rows, with context lines spanning both. */
  function toSplitRows(hunk: DiffHunk): SplitRow[] {
    const rows: SplitRow[] = [];
    let bi = 0;
    let ai = 0;

    let i = 0;
    const lines = hunk.lines;

    while (i < lines.length) {
      const line = lines[i];

      if (line.kind === "context") {
        rows.push({ oldLine: line, newLine: line, oldBeforeIdx: bi, oldAfterIdx: ai, newBeforeIdx: bi, newAfterIdx: ai });
        bi++;
        ai++;
        i++;
        continue;
      }

      // Collect a contiguous run of del/add lines and pair them up.
      const dels: Array<{ line: DiffLine; bi: number }> = [];
      const adds: Array<{ line: DiffLine; ai: number }> = [];

      while (i < lines.length && (lines[i].kind === "del" || lines[i].kind === "add")) {
        if (lines[i].kind === "del") {
          dels.push({ line: lines[i], bi });
          bi++;
        } else {
          adds.push({ line: lines[i], ai });
          ai++;
        }
        i++;
      }

      const maxLen = Math.max(dels.length, adds.length);
      for (let j = 0; j < maxLen; j++) {
        const d = dels[j] ?? null;
        const a = adds[j] ?? null;
        rows.push({
          oldLine: d?.line ?? null,
          newLine: a?.line ?? null,
          oldBeforeIdx: d?.bi ?? 0,
          oldAfterIdx: 0,      // dels use before-tokens, index not used for after
          newBeforeIdx: 0,
          newAfterIdx: a?.ai ?? 0,
        });
      }
    }

    return rows;
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
        <div class="diff-table-wrap">
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
            <tbody>
              {#each file.hunks as hunk, hi (hi)}
                <!-- Hunk header row -->
                <tr class="hunk-header-row">
                  {#if appState.diffSplit}
                    <td class="hunk-header-cell" colspan="4">
                      <span class="hunk-range">{hunk.header}</span>
                      {#if onStageHunk || onUnstageHunk}
                        {#if onStageHunk && !staged}
                          <button class="hunk-btn" onclick={() => onStageHunk!(hi)}>Stage hunk</button>
                        {/if}
                        {#if onUnstageHunk && staged}
                          <button class="hunk-btn" onclick={() => onUnstageHunk!(hi)}>Unstage hunk</button>
                        {/if}
                      {/if}
                      {#if selCount(fi, hi) > 0}
                        {#if !staged && onStageLines}
                          <button class="hunk-btn primary" onclick={() => applyLines(fi, hi)}>Stage {selCount(fi, hi)} line(s)</button>
                        {/if}
                        {#if staged && onUnstageLines}
                          <button class="hunk-btn primary" onclick={() => applyLines(fi, hi)}>Unstage {selCount(fi, hi)} line(s)</button>
                        {/if}
                      {/if}
                    </td>
                  {:else}
                    <td class="gutter" colspan="2"></td>
                    <td class="hunk-header-cell">
                      <span class="hunk-range">{hunk.header}</span>
                      {#if onStageHunk || onUnstageHunk}
                        {#if onStageHunk && !staged}
                          <button class="hunk-btn" onclick={() => onStageHunk!(hi)}>Stage hunk</button>
                        {/if}
                        {#if onUnstageHunk && staged}
                          <button class="hunk-btn" onclick={() => onUnstageHunk!(hi)}>Unstage hunk</button>
                        {/if}
                      {/if}
                      {#if selCount(fi, hi) > 0}
                        {#if !staged && onStageLines}
                          <button class="hunk-btn primary" onclick={() => applyLines(fi, hi)}>Stage {selCount(fi, hi)} line(s)</button>
                        {/if}
                        {#if staged && onUnstageLines}
                          <button class="hunk-btn primary" onclick={() => applyLines(fi, hi)}>Unstage {selCount(fi, hi)} line(s)</button>
                        {/if}
                      {/if}
                    </td>
                  {/if}
                </tr>

                {#if !appState.diffSplit}
                  <!-- ── UNIFIED view ── -->
                  {@const ords = hunkOrdinals(hunk)}
                  {#each indexHunkLines(hunk) as { line, beforeIdx, afterIdx } (line.oldNo ?? `a${line.newNo}`)}
                    {@const toks = lineTokens(fi, hi, line, beforeIdx, afterIdx)}
                    {@const ord = line.kind !== "context" ? ords.get(line) : undefined}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <tr
                      class="diff-row {line.kind}"
                      class:selected={ord !== undefined && isSelected(fi, hi, ord)}
                      style={line.kind !== "context" ? "cursor: pointer" : ""}
                      onclick={() => { if (ord !== undefined) toggleLine(fi, hi, ord); }}
                      onkeydown={(e) => { if (ord !== undefined && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); toggleLine(fi, hi, ord); } }}
                    >
                      <td class="gutter old-gutter">{line.oldNo ?? ""}</td>
                      <td class="gutter new-gutter">{line.newNo ?? ""}</td>
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
                  {@const ords = hunkOrdinals(hunk)}
                  {#each toSplitRows(hunk) as row, ri (ri)}
                    {@const oldOrd = row.oldLine?.kind === "del" ? ords.get(row.oldLine) : undefined}
                    {@const newOrd = row.newLine?.kind === "add" ? ords.get(row.newLine) : undefined}
                    <tr class="diff-row split-row">
                      <!-- Old side -->
                      <td class="gutter old-gutter">{row.oldLine?.oldNo ?? ""}</td>
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <td
                        class="diff-cell split-cell"
                        class:del={row.oldLine?.kind === "del"}
                        class:context={row.oldLine?.kind === "context"}
                        class:selected={oldOrd !== undefined && isSelected(fi, hi, oldOrd)}
                        style={row.oldLine?.kind === "del" ? "cursor: pointer" : ""}
                        onclick={() => { if (oldOrd !== undefined) toggleLine(fi, hi, oldOrd); }}
                        onkeydown={(e) => { if (oldOrd !== undefined && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); toggleLine(fi, hi, oldOrd); } }}
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

                      <!-- New side -->
                      <td class="gutter new-gutter">{row.newLine?.newNo ?? ""}</td>
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <td
                        class="diff-cell split-cell"
                        class:add={row.newLine?.kind === "add"}
                        class:context={row.newLine?.kind === "context"}
                        class:selected={newOrd !== undefined && isSelected(fi, hi, newOrd)}
                        style={row.newLine?.kind === "add" ? "cursor: pointer" : ""}
                        onclick={() => { if (newOrd !== undefined) toggleLine(fi, hi, newOrd); }}
                        onkeydown={(e) => { if (newOrd !== undefined && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); toggleLine(fi, hi, newOrd); } }}
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
              {/each}
            </tbody>
          </table>
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
    font-family: ui-monospace, SFMono-Regular, Menlo, "Cascadia Code", monospace;
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
    color: #fff;
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
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
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
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
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
  .diff-table-wrap {
    overflow: auto;
    flex: 1 1 auto;
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
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    color: var(--accent);
    margin-right: auto;
  }

  /* ── Stage/unstage hunk buttons ─────────────────────────────────────────────── */
  .hunk-btn {
    padding: 1px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .hunk-btn:hover {
    background: var(--btn-hover);
  }
  .hunk-btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .hunk-btn.primary:hover {
    opacity: 0.88;
  }

  /* ── Line-level selection highlight ─────────────────────────────────────────── */
  .diff-row.selected {
    box-shadow: inset 2px 0 0 var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }
  .split-cell.selected {
    box-shadow: inset 2px 0 0 var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }

  /* ── Mono utility ───────────────────────────────────────────────────────────── */
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, "Cascadia Code", monospace;
  }
</style>
