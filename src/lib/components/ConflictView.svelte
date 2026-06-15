<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { gitActions } from "../gitActions";
  import type { ConflictEntry } from "../types";

  function inTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  const op = $derived(appState.repoStatus?.operation ?? null);
  const conflicted = $derived(appState.repoStatus?.conflicted ?? 0);

  let details = $state<ConflictEntry[]>([]);

  async function refreshDetails() {
    if (!inTauri() || !appState.repo || !op) {
      details = [];
      return;
    }
    try {
      details = await api.conflictDetails(appState.repo);
    } catch {
      details = [];
    }
  }

  // Re-fetch the file list whenever an op is present and the conflict count changes
  // (each resolve decrements it). Reading both signals registers them as deps.
  $effect(() => {
    const o = appState.repoStatus?.operation;
    void appState.repoStatus?.conflicted;
    if (o) refreshDetails();
    else details = [];
  });

  function opLabel(kind: string): string {
    if (kind === "cherry-pick") return "Cherry-pick";
    return kind.charAt(0).toUpperCase() + kind.slice(1);
  }
</script>

{#if op}
  <section class="conflict panel">
    <header class="ch">
      <span class="title">{opLabel(op)} in progress</span>
      <span class="n" class:clear={conflicted === 0}>
        {conflicted === 0 ? "all conflicts resolved" : `${conflicted} conflict(s)`}
      </span>
    </header>

    {#if details.length}
      <ul class="files">
        {#each details as f (f.path)}
          <li>
            <span class="path mono" title={f.path}>{f.path}</span>
            <span class="acts">
              {#if f.kind === "both"}
                <button onclick={() => gitActions.resolveConflict(f.path, true)}>Use ours</button>
                <button onclick={() => gitActions.resolveConflict(f.path, false)}>Use theirs</button>
              {:else if f.kind === "modify-delete"}
                <button onclick={() => gitActions.resolveKeep(f.path)}>Keep file</button>
                <button onclick={() => gitActions.resolveRemove(f.path)}>Remove file</button>
              {:else}
                <button onclick={() => gitActions.resolveRemove(f.path)}>Remove file</button>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {:else if conflicted === 0}
      <p class="done">All conflicts resolved — continue to finish, or abort.</p>
    {/if}

    <footer class="cf">
      <button class="primary" disabled={conflicted > 0} onclick={() => gitActions.opContinue(op)}>
        Continue {opLabel(op).toLowerCase()}
      </button>
      {#if op === "cherry-pick" || op === "revert"}
        <button onclick={() => gitActions.opSkip(op)}>Skip this commit</button>
      {/if}
      <button class="danger" onclick={() => gitActions.opAbort(op)}>Abort</button>
    </footer>
  </section>
{/if}

<style>
  .conflict {
    border: 1px solid var(--err);
    border-radius: 8px;
    padding: 10px 12px;
    background: var(--panel-bg);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ch {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .title {
    font-weight: 600;
    color: var(--err);
  }
  .n {
    font-size: 12px;
    color: var(--text-muted);
  }
  .n.clear {
    color: var(--accent);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 220px;
    overflow: auto;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .files li:hover {
    background: var(--row-hover);
  }
  .path {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--text);
  }
  .acts {
    flex: 0 0 auto;
    display: flex;
    gap: 6px;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .cf {
    display: flex;
    gap: 8px;
    padding-top: 2px;
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
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  button.danger {
    margin-left: auto;
    color: var(--danger);
    border-color: var(--danger);
  }
</style>
