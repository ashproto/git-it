<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) void githubState.loadRuns(repo);
  });

  const panel = $derived(githubState.runs);
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  // A run's pass/fail is only meaningful once status === "completed".
  function glyph(status: string, conclusion: string): string {
    if (status !== "completed") return "○";
    if (conclusion === "success") return "✓";
    if (conclusion === "failure" || conclusion === "timed_out") return "✗";
    return "–";
  }
  function cls(status: string, conclusion: string): string {
    if (status !== "completed") return "pending";
    if (conclusion === "success") return "ok";
    if (conclusion === "failure" || conclusion === "timed_out") return "fail";
    return "other";
  }
</script>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading workflow runs…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load runs ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No workflow runs.</p>
{:else if panel.data}
  <ul class="list">
    {#each panel.data as run (run.id)}
      <li class="row">
        <span class="glyph {cls(run.status, run.conclusion)}" aria-hidden="true">{glyph(run.status, run.conclusion)}</span>
        <a class="title" href={run.url} target="_blank" rel="noreferrer">{run.title}</a>
        <span class="wf">{run.workflowName}</span>
        <span class="branch mono">{run.headBranch}</span>
        <span class="ev">{run.event}</span>
        <span class="when">{rel(run.updatedAt)}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    font-size: 12.5px;
  }
  .glyph {
    width: 16px;
    text-align: center;
    font-weight: 700;
  }
  .glyph.ok {
    color: var(--status-add, #2ea043);
  }
  .glyph.fail {
    color: var(--err, #c0392b);
  }
  .glyph.pending {
    color: var(--status-mod, #d29922);
  }
  .title {
    color: var(--text);
    text-decoration: none;
    font-weight: 500;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title:hover {
    color: var(--accent);
  }
  .wf,
  .ev,
  .when {
    color: var(--text-muted);
  }
  .branch {
    color: var(--text-muted);
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
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
