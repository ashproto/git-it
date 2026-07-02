<script lang="ts">
  import { appState } from "../../store.svelte";
  import { reviewDraft, type ReviewVerdict } from "../../reviewDraft.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { dialogs } from "../../dialogs.svelte";

  // Pending-review bar for one PR: collapsed it shows either a subtle
  // "Review changes" entry point or the pending-comment count; expanded it
  // holds the summary + verdict and submits the whole draft in one shot.
  let { number }: { number: number } = $props();

  let expanded = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Safe to read/write the global draft store from this bar only when it is
  // unbound or bound to THIS (repo, PR) — a draft pending on another PR must
  // never surface here (no count, no prefill) nor be touched by submit/discard.
  const mine = $derived(
    !reviewDraft.bound || (!!appState.repo && reviewDraft.belongsTo(appState.repo, number)),
  );
  const count = $derived(mine ? reviewDraft.count : 0);

  // LOCAL summary/verdict — what this panel edits and submits. When `mine` they
  // seed from the store on expand and write back through as the user types (so
  // the draft's summary survives collapse/reopen); when the draft belongs to
  // another PR they start blank/COMMENT and NEVER touch the store.
  let summary = $state("");
  let verdict = $state<ReviewVerdict>("COMMENT");

  function expand() {
    summary = mine ? reviewDraft.summary : "";
    verdict = mine ? reviewDraft.verdict : "COMMENT";
    error = null;
    expanded = true;
  }
  function setSummary(s: string) {
    summary = s;
    if (mine) reviewDraft.setSummary(s);
  }
  function setVerdict(v: ReviewVerdict) {
    verdict = v;
    if (mine) reviewDraft.setVerdict(v);
  }

  const verdicts: Array<{ value: ReviewVerdict; label: string }> = [
    { value: "COMMENT", label: "Comment" },
    { value: "APPROVE", label: "Approve" },
    { value: "REQUEST_CHANGES", label: "Request changes" },
  ];

  async function submit() {
    if (submitting) return;
    submitting = true;
    error = null;
    const res = await githubActions.submitReview(number, verdict, summary);
    submitting = false;
    if (res.ok) {
      summary = "";
      verdict = "COMMENT";
      expanded = false;
    } else error = res.error ?? "Could not submit the review.";
  }

  async function discard() {
    if (submitting) return;
    const ok = await dialogs.confirm({
      title: "Discard review draft",
      message:
        count > 0
          ? `Discard your pending review (${count} draft comment${count === 1 ? "" : "s"})? This cannot be undone.`
          : "Discard your pending review draft? This cannot be undone.",
      confirmLabel: "Discard draft",
      danger: true,
    });
    if (!ok) return;
    // The button only renders when `mine`, so this never touches another PR's draft.
    reviewDraft.discard();
    summary = "";
    verdict = "COMMENT";
    error = null;
    expanded = false;
  }
</script>

<div class="review-bar">
  {#if !expanded}
    {#if count > 0}
      <button type="button" class="finish" onclick={expand}>
        {count} pending comment{count === 1 ? "" : "s"} · Finish review
      </button>
    {:else}
      <button type="button" class="start" onclick={expand}>Review changes</button>
    {/if}
  {:else}
    <div class="panel">
      <div class="p-head">
        <strong>Submit review</strong>
        {#if count > 0}
          <span class="p-count">{count} pending comment{count === 1 ? "" : "s"}</span>
        {/if}
      </div>
      <textarea
        value={summary}
        oninput={(e) => setSummary(e.currentTarget.value)}
        placeholder="Leave a summary (optional for Approve)"
        rows="3"
        disabled={submitting}
        aria-label="Review summary"
      ></textarea>
      <div class="verdicts" role="radiogroup" aria-label="Review verdict">
        {#each verdicts as v (v.value)}
          <label class="verdict">
            <input
              type="radio"
              name="review-verdict"
              value={v.value}
              checked={verdict === v.value}
              disabled={submitting}
              onchange={() => setVerdict(v.value)}
            />
            {v.label}
          </label>
        {/each}
      </div>
      {#if error}<p class="err">{error}</p>{/if}
      <div class="p-row">
        {#if mine}
          <button type="button" class="discard" onclick={discard} disabled={submitting}>
            Discard draft
          </button>
        {/if}
        <span class="spacer"></span>
        <button type="button" class="collapse" onclick={() => (expanded = false)} disabled={submitting}>
          Collapse
        </button>
        <button type="button" class="submit" onclick={submit} disabled={submitting}>
          {submitting ? "Submitting…" : "Submit review"}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .review-bar {
    display: flex;
    flex-direction: column;
  }
  .start,
  .finish {
    align-self: flex-start;
    padding: 4px 14px;
    border-radius: 7px;
    font-size: 12.5px;
    cursor: pointer;
  }
  .start {
    background: var(--btn-bg);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }
  .start:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  .finish {
    background: var(--accent);
    border: 1px solid var(--accent);
    color: #fff;
    font-weight: 500;
  }
  .finish:hover {
    filter: brightness(1.06);
  }
  .panel {
    border: 1px solid var(--accent);
    border-radius: 10px;
    background: var(--panel-bg);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .p-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 13px;
  }
  .p-count {
    font-size: 11.5px;
    color: var(--text-muted);
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 56px;
    font: inherit;
    font-size: 13px;
    line-height: 1.5;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--input-bg, var(--panel-bg));
    color: var(--text);
  }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  textarea:disabled {
    opacity: 0.6;
  }
  .verdicts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
  }
  .verdict {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    cursor: pointer;
  }
  .verdict input {
    accent-color: var(--accent);
    margin: 0;
  }
  .err {
    margin: 0;
    font-size: 12px;
    color: var(--err, #c0392b);
  }
  .p-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .spacer {
    flex: 1;
  }
  .p-row button {
    padding: 4px 14px;
    border-radius: 7px;
    font-size: 12.5px;
    cursor: pointer;
  }
  .p-row button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .discard {
    background: var(--btn-bg);
    border: 1px solid var(--err, #c0392b);
    color: var(--err, #c0392b);
  }
  .discard:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  .collapse {
    background: var(--btn-bg);
    border: 1px solid var(--border);
    color: var(--text);
  }
  .collapse:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  .submit {
    background: var(--accent);
    border: none;
    color: #fff;
    font-weight: 500;
  }
  .submit:hover:not(:disabled) {
    filter: brightness(1.06);
  }
</style>
