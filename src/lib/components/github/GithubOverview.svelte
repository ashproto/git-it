<script lang="ts">
  import type { GhRepoStats } from "../../types";
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import { readmeMdToSafeHtml } from "../../github/markdown";
  import GithubSkeleton from "./GithubSkeleton.svelte";

  let { stats }: { stats: GhRepoStats } = $props();

  function lastPush(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }

  // Load the README whenever the active repo changes (or after a refresh).
  $effect(() => {
    const repo = appState.repo;
    if (repo) void githubState.loadReadme(repo);
  });

  // Resolve the render context from githubState so we don't need extra props.
  const readmeCtx = $derived(() => {
    const avail = githubState.availability;
    if (!avail || avail.kind !== "Ok") return null;
    return { owner: avail.owner, repo: avail.repo, branch: stats.defaultBranch };
  });

  const readmeHtml = $derived(() => {
    const md = githubState.readme.data;
    const ctx = readmeCtx();
    if (!md || !ctx) return "";
    return readmeMdToSafeHtml(md, ctx);
  });
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

  {#if githubState.readme.status === "loading"}
    <div class="readme-section">
      <h3 class="readme-heading">README</h3>
      <GithubSkeleton variant="card" />
    </div>
  {:else if readmeHtml()}
    <div class="readme-section">
      <h3 class="readme-heading">README</h3>
      <div class="readme md">{@html readmeHtml()}</div>
    </div>
  {/if}
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
  .readme-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 6px;
  }
  .readme-heading {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  /* Markdown body styles — mirrors the .md block used in Markdown.svelte */
  .readme :global(h1),
  .readme :global(h2),
  .readme :global(h3),
  .readme :global(h4),
  .readme :global(h5),
  .readme :global(h6) {
    margin: 0.8em 0 0.3em;
    font-weight: 600;
    line-height: 1.3;
    color: var(--text);
  }
  .readme :global(h1) { font-size: 1.3em; }
  .readme :global(h2) { font-size: 1.15em; }
  .readme :global(h3) { font-size: 1em; }
  .readme :global(p) {
    margin: 0.5em 0;
    font-size: 13px;
    line-height: 1.6;
    color: var(--text);
  }
  .readme :global(a) {
    color: var(--accent, #3b82f6);
    text-decoration: none;
  }
  .readme :global(a:hover) {
    text-decoration: underline;
  }
  .readme :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: 4px;
  }
  .readme :global(code) {
    font-family: ui-monospace, monospace;
    font-size: 0.85em;
    background: var(--code-bg, rgba(128,128,128,0.12));
    padding: 0.1em 0.35em;
    border-radius: 3px;
  }
  .readme :global(pre) {
    background: var(--code-bg, rgba(128,128,128,0.12));
    border-radius: 6px;
    padding: 10px 14px;
    overflow-x: auto;
    font-size: 12px;
    line-height: 1.5;
  }
  .readme :global(pre code) {
    background: none;
    padding: 0;
    font-size: inherit;
  }
  .readme :global(blockquote) {
    border-left: 3px solid var(--border);
    margin: 0.5em 0;
    padding: 0.2em 0 0.2em 12px;
    color: var(--text-muted);
  }
  .readme :global(ul),
  .readme :global(ol) {
    margin: 0.4em 0;
    padding-left: 1.4em;
    font-size: 13px;
    color: var(--text);
  }
  .readme :global(li) {
    margin: 0.2em 0;
    line-height: 1.5;
  }
  .readme :global(table) {
    border-collapse: collapse;
    font-size: 12px;
    width: 100%;
    margin: 0.5em 0;
  }
  .readme :global(th),
  .readme :global(td) {
    border: 1px solid var(--border);
    padding: 4px 8px;
    text-align: left;
  }
  .readme :global(th) {
    background: var(--panel-bg);
    font-weight: 600;
  }
  .readme :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.8em 0;
  }
</style>
