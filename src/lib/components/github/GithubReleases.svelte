<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import { formatCompact } from "../../github/format";

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) void githubState.loadReleases(repo);
  });

  const panel = $derived(githubState.releases);
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
  <p class="note">Loading releases…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load releases ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No releases.</p>
{:else if panel.data}
  <div class="rels">
    {#each panel.data as r (r.tagName)}
      <section class="rel">
        <header>
          <a class="rtitle" href={r.htmlUrl} target="_blank" rel="noreferrer">{r.name || r.tagName}</a>
          <span class="tag mono">{r.tagName}</span>
          {#if r.prerelease}<span class="badge">pre-release</span>{/if}
          {#if r.draft}<span class="badge">draft</span>{/if}
          <span class="when">{rel(r.publishedAt)}</span>
          <span class="total">{formatCompact(r.totalDownloads)} downloads</span>
        </header>
        {#if r.assets.length}
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
{/if}

<style>
  .rels {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .rel header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .rtitle {
    color: var(--text);
    text-decoration: none;
    font-weight: 600;
    font-size: 14px;
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
  .assets {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
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
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
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
