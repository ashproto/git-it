<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";

  // The latest streamed line (last element of remoteLog), trimmed.
  const latestLine = $derived(
    appState.remoteLog.length > 0
      ? appState.remoteLog[appState.remoteLog.length - 1].trim()
      : "",
  );
</script>

{#if appState.remoteOpActive}
  <div class="remote-progress" role="status" aria-live="polite" aria-label="Remote operation in progress">
    <div class="bar">
      <div class="spinner" aria-hidden="true"></div>
      <span class="line">{latestLine || "Working…"}</span>
    </div>
    <button
      type="button"
      class="cancel-btn"
      onclick={() => gitActions.cancelRemote()}
      title="Cancel remote operation"
    >
      Cancel
    </button>
  </div>
{/if}

<style>
  .remote-progress {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 14px;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 12px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    flex-shrink: 0;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .line {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
  }

  .cancel-btn {
    flex-shrink: 0;
    padding: 3px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
  }
  .cancel-btn:hover {
    background: var(--btn-hover);
  }
</style>
