<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { GhCommentKind, GhPullDetail, GhCheckRun, GhReviewThread } from "../../types";
  import { buildPrTimeline } from "../../github/prTimeline";
  import Markdown from "./Markdown.svelte";
  import ReactionBar from "./ReactionBar.svelte";
  import CommentActions from "./CommentActions.svelte";

  let { pr }: { pr: GhPullDetail } = $props();

  // Per-view sort direction, seeded from the persisted default. Track only the
  // PR's identity (not the global default) so toggling the in-view control
  // doesn't get yanked, and so changing the Settings default mid-view doesn't
  // reset what the user already chose for the PR they're looking at.
  let newestFirst = $state(appState.prTimelineNewestFirst);
  let seededFor: number | undefined;
  $effect(() => {
    if (seededFor !== pr.number) {
      seededFor = pr.number;
      newestFirst = appState.prTimelineNewestFirst;
      // Thread-action state is per-PR — never carry a half-typed reply or a
      // stale error over to another PR's timeline.
      replyOpenFor = null;
      replyBody = "";
      threadErr = {};
      editingKey = null;
      editBody = "";
      editErr = null;
    }
  });

  // ---- inline thread actions (reply + resolve/unresolve) ----
  // Threads are identified by their GraphQL node id; path:line is only a
  // display-key fallback for threads the API returned without an id.
  function threadKey(t: GhReviewThread): string {
    return t.id || `${t.path}:${t.line}`;
  }
  let replyOpenFor = $state<string | null>(null); // one composer across the whole timeline
  let replyBody = $state("");
  let replyBusy = $state(false);
  let resolveBusyFor = $state<string | null>(null);
  let threadErr = $state<Record<string, string>>({});

  function openReply(t: GhReviewThread) {
    if (replyBusy) return;
    replyOpenFor = threadKey(t); // opening one closes any other
    replyBody = "";
  }
  function cancelReply() {
    if (replyBusy) return;
    replyOpenFor = null;
    replyBody = "";
  }
  async function sendReply(t: GhReviewThread) {
    const commentId = t.comments[0]?.databaseId;
    const text = replyBody.trim();
    if (commentId == null || !text || replyBusy) return;
    const k = threadKey(t);
    replyBusy = true;
    delete threadErr[k];
    const res = await githubActions.replyThread(pr.number, commentId, text);
    replyBusy = false;
    if (res.ok) {
      // Collapse; the bumpReload re-fetch will show the new reply.
      replyOpenFor = null;
      replyBody = "";
    } else {
      threadErr[k] = res.error ?? "Could not post reply.";
    }
  }
  // ---- edit-in-place (own comments) ----
  // One edit open at a time across the whole timeline; the key namespaces the
  // two comment families ("ic:<id>" timeline comments, "rc:<databaseId>"
  // inline review comments). The textarea is prefilled with the RAW markdown
  // (`body` is the raw source; Markdown.svelte only renders it).
  let editingKey = $state<string | null>(null);
  let editBody = $state("");
  let editBusy = $state(false);
  let editErr = $state<string | null>(null);

  function startEdit(key: string, raw: string) {
    if (editBusy) return;
    editingKey = key;
    editBody = raw;
    editErr = null;
  }
  function cancelEdit() {
    if (editBusy) return;
    editingKey = null;
    editBody = "";
    editErr = null;
  }
  async function saveEdit(editKind: GhCommentKind, target: number | null) {
    if (target == null || !editBody.trim() || editBusy) return;
    editBusy = true;
    editErr = null;
    const res = await githubActions.editComment(editKind, target, editBody);
    editBusy = false;
    if (res.ok) {
      // Collapse; the bumpReload re-fetch shows the new text.
      editingKey = null;
      editBody = "";
    } else {
      editErr = res.error ?? "Could not save the edit.";
    }
  }

  async function toggleResolve(t: GhReviewThread) {
    if (!t.id || resolveBusyFor !== null) return;
    const k = threadKey(t);
    resolveBusyFor = k;
    delete threadErr[k];
    const res = await githubActions.resolveThread(t.id, !t.resolved);
    resolveBusyFor = null;
    if (!res.ok) threadErr[k] = res.error ?? "Could not update thread.";
  }

  const events = $derived.by(() => {
    const asc = buildPrTimeline(pr);
    return newestFirst ? [...asc].reverse() : asc;
  });

  function rel(iso: string): string {
    if (!iso) return "";
    const dt = parseISO(iso);
    return dt ? formatCommitDate(dt, appState.dateFormat, appState.relativeDates) : iso;
  }

  // Decision label → colour class for a review.
  function reviewClass(state: string): string {
    if (state === "APPROVED") return "ok";
    if (state === "CHANGES_REQUESTED") return "warn";
    return "neutral";
  }

  // CI conclusion/status → glyph + colour class.
  function ciBucket(run: GhCheckRun): "pass" | "fail" | "pending" | "neutral" {
    if (run.status !== "completed") return "pending";
    switch (run.conclusion) {
      case "success":
        return "pass";
      case "failure":
      case "timed_out":
        return "fail";
      case "":
        return "pending";
      default:
        return "neutral";
    }
  }
  function ciGlyph(b: string): string {
    return b === "pass" ? "✓" : b === "fail" ? "✗" : b === "pending" ? "…" : "–";
  }

  // Duration between two ISO timestamps: "m:ss" under an hour, "h:mm:ss" at or
  // over an hour. "" when either timestamp is missing/unparseable.
  function duration(startIso: string, endIso: string): string {
    const a = parseISO(startIso);
    const b = parseISO(endIso);
    if (!a || !b) return "";
    let secs = Math.round((b.getTime() - a.getTime()) / 1000);
    if (secs < 0) secs = 0;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    const ss = s.toString().padStart(2, "0");
    if (h > 0) return `${h}:${m.toString().padStart(2, "0")}:${ss}`;
    return `${m}:${ss}`;
  }
