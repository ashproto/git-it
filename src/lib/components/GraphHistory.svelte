<script lang="ts">
  import { untrack } from "svelte";
  import { appState } from "../store.svelte";
  import { graphView } from "../graphView.svelte";
  import { parseISO, formatCommitDate } from "../dates";
  import { contextMenu } from "../contextMenu.svelte";
  import { dialogs } from "../dialogs.svelte";
  import { gitActions, loadMoreGraph } from "../gitActions";
  import { amendDialog } from "../amendDialog.svelte";
  import { rebaseEditor } from "../rebaseEditor.svelte";
  import { timeEditDrawer } from "../timeEditDrawer.svelte";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import GraphGutter from "./GraphGutter.svelte";
  import { LANE_WIDTH, OFFSET_X, commitWindow } from "../graph";

  const rowHeight = 30;
  // Overscan rows above/below the viewport so fast scrolling never reveals a gap.
  const BUFFER = 8;
  let anchorIndex = $state<number | null>(null);

  // Virtualization: only the rows in [winStart, winEnd) are in the DOM; two
  // spacer divs reserve the height of the rows above/below so the scrollbar,
  // scroll position and infinite-scroll trigger behave exactly as a full list.
  let wrapEl = $state<HTMLElement>();
  let headEl = $state<HTMLElement>();
  let histEl = $state<HTMLElement>();
  let winStart = $state(0);
  // Seed a generous initial window so the very first paint shows rows before the
  // geometry effect refines the range (avoids an empty flash on mount).
  let winEnd = $state(50);

  const commits = $derived(appState.graphCommits);
  const rows = $derived(appState.rows);

  // Synthetic "Uncommitted changes" row: visible whenever there are any working
  // changes (by count) OR repoStatus signals changes.
  const hasWorkingChanges = $derived(
    appState.workingChanges.length > 0 ||
      (appState.repoStatus !== null &&
        (appState.repoStatus.staged +
          appState.repoStatus.unstaged +
          appState.repoStatus.untracked +
          appState.repoStatus.conflicted) >
          0),
  );
  const workingChangeCount = $derived(
    Math.max(
      appState.workingChanges.length,
      appState.repoStatus !== null
        ? appState.repoStatus.staged +
            appState.repoStatus.unstaged +
            appState.repoStatus.untracked +
            appState.repoStatus.conflicted
        : 0,
    ),
  );

  // The synthetic wc-row occupies the first row slot (height = rowHeight) when
  // present, so the commit list — and the gutter SVG overlay — start one row down.
  const wcOffset = $derived(hasWorkingChanges ? rowHeight : 0);

  function selectWorkingCopy() {
    appState.setWorkingCopySelected(true);
    anchorIndex = null;
  }
  const heads = $derived(commits.map((c) => c.refs.some((r) => r.is_head)));
  const gutterWidth = $derived(
    OFFSET_X + Math.max(1, rows.reduce((m, r) => Math.max(m, r.width), 1)) * LANE_WIDTH,
  );

  function localDate(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  function newDateLabel(sha: string): string {
    const d = appState.newDates.get(sha);
    return d ? formatCommitDate(d, appState.dateFormat) : "";
  }

  function currentBranchName(): string {
    return appState.refsByKind.local.find((r) => r.isHead)?.name ?? "HEAD";
  }

  function doReset(sha: string, mode: "soft" | "mixed" | "hard") {
    const branch = currentBranchName();
    const subject = appState.graphCommits.find((c) => c.sha === sha)?.subject ?? "";
    // SHA/subject wording, not a positional count: the graph is date-sorted across
    // branches, so "N commits back" can't be trusted (could read negative/zero).
    const consequence =
      `Reset ${branch} to ${sha.slice(0, 9)}${subject ? ` (${subject})` : ""}` +
      (mode === "hard" ? " — uncommitted changes will be lost." : ".");
    gitActions.reset(sha, mode, consequence);
  }

  function onRowContext(event: MouseEvent, sha: string, index: number) {
    event.preventDefault();
    appState.setCurrent(sha);
    appState.selected = new Set([sha]);
    anchorIndex = index;
    const short = sha.slice(0, 9);
    const commit = commits[index];
    const isHead = commit.refs.some((r) => r.is_head);
    contextMenu.openAt(event.clientX, event.clientY, [
      {
        label: `Checkout ${short} (detached)`,
        action: () => gitActions.checkout(sha, `Checkout ${short}`),
      },
      {
        label: "Create branch here…",
        action: async () => {
          const name = await dialogs.prompt({
            title: "New branch",
            label: "Branch name",
            placeholder: "feature/x",
          });
          if (name) gitActions.createBranch(name, sha);
        },
      },
      {
        label: "Create tag here…",
        action: async () => {
          const name = await dialogs.prompt({
            title: "New tag",
            label: "Tag name",
            placeholder: "v1.0.0",
          });
          if (name) gitActions.createTag(name, sha);
        },
      },
      { separator: true },
      {
        label: "Cherry-pick onto current",
        action: () => gitActions.cherryPick([sha], `Cherry-pick ${short}`),
      },
      {
        label: "Revert commit",
        action: () => gitActions.revert([sha], `Revert ${short}`),
      },
      { separator: true },
      {
        label: `Reset ${currentBranchName()} here (mixed)`,
        action: () => doReset(sha, "mixed"),
      },
      {
        label: `Reset ${currentBranchName()} here (soft)`,
        action: () => doReset(sha, "soft"),
      },
      {
        label: `Reset ${currentBranchName()} here (hard)`,
        danger: true,
        action: () => doReset(sha, "hard"),
      },
      ...(isHead
        ? [
            {
              label: "Amend this commit…",
              action: () => amendDialog.openWith(sha, commit.subject),
            },
          ]
        : []),
      { separator: true },
      {
        label: `Rebase ${currentBranchName()} onto here`,
        action: () =>
          gitActions.rebaseOnto(
            sha,
            `Replay ${currentBranchName()}'s commits onto ${sha.slice(0, 9)}.`,
          ),
      },
      {
        label: "Interactive rebase from here…",
        action: () => rebaseEditor.openWith(sha),
      },
      { separator: true },
      {
        label: "Edit timestamps…",
        action: () => timeEditDrawer.openDrawer(),
      },
      { separator: true },
      { label: "Copy SHA", action: () => navigator.clipboard?.writeText(sha) },
    ]);
  }

  function onRowMouseDown(event: MouseEvent, sha: string, index: number) {
    event.preventDefault();
    appState.setCurrent(sha);
    const cmd = event.metaKey || event.ctrlKey;
    const shift = event.shiftKey;
    if (shift && anchorIndex !== null) {
      const [lo, hi] = anchorIndex <= index ? [anchorIndex, index] : [index, anchorIndex];
      const next = new Set<string>(cmd ? appState.selected : []);
      for (let i = lo; i <= hi; i++) next.add(commits[i].sha);
      appState.selected = next;
    } else if (cmd) {
      const next = new Set(appState.selected);
      if (next.has(sha)) next.delete(sha);
      else next.add(sha);
      appState.selected = next;
      anchorIndex = index;
    } else {
      appState.selected = new Set([sha]);
      anchorIndex = index;
    }
  }

  function selectAll() {
    appState.selectAll();
    anchorIndex = commits.length > 0 ? 0 : null;
  }
  function clearSel() {
    appState.clearSelection();
    anchorIndex = null;
  }

  // Map the measured scroll geometry to a visible commit range. Reads layout
  // directly (getBoundingClientRect) rather than hard-coding the sticky header
  // height, so it stays correct if the chrome changes. The wc-row offset is
  // subtracted because commit 0 starts one row down when it is present.
  function recomputeWindow() {
    if (!wrapEl || !histEl) return;
    const wrapRect = wrapEl.getBoundingClientRect();
    const histTop = histEl.getBoundingClientRect().top;
    const headBottom = headEl ? headEl.getBoundingClientRect().bottom : wrapRect.top;
    const scrolledPx = headBottom - histTop - wcOffset;
    const viewportPx = wrapRect.bottom - headBottom;
    const w = commitWindow(scrolledPx, viewportPx, rowHeight, commits.length, BUFFER);
    winStart = w.start;
    winEnd = w.end;
  }

  // Re-window when the data set or the wc-row's presence changes (and on mount).
  // recomputeWindow reads commits.length and wcOffset, so this effect re-runs
  // exactly when either changes; the geometry reads are non-reactive.
  $effect(() => {
    recomputeWindow();
  });

  // Recompute when the scroll container resizes (panel collapse, window resize,
  // diff pane opening) — a resize with no scroll would otherwise leave a stale window.
  $effect(() => {
    if (!wrapEl) return;
    const ro = new ResizeObserver(() => recomputeWindow());
    ro.observe(wrapEl);
    return () => ro.disconnect();
  });

  // Resolve an external "scroll to commit" request (Sidebar jump-to-ref). Keyed
  // only on the nonce — untrack the rest so appending commits doesn't re-scroll.
  $effect(() => {
    const n = graphView.nonce;
    if (n === 0) return;
    untrack(() => {
      const sha = graphView.requestSha;
      if (!sha || !wrapEl) return;
      const idx = commits.findIndex((c) => c.sha === sha);
      if (idx < 0) return;
      const headH = headEl ? headEl.getBoundingClientRect().height : 0;
      const usable = Math.max(0, wrapEl.clientHeight - headH);
      // Center the target row; the scroll event then renders it via the window.
      const target = wcOffset + idx * rowHeight + rowHeight / 2 - usable / 2;
      wrapEl.scrollTo({ top: Math.max(0, target), behavior: "smooth" });
    });
  });

  function onWrapScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    recomputeWindow();
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 200) {
      loadMoreGraph();
    }
  }
