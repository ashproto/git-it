<script lang="ts">
  import { githubState } from "../../githubState.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { dialogs } from "../../dialogs.svelte";
  import type { GhCommentKind } from "../../types";

  // Edit/delete affordances for the viewer's OWN comment/description. Renders
  // nothing unless the signed-in login matches the author and the target id is
  // known. Parents reveal it on row hover via `:global(.cacts)`; it also
  // reveals itself on keyboard focus. `body` rides along so parents can hand
  // the raw markdown to their edit mode without re-plumbing.
  let {
    author,
    kind,
    target,
    body,
    onEditStart,
  }: {
    author: string;
    kind: GhCommentKind;
    target: number | null;
    body: string; // pass-through: parents seed their edit mode from it
    onEditStart: () => void;
  } = $props();

  const show = $derived(
    githubState.login !== null && githubState.login === author && target !== null,
  );
  // Descriptions (body kinds) can be edited but never deleted.
  const deletable = $derived(kind === "issueComment" || kind === "reviewComment");

  let deleting = $state(false);
  let error = $state<string | null>(null);

  async function del() {
    if (target == null || deleting) return;
    const ok = await dialogs.confirm({
      title: "Delete comment",
      message: "Delete this comment? This can't be undone.",
      confirmLabel: "Delete",
      danger: true,
    });
    if (!ok) return;
    deleting = true;
    error = null;
    const res = await githubActions.deleteComment(kind, target);
    deleting = false;
    if (!res.ok) error = res.error ?? "Could not delete comment.";
  }
</script>

{#if show}
  <span class="cacts">
    <button type="button" class="ib" aria-label="Edit comment" title="Edit" onclick={onEditStart}>✏️</button>
    {#if deletable}
      <button
        type="button"
        class="ib"
        aria-label="Delete comment"
        title="Delete"
        disabled={deleting}
        onclick={() => void del()}
      >🗑</button>
    {/if}
  </span>
  <!-- Outside the hover-revealed span: a failed delete must stay visible after
       the pointer leaves the row. -->
  {#if error}<span class="cerr">{error}</span>{/if}
{/if}

<style>
  .cacts {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .cacts:focus-within {
    opacity: 1;
  }
  .ib {
    background: none;
    border: none;
    padding: 0 3px;
    font-size: 12px;
    line-height: 1;
    color: var(--text-muted);
    cursor: pointer;
  }
  .ib:hover:not(:disabled) {
    color: var(--accent);
  }
  .ib:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .cerr {
    font-size: 11.5px;
    color: var(--status-del, #d22323);
  }
</style>