</script>

<div class="order-row">
  <button
    type="button"
    class="order-btn"
    aria-pressed={newestFirst}
    onclick={() => (newestFirst = !newestFirst)}
    title={newestFirst ? "Showing newest first — click for oldest first" : "Showing oldest first — click for newest first"}
  >
    <span class="arrow">{newestFirst ? "↓" : "↑"}</span>
    {newestFirst ? "Newest first" : "Oldest first"}
  </button>
</div>

<ol class="timeline">
  {#each events as ev, i (ev.kind + i)}
    <li class="row {ev.kind}">
      <span class="rail" aria-hidden="true"><span class="dot {ev.kind}"></span></span>
      <div class="card">
        {#if ev.kind === "commit"}
          <div class="line">
            <span class="who">{ev.commit.author || "unknown"}</span>
            <span class="sha mono">{ev.commit.oid.slice(0, 7)}</span>
            <span class="msg">{ev.commit.message}</span>
            <span class="when">{rel(ev.commit.committedDate)}</span>
          </div>
        {:else if ev.kind === "comment"}
          <div class="chead">
            <strong>{ev.comment.author}</strong>
            <span class="when">commented {rel(ev.comment.createdAt)}</span>
            <CommentActions
              author={ev.comment.author}
              kind="issueComment"
              target={ev.comment.id}
              body={ev.comment.body}
              onEditStart={() => startEdit(`ic:${ev.comment.id}`, ev.comment.body)}
            />
          </div>
          {#if ev.comment.id != null && editingKey === `ic:${ev.comment.id}`}
            {@render editBox("issueComment", ev.comment.id)}
          {:else}
            <Markdown src={ev.comment.body} />
            <ReactionBar reactions={ev.comment.reactions} kind="issueComment" target={ev.comment.id} />
          {/if}
        {:else if ev.kind === "review"}
          <div class="chead">
            <strong>{ev.review.author}</strong>
            <span class="rv {reviewClass(ev.review.state)}">{ev.review.state.replace(/_/g, " ").toLowerCase()}</span>
            <span class="when">{rel(ev.review.submittedAt)}</span>
          </div>
          {#if ev.review.body.trim()}<Markdown src={ev.review.body} />{/if}
          {#if ev.threads.length}
            <ul class="threads">
              {#each ev.threads as t, ti (t.path + ":" + t.line + ":" + ti)}
                {@render threadItem(t)}
              {/each}
            </ul>
          {/if}
        {:else if ev.kind === "reviewThread"}
          <ul class="threads">
            {@render threadItem(ev.thread)}
          </ul>
        {:else if ev.kind === "ciRun"}
          {@const b = ciBucket(ev.run)}
          {@const dur = duration(ev.run.startedAt, ev.run.updatedAt)}
          <div class="line">
            <span class="g {b}">{ciGlyph(b)}</span>
            {#if ev.run.url}
              <a class="ci-name" href={ev.run.url} target="_blank" rel="noreferrer">{ev.run.name || "workflow"}</a>
            {:else}
              <span class="ci-name">{ev.run.name || "workflow"}</span>
            {/if}
            <span class="when">
              started {rel(ev.run.startedAt)}{#if dur} · ran {dur}{/if}
            </span>
          </div>
        {/if}
      </div>
    </li>
  {/each}
</ol>

{#snippet threadItem(t: GhReviewThread)}
  {#if t.resolved}
    <li class="thread resolved">
      <details>
        <summary>
          <span class="badge resolved">resolved</span>
          <span class="loc mono">{t.path}:{t.line}</span>
          <span class="cnt">{t.comments.length} {t.comments.length === 1 ? "comment" : "comments"}</span>
        </summary>
        <div class="thread-body">
          {#each t.comments as c, ci (c.author + ci)}
            <div class="icomment">
              <div class="ihead">
                <strong>{c.author}</strong>
                <span class="when">{rel(c.createdAt)}</span>
                <CommentActions
                  author={c.author}
                  kind="reviewComment"
                  target={c.databaseId}
                  body={c.body}
                  onEditStart={() => startEdit(`rc:${c.databaseId}`, c.body)}
                />
              </div>
              {#if c.databaseId != null && editingKey === `rc:${c.databaseId}`}
                {@render editBox("reviewComment", c.databaseId)}
              {:else}
                <Markdown src={c.body} />
                <ReactionBar reactions={c.reactions} kind="reviewComment" target={c.databaseId} />
              {/if}
            </div>
          {/each}
          {@render threadActions(t)}
        </div>
      </details>
    </li>
  {:else}
    <li class="thread">
      <div class="thead">
        <span class="badge unresolved">unresolved</span>
        <span class="loc mono">{t.path}:{t.line}</span>
      </div>
      <div class="thread-body">
        {#each t.comments as c, ci (c.author + ci)}
          <div class="icomment">
            <div class="ihead"><strong>{c.author}</strong> <span class="when">{rel(c.createdAt)}</span></div>
            <Markdown src={c.body} />
          </div>
        {/each}
        {@render threadActions(t)}
      </div>
    </li>
  {/if}
{/snippet}

{#snippet threadActions(t: GhReviewThread)}
  {@const k = threadKey(t)}
  {@const canReply = (t.comments[0]?.databaseId ?? null) !== null}
  {#if canReply || t.id}
    <div class="tactions">
      {#if canReply && replyOpenFor !== k}
        <button type="button" class="tbtn" onclick={() => openReply(t)}>Reply</button>
      {/if}
      {#if t.id}
        <button
          type="button"
          class="tbtn"
          disabled={resolveBusyFor !== null}
          onclick={() => void toggleResolve(t)}
        >
          {resolveBusyFor === k
            ? t.resolved
              ? "Unresolving…"
              : "Resolving…"
            : t.resolved
              ? "Unresolve"
              : "Resolve"}
        </button>
      {/if}
    </div>
    {#if canReply && replyOpenFor === k}
      <div class="reply-box">
        <textarea
          bind:value={replyBody}
          placeholder="Reply…"
          rows="2"
          disabled={replyBusy}
          aria-label="Reply to review thread"
          onkeydown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
              e.preventDefault();
              void sendReply(t);
            }
          }}
        ></textarea>
        <div class="reply-row">
          <span class="hint">⌘⏎ to submit</span>
          <button type="button" class="tbtn" disabled={replyBusy} onclick={cancelReply}>Cancel</button>
          <button
            type="button"
            class="tbtn primary"
            disabled={replyBusy || !replyBody.trim()}
            onclick={() => void sendReply(t)}
          >
            {replyBusy ? "Replying…" : "Reply"}
          </button>
        </div>
      </div>
    {/if}
    {#if threadErr[k]}<p class="terr">{threadErr[k]}</p>{/if}
  {/if}
{/snippet}

{#snippet editBox(editKind: GhCommentKind, target: number)}
  <div class="reply-box">
    <textarea
      bind:value={editBody}
      rows="4"
      disabled={editBusy}
      aria-label="Edit comment"
      onkeydown={(e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
          e.preventDefault();
          void saveEdit(editKind, target);
        }
      }}
    ></textarea>
    <div class="reply-row">
      <span class="hint">⌘⏎ to save</span>
      <button type="button" class="tbtn" disabled={editBusy} onclick={cancelEdit}>Cancel</button>
      <button
        type="button"
        class="tbtn primary"
        disabled={editBusy || !editBody.trim()}
        onclick={() => void saveEdit(editKind, target)}
      >
        {editBusy ? "Saving…" : "Save"}
      </button>
    </div>
    {#if editErr}<p class="terr">{editErr}</p>{/if}
  </div>
{/snippet}

<style>
  .order-row {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 8px;
  }
  .order-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    color: var(--text-muted);
    background: var(--btn-bg);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 3px 10px;
    cursor: pointer;
  }
  .order-btn:hover {
    color: var(--text);
    border-color: var(--text-muted);
  }
  .order-btn .arrow {
    color: var(--accent);
    font-weight: 700;
  }
  .timeline {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: 26px 1fr;
    gap: 8px;
    position: relative;
  }
  .rail {
    position: relative;
    display: flex;
    justify-content: center;
  }
  /* vertical line spanning each row, drawn behind the dot */
  .rail::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 2px;
    transform: translateX(-50%);
    background: var(--border);
  }
  .row:first-child .rail::before { top: 10px; }
  .row:last-child .rail::before { bottom: calc(100% - 18px); }
  .dot {
    position: relative;
    z-index: 1;
    margin-top: 6px;
    width: 10px;
    height: 10px;
    border-radius: 999px;
    background: var(--panel-bg);
    border: 2px solid var(--text-muted);
  }
  .dot.commit { border-color: var(--accent); }
  .dot.comment { border-color: var(--text-muted); }
  .dot.review,
  .dot.reviewThread { border-color: var(--status-mod, #d29922); }
  .dot.ciRun { border-color: var(--status-add, #2ea043); }
  .card {
    min-width: 0;
    padding: 4px 0 12px;
  }
  .row.comment .card,
  .row.review .card {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 10px 12px;
    margin-bottom: 12px;
    background: var(--panel-bg);
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
    font-size: 12.5px;
  }
  .who { color: var(--text); }
  .sha {
    color: var(--accent);
  }
  .msg { color: var(--text); flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; }
  .when { color: var(--text-muted); font-size: 12px; }
  .chead { font-size: 12.5px; margin-bottom: 4px; display: flex; flex-wrap: wrap; gap: 8px; align-items: baseline; }
  .rv {
    text-transform: capitalize;
    font-size: 11px;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid currentColor;
  }
  .rv.ok { color: var(--status-add, #2ea043); }
  .rv.warn { color: var(--status-mod, #d29922); }
  .rv.neutral { color: var(--text-muted); }
  .threads { list-style: none; margin: 8px 0 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .thread {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
    background: var(--btn-bg);
  }
  .thread.resolved { opacity: 0.7; }
  .thread.resolved summary { cursor: pointer; list-style: none; }
  .thread.resolved summary::-webkit-details-marker { display: none; }
  .thead, .thread.resolved summary {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    font-size: 12px;
  }
  .badge {
    font-size: 10px;
    padding: 0 6px;
    border-radius: 999px;
    border: 1px solid currentColor;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .badge.resolved { color: var(--text-muted); }
  .badge.unresolved { color: var(--status-mod, #d29922); }
  .loc { color: var(--text-muted); }
  .cnt { color: var(--text-muted); font-size: 11.5px; }
  .thread-body { margin-top: 6px; display: flex; flex-direction: column; gap: 6px; }
  .icomment { border-top: 1px solid var(--border); padding-top: 6px; }
  .icomment:first-child { border-top: none; padding-top: 0; }
  .ihead { font-size: 12px; margin-bottom: 2px; display: flex; flex-wrap: wrap; gap: 6px; align-items: baseline; }
  /* Reveal own-comment edit/delete actions on row hover (GitHub-style). */
  .row.comment .card:hover :global(.cacts),
  .icomment:hover :global(.cacts) { opacity: 1; }
  .tactions {
    display: flex;
    gap: 10px;
    margin-top: 4px;
  }
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
  .reply-box {
    margin-top: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .reply-box textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 48px;
    font: inherit;
    font-size: 12.5px;
    line-height: 1.5;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--input-bg, var(--panel-bg));
    color: var(--text);
  }
  .reply-box textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .reply-box textarea:disabled { opacity: 0.6; }
  .reply-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .reply-row .hint {
    margin-right: auto;
    font-size: 11px;
    color: var(--text-muted);
  }
  .terr {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--status-del, #d22323);
  }
  .ci-name { color: var(--text); text-decoration: none; }
  a.ci-name:hover { color: var(--accent); }
  .g { width: 14px; text-align: center; font-weight: 700; }
  .g.pass { color: var(--status-add, #2ea043); }
  .g.fail { color: var(--err, #c0392b); }
  .g.pending { color: var(--status-mod, #d29922); }
  .g.neutral { color: var(--text-muted); }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11.5px; }
</style>
