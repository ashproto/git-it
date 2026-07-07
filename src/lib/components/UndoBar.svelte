<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";
</script>

{#if appState.lastUndo}
  <div class="undo-bar" role="status" aria-live="polite">
    <span class="label">Undo <strong>{appState.lastUndo.label}</strong> available</span>
    <button type="button" class="undo-btn" onclick={() => gitActions.undo()}>
      Undo {appState.lastUndo.label}
    </button>
    <button
      type="button"
      class="dismiss-btn"
      aria-label="Dismiss undo"
      onclick={() => appState.setLastUndo(null)}
    >×</button>
  </div>
{/if}

<style>
  .undo-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    border-radius: var(--radius-dialog);
    border: 1px solid var(--accent);
    background: var(--panel-bg);
    font-size: 12.5px;
    color: var(--text);
  }
  .label {
    flex: 1 1 auto;
    color: var(--text-muted);
  }
  .label strong {
    color: var(--text);
    font-weight: 600;
  }
  .undo-btn {
    padding: 4px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }
  .undo-btn:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }
  .dismiss-btn {
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
  }
  .dismiss-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }
</style>
