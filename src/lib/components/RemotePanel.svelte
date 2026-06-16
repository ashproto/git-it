<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";
  import { dialogs } from "../dialogs.svelte";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  let { bare = false } = $props();

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  let collapsed = $state(true);

  // Ellipsize a URL for display: show up to 40 chars, then "…".
  function ellipsize(url: string, maxLen = 40): string {
    return url.length > maxLen ? url.slice(0, maxLen - 1) + "…" : url;
  }

  async function doAddRemote() {
    const name = await dialogs.prompt({
      title: "Add remote",
      label: "Remote name",
      placeholder: "origin",
    });
    if (!name) return;
    const url = await dialogs.prompt({
      title: `Add remote "${name}"`,
      label: "URL",
      placeholder: "https://github.com/…",
    });
    if (!url) return;
    await gitActions.remoteAdd(name, url);
  }

  async function doSetUrl(name: string, currentUrl: string) {
    const url = await dialogs.prompt({
      title: `Set URL for "${name}"`,
      label: "New URL",
      value: currentUrl,
    });
    if (!url || url === currentUrl) return;
    await gitActions.remoteSetUrl(name, url);
  }

  async function doRemove(name: string) {
    const ok = await dialogs.confirm({
      title: "Remove remote",
      message: `Remove remote "${name}"? This only removes the local reference — the remote repository is not affected.`,
      confirmLabel: "Remove",
      danger: true,
    });
    if (!ok) return;
    await gitActions.remoteRemove(name);
  }
</script>

<CollapsiblePanel title="Remotes" bind:collapsed {bare}>
  {#if !isTauri()}
    <p class="note">Desktop app only — remote management is not available in the browser preview.</p>
  {:else}
    <div class="actions-row">
      <button type="button" class="primary" onclick={doAddRemote}>Add remote…</button>
    </div>
    {#if appState.remotes.length === 0}
      <p class="note">No remotes configured. Add one to enable Pull and Push.</p>
    {:else}
      <ul class="list">
        {#each appState.remotes as remote (remote.name)}
          <li class="entry">
            <div class="entry-info">
              <span class="rname">{remote.name}</span>
              <span class="rurl" title={remote.url}>{ellipsize(remote.url)}</span>
            </div>
            <div class="entry-actions">
              <button type="button" onclick={() => doSetUrl(remote.name, remote.url)}>Set URL</button>
              <button type="button" class="danger" onclick={() => doRemove(remote.name)}>Remove</button>
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
    max-height: 200px;
    overflow-y: auto;
  }
  .entry {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 4px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12px;
  }
  .entry:last-child {
    border-bottom: none;
  }
  .entry-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }
  .rname {
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rurl {
    color: var(--text-muted);
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    color: white;
    padding: 4px 12px;
    font-size: 12px;
  }
  button.primary:hover {
    opacity: 0.9;
  }
  button.danger {
    border-color: var(--danger);
    color: var(--danger);
  }
  button.danger:hover {
    background: var(--danger);
    color: white;
  }
</style>
