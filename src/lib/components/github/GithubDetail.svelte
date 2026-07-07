<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { gitActions } from "../../gitActions";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { GhComment, GhCommentKind, GhPullDetail, GhIssueDetail } from "../../types";
  import Markdown from "./Markdown.svelte";
  import GithubSkeleton from "./GithubSkeleton.svelte";
  import ReactionBar from "./ReactionBar.svelte";
  import CommentActions from "./CommentActions.svelte";
  import PrTimeline from "./PrTimeline.svelte";
  import PrFilesTab from "./PrFilesTab.svelte";
  import PrCommitsTab from "./PrCommitsTab.svelte";
  import ReviewBar from "./ReviewBar.svelte";
  import GithubCommentBox from "./GithubCommentBox.svelte";
  import { splitPatchByFile } from "../../github/prDiff";
  import { revealIn } from "../../github/motion";

  let { kind, number }: { kind: "pr" | "issue"; number: number } = $props();

  // PR sub-tab (Conversation | Files). Reset to Conversation when a DIFFERENT
  // PR is opened; track the number the tab was set for so re-renders of the
  // same PR don't yank the user back.
  let detailTab = $state<"conversation" | "files" | "commits">("conversation");
  let tabFor: number | undefined;
  $effect(() => {
    if (tabFor !== number) {
      tabFor = number;
      detailTab = "conversation";
      // Edit modes are per-item — never carry a half-typed edit to another
      // PR/issue's detail.
      bodyEditing = false;
      bodyText = "";
      bodyErr = null;
      editingId = null;
      editText = "";
      editErr = null;
    }
  });

  // The description reacts/edits under the issues endpoints keyed by NUMBER.
  const bodyKind = $derived<GhCommentKind>(kind === "pr" ? "prBody" : "issueBody");

  // ---- description edit-in-place ----
  // The textarea is prefilled with the RAW markdown (`d.body` is the raw
  // source; Markdown.svelte only renders it). Empty is allowed — it clears
  // the description.
  let bodyEditing = $state(false);
  let bodyText = $state("");
  let bodyBusy = $state(false);
  let bodyErr = $state<string | null>(null);
  function startBodyEdit() {
    if (!d || bodyBusy) return;
    bodyText = d.body;
    bodyErr = null;
    bodyEditing = true;
  }
  function cancelBodyEdit() {
    if (bodyBusy) return;
    bodyEditing = false;
    bodyText = "";
    bodyErr = null;
  }
  async function saveBodyEdit() {
    if (!d || bodyBusy) return;
    bodyBusy = true;
    bodyErr = null;
    const res = await githubActions.editComment(bodyKind, d.number, bodyText);
    bodyBusy = false;
    if (res.ok) {
      bodyEditing = false;
      bodyText = "";
    } else {
      bodyErr = res.error ?? "Could not save the edit.";
    }
  }

  // ---- issue-comment edit-in-place (issue path; PR comments live in PrTimeline) ----
  let editingId = $state<number | null>(null);
  let editText = $state("");
  let editBusy = $state(false);
  let editErr = $state<string | null>(null);
  function startCommentEdit(c: GhComment) {
    if (c.id == null || editBusy) return;
    editingId = c.id;
    editText = c.body; // raw markdown
    editErr = null;
  }
  function cancelCommentEdit() {
    if (editBusy) return;
    editingId = null;
    editText = "";
    editErr = null;
  }
  async function saveCommentEdit(target: number) {
    if (!editText.trim() || editBusy) return;
    editBusy = true;
    editErr = null;
    const res = await githubActions.editComment("issueComment", target, editText);
    editBusy = false;
    if (res.ok) {
      editingId = null;
      editText = "";
    } else {
      editErr = res.error ?? "Could not save the edit.";
    }
  }

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
  // Files-tab count: the loaded diff's real file count once available, else the
  // PR's changedFiles field.
  const filesCount = $derived.by(() => {
    if (!pr) return 0;
    const diff = githubState.prDiff.data;
    return diff != null ? splitPatchByFile(diff).length : pr.changedFiles;
  });
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
        {#if pr}
          <button
            type="button"
            title="Check out this pull request's branch locally (gh pr checkout)"
            onclick={() => gitActions.checkoutPullRequest(d.number)}
          >Checkout</button>
        {/if}
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
      {#if bodyEditing}
        <div class="edit-box">
          <textarea
            bind:value={bodyText}
            rows="6"
            disabled={bodyBusy}
            aria-label="Edit description"
            onkeydown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                e.preventDefault();
                void saveBodyEdit();
              }
            }}
          ></textarea>
          <div class="edit-row">
            <span class="hint">⌘⏎ to save</span>
            <button type="button" class="tbtn" disabled={bodyBusy} onclick={cancelBodyEdit}>Cancel</button>
            <button type="button" class="tbtn primary" disabled={bodyBusy} onclick={() => void saveBodyEdit()}>
              {bodyBusy ? "Saving…" : "Save"}
            </button>
          </div>
          {#if bodyErr}<p class="eerr">{bodyErr}</p>{/if}
        </div>
      {:else}
        <div class="body-acts">
          <CommentActions author={d.author} kind={bodyKind} target={d.number} body={d.body} onEditStart={startBodyEdit} />
        </div>
        {#if d.body.trim()}<Markdown src={d.body} />{:else}<p class="note">No description.</p>{/if}
        <ReactionBar reactions={d.bodyReactions} kind={bodyKind} target={d.number} />
      {/if}
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
        <!-- Pending-review bar — under the checks strip, visible on both tabs. -->
        <ReviewBar number={d.number} />
      </div>
    {/if}

    {#if pr}
      <div class="dtabs">
        <div class="seg" role="group" aria-label="Pull request detail view">
          <button
            type="button"
            class:active={detailTab === "conversation"}
            onclick={() => (detailTab = "conversation")}
            aria-pressed={detailTab === "conversation"}
          >Conversation</button><button
            type="button"
            class:active={detailTab === "files"}
            onclick={() => (detailTab = "files")}
            aria-pressed={detailTab === "files"}
          >Files ({filesCount})</button><button
            type="button"
            class:active={detailTab === "commits"}
            onclick={() => (detailTab = "commits")}
            aria-pressed={detailTab === "commits"}
          >Commits ({pr.commits.length})</button>
        </div>
      </div>
      {#if detailTab === "conversation"}
        <section class="activity">
          <h3>Activity</h3>
          <PrTimeline {pr} />
        </section>
      {:else if detailTab === "files"}
        <PrFilesTab number={d.number} />
      {:else}
        <PrCommitsTab commits={pr.commits} />
      {/if}
    {:else}
      <section class="comments">
        <h3>{d.comments.length} {d.comments.length === 1 ? "comment" : "comments"}</h3>
        {#if d.comments.length === 0}
          <p class="note">No comments yet.</p>
        {:else}
          {#each d.comments as c, i (c.author + i)}
            <article class="comment">
              <div class="chead">
                <strong>{c.author}</strong>
                <span class="when">{rel(c.createdAt)}</span>
                <CommentActions
                  author={c.author}
                  kind="issueComment"
                  target={c.id}
                  body={c.body}
                  onEditStart={() => startCommentEdit(c)}
                />
              </div>
              {#if c.id != null && editingId === c.id}
                <div class="edit-box">
                  <textarea
                    bind:value={editText}
                    rows="4"
                    disabled={editBusy}
                    aria-label="Edit comment"
                    onkeydown={(e) => {
                      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                        e.preventDefault();
                        void saveCommentEdit(c.id!);
                      }
                    }}
                  ></textarea>
                  <div class="edit-row">
                    <span class="hint">⌘⏎ to save</span>
                    <button type="button" class="tbtn" disabled={editBusy} onclick={cancelCommentEdit}>Cancel</button>
                    <button
                      type="button"
                      class="tbtn primary"
                      disabled={editBusy || !editText.trim()}
                      onclick={() => void saveCommentEdit(c.id!)}
                    >
                      {editBusy ? "Saving…" : "Save"}
                    </button>
                  </div>
                  {#if editErr}<p class="eerr">{editErr}</p>{/if}
                </div>
              {:else}
                <Markdown src={c.body} />
                <ReactionBar reactions={c.reactions} kind="issueComment" target={c.id} />
              {/if}
            </article>
          {/each}
        {/if}
      </section>
    {/if}

    {#if !pr || detailTab === "conversation"}
      <GithubCommentBox target={kind} number={d.number} />
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
  .body { position: relative; border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 12px 14px; background: var(--panel-bg); }
  .body-acts { position: absolute; top: 8px; right: 12px; }
  /* Reveal own-comment/description actions on hover (GitHub-style). */
  .body:hover :global(.cacts),
  .comment:hover :global(.cacts) { opacity: 1; }
  .edit-box { display: flex; flex-direction: column; gap: 6px; }
  .edit-box textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 60px;
    font: inherit;
    font-size: 12.5px;
    line-height: 1.5;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--input-bg, var(--panel-bg));
    color: var(--text);
  }
  .edit-box textarea:focus { outline: none; border-color: var(--accent); }
  .edit-box textarea:disabled { opacity: 0.6; }
  .edit-row { display: flex; align-items: center; gap: 10px; }
  .edit-row .hint { margin-right: auto; font-size: 11px; color: var(--text-muted); }
  .tbtn {
    background: none;
    border: none;
    padding: 0;
    font-size: 11.5px;
    color: var(--text-muted);
    cursor: pointer;
  }
  .tbtn:hover:not(:disabled) { color: var(--accent); }
  .tbtn:disabled { opacity: 0.5; cursor: default; }
  .tbtn.primary { color: var(--accent); font-weight: 600; }
  .eerr { margin: 4px 0 0; font-size: 12px; color: var(--status-del, #d22323); }
  .pr-extra { display: flex; flex-direction: column; gap: 8px; }
  .readiness { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; font-size: 12px; }
  .pill { padding: 1px 8px; border: 1px solid var(--border); border-radius: 999px; text-transform: capitalize; color: var(--text-muted); }
  .diffstat { color: var(--text-muted); }
  .add { color: var(--status-add, #2ea043); }
  .del { color: var(--err, #c0392b); }
  .block { border: 1px solid var(--border); border-radius: var(--radius-dialog); padding: 6px 10px; }
  .block summary { cursor: pointer; font-size: 12.5px; }
  .checks { list-style: none; margin: 8px 0 0; padding: 0; }
  .checks li { display: flex; align-items: center; gap: 8px; padding: 2px 0; font-size: 12px; }
  .checks a { color: var(--text); text-decoration: none; }
  .checks a:hover { color: var(--accent); }
  .dtabs { display: flex; margin-top: 2px; }
  .seg {
    display: flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    overflow: hidden;
    flex-shrink: 0;
  }
  .seg button {
    padding: 3px 12px;
    border: none;
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 11.5px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;
  }
  .seg button + button { border-left: 1px solid var(--border); }
  .seg button:hover { background: var(--btn-hover); color: var(--text); }
  .seg button.active { background: var(--accent); color: #fff; }
  .seg button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
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
  .comment { border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 10px 12px; margin-bottom: 8px; background: var(--panel-bg); }
  .chead { font-size: 12.5px; margin-bottom: 4px; display: flex; flex-wrap: wrap; gap: 8px; align-items: baseline; }
  .when { color: var(--text-muted); }
  .mono { font-family: var(--font-mono); font-size: 11.5px; }
  .note { margin: 8px 2px; color: var(--text-muted); font-size: 12.5px; }
  .note.err { color: var(--err, #c0392b); }
</style>
