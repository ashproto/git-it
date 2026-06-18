<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { IssueStateFilter } from "../../types";
  import { githubActions } from "../../githubActions.svelte";
  import GithubDetail from "./GithubDetail.svelte";
  import GithubSkeleton from "./GithubSkeleton.svelte";
  import { revealIn } from "../../github/motion";
  import { issueStateBadge } from "../../github/itemState";

  const FILTERS: IssueStateFilter[] = ["open", "closed", "all"];

  $effect(() => {
    const repo = appState.repo;
    void githubState.issueState;
    void githubState.reloadNonce;
    if (repo) void githubState.loadIssues(repo);
  });

  const panel = $derived(githubState.issues);
  const detailNumber = $derived(
    githubState.selectedItem?.kind === "issue" ? githubState.selectedItem.number : null,
  );
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

{#if detailNumber !== null}
  <GithubDetail kind="issue" number={detailNumber} />
{:else}
<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.issueState === f} type="button" onclick={() => githubState.setIssueState(f)}>{f}</button>
  {/each}
  <button class="new-issue" type="button" onclick={() => githubActions.open({ kind: "create" })}>New issue</button>
</div>

{#if panel.status === "loading" && !panel.data}
  <GithubSkeleton variant="list" />
{:else if panel.status === "error"}
  <p class="note err">Could not load issues ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No {githubState.issueState === "all" ? "" : githubState.issueState} issues.</p>
{:else if panel.data}
  <ul class="list" in:revealIn>
    {#each panel.data as it (it.number)}
      {@const badge = issueStateBadge(it)}
      <li class="row" style="--state:{badge.color}">
        <div class="title-row">
          <span class="state-badge">{badge.label}</span>
          <button class="title" type="button" onclick={() => githubState.openItem("issue", it.number)}>
            <span class="num">#{it.number}</span>{it.title}
          </button>
          <a class="ext" href={it.url} target="_blank" rel="noreferrer" title="Open on github.com">↗</a>
        </div>
        <div class="meta">
          <span>{it.author}</span>
          {#if it.assignees.length}<span>→ {it.assignees.join(", ")}</span>{/if}
          <span class="when">{rel(it.updatedAt)}</span>
        </div>
        <div class="row-actions">
          <button type="button" onclick={() => githubActions.open({ kind: "comment", target: "issue", number: it.number, title: it.title })}>Comment</button>
          {#if it.state === "OPEN"}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: it.number, title: it.title, to: "closed" })}>Close</button>
          {:else}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: it.number, title: it.title, to: "open" })}>Reopen</button>
          {/if}
        </div>
        {#if it.labels.length}
          <div class="labels">
            {#each it.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
  {#if panel.data.length >= 50}
    <p class="more">Showing the 50 most recent — open the repo on github.com for the full list.</p>
  {/if}
{/if}
{/if}

<style>
  .more {
    margin: 10px 2px 2px;
    font-size: 11.5px;
    color: var(--text-muted);
    font-style: italic;
  }
  .new-issue {
    margin-left: auto;
    padding: 3px 12px;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    cursor: pointer;
  }
  .row-actions {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }
  .row-actions button {
    padding: 2px 10px;
    font-size: 11.5px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--btn-bg);
    color: var(--text);
    cursor: pointer;
  }
  .row-actions button:hover {
    border-color: var(--accent);
  }
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
    padding: 10px 10px 10px 11px;
    border-left: 3px solid var(--state);
    border-bottom: 1px solid var(--border-subtle, var(--border));
    background: color-mix(in srgb, var(--state) 20%, transparent);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .state-badge {
    flex: none;
    background: var(--state);
    color: #fff;
    font-size: 10.5px;
    font-weight: 500;
    padding: 1px 9px;
    border-radius: 999px;
    white-space: nowrap;
  }
  .title {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    color: var(--text);
    font-weight: 500;
    font-size: 13.5px;
  }
  .ext {
    color: var(--text-muted);
    text-decoration: none;
    font-size: 12px;
    margin-left: 6px;
  }
  .ext:hover {
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
