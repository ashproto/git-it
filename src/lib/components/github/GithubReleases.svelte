<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import { formatCompact } from "../../github/format";
  import { aggregateDownloads } from "../../github/downloads";
  import GithubFilter from "./GithubFilter.svelte";
  import GithubSkeleton from "./GithubSkeleton.svelte";
  import { revealIn } from "../../github/motion";
  import { filterItems } from "../../github/listFilter";

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) void githubState.loadReleases(repo);
  });

  const panel = $derived(githubState.releases);
  let filterQuery = $state("");
  const filtered = $derived(filterItems(panel.data ?? [], filterQuery, ["name", "tagName"]));
  const summary = $derived(panel.data ? aggregateDownloads(panel.data) : null);
  const releaseMax = $derived(summary && summary.byRelease.length ? Math.max(1, summary.byRelease[0].total) : 1);
  const platformMax = $derived(summary && summary.byPlatform.length ? Math.max(1, summary.byPlatform[0].total) : 1);

  // Which per-release asset tables are expanded (collapsed by default).
  let expanded = $state(new Set<string>());
  function toggle(tag: string) {
    const next = new Set(expanded);
    if (next.has(tag)) next.delete(tag);
    else next.add(tag);
    expanded = next;
  }

  function rel(iso: string | null): string {
    if (!iso) return "—";
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  function kb(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
</script>

{#if panel.status === "loading" && !panel.data}
  <GithubSkeleton variant="releases" />
{:else if panel.status === "error"}
  <p class="note err">Could not load releases ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No releases.</p>
{:else if panel.data && summary}
  <div in:revealIn>
  <!-- All-time summary -->
  <div class="metrics">
    <div class="metric"><span class="big">{formatCompact(summary.grandTotal)}</span><span class="lbl">total downloads</span></div>
    <div class="metric"><span class="big">{summary.releaseCount}</span><span class="lbl">releases</span></div>
    <div class="metric"><span class="big">{formatCompact(summary.avgPerRelease)}</span><span class="lbl">avg / release</span></div>
    {#if summary.topRelease}
      <div class="metric"><span class="big mono">{summary.topRelease.tag}</span><span class="lbl">top · {formatCompact(summary.topRelease.total)} ↓</span></div>
    {/if}
  </div>

  {#if summary.grandTotal > 0}
    <div class="charts">
      {#if summary.byRelease.length}
        <section class="chart">
          <h4>Downloads by release</h4>
          {#each summary.byRelease.slice(0, 8) as b (b.tag)}
            <div class="bar-row">
              <span class="bl mono" title={b.name}>{b.tag}</span>
              <div class="track"><div class="fill" style={`width:${(b.total / releaseMax) * 100}%`}></div></div>
              <span class="bn">{formatCompact(b.total)}</span>
            </div>
          {/each}
        </section>
      {/if}
      {#if summary.byPlatform.length}
        <section class="chart">
          <h4>Downloads by platform <span class="hint">inferred from filenames</span></h4>
          {#each summary.byPlatform as p (p.platform)}
            <div class="bar-row">
              <span class="bl">{p.platform}</span>
              <div class="track"><div class="fill alt" style={`width:${(p.total / platformMax) * 100}%`}></div></div>
              <span class="bn">{formatCompact(p.total)}</span>
            </div>
          {/each}
        </section>
      {/if}
    </div>

    {#if summary.topAssets.length}
      <section class="chart">
        <h4>Top assets</h4>
        <ul class="top">
          {#each summary.topAssets as a (a.release + " " + a.name)}
            <li>
              <span class="an" title={a.name}>{a.name}</span>
              <span class="ar mono">{a.release}</span>
              <span class="ac">{formatCompact(a.count)} ↓</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}

  <!-- Per-release list (asset tables collapsed) -->
  <div class="rels-head">
    <h4 class="rels-h">All releases</h4>
    <GithubFilter bind:query={filterQuery} placeholder="Filter releases…" shown={filtered.length} total={panel.data.length} />
  </div>
  {#if filtered.length === 0}
    <p class="note">No releases match “{filterQuery.trim()}”.</p>
  {/if}
  <div class="rels">
    {#each filtered as r (r.tagName)}
      <section class="rel">
        <header>
          <a class="rtitle" href={r.htmlUrl} target="_blank" rel="noreferrer">{r.name || r.tagName}</a>
          <span class="tag mono">{r.tagName}</span>
          {#if r.prerelease}<span class="badge">pre-release</span>{/if}
          {#if r.draft}<span class="badge">draft</span>{/if}
          <span class="when">{rel(r.publishedAt)}</span>
          <span class="total">{formatCompact(r.totalDownloads)} ↓</span>
          {#if r.assets.length}
            <button class="exp" type="button" onclick={() => toggle(r.tagName)} aria-expanded={expanded.has(r.tagName)}>
              {expanded.has(r.tagName) ? "▾" : "▸"} {r.assets.length} {r.assets.length === 1 ? "asset" : "assets"}
            </button>
          {/if}
        </header>
        {#if r.assets.length && expanded.has(r.tagName)}
          <table class="assets">
            <tbody>
              {#each r.assets as a (a.name)}
                <tr>
                  <td><a href={a.downloadUrl} target="_blank" rel="noreferrer">{a.name}</a></td>
                  <td class="num">{kb(a.size)}</td>
                  <td class="num">{formatCompact(a.downloadCount)} ↓</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </section>
    {/each}
  </div>
  </div>
{/if}

<style>
  .metrics {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    margin-bottom: 16px;
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
  .charts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 16px;
    margin-bottom: 16px;
  }
  .chart {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    background: var(--panel-bg);
  }
  h4 {
    margin: 0 0 10px 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .hint {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 10.5px;
  }
  .bar-row {
    display: grid;
    grid-template-columns: 90px 1fr 54px;
    align-items: center;
    gap: 8px;
    margin-bottom: 5px;
    font-size: 12px;
  }
  .bl {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .track {
    height: 8px;
    background: var(--btn-bg);
    border-radius: 999px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
  }
  .fill.alt {
    background: color-mix(in srgb, var(--accent) 65%, var(--text-muted));
  }
  .bn {
    text-align: right;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .top {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .top li {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 10px;
    padding: 2px 0;
    font-size: 12px;
  }
  .an {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ar {
    color: var(--text-muted);
    font-size: 11px;
  }
  .ac {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .rels-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 4px 0 10px;
  }
  .rels-h {
    margin: 0;
  }
  .rels {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .rel header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .rtitle {
    color: var(--text);
    text-decoration: none;
    font-weight: 600;
    font-size: 13.5px;
  }
  .rtitle:hover {
    color: var(--accent);
  }
  .tag {
    color: var(--text-muted);
    font-size: 11.5px;
  }
  .badge {
    font-size: 10px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    text-transform: uppercase;
  }
  .when {
    font-size: 12px;
    color: var(--text-muted);
  }
  .total {
    margin-left: auto;
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
  }
  .exp {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 11.5px;
    cursor: pointer;
    padding: 0;
  }
  .exp:hover {
    color: var(--accent);
  }
  .assets {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    margin-top: 6px;
  }
  .assets td {
    padding: 3px 8px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
  }
  .assets a {
    color: var(--text);
    text-decoration: none;
  }
  .assets a:hover {
    color: var(--accent);
  }
  .num {
    text-align: right;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: var(--font-mono);
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
