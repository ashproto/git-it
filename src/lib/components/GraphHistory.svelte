<script lang="ts">
  import { appState } from "../store.svelte";
  import { parseISO, formatCommitDate } from "../dates";
  import { contextMenu } from "../contextMenu.svelte";
  import { dialogs } from "../dialogs.svelte";
  import { gitActions } from "../gitActions";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import GraphGutter from "./GraphGutter.svelte";

  const rowHeight = 30;
  let anchorIndex = $state<number | null>(null);

  const commits = $derived(appState.graphCommits);
  const rows = $derived(appState.rows);
  const heads = $derived(commits.map((c) => c.refs.some((r) => r.is_head)));
  const gutterWidth = $derived(
    12 + Math.max(1, rows.reduce((m, r) => Math.max(m, r.width), 1)) * 16,
  );

  function localDate(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat) : iso;
  }
  function newDateLabel(sha: string): string {
    const d = appState.newDates.get(sha);
    return d ? formatCommitDate(d, appState.dateFormat) : "";
  }

  function onRowContext(event: MouseEvent, sha: string, index: number) {
    event.preventDefault();
    appState.setCurrent(sha);
    appState.selected = new Set([sha]);
    anchorIndex = index;
    const short = sha.slice(0, 9);
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
</script>

<CollapsiblePanel title="Commits">
  {#snippet headerActions()}
    <span class="count">{appState.selected.size} selected of {commits.length}</span>
    <div class="seg" role="group" aria-label="Graph line style">
      <button
        type="button"
        class:active={appState.graphLineStyle === "curved"}
        onclick={() => appState.setGraphLineStyle("curved")}
      >Curved</button>
      <button
        type="button"
        class:active={appState.graphLineStyle === "angular"}
        onclick={() => appState.setGraphLineStyle("angular")}
      >Angular</button>
    </div>
    <button type="button" onclick={selectAll}>Select all</button>
    <button type="button" onclick={clearSel}>Clear</button>
  {/snippet}

  <div class="wrap">
    <div class="head-row" style={`padding-left:${gutterWidth}px`}>
      <span class="h subject">Description</span>
      <span class="h author">Author</span>
      <span class="h date">Date</span>
      <span class="h sha">Commit</span>
      <span class="h newdate">New date</span>
    </div>

    <div class="history">
      <div class="gutter-layer" style={`width:${gutterWidth}px`}>
        <GraphGutter {rows} {heads} {rowHeight} lineStyle={appState.graphLineStyle} />
      </div>

      {#each commits as commit, i (commit.sha)}
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
          </div>
          <div class="author">{commit.author_name}</div>
          <div class="date mono">{localDate(commit.author_date)}</div>
          <div class="sha mono">{commit.sha.slice(0, 9)}</div>
          <div class="newdate mono">{newDateLabel(commit.sha)}</div>
        </div>
      {/each}

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
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .seg button {
    padding: 3px 9px;
    border: none;
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .seg button.active {
    background: var(--accent);
    color: #fff;
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
    max-height: 440px;
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
  }
  .row:hover {
    background: var(--row-hover);
  }
  .row.selected {
    background: var(--row-selected);
  }
  .spacer {
    flex: 0 0 auto;
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
  .newdate {
    flex: 0 0 168px;
    color: var(--accent);
    padding-right: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row.edited .newdate {
    font-weight: 600;
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
</style>
