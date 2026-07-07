<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { PullStateFilter } from "../../types";
  import { githubActions } from "../../githubActions.svelte";
  import { gitActions } from "../../gitActions";
  import GithubDetail from "./GithubDetail.svelte";
  import GithubFilter from "./GithubFilter.svelte";
  import GithubSkeleton from "./GithubSkeleton.svelte";
  import { revealIn } from "../../github/motion";
  import { pullStateBadge } from "../../github/itemState";
  import { filterItems } from "../../github/listFilter";

  const FILTERS: PullStateFilter[] = ["open", "closed", "merged", "all"];

  // Load when the tab mounts, the filter changes, or Refresh bumps the nonce.
  $effect(() => {
    const repo = appState.repo;
    void githubState.pullState; // track
    void githubState.reloadNonce; // track (Refresh)
    if (repo) void githubState.loadPulls(repo);
  });

  const panel = $derived(githubState.pulls);
  let filterQuery = $state("");
  const filtered = $derived(filterItems(panel.data ?? [], filterQuery, ["title", "author"]));
  const detailNumber = $derived(
    githubState.selectedItem?.kind === "pr" ? githubState.selectedItem.number : null,
  );
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

{#if detailNumber !== null}
  <GithubDetail kind="pr" number={detailNumber} />
{:else}
<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.pullState === f} type="button" onclick={() => githubState.setPullState(f)}>{f}</button>
  {/each}
  <span class="filter-slot"><GithubFilter bind:query={filterQuery} placeholder="Filter pull requests…" shown={filtered.length} total={panel.data?.length ?? 0} /></span>
</div>

{#if panel.status === "loading" && !panel.data}
  <GithubSkeleton variant="list" />
{:else if panel.status === "error"}
  <p class="note err">Could not load pull requests ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No {githubState.pullState === "all" ? "" : githubState.pullState} pull requests.</p>
{:else if panel.data && filtered.length === 0}
  <p class="note">No pull requests match “{filterQuery.trim()}”.</p>
{:else if panel.data}
  <ul class="list" in:revealIn>
    {#each filtered as pr (pr.number)}
      {@const badge = pullStateBadge(pr)}
      <li class="row" style="--state:{badge.color}">
        <div class="title-row">
          <span class="state-badge">{badge.label}</span>
          <button class="title" type="button" onclick={() => githubState.openItem("pr", pr.number)}>
            <span class="num">#{pr.number}</span>{pr.title}
          </button>
          <a class="ext" href={pr.url} target="_blank" rel="noreferrer" title="Open on github.com">↗</a>
        </div>
        <div class="meta">
          <span>{pr.author}</span>
          <span class="mono">{pr.headRefName} → {pr.baseRefName}</span>
          {#if pr.reviewDecision}<span class="rev {pr.reviewDecision}">{pr.reviewDecision.replace(/_/g, " ").toLowerCase()}</span>{/if}
          <span class="when">{rel(pr.updatedAt)}</span>
        </div>
        <div class="row-actions">
          <button
            type="button"
            title="Checkout branch locally"
            onclick={(e) => {
              e.stopPropagation();
              void gitActions.checkoutPullRequest(pr.number);
            }}
          >Checkout</button>
          {#if pr.state === "OPEN" && !pr.isDraft}
            <button type="button" onclick={() => githubActions.open({ kind: "merge", number: pr.number, title: pr.title })}>Merge</button>
          {/if}
          <button type="button" onclick={() => githubActions.open({ kind: "comment", target: "pr", number: pr.number, title: pr.title })}>Comment</button>
        </div>
        {#if pr.labels.length}
          <div class="labels">
            {#each pr.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
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
  .row-actions {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }
  .row-actions button {
    padding: 2px 10px;
    font-size: 11.5px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--btn-bg);
    color: var(--text);
    cursor: pointer;
  }
  .row-actions button:hover {
    border-color: var(--accent);
  }
  .filters {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 10px;
  }
  .filter-slot {
    margin-left: auto;
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
    color: var(--on-accent);
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
  .mono {
    font-family: var(--font-mono);
    font-size: 11px;
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
