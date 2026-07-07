<script lang="ts">
  import { githubActions } from "../../githubActions.svelte";

  // GitHub-style inline comment composer, mounted at the bottom of a PR/issue
  // detail. Posts via the existing github_pr_comment/github_issue_comment command
  // and bumps the reload nonce, so the new comment re-loads into the timeline.
  let { target, number }: { target: "pr" | "issue"; number: number } = $props();

  let body = $state("");
  let posting = $state(false);
  let err = $state<string | null>(null);

  async function submit() {
    const text = body.trim();
    if (!text || posting) return;
    posting = true;
    err = null;
    const res = await githubActions.commentInline(target, number, text);
    posting = false;
    if (res.ok) body = "";
    else err = res.error ?? "Could not post comment.";
  }
</script>

<section class="comment-box">
  <textarea
    bind:value={body}
    placeholder="Add a comment…"
    rows="3"
    disabled={posting}
    aria-label="Add a comment"
    onkeydown={(e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        void submit();
      }
    }}
  ></textarea>
  {#if err}<p class="err">{err}</p>{/if}
  <div class="row">
    <span class="hint">⌘⏎ to submit</span>
    <button type="button" class="post" onclick={submit} disabled={posting || !body.trim()}>
      {posting ? "Commenting…" : "Comment"}
    </button>
  </div>
</section>

<style>
  .comment-box {
    margin-top: 14px;
    border-top: 1px solid var(--border);
    padding-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 64px;
    font: inherit;
    font-size: 13px;
    line-height: 1.5;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-dialog);
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
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .hint {
    margin-right: auto;
    font-size: 11px;
    color: var(--text-muted);
  }
  .err {
    margin: 0;
    font-size: 12px;
    color: var(--status-del, #d22323);
  }
  .post {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 7px;
    padding: 5px 16px;
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
  }
  .post:hover:not(:disabled) {
    filter: brightness(1.06);
  }
  .post:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
