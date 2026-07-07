<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { gitActions } from "../gitActions";
  import type { ReflogEntry } from "../types";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  let { bare = false } = $props();

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  function currentBranchName(): string {
    return appState.refsByKind.local.find((r) => r.isHead)?.name ?? "HEAD";
  }

  let entries = $state<ReflogEntry[]>([]);
  let collapsed = $state(true);

  // Load reflog when expanded and whenever a destructive op completes (lastUndo changes).
  $effect(() => {
    // Reading appState.lastUndo subscribes this effect to its changes.
    const _lastUndo = appState.lastUndo;
    // When `bare` (inside the Manage Repository modal) the panel is always
    // expanded, so load regardless of `collapsed`.
    if ((bare || !collapsed) && isTauri() && appState.repo) {
      loadReflog();
    }
  });

  async function loadReflog() {
    if (!isTauri() || !appState.repo) return;
    try {
      entries = await api.reflog(appState.repo, 50);
    } catch (e) {
      console.warn("[gte] reflog load failed", e);
      entries = [];
    }
  }

  function onToggle() {
    collapsed = !collapsed;
    if (!collapsed && isTauri() && appState.repo) {
      loadReflog();
    }
  }

  function doResetMixed(entry: ReflogEntry) {
    const branch = currentBranchName();
    gitActions.reset(
      entry.sha,
      "mixed",
      `Reset ${branch} to ${entry.short} (${entry.subject}).`,
    );
  }

  function doResetHard(entry: ReflogEntry) {
    const branch = currentBranchName();
    gitActions.reset(
      entry.sha,
      "hard",
      `Reset ${branch} to ${entry.short} (${entry.subject}).`,
    );
  }
</script>

<CollapsiblePanel title="History (reflog)" bind:collapsed {bare}>
  {#if !isTauri()}
    <p class="note">Desktop app only — reflog is not available in the browser preview.</p>
  {:else if entries.length === 0}
    <p class="note">No reflog entries. Open a repository to see history.</p>
  {:else}
    <ul class="list">
      {#each entries as entry (entry.selector)}
        <li class="entry">
          <div class="entry-meta">
            <span class="mono selector">{entry.selector}</span>
            <span class="mono short">{entry.short}</span>
            <span class="subject">{entry.subject}</span>
          </div>
          <div class="entry-actions">
            <button type="button" class="reset-mixed" onclick={() => doResetMixed(entry)}>
              Reset here
            </button>
            <button type="button" class="reset-hard danger" onclick={() => doResetHard(entry)}>
              hard
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</CollapsiblePanel>

<style>
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 280px;
    overflow-y: auto;
  }
  .entry {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 4px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12px;
  }
  .entry:last-child {
    border-bottom: none;
  }
  .entry-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: baseline;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }
  .selector {
    color: var(--text-muted);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .short {
    color: var(--accent);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .subject {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .entry-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    align-items: center;
  }
  button {
    padding: 3px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
  }
  button:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  button.danger {
    border-color: var(--err, #c0392b);
    color: var(--err, #c0392b);
  }
  button.danger:hover {
    background: var(--err, #c0392b);
    color: white;
  }
</style>
