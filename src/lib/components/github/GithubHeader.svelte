<script lang="ts">
  import type { GhRepoStats } from "../../types";
  import StatTile from "./StatTile.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { githubState } from "../../githubState.svelte";

  let {
    stats,
    owner,
    repo,
    onrefresh,
  }: { stats: GhRepoStats | null; owner: string; repo: string; onrefresh: () => void } = $props();
</script>

<header class="gh-header">
  <div class="id">
    <a class="slug" href={stats?.htmlUrl ?? `https://github.com/${owner}/${repo}`} target="_blank" rel="noreferrer">
      {owner}/{repo} ↗
    </a>
    {#if stats?.description}<p class="desc">{stats.description}</p>{/if}
  </div>
  <div class="tiles">
    {#if stats}
      <StatTile label="Stars" value={stats.stars} />
      <StatTile label="Forks" value={stats.forks} />
      <StatTile label="Watchers" value={stats.watchers} />
      <StatTile label="Open PRs" value={stats.openPulls} onclick={() => githubState.setActiveTab("pulls")} />
      <StatTile label="Open Issues" value={stats.openIssues} onclick={() => githubState.setActiveTab("issues")} />
    {:else}
      {#each Array.from({ length: 5 }) as _, i (i)}
        <Skeleton w="80px" h="46px" radius="8px" />
      {/each}
    {/if}
    <button
      class="refresh"
      class:spinning={githubState.anyLoading}
      type="button"
      title={githubState.anyLoading ? "Refreshing…" : "Refresh"}
      aria-busy={githubState.anyLoading}
      onclick={onrefresh}
    >↻</button>
  </div>
</header>

<style>
  .gh-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 2px 12px 2px;
  }
  .slug {
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
    text-decoration: none;
  }
  .slug:hover {
    color: var(--accent);
  }
  .desc {
    margin: 4px 0 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  .tiles {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .refresh {
    margin-left: auto;
    width: 30px;
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel-bg);
    color: var(--text);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
  }
  .refresh:hover {
    border-color: var(--accent);
  }
  /* Spin the ↻ glyph while any github fetch is in flight — the only feedback on a
     soft refresh, which keeps stale data on screen (no skeleton). */
  .refresh.spinning {
    color: var(--accent);
    animation: gh-refresh-spin 0.8s linear infinite;
  }
  @keyframes gh-refresh-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .refresh.spinning {
      animation: none;
      opacity: 0.6;
    }
  }
</style>
