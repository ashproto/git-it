<script lang="ts">
  import { appState } from "../store.svelte";
  import { tick } from "svelte";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  let logEl: HTMLDivElement | null = $state(null);

  $effect(() => {
    // Auto-scroll to bottom when log grows.
    appState.logLines.length;
    tick().then(() => {
      if (logEl) logEl.scrollTop = logEl.scrollHeight;
    });
  });

  function clearLog() {
    appState.clearLog();
  }
</script>

<CollapsiblePanel title="Output" collapsed>
  {#snippet headerActions()}
    <span class="status">{appState.status}</span>
    <button type="button" onclick={clearLog}>Clear</button>
  {/snippet}
  <div class="log" bind:this={logEl}>
    {#each appState.logLines as line, i (i)}
      <div class="line" class:err={line.startsWith("[stderr]")}>{line}</div>
    {/each}
    {#if appState.logLines.length === 0}
      <div class="empty">No output yet.</div>
    {/if}
  </div>
</CollapsiblePanel>

<style>
  .status {
    color: var(--text-muted);
    font-size: 12px;
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .log {
    height: 120px;
    overflow: auto;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--input-bg);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.45;
  }
  .line {
    white-space: pre-wrap;
    word-break: break-all;
  }
  .line.err {
    color: var(--err);
  }
  .empty {
    color: var(--text-muted);
    font-style: italic;
  }
</style>
