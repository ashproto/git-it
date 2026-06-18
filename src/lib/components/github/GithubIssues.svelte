<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { IssueStateFilter } from "../../types";

  const FILTERS: IssueStateFilter[] = ["open", "closed", "all"];

  $effect(() => {
    const repo = appState.repo;
    void githubState.issueState;
    void githubState.reloadNonce;
    if (repo) void githubState.loadIssues(repo);
  });

  const panel = $derived(githubState.issues);
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.issueState === f} type="button" onclick={() => githubState.setIssueState(f)}>{f}</button>
  {/each}
</div>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading issues…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load issues ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No {githubState.issueState === "all" ? "" : githubState.issueState} issues.</p>
{:else if panel.data}
  <ul class="list">
    {#each panel.data as it (it.number)}
      <li class="row">
        <a class="title" href={it.url} target="_blank" rel="noreferrer">
          <span class="num">#{it.number}</span>{it.title}
        </a>
        <div class="meta">
          <span>{it.author}</span>
          {#if it.assignees.length}<span>→ {it.assignees.join(", ")}</span>{/if}
          <span class="when">{rel(it.updatedAt)}</span>
        </div>
        {#if it.labels.length}
          <div class="labels">
            {#each it.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
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
