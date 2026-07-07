<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { gitActions } from "../gitActions";
  import { dialogs } from "../dialogs.svelte";
  import type { StashEntry } from "../types";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  let entries = $state<StashEntry[]>([]);
  let collapsed = $state(true);

  // Reload stash list when expanded and whenever working changes identity changes
  // (a stash push/pop/drop refreshes workingChanges, which triggers this).
  $effect(() => {
    // Subscribe to workingChanges and repoStatus to re-fetch after stash ops.
    const _changes = appState.workingChanges;
    const _status = appState.repoStatus;
    if (!collapsed && isTauri() && appState.repo) {
      loadStash();
    }
  });

  async function loadStash() {
    if (!isTauri() || !appState.repo) return;
    try {
      entries = await api.stashList(appState.repo);
    } catch (e) {
      console.warn("[gte] stash list failed", e);
      entries = [];
    }
  }



  async function doStashPush() {
    const msg = await dialogs.prompt({
      title: "Stash changes",
      label: "Message (optional)",
      placeholder: "WIP: …",
    });
    // msg is null if the user cancelled, empty string is fine (no message)
    if (msg === null) return;
    await gitActions.stashPush(msg.trim() || null);
    // workingChanges will be refreshed by gitActions, which triggers the $effect above.
  }

  function doApply(entry: StashEntry) {
    gitActions.stashApply(entry.index);
  }

  function doPop(entry: StashEntry) {
    gitActions.stashPop(entry.index);
  }

  async function doDrop(entry: StashEntry) {
    const ok = await dialogs.confirm({
      title: "Drop stash",
      message: `Drop "${entry.message}"? This cannot be undone.`,
      confirmLabel: "Drop",
      danger: true,
    });
    if (!ok) return;
    gitActions.stashDrop(entry.index);
  }
</script>

<CollapsiblePanel title="Stashes" bind:collapsed>
  {#if !isTauri()}
    <p class="note">Desktop app only — stash is not available in the browser preview.</p>
  {:else}
    <div class="actions-row">
      <button type="button" class="primary" onclick={doStashPush}>Stash changes…</button>
    </div>
    {#if entries.length === 0}
      <p class="note">No stashes. Stash changes to save them temporarily.</p>
    {:else}
      <ul class="list">
        {#each entries as entry (entry.index)}
          <li class="entry">
            <div class="entry-meta">
              <span class="mono idx">stash@&#123;{entry.index}&#125;</span>
              <span class="msg">{entry.message}</span>
            </div>
            <div class="entry-actions">
              <button type="button" onclick={() => doApply(entry)}>Apply</button>
              <button type="button" onclick={() => doPop(entry)}>Pop</button>
              <button type="button" class="danger" onclick={() => doDrop(entry)}>Drop</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</CollapsiblePanel>

<style>
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
  .actions-row {
    margin-bottom: 8px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 240px;
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
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }
  .idx {
    color: var(--text-muted);
    font-size: 11px;
    white-space: nowrap;
  }
  .msg {
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
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
    padding: 4px 12px;
    font-size: 12px;
  }
  button.primary:hover {
    opacity: 0.9;
  }
  button.danger {
    border-color: var(--err, #c0392b);
    color: var(--err, #c0392b);
  }
  button.danger:hover {
    background: var(--err, #c0392b);
    color: var(--on-accent);
  }
</style>
