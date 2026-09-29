<script lang="ts">
  import { errorText, summarizeGitOutput } from "../errorReport";

  let { what, error, onRetry }: { what: string; error: unknown; onRetry: () => void } = $props();
  const headline = $derived(summarizeGitOutput(errorText(error)).headline.split("\n")[0]);
</script>

<div class="load-error" role="status">
  <p class="selectable">Couldn't load {what}: {headline}</p>
  <button type="button" onclick={onRetry}>Retry</button>
</div>

<style>
  /* No outer margin: each panel spaces it like its own notes (StashPanel's `.note` is
     margin 0; ConflictView uses a flex gap; the diff pane wraps it in `.diff-error`). */
  .load-error {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 12px;
    line-height: 1.45;
  }
  p {
    flex: 1 1 180px;
    min-width: 0;
    margin: 0;
    color: var(--danger);
    overflow-wrap: anywhere;
  }
  /* Same recipe as the sibling buttons in StashPanel / ReflogPanel / BackupsPanel. */
  button {
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--btn-bg);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
</style>
