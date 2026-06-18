<script lang="ts">
  import type { GhRepoStats } from "../../types";
  import { appState } from "../../store.svelte";
  import { parseISO, formatCommitDate } from "../../dates";

  let { stats }: { stats: GhRepoStats } = $props();

  function lastPush(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

<div class="overview">
  <dl class="meta">
    <dt>Default branch</dt>
    <dd>{stats.defaultBranch}</dd>
    <dt>Visibility</dt>
    <dd>{stats.visibility}{stats.isFork ? " · fork" : ""}{stats.archived ? " · archived" : ""}</dd>
    <dt>Language</dt>
    <dd>{stats.language ?? "—"}</dd>
    <dt>License</dt>
    <dd>{stats.licenseSpdxId ?? "—"}</dd>
    <dt>Last push</dt>
    <dd>{lastPush(stats.pushedAt)}</dd>
  </dl>
  {#if stats.topics.length}
    <div class="topics">
      {#each stats.topics as t (t)}<span class="topic">{t}</span>{/each}
    </div>
  {/if}
  <p class="note">Pull requests, issues, releases and CI summaries appear in their tabs.</p>
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .meta {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 18px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    color: var(--text);
  }
  .topics {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .topic {
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
