<script lang="ts">
  import { untrack } from "svelte";
  import { appState } from "../store.svelte";
  import { graphView } from "../graphView.svelte";
  import { parseISO, formatCommitDate } from "../dates";
  import { contextMenu } from "../contextMenu.svelte";
  import { dialogs } from "../dialogs.svelte";
  import { gitActions, loadMoreGraph } from "../gitActions";
  import { panelAnim } from "../panelAnim.svelte";
  import { amendDialog } from "../amendDialog.svelte";
  import { rebaseEditor } from "../rebaseEditor.svelte";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import GraphGutter from "./GraphGutter.svelte";
  import RefIcon from "./RefIcon.svelte";
  import { LANE_WIDTH, OFFSET_X, commitWindow, laneX, type GeomConfig } from "../graph";

  // Collapsed state is lifted to the parent (+page) so the slide-up details pane can
  // grow to fill the freed space when the graph is collapsed.
  // searchHits/searchActiveRow: ⌘F match highlighting, passed down as commit
  // INDICES (data, not DOM) so it survives the row virtualization below.
  let {
    collapsed = $bindable(false),
    searchHits = null,
    searchActiveRow = -1,
  }: {
    collapsed?: boolean;
    searchHits?: Set<number> | null;
    searchActiveRow?: number;
  } = $props();

  const rowHeight = 30;
  // Overscan rows above/below the viewport so fast scrolling never reveals a gap.
  const BUFFER = 8;
  let anchorIndex = $state<number | null>(null);

  // ── Task 9: one-shot "graph builds itself" reveal ───────────────────────────
  // While appState.revealing (armed once per repo-identity change / same-repo
  // view switch — see store.svelte.ts), the scroll viewport (.wrap) plays a
  // single bottom→top clip-path wipe (.nerv-graph-reveal, nerv-motion.css),
  // double-gated on [data-theme="nerv"][data-motion="on"] + reduced-motion, so
  // Classic / motion-off / reduced-motion render instantly.
  const revealing = $derived(appState.revealing);

  // Virtualization: only the rows in [winStart, winEnd) are in the DOM; two
  // spacer divs reserve the height of the rows above/below so the scrollbar,
  // scroll position and infinite-scroll trigger behave exactly as a full list.
  let wrapEl = $state<HTMLElement>();
  let headEl = $state<HTMLElement>();
  let histEl = $state<HTMLElement>();

  // NERV timeline reveal — genuine per-element draw (nerv-motion.css): each gutter
  // edge stroke-draws (.nerv-edge-draw) and each dot pops (.nerv-node-in), and each
  // commit row fades in (.nerv-row-in), staggered bottom→top so the whole timeline
  // reads as one continuous line growing upward with the entries arriving under it.
  // `revealBottomIndex` (the last row of the first viewport) gets delay 0; rows above
  // it climb by `revealStep` ms each. Both are captured once when `revealing` flips on
  // (untracked geometry read) so mid-reveal scrolling doesn't recompute the stagger.
  // Gating (theme/motion/reduced-motion) lives entirely in the CSS classes, so these
  // values are inert in Classic / motion-off.
  let revealBottomIndex = $state(0);
  let revealStep = $state(20);
  // Base delay (store): on a repo-switch materialize the draw is pushed past the frame,
  // while `revealing` is already true so the rows carry the hidden from-state immediately
  // (no flash of the timeline at rest). 0 on a view switch → draw plays now.
  const revealBase = $derived(appState.revealBaseMs);
  const revealRowDelay = (i: number) => revealBase + Math.max(0, revealBottomIndex - i) * revealStep;
  $effect(() => {
    if (!revealing) return;
    untrack(() => {
      if (!wrapEl) return;
      const visRows = Math.max(1, Math.ceil(wrapEl.clientHeight / rowHeight));
      // Anchor the sweep's delay-0 origin to the bottom of the VISIBLE CONTENT, not
      // the viewport bottom: with few commits in a tall pane, the last real commit
      // (commits.length-1) is well above the fold, so anchoring to the viewport would
      // waste the first ~½s sweeping empty space below it before anything animates.
      revealBottomIndex = Math.max(0, Math.min(winStart + visRows - 1, commits.length - 1));
      // Base the stagger on the rows actually swept so short graphs stay visibly
      // staggered (not instant) and tall ones stay snappy (~≤600ms of stagger).
      const sweptRows = Math.max(1, revealBottomIndex - winStart + 1);
      revealStep = Math.min(24, Math.max(10, Math.floor(600 / sweptRows)));
    });
  });

  let winStart = $state(0);
  // Seed a generous initial window so the very first paint shows rows before the
  // geometry effect refines the range (avoids an empty flash on mount).
  let winEnd = $state(50);
  // Jump-to-commit requests are tracked by a monotonic nonce on the graphView
  // singleton, which outlives this component. Capture the value seen at mount so a
  // stale request from before this mount (e.g. after a repo switch remounts the
  // graph) doesn't auto-scroll; only act on nonces newer than this.
  let seenJumpNonce = graphView.nonce;

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
  const autoGutterWidth = $derived(
    OFFSET_X + Math.max(1, rows.reduce((m, r) => Math.max(m, r.width), 1)) * LANE_WIDTH,
  );
  // G3c: the user can widen the graph area past the auto lane width by dragging
  // the graph↔description boundary. graphWidth=0 ⇒ auto; clamped so the rendered
  // width never drops below the auto lane width (lanes are never clipped).
  const gutterWidth = $derived(
    appState.graphWidth > 0 ? Math.max(autoGutterWidth, appState.graphWidth) : autoGutterWidth,
  );

  // ── G2: lane line into the synthetic "Uncommitted changes" row ──────────────
  // The gutter SVG starts at top:wcOffset, so HEAD's lane line only descends FROM
  // HEAD's dot — it never reaches the wc-row above it. This contained overlay draws
  // a vertical segment from the wc-row node down to HEAD's dot so the uncommitted
  // changes visibly belong to HEAD's lineage. It positions by ABSOLUTE pixel y
  // (headRowIndex, not the virtualization window), so it stays aligned even when
  // HEAD's row is scrolled out of the rendered DOM window.
  const geom: GeomConfig = $derived({
    laneWidth: LANE_WIDTH,
    rowHeight,
    offsetX: OFFSET_X,
  });
  const headRowIndex = $derived(commits.findIndex((c) => c.refs.some((r) => r.is_head)));
  const headLane = $derived(headRowIndex >= 0 ? (rows[headRowIndex]?.lane ?? null) : null);
  const headColorIndex = $derived(
    headRowIndex >= 0 ? (rows[headRowIndex]?.colorIndex ?? null) : null,
  );
  const showWcConnector = $derived(
    hasWorkingChanges && headRowIndex >= 0 && headLane != null,
  );
  // Coordinates (only valid when showWcConnector): x = HEAD's lane centre; topY =
  // the wc-row dot centre (first row); headDotY = HEAD's dot centre, one row per
  // commit below the wc-row offset.
  const wcConnX = $derived(headLane != null ? laneX(headLane, geom) : 0);
  const wcConnTopY = rowHeight / 2;
  const wcConnHeadY = $derived(wcOffset + headRowIndex * rowHeight + rowHeight / 2);
  const wcConnColor = $derived(
    headColorIndex != null ? appState.colorForIndex(headColorIndex) : "var(--accent)",
  );

  // ── G3b: resizable commit-list columns ──────────────────────────────────────
  // The three fixed columns are driven by CSS custom properties on .wrap so a width
  // change updates the sticky header AND every (virtualized) row at once.
  const colVars = $derived(
    `--col-author:${appState.commitColWidths.author}px;` +
      `--col-date:${appState.commitColWidths.date}px;` +
      `--col-sha:${appState.commitColWidths.sha}px`,
  );

  function startColResize(e: PointerEvent, key: "author" | "date" | "sha") {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = appState.commitColWidths[key];
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    // The handle sits on each fixed column's LEFT edge. The fixed columns are
    // right-anchored (the flexible Description column absorbs the slack), so a
    // column grows when its left edge is dragged LEFT — hence `startW - delta`.
    const onMove = (ev: PointerEvent) =>
      appState.setCommitColWidth(key, startW - (ev.clientX - startX));
    const onUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }
  // Resize the graph gutter (graph↔description boundary). The gutter is
  // left-anchored, so it grows when the handle is dragged RIGHT — hence `+ delta`
  // (the opposite sign from the right-anchored data columns above).
  function startGutterResize(e: PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = gutterWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    const onMove = (ev: PointerEvent) =>
      appState.setGraphWidth(startW + (ev.clientX - startX));
    const onUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }
  const COL_DEFAULTS = { author: 110, date: 168, sha: 84 } as const;

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
    // Freeze the windowing while a panel is mid-collapse: the panel-height animation
    // resizes wrapEl every frame (firing the ResizeObserver), and re-windowing + the lane
    // SVG rebuild per frame is what makes the collapse choppy. The `$effect` below reads
    // panelAnim.active, so it re-runs and recomputes once the animation clears.
    if (panelAnim.active) return;
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

  // Re-window on mount and whenever the data set or wc-row presence changes. This
  // effect tracks the reactive reads inside recomputeWindow (commits.length,
  // wcOffset, the bound refs); the geometry reads are non-reactive. It can't loop:
  // it writes winStart/winEnd but never reads them.
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
  // Ignore any nonce at-or-below the mount-time value (stale request / remount).
  $effect(() => {
    const n = graphView.nonce;
    if (n === seenJumpNonce) return;
    seenJumpNonce = n;
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

<CollapsiblePanel title="Commits" fill bind:collapsed>
  {#snippet headerActions()}
    <span class="count">{appState.selected.size} selected of {commits.length}</span>
    <button type="button" onclick={selectAll}>Select all</button>
    <button type="button" onclick={clearSel}>Clear</button>
  {/snippet}

  <div
    class="wrap"
    bind:this={wrapEl}
    onscroll={onWrapScroll}
    style={colVars}
  >
    <div class="head-row" bind:this={headEl} style={`padding-left:${gutterWidth}px`}>
      <span class="h subject"
        >Description<span
          class="col-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize graph column"
          title="Drag to resize the graph · double-click to reset"
          onpointerdown={startGutterResize}
          ondblclick={() => appState.setGraphWidth(0)}
        ></span></span>
      <span class="h author"
        >Author<span
          class="col-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize Author column"
          title="Drag to resize · double-click to reset"
          onpointerdown={(e) => startColResize(e, "author")}
          ondblclick={() => appState.setCommitColWidth("author", COL_DEFAULTS.author)}
        ></span></span>
      <span class="h date"
        >Date<span
          class="col-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize Date column"
          title="Drag to resize · double-click to reset"
          onpointerdown={(e) => startColResize(e, "date")}
          ondblclick={() => appState.setCommitColWidth("date", COL_DEFAULTS.date)}
        ></span></span>
      <span class="h sha"
        >Commit<span
          class="col-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize Commit column"
          title="Drag to resize · double-click to reset"
          onpointerdown={(e) => startColResize(e, "sha")}
          ondblclick={() => appState.setCommitColWidth("sha", COL_DEFAULTS.sha)}
        ></span></span>
    </div>

    <div class="history" bind:this={histEl}>
      <div class="gutter-layer" style={`width:${gutterWidth}px; top:${wcOffset}px`}>
        <GraphGutter
          {rows}
          {heads}
          {rowHeight}
          lineStyle={appState.effectiveGraphLineStyle}
          mergeInStyle={appState.graphMergeInStyle}
          curviness={appState.graphCurviness}
          renderStart={winStart}
          renderEnd={winEnd}
          colorOf={(idx) => appState.colorForIndex(idx)}
          reveal={revealing}
          {revealBottomIndex}
          {revealStep}
          {revealBase}
        />
      </div>

      {#if showWcConnector}
        <svg
          class="wc-connector"
          style={`left:0; top:0; width:${gutterWidth}px; height:${wcConnHeadY + rowHeight}px`}
          width={gutterWidth}
          height={wcConnHeadY + rowHeight}
          viewBox={`0 0 ${gutterWidth} ${wcConnHeadY + rowHeight}`}
          aria-hidden="true"
        >
          <line
            x1={wcConnX}
            y1={wcConnTopY}
            x2={wcConnX}
            y2={wcConnHeadY}
            stroke={wcConnColor}
            stroke-width="2"
          />
          <circle
            cx={wcConnX}
            cy={wcConnTopY}
            r="4.5"
            fill="var(--panel-bg)"
            stroke={wcConnColor}
            stroke-width="2"
          />
        </svg>
      {/if}

      {#if hasWorkingChanges}
        <div
          class="row wc-row"
          class:selected={appState.workingCopySelected}
          class:nerv-row-in={revealing}
          role="row"
          tabindex="0"
          style={`height:${rowHeight}px${revealing ? `;animation-delay:${revealRowDelay(-1)}ms` : ""}`}
          onmousedown={selectWorkingCopy}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") selectWorkingCopy(); }}
        >
          <div class="spacer" style={`width:${gutterWidth}px`}></div>
          <div class="subject wc-subject">
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
          class:search-hit={searchHits?.has(i) ?? false}
          class:search-active={i === searchActiveRow}
          class:nerv-row-in={revealing}
          style={`height:${rowHeight}px${revealing ? `;animation-delay:${revealRowDelay(i)}ms` : ""}`}
          onmousedown={(e) => onRowMouseDown(e, commit.sha, i)}
          oncontextmenu={(e) => onRowContext(e, commit.sha, i)}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") onRowMouseDown(e as unknown as MouseEvent, commit.sha, i); }}
        >
          <div class="spacer" style={`width:${gutterWidth}px`}></div>
          <div class="subject">
            {#each commit.refs as r}
              <span
                class="badge {r.kind}"
                class:current={r.is_head}
                style={`--ref-color:${appState.colorForRef(r.name, commit.sha)}`}
              ><RefIcon kind={r.kind} />{r.name}</span>
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
    border-radius: var(--radius-md);
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
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    overflow: auto;
    /* Fill the (fill-mode) panel body so the graph occupies the full timeline
       height; the commit list scrolls inside. min-height:0 lets it shrink. */
    flex: 1;
    min-height: 0;
    user-select: none;
    -webkit-user-select: none;
  }
  /* Narrow/stacked layout (≤900px): the shell reverts to content height, so the
     graph has no definite parent height to fill — give it a floor so it stays
     visible and the page scrolls, instead of collapsing toward its min-content. */
  @media (max-width: 900px) {
    .wrap {
      min-height: 360px;
    }
  }
  .head-row {
    display: flex;
    align-items: center;
    position: sticky;
    top: 0;
    z-index: 2;
    /* Use the more-opaque popover token (≈0.85–0.88 alpha in glass mode) instead
       of the translucent --header-bg so the column titles stay clearly legible as
       commit rows scroll underneath. No backdrop-filter: this sticky header is an
       always-on surface, and (being nearly opaque) it needs no blur — keeping one
       here would re-blur the whole header every frame of any animation. */
    background: var(--popover-bg, var(--header-bg));
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
  /* ⌘F search: matching rows carry an accent tint; the ACTIVE match is stronger
     and adds an inset ring. Declared after .selected/.edited so a search
     highlight stays visible on a selected row. */
  .row.search-hit {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .row.search-active {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .spacer {
    flex: 0 0 auto;
  }
  /* Virtualization height reservers for the off-window rows above/below. Forced
     block + full width so their inline height is exact and a future change to
     .history layout can't silently collapse them. */
  .spacer-v {
    display: block;
    width: 100%;
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
    flex: 0 0 var(--col-author, 110px);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 8px;
  }
  .date {
    flex: 0 0 var(--col-date, 168px);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sha {
    flex: 0 0 var(--col-sha, 84px);
    color: var(--text-muted);
  }
  /* Header cells mirror the row column bases so the sticky header lines up with the
     virtualized rows; position:relative anchors each column's right-edge drag handle. */
  .head-row .h {
    position: relative;
  }
  .head-row .h.subject {
    flex: 1 1 auto;
    min-width: 160px;
  }
  .head-row .h.author {
    flex: 0 0 var(--col-author, 110px);
    padding: 0 8px;
  }
  .head-row .h.date {
    flex: 0 0 var(--col-date, 168px);
  }
  .head-row .h.sha {
    flex: 0 0 var(--col-sha, 84px);
  }
  /* Column resize handle: a thin grabbable strip on the right edge of each header
     cell. Mirrors the sidebar resize-handle visual (1px gutter → accent on hover). */
  .col-resize-handle {
    /* Sits centred on the column's LEFT edge (the boundary with the previous
       column) so dragging a visible divider resizes the column to its right. */
    position: absolute;
    top: 0;
    left: -3px;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    touch-action: none;
    z-index: 3;
  }
  .col-resize-handle::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 1px;
    background: var(--border);
    transform: translateX(-50%);
    transition: background 0.1s, width 0.1s;
  }
  .col-resize-handle:hover::before {
    background: var(--accent);
    width: 2px;
  }
  .new-pill {
    /* Show the full previewed date — never clip the end (e.g. the tz offset).
       The .subject row clips overall, and .msg ellipsis-truncates first, so a
       wide pill can't break the layout. */
    flex: 0 0 auto;
    margin-left: 6px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent);
    color: var(--accent);
    font-size: 11px;
    line-height: 1.6;
    white-space: nowrap;
  }
  .badge {
    /* Colour matches the ref's graph lane (or manual override) via --ref-color;
       the RefIcon glyph conveys local / remote / tag. */
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex: 0 0 auto;
    font-size: 11px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--ref-color, var(--border));
    color: var(--ref-color, var(--text-muted));
    white-space: nowrap;
  }
  /* HEAD's branch: keep its lane colour but mark "you are here" with a heavier
     weight + a faint fill of that same colour. */
  .badge.current {
    font-weight: 600;
    background: color-mix(in srgb, var(--ref-color, var(--accent)) 16%, transparent);
  }
  .mono {
    font-family: var(--font-mono);
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
    flex: 0 0 auto;
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
  /* G2: lane line continuing up from HEAD's dot into the wc-row's node. Absolutely
     positioned inside .history (position:relative), one z-layer above the gutter. */
  .wc-connector {
    position: absolute;
    pointer-events: none;
    z-index: 1;
    display: block;
    overflow: visible;
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