</script>

<CollapsiblePanel title="Commits">
  {#snippet headerActions()}
    <span class="count">{appState.selected.size} selected of {commits.length}</span>
    <button type="button" onclick={selectAll}>Select all</button>
    <button type="button" onclick={clearSel}>Clear</button>
    <button
      type="button"
      class="edit-ts"
      disabled={appState.selected.size === 0}
      title={appState.selected.size === 0 ? "Select one or more commits first" : "Edit timestamps for the selected commits"}
      onclick={() => timeEditDrawer.openDrawer()}
    >Edit timestamps…</button>
  {/snippet}

  <div class="wrap" bind:this={wrapEl} onscroll={onWrapScroll}>
    <div class="head-row" bind:this={headEl} style={`padding-left:${gutterWidth}px`}>
      <span class="h subject">Description</span>
      <span class="h author">Author</span>
      <span class="h date">Date</span>
      <span class="h sha">Commit</span>
    </div>

    <div class="history" bind:this={histEl}>
      <div class="gutter-layer" style={`width:${gutterWidth}px; top:${wcOffset}px`}>
        <GraphGutter
          {rows}
          {heads}
          {rowHeight}
          lineStyle={appState.graphLineStyle}
          renderStart={winStart}
          renderEnd={winEnd}
        />
      </div>

      {#if hasWorkingChanges}
        <div
          class="row wc-row"
          class:selected={appState.workingCopySelected}
          role="row"
          tabindex="0"
          style={`height:${rowHeight}px`}
          onmousedown={selectWorkingCopy}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") selectWorkingCopy(); }}
        >
          <div class="spacer" style={`width:${gutterWidth}px`}></div>
          <div class="subject wc-subject">
            <span class="wc-dot" aria-hidden="true">●</span>
            <span class="msg">Uncommitted changes ({workingChangeCount})</span>
          </div>
          <div class="author"></div>
          <div class="date mono"></div>
          <div class="sha mono"></div>
        </div>
      {/if}

      <div class="spacer-v" style={`height:${winStart * rowHeight}px`}></div>

      {#each commits.slice(winStart, winEnd) as commit, k (commit.sha)}
        {@const i = winStart + k}
        <div
          class="row"
          id={`gc-row-${commit.sha}`}
          role="row"
          tabindex="0"
          class:selected={appState.selected.has(commit.sha)}
          class:edited={appState.newDates.has(commit.sha)}
          style={`height:${rowHeight}px`}
          onmousedown={(e) => onRowMouseDown(e, commit.sha, i)}
          oncontextmenu={(e) => onRowContext(e, commit.sha, i)}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") onRowMouseDown(e as unknown as MouseEvent, commit.sha, i); }}
        >
          <div class="spacer" style={`width:${gutterWidth}px`}></div>
          <div class="subject">
            {#each commit.refs as r}
              <span class="badge {r.kind}" class:current={r.is_head}>{r.name}</span>
            {/each}
            <span class="msg">{commit.subject}</span>
            {#if appState.newDates.has(commit.sha)}
              <span class="new-pill mono" title={`New date: ${newDateLabel(commit.sha)}`}
                >→ {newDateLabel(commit.sha)}</span>
            {/if}
          </div>
          <div class="author">{commit.author_name}</div>
          <div class="date mono">{localDate(commit.author_date)}</div>
          <div class="sha mono">{commit.sha.slice(0, 9)}</div>
        </div>
      {/each}

      <div
        class="spacer-v"
        style={`height:${Math.max(0, commits.length - winEnd) * rowHeight}px`}
      ></div>

      {#if appState.graphLoadingMore}
        <div class="load-hint">Loading more…</div>
      {:else if !appState.graphHasMore && commits.length > 0}
        <div class="load-hint muted">— end of history —</div>
      {/if}

      {#if commits.length === 0}
        <div class="empty">No commits loaded. Pick a repository and click Reload.</div>
      {/if}
    </div>
  </div>
  <p class="hint">Click to select · ⌘-click to add/remove · Shift-click to select range</p>
</CollapsiblePanel>

<style>
  .count {
    color: var(--text-muted);
    font-size: 12px;
  }
  button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
  .wrap {
    border-radius: 6px;
    border: 1px solid var(--border);
    overflow: auto;
    max-height: clamp(360px, 58vh, 900px);
    user-select: none;
    -webkit-user-select: none;
  }
  .head-row {
    display: flex;
    align-items: center;
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--header-bg);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    color: var(--text-muted);
    padding-top: 6px;
    padding-bottom: 6px;
  }
  .history {
    position: relative;
  }
  .gutter-layer {
    position: absolute;
    left: 0;
    top: 0;
    pointer-events: none;
    z-index: 1;
  }
  .row {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    font-size: 12.5px;
    position: relative;
    z-index: 0;
    /* The fixed-height window model (spacers + full-height gutter SVG) assumes
       every row is EXACTLY rowHeight px. border-box folds the 1px border into the
       declared height:30px so rows can't drift 1px/row out of lane alignment. */
    box-sizing: border-box;
  }
  .row:hover {
    background: var(--row-hover);
  }
  .row.selected {
    background: var(--row-selected);
  }
  /* Queued-edit accent: an always-visible left bar so an edited row stays legible
     even when the inline .new-pill is clipped on a long subject / narrow window.
     inset box-shadow (not border-left) avoids shifting the flex cells out of
     alignment with the sticky header's gutter padding. */
  .row.edited {
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .spacer {
    flex: 0 0 auto;
  }
  /* Virtualization height reservers for the off-window rows above/below. Block
     divs (the .history is not a flex container) so their inline height is exact. */
  .spacer-v {
    pointer-events: none;
  }
  .subject {
    flex: 1 1 auto;
    /* Stay readable on narrow windows: keep the description from collapsing to
       zero — the .wrap scrolls horizontally instead of hiding messages. */
    min-width: 160px;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }
  .msg {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .author {
    flex: 0 0 110px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 8px;
  }
  .date {
    flex: 0 0 168px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sha {
    flex: 0 0 84px;
    color: var(--text-muted);
  }
  .new-pill {
    flex: 0 0 auto;
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--accent);
    color: var(--accent);
    font-size: 11px;
    line-height: 1.6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }
  button.edit-ts:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  button.edit-ts:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .badge {
    flex: 0 0 auto;
    font-size: 11px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--text-muted);
    white-space: nowrap;
  }
  .badge.current {
    border-color: var(--accent);
    color: var(--accent);
  }
  .badge.head {
    border-color: var(--text-muted);
    color: var(--text);
    font-weight: 600;
  }
  .badge.tag {
    border-color: var(--err);
    color: var(--err);
  }
  .badge.remote {
    opacity: 0.7;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .empty {
    text-align: center;
    color: var(--text-muted);
    padding: 24px;
    font-style: italic;
  }
  .hint {
    margin: 6px 0 0 0;
    font-size: 11px;
    color: var(--text-muted);
  }

  /* ── Synthetic "Uncommitted changes" row ──────────────────────────────────── */
  .wc-row {
    border-bottom: 1px solid var(--border);
    background: var(--header-bg);
  }
  .wc-row:hover {
    background: var(--row-hover);
  }
  .wc-row.selected {
    background: var(--row-selected);
  }
  .wc-subject {
    gap: 8px;
    font-style: italic;
  }
  .wc-dot {
    font-style: normal;
    color: var(--accent);
    font-size: 10px;
    /* Dashed-look via outline trick — presentational only */
    border: 1.5px dashed var(--accent);
    border-radius: 50%;
    width: 14px;
    height: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    line-height: 1;
  }

  /* ── Infinite-scroll loading / end-of-history hints ──────────────────────── */
  .load-hint {
    text-align: center;
    padding: 8px;
    font-size: 11px;
    color: var(--accent);
  }
  .load-hint.muted {
    color: var(--text-muted);
  }
</style>
