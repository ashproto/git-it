<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { PullStateFilter } from "../../types";

  const FILTERS: PullStateFilter[] = ["open", "closed", "merged", "all"];

  // Load when the tab mounts, the filter changes, or Refresh bumps the nonce.
  $effect(() => {
    const repo = appState.repo;
    void githubState.pullState; // track
    void githubState.reloadNonce; // track (Refresh)
    if (repo) void githubState.loadPulls(repo);
  });

  const panel = $derived(githubState.pulls);
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.pullState === f} type="button" onclick={() => githubState.setPullState(f)}>{f}</button>
  {/each}
</div>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading pull requests…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load pull requests ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No {githubState.pullState === "all" ? "" : githubState.pullState} pull requests.</p>
{:else if panel.data}
  <ul class="list">
    {#each panel.data as pr (pr.number)}
      <li class="row">
        <a class="title" href={pr.url} target="_blank" rel="noreferrer">
          <span class="num">#{pr.number}</span>{pr.title}
          {#if pr.isDraft}<span class="badge">draft</span>{/if}
        </a>
        <div class="meta">
          <span>{pr.author}</span>
          <span class="mono">{pr.headRefName} → {pr.baseRefName}</span>
          {#if pr.reviewDecision}<span class="rev {pr.reviewDecision}">{pr.reviewDecision.replace(/_/g, " ").toLowerCase()}</span>{/if}
          <span class="when">{rel(pr.updatedAt)}</span>
        </div>
        {#if pr.labels.length}
          <div class="labels">
            {#each pr.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .filters {
    display: flex;
    gap: 6px;
    margin-bottom: 10px;
  }
  .chip {
    padding: 3px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel-bg);
    color: var(--text-muted);
    font-size: 12px;
    text-transform: capitalize;
    cursor: pointer;
  }
  .chip.active {
    color: #fff;
    background: var(--accent);
    border-color: var(--accent);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    padding: 10px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title {
    color: var(--text);
    text-decoration: none;
    font-weight: 500;
    font-size: 13.5px;
  }
  .title:hover {
    color: var(--accent);
  }
  .num {
    color: var(--text-muted);
    margin-right: 6px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
  }
  .badge {
    font-size: 10px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-left: 6px;
    text-transform: uppercase;
  }
  .rev {
    text-transform: capitalize;
  }
  .rev.APPROVED {
    color: var(--status-add, #2ea043);
  }
  .rev.CHANGES_REQUESTED {
    color: var(--err, #c0392b);
  }
  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .label {
    font-size: 10.5px;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid var(--lc);
    color: var(--lc);
  }
  .note {
    margin: 14px 2px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
