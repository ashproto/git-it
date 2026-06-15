<script lang="ts">
  import { appState } from "../store.svelte";
  import { parseISO, formatCommitDate } from "../dates";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  // Anchor index for shift-click range selection.
  let anchorIndex = $state<number | null>(null);

  // These read appState.dateFormat, so Svelte 5 re-invokes them (and re-renders
  // the cells) whenever the user changes a display toggle.
  function localDate(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat) : iso;
  }

  function newDateLabel(sha: string): string {
    const d = appState.newDates.get(sha);
    return d ? formatCommitDate(d, appState.dateFormat) : "";
  }

  function onRowClick(event: MouseEvent, sha: string, index: number) {
    // Prevent the text-selection that comes with click-drag in a table.
    event.preventDefault();
    const cmd = event.metaKey || event.ctrlKey;
    const shift = event.shiftKey;

    if (shift && anchorIndex !== null) {
      // Range select from anchor to current.
      const [lo, hi] =
        anchorIndex <= index ? [anchorIndex, index] : [index, anchorIndex];
      const next = new Set<string>(cmd ? appState.selected : []);
      for (let i = lo; i <= hi; i++) next.add(appState.commits[i].sha);
      appState.selected = next;
    } else if (cmd) {
      // Toggle this row in the existing selection.
      const next = new Set(appState.selected);
      if (next.has(sha)) next.delete(sha);
      else next.add(sha);
      appState.selected = next;
      anchorIndex = index;
    } else {
      // Plain click: replace selection.
      appState.selected = new Set([sha]);
      anchorIndex = index;
    }
  }

  function selectAll() {
    appState.selectAll();
    anchorIndex = appState.commits.length > 0 ? 0 : null;
  }
  function clearSel() {
    appState.clearSelection();
    anchorIndex = null;
  }
</script>

<CollapsiblePanel title="Commits">
  {#snippet headerActions()}
    <span class="count">{appState.selected.size} selected of {appState.commits.length}</span>
    <button type="button" onclick={selectAll}>Select all</button>
    <button type="button" onclick={clearSel}>Clear</button>
  {/snippet}
  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th class="sha">SHA</th>
          <th>Author</th>
          <th>Author date</th>
          <th>Committer date</th>
          <th>Subject</th>
          <th>New date</th>
        </tr>
      </thead>
      <tbody>
        {#each appState.commits as commit, i (commit.sha)}
          <tr
            class:selected={appState.selected.has(commit.sha)}
            class:edited={appState.newDates.has(commit.sha)}
            onmousedown={(e) => onRowClick(e, commit.sha, i)}
          >
            <td class="sha mono">{commit.sha.slice(0, 12)}</td>
            <td>{commit.author_name}</td>
            <td class="mono">{localDate(commit.author_date)}</td>
            <td class="mono">{localDate(commit.committer_date)}</td>
            <td class="subject">{commit.subject}</td>
            <td class="mono new-date">{newDateLabel(commit.sha)}</td>
          </tr>
        {/each}
        {#if appState.commits.length === 0}
          <tr>
            <td colspan="6" class="empty">
              No commits loaded. Pick a repository and click Reload.
            </td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>
  <p class="hint">
    Click to select · ⌘-click to add/remove · Shift-click to select range
  </p>
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
  .table-wrap {
    max-height: 380px;
    overflow: auto;
    border-radius: 6px;
    border: 1px solid var(--border);
    /* Stop click-drag from selecting cell text mid-row. */
    user-select: none;
    -webkit-user-select: none;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  thead {
    position: sticky;
    top: 0;
    background: var(--header-bg);
    z-index: 1;
  }
  th {
    text-align: left;
    padding: 8px 10px;
    font-weight: 600;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 360px;
    /* Keep the left border space stable so layout doesn't shift on selection. */
    border-left: 3px solid transparent;
  }
  td:first-child {
    border-left-width: 3px;
  }
  td.subject {
    max-width: 400px;
  }
  tr {
    cursor: pointer;
  }
  tr:hover {
    background: var(--row-hover);
  }
  tr.selected {
    background: var(--row-selected);
  }
  tr.selected td {
    color: var(--text);
    font-weight: 500;
  }
  tr.selected td:first-child {
    border-left-color: var(--row-selected-border);
  }
  tr.edited td.new-date {
    color: var(--accent);
    font-weight: 600;
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
