<script lang="ts">
  import { onMount } from "svelte";
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { prerequisites } from "../prerequisites.svelte";
  import { toEpochTz } from "../dates";
  import type { DateMapping, RewriteOptions } from "../types";

  // When bare, drop the .panel card chrome so this nests cleanly inside a parent
  // panel (the Commit panel's inline edit mode in CommitDetail).
  let { bare = false }: { bare?: boolean } = $props();

  let updateAuthor = $state(true);
  let autoBundle = $state(true);
  let preserveRemote = $state(true);
  let optimizeRange = $state(true);

  let busy = $state(false);

  onMount(() => {
    void prerequisites.load();
  });

  async function rewrite() {
    // Defense in depth: the button is already disabled when the probe failed, but never
    // start a destructive history rewrite we know will fail for lack of git-filter-repo.
    if (!prerequisites.canEditHistory) {
      appState.status =
        "Commit-time editing is unavailable — the bundled git-filter-repo couldn't run.";
      return;
    }
    if (appState.newDates.size === 0) {
      appState.status = "No new dates queued. Use an edit mode to preview changes first.";
      return;
    }
    const mappings: DateMapping[] = [];
    for (const [sha, d] of appState.newDates.entries()) {
      const { epoch, tz } = toEpochTz(d);
      mappings.push({ sha, epoch, tz_offset: tz });
    }
    const options: RewriteOptions = {
      updateAuthor,
      autoBundle,
      preserveRemote,
      optimizeRange,
    };

    if (
      !confirm(
        `Rewrite ${mappings.length} commit(s) in ${appState.repo}?\n\nThis runs git-filter-repo and rewrites history. A bundle backup will be created first if enabled.`,
      )
    ) {
      return;
    }

    busy = true;
    appState.isRewriting = true;
    appState.status = "Rewriting…";
    try {
      await api.rewriteHistory(appState.repo, mappings, options, (line) => {
        appState.appendLog(line);
      });
      appState.status = "Rewrite finished.";
      appState.appendLog("[rewrite] done.");
      appState.clearNewDates();
      // Reload commits so the table reflects new dates
      try {
        const fresh = await api.loadCommits(appState.repo, appState.commits.length);
        appState.commits = fresh;
      } catch {}
    } catch (e) {
      appState.status = `Rewrite failed: ${e}`;
      appState.appendLog(`[rewrite] ERROR: ${e}`);
    } finally {
      busy = false;
      appState.isRewriting = false;
    }
  }
</script>

<section class="apply-row" class:panel={!bare}>
  <div class="opts">
    <label class="check">
      <input type="checkbox" bind:checked={updateAuthor} />
      Also change author date
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={autoBundle} />
      Auto-create bundle backup
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={preserveRemote} />
      Preserve 'origin' remote
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={optimizeRange} />
      Limit rewrite range
    </label>
  </div>
  <div class="apply">
    <span class="count">{appState.newDates.size} commit(s) queued</span>
    <button
      type="button"
      class="primary danger"
      disabled={busy || appState.newDates.size === 0 || !prerequisites.canEditHistory}
      title={!prerequisites.canEditHistory
        ? "Commit-time editing needs the bundled git-filter-repo, which couldn't run on this machine."
        : undefined}
      onclick={rewrite}
    >
      {busy ? "Rewriting…" : "Rewrite history"}
    </button>
  </div>
</section>

<style>
  .apply-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .apply-row.panel {
    background: var(--panel-bg);
    border-radius: var(--radius-lg);
    padding: 10px 14px;
    border: 1px solid var(--border);
  }
  .opts {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12.5px;
    color: var(--text);
  }
  .apply {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .count {
    color: var(--text-muted);
    font-size: 12px;
  }
  button {
    padding: 8px 14px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  button.danger.primary {
    background: var(--danger);
    border-color: var(--danger);
  }
  button.danger.primary:hover:not(:disabled) {
    background: var(--danger-hover);
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
