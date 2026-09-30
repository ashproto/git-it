<script lang="ts">
  import { untrack } from "svelte";
  import { appState } from "../store.svelte";
  import { errorText, summarizeGitOutput } from "../errorReport";

  let { what, error, onRetry }: { what: string; error: unknown; onRetry: () => void | Promise<unknown> } = $props();
  const raw = $derived(errorText(error));
  const headline = $derived(summarizeGitOutput(raw).headline.split("\n")[0]);
  // The panel shows one line; the Output log keeps the whole failure. `raw` compares by value,
  // so a refresh that fails the same way (the stash list reloads on every watcher tick) logs
  // nothing new. untrack: appendLog reads the log it writes, which would re-run this effect.
  $effect(() => {
    const line = `[load] Couldn't load ${what}\n${raw}`;
    untrack(() => appState.appendLog(line));
  });
  // True while a retry runs, so one that fails the same way again still shows it did something.
  let retrying = $state(false);

  async function retry() {
    retrying = true;
    try {
      await onRetry();
    } catch (e) {
      // A host's loader shows its own outcome and shouldn't reject. If one does, log it
      // rather than leave an unhandled rejection.
      console.warn(`[gte] retry loading ${what} failed`, e);
    } finally {
      retrying = false;
    }
  }
</script>

<div class="load-error" role="status" aria-live="polite">
  <p class="selectable">Couldn't load {what}: {headline}</p>
  <button type="button" aria-label={`Retry loading ${what}`} disabled={retrying} onclick={retry}>
    {retrying ? "Retrying…" : "Retry"}
  </button>
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
  /* Padding, border and colours match the row buttons in StashPanel / ReflogPanel. The radius
     is the theme token rather than their fixed 5px, so NERV's sharper corners apply. The font
     is this note's 12px at the normal line height buttons have by default: the note's 1.45
     would make Retry taller than the buttons around it. */
  button {
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--btn-bg);
    color: var(--text);
    font: inherit;
    line-height: normal;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
