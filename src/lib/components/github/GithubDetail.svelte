<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { GhPullDetail, GhIssueDetail } from "../../types";
  import Markdown from "./Markdown.svelte";
  import GithubSkeleton from "./GithubSkeleton.svelte";
  import PrTimeline from "./PrTimeline.svelte";
  import { revealIn } from "../../github/motion";

  let { kind, number }: { kind: "pr" | "issue"; number: number } = $props();

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) {
      if (kind === "pr") void githubState.loadPrDetail(repo, number);
      else void githubState.loadIssueDetail(repo, number);
    }
  });

  const prPanel = $derived(githubState.prDetail);
  const issuePanel = $derived(githubState.issueDetail);
  const panel = $derived(kind === "pr" ? prPanel : issuePanel);
  // Narrowed typed accessors
  const d = $derived(panel.data);
  const pr = $derived(kind === "pr" ? (prPanel.data as GhPullDetail | null) : null);
  const iss = $derived(kind === "issue" ? (issuePanel.data as GhIssueDetail | null) : null);
  function rel(iso: string): string {
    if (!iso) return "";
    const dt = parseISO(iso);
    return dt ? formatCommitDate(dt, appState.dateFormat, appState.relativeDates) : iso;
  }
  function glyph(b: string): string {
    return b === "pass" ? "✓" : b === "fail" ? "✗" : b === "pending" ? "○" : "–";
  }
  // Roll up the statusCheckRollup buckets for the pinned "latest checks" strip.
  const checkSummary = $derived.by(() => {
    const c = pr?.checks ?? [];
    const passed = c.filter((x) => x.bucket === "pass").length;
    const failing = c.filter((x) => x.bucket === "fail").length;
    const pending = c.filter((x) => x.bucket === "pending").length;
    const neutral = c.filter((x) => x.bucket === "neutral").length;
    return { total: c.length, passed, failing, pending, neutral };
  });
</script>

