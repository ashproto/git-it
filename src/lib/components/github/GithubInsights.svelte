<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { formatCompact } from "../../github/format";
  import { sparklinePoints } from "../../github/activity";
  import GithubSkeleton from "./GithubSkeleton.svelte";

  // Load all four insight panels when the tab mounts (and on Refresh).
  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) {
      void githubState.loadTraffic(repo);
      void githubState.loadContributors(repo);
      void githubState.loadActivity(repo);
      void githubState.loadMilestones(repo);
      void githubState.loadLabels(repo);
    }
  });

  const traffic = $derived(githubState.traffic);
  const contributors = $derived(githubState.contributors);
  const activity = $derived(githubState.activity);
  const milestones = $derived(githubState.milestones);
  const labels = $derived(githubState.labels);

  const weekTotals = $derived(activity.data ? activity.data.weeks.map((w) => w.total) : []);
  const peakWeek = $derived(weekTotals.length ? Math.max(...weekTotals) : 0);
  const yearTotal = $derived(weekTotals.reduce((s, n) => s + n, 0));
  function initials(login: string): string {
    return login.slice(0, 2).toUpperCase();
  }
</script>

<div class="insights">
  <!-- Traffic (owner-only) -->
  <section class="card">
    <h3>Traffic <span class="hint">last 14 days</span></h3>
    {#if traffic.status === "loading" && !traffic.data}
      <GithubSkeleton variant="card" />
    {:else if traffic.status === "error"}
      {#if traffic.error?.kind === "Forbidden"}
        <p class="note">Traffic is only available for repositories you have push access to.</p>
      {:else}
        <p class="note err">Could not load traffic ({traffic.error?.kind}).</p>
      {/if}
    {:else if traffic.data}
      <div class="metrics">
        <div class="metric"><span class="big">{formatCompact(traffic.data.views.count)}</span><span class="lbl">Views · {formatCompact(traffic.data.views.uniques)} unique</span></div>
        <div class="metric"><span class="big">{formatCompact(traffic.data.clones.count)}</span><span class="lbl">Clones · {formatCompact(traffic.data.clones.uniques)} unique</span></div>
      </div>
      {#if traffic.data.views.count === 0 && traffic.data.clones.count === 0}
        <p class="note">No traffic in the last 14 days.</p>
      {/if}
      {#if traffic.data.paths.length}
        <h4>Popular content</h4>
        <ul class="mini">
          {#each traffic.data.paths.slice(0, 6) as p (p.path)}
            <li><span class="t">{p.title || p.path}</span><span class="n">{formatCompact(p.count)}</span></li>
          {/each}
        </ul>
      {/if}
      {#if traffic.data.referrers.length}
        <h4>Referrers</h4>
        <ul class="mini">
          {#each traffic.data.referrers.slice(0, 6) as r (r.referrer)}
            <li><span class="t">{r.referrer}</span><span class="n">{formatCompact(r.count)}</span></li>
          {/each}
        </ul>
      {/if}
    {/if}
  </section>

  <!-- Commit activity -->
  <section class="card">
    <h3>Commit activity <span class="hint">52 weeks</span></h3>
    {#if activity.status === "loading" && !activity.data}
      <GithubSkeleton variant="card" />
    {:else if activity.status === "error"}
      <p class="note err">Could not load activity ({activity.error?.kind}).</p>
    {:else if activity.data?.computing}
      <p class="note">GitHub is still computing these stats — try Refresh in a moment.</p>
    {:else if activity.data}
      <svg class="spark" viewBox="0 0 300 48" preserveAspectRatio="none" role="img" aria-label="Commits per week">
        <polyline points={sparklinePoints(weekTotals, 300, 44)} fill="none" stroke="var(--accent)" stroke-width="1.5" />
      </svg>
      <p class="note">{yearTotal} commits in the last year · peak {peakWeek}/week</p>
    {/if}
  </section>

  <!-- Contributors -->
  <section class="card">
    <h3>Top contributors</h3>
    {#if contributors.status === "loading" && !contributors.data}
      <GithubSkeleton variant="card" />
    {:else if contributors.status === "error"}
      <p class="note err">Could not load contributors ({contributors.error?.kind}).</p>
    {:else if contributors.data && contributors.data.length === 0}
      <p class="note">No contributors.</p>
    {:else if contributors.data}
      <ul class="contribs">
        {#each contributors.data as c (c.login)}
          <li>
            <a href={c.htmlUrl} target="_blank" rel="noreferrer">
              <span class="av" aria-hidden="true">{initials(c.login)}</span>
              <span class="login">{c.login}</span>
              {#if c.isBot}<span class="bot">bot</span>{/if}
            </a>
            <span class="count">{formatCompact(c.contributions)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- Milestones & labels -->
  <section class="card">
    <h3>Milestones &amp; labels</h3>
    {#if milestones.status === "ok" || labels.status === "ok"}
      {#if milestones.data && milestones.data.length}
        <ul class="miles">
          {#each milestones.data as m (m.number)}
            {@const total = m.openIssues + m.closedIssues}
            <li>
              <a href={m.htmlUrl} target="_blank" rel="noreferrer">{m.title}</a>
              <div class="bar"><div class="fill" style={`width:${total ? (m.closedIssues / total) * 100 : 0}%`}></div></div>
              <span class="prog">{m.closedIssues}/{total}</span>
            </li>
          {/each}
        </ul>
      {:else if milestones.status === "ok"}
        <p class="note">No open milestones.</p>
      {:else if milestones.status === "error"}
        <p class="note err">Milestones failed to load.</p>
      {/if}
      {#if labels.data && labels.data.length}
        <div class="labels">
          {#each labels.data as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
        </div>
      {:else if labels.status === "error"}
        <p class="note err">Labels failed to load.</p>
      {/if}
    {:else if milestones.status === "error" || labels.status === "error"}
      <p class="note err">Could not load milestones/labels.</p>
    {:else}
      <GithubSkeleton variant="card" />
    {/if}
  </section>
</div>

<style>
  .insights {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 14px;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px;
    background: var(--panel-bg);
  }
  h3 {
    margin: 0 0 10px 0;
    font-size: 13px;
  }
  h4 {
    margin: 12px 0 4px 0;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hint {
    font-weight: 400;
    color: var(--text-muted);
    font-size: 11px;
  }
  .metrics {
    display: flex;
    gap: 18px;
  }
  .metric {
    display: flex;
    flex-direction: column;
  }
  .big {
    font-size: 20px;
    font-weight: 600;
  }
  .lbl {
    font-size: 11px;
    color: var(--text-muted);
  }
  .spark {
    width: 100%;
    height: 48px;
  }
  .mini {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .mini li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    padding: 2px 0;
  }
  .mini .t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .mini .n {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .contribs {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .contribs li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 0;
  }
  .contribs a {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
    text-decoration: none;
  }
  .contribs a:hover .login {
    color: var(--accent);
  }
  .av {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--row-selected);
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 600;
  }
  .login {
    font-size: 12.5px;
  }
  .bot {
    font-size: 9px;
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .count {
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .miles {
    list-style: none;
    margin: 0 0 10px 0;
    padding: 0;
  }
  .miles li {
    display: grid;
    grid-template-columns: 1fr 80px auto;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
    font-size: 12.5px;
  }
  .miles a {
    color: var(--text);
    text-decoration: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .miles a:hover {
    color: var(--accent);
  }
  .bar {
    height: 6px;
    background: var(--btn-bg);
    border-radius: 999px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
  }
  .prog {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
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
    margin: 8px 2px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
