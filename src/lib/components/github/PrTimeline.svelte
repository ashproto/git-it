<script lang="ts">
  import { appState } from "../../store.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { GhPullDetail, GhCheckRun, GhReviewThread } from "../../types";
  import { buildPrTimeline } from "../../github/prTimeline";
  import Markdown from "./Markdown.svelte";

  let { pr }: { pr: GhPullDetail } = $props();

  const events = $derived(buildPrTimeline(pr));

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
          </div>
          <Markdown src={ev.comment.body} />
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
              <div class="ihead"><strong>{c.author}</strong> <span class="when">{rel(c.createdAt)}</span></div>
              <Markdown src={c.body} />
            </div>
          {/each}
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
      </div>
    </li>
  {/if}
{/snippet}

<style>
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
  .ihead { font-size: 12px; margin-bottom: 2px; }
  .ci-name { color: var(--text); text-decoration: none; }
  a.ci-name:hover { color: var(--accent); }
  .g { width: 14px; text-align: center; font-weight: 700; }
  .g.pass { color: var(--status-add, #2ea043); }
  .g.fail { color: var(--err, #c0392b); }
  .g.pending { color: var(--status-mod, #d29922); }
  .g.neutral { color: var(--text-muted); }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11.5px; }
</style>