<div class="detail">
  <div class="topbar">
    <button class="back" type="button" onclick={() => githubState.closeItem()}>← Back</button>
  </div>

  {#if panel.status === "loading" && !d}
    <GithubSkeleton variant="detail" />
  {:else if panel.status === "error"}
    <p class="note err">Could not load ({panel.error?.kind}).</p>
  {:else if d}
    <header class="hd" in:revealIn>
      <h2><span class="num">#{d.number}</span> {d.title}</h2>
      <div class="sub">
        <span class="state {d.state.toLowerCase()}">{pr?.isDraft ? "DRAFT" : d.state}</span>
        <span>{d.author}</span>
        {#if pr}<span class="mono">{pr.headRefName} → {pr.baseRefName}</span>{/if}
        <span class="when">opened {rel(d.createdAt)}</span>
        <a class="ext" href={d.url} target="_blank" rel="noreferrer">Open on github.com ↗</a>
      </div>
      <div class="acts">
        <button type="button" onclick={() => githubActions.open({ kind: "comment", target: kind, number: d.number, title: d.title })}>Comment</button>
        {#if pr && pr.state === "OPEN" && !pr.isDraft}
          <button type="button" onclick={() => githubActions.open({ kind: "merge", number: d.number, title: d.title })}>Merge</button>
        {/if}
        {#if iss}
          {#if iss.state === "OPEN"}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: d.number, title: d.title, to: "closed" })}>Close</button>
          {:else}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: d.number, title: d.title, to: "open" })}>Reopen</button>
          {/if}
        {/if}
      </div>
    </header>

    {#if d.labels.length || d.assignees.length || d.milestone}
      <div class="meta">
        {#each d.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
        {#if d.assignees.length}<span class="m">assigned: {d.assignees.join(", ")}</span>{/if}
        {#if d.milestone}<span class="m">milestone: {d.milestone}</span>{/if}
      </div>
    {/if}

    <section class="body">
      {#if d.body.trim()}<Markdown src={d.body} />{:else}<p class="note">No description.</p>{/if}
    </section>

    {#if pr}
      <div class="pr-extra">
        <div class="readiness">
          {#if pr.reviewDecision}<span class="pill">{pr.reviewDecision.replace(/_/g, " ").toLowerCase()}</span>{/if}
          {#if pr.mergeable}<span class="pill">{pr.mergeable.toLowerCase()}</span>{/if}
          <span class="diffstat"><span class="add">+{pr.additions}</span> <span class="del">−{pr.deletions}</span> · {pr.changedFiles} files</span>
        </div>
        {#if pr.checks.length}
          <details class="block checks-strip">
            <summary>
              <span class="ck-sum">
                {#if checkSummary.passed}<span class="g pass">{glyph("pass")}</span> {checkSummary.passed} passed{/if}
                {#if checkSummary.failing}<span class="sep">·</span> <span class="g fail">{glyph("fail")}</span> {checkSummary.failing} failing{/if}
                {#if checkSummary.pending}<span class="sep">·</span> <span class="g pending">{glyph("pending")}</span> {checkSummary.pending} pending{/if}
                {#if checkSummary.neutral}<span class="sep">·</span> <span class="g">{glyph("neutral")}</span> {checkSummary.neutral} other{/if}
              </span>
              <span class="ck-label">checks</span>
            </summary>
            <ul class="checks">
              {#each pr.checks as c, i (c.name + i)}
                <li><span class="g {c.bucket}">{glyph(c.bucket)}</span><a href={c.url} target="_blank" rel="noreferrer">{c.name}</a></li>
              {/each}
            </ul>
          </details>
        {/if}
        {#if pr.files.length}
          <details class="block"><summary>Files ({pr.files.length})</summary>
            <ul class="files">
              {#each pr.files as f (f.path)}
                <li><a href={`${pr.url}/files`} target="_blank" rel="noreferrer" class="mono">{f.path}</a><span class="fstat"><span class="add">+{f.additions}</span> <span class="del">−{f.deletions}</span></span></li>
              {/each}
            </ul>
          </details>
        {/if}
      </div>
    {/if}

    {#if pr}
      <section class="activity">
        <h3>Activity</h3>
        <PrTimeline {pr} />
      </section>
    {:else}
      <section class="comments">
        <h3>{d.comments.length} {d.comments.length === 1 ? "comment" : "comments"}</h3>
        {#if d.comments.length === 0}
          <p class="note">No comments yet.</p>
        {:else}
          {#each d.comments as c, i (c.author + i)}
            <article class="comment">
              <div class="chead"><strong>{c.author}</strong> <span class="when">{rel(c.createdAt)}</span></div>
              <Markdown src={c.body} />
            </article>
          {/each}
        {/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  .detail { display: flex; flex-direction: column; gap: 12px; }
  .topbar { position: sticky; top: 0; }
  .back {
    background: none; border: 1px solid var(--border); border-radius: 7px;
    color: var(--text); padding: 4px 12px; font-size: 12.5px; cursor: pointer;
  }
  .back:hover { border-color: var(--accent); }
  .hd h2 { margin: 0; font-size: 17px; }
  .num { color: var(--text-muted); }
  .sub { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; font-size: 12.5px; color: var(--text-muted); margin-top: 6px; }
  .state { font-size: 11px; padding: 1px 8px; border-radius: 999px; border: 1px solid var(--border); text-transform: uppercase; }
  .state.open { color: var(--status-add, #2ea043); border-color: currentColor; }
  .state.closed { color: var(--err, #c0392b); border-color: currentColor; }
  .state.merged { color: var(--accent); border-color: currentColor; }
  .ext { color: var(--accent); text-decoration: none; }
  .acts { display: flex; gap: 8px; margin-top: 10px; }
  .acts button {
    padding: 4px 14px; border: 1px solid var(--border); border-radius: 7px;
    background: var(--btn-bg); color: var(--text); font-size: 12.5px; cursor: pointer;
  }
  .acts button:hover { border-color: var(--accent); }
  .meta { display: flex; flex-wrap: wrap; gap: 6px 10px; align-items: center; }
  .label { font-size: 10.5px; padding: 0 7px; border-radius: 999px; border: 1px solid var(--lc); color: var(--lc); }
  .m { font-size: 12px; color: var(--text-muted); }
  .body { border: 1px solid var(--border); border-radius: 10px; padding: 12px 14px; background: var(--panel-bg); }
  .pr-extra { display: flex; flex-direction: column; gap: 8px; }
  .readiness { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; font-size: 12px; }
  .pill { padding: 1px 8px; border: 1px solid var(--border); border-radius: 999px; text-transform: capitalize; color: var(--text-muted); }
  .diffstat { color: var(--text-muted); }
  .add { color: var(--status-add, #2ea043); }
  .del { color: var(--err, #c0392b); }
  .block { border: 1px solid var(--border); border-radius: 8px; padding: 6px 10px; }
  .block summary { cursor: pointer; font-size: 12.5px; }
  .checks, .files { list-style: none; margin: 8px 0 0; padding: 0; }
  .checks li, .files li { display: flex; align-items: center; gap: 8px; padding: 2px 0; font-size: 12px; }
  .files li { justify-content: space-between; }
  .checks a, .files a { color: var(--text); text-decoration: none; }
  .checks a:hover, .files a:hover { color: var(--accent); }
  .g { width: 14px; text-align: center; font-weight: 700; }
  .g.pass { color: var(--status-add, #2ea043); }
  .g.fail { color: var(--err, #c0392b); }
  .g.pending { color: var(--status-mod, #d29922); }
  .checks-strip summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .ck-sum { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 4px; font-size: 12.5px; }
  .ck-sum .g { width: auto; }
  .ck-sum .sep { color: var(--text-muted); margin: 0 2px; }
  .ck-label { color: var(--text-muted); font-size: 11.5px; text-transform: uppercase; letter-spacing: 0.03em; }
  .comments h3, .activity h3 { font-size: 13px; margin: 6px 0; }
  .comment { border: 1px solid var(--border); border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; background: var(--panel-bg); }
  .chead { font-size: 12.5px; margin-bottom: 4px; }
  .when { color: var(--text-muted); }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11.5px; }
  .fstat { font-size: 11.5px; }
  .note { margin: 8px 2px; color: var(--text-muted); font-size: 12.5px; }
  .note.err { color: var(--err, #c0392b); }
</style>
