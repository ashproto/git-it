<script lang="ts">
  import { untrack } from "svelte";
  import { githubActions } from "../../githubActions.svelte";
  import type { MergeMethod } from "../../types";

  const pending = $derived(githubActions.pending);

  let body = $state("");
  let title = $state("");
  let method = $state<MergeMethod>("squash");
  let seen: unknown = null;

  // Reset the fields whenever a new action is opened.
  $effect(() => {
    const p = githubActions.pending;
    untrack(() => {
      if (p !== seen) {
        seen = p;
        body = "";
        title = "";
        method = "squash";
      }
    });
  });

  const METHODS: MergeMethod[] = ["merge", "squash", "rebase"];

  function backdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) githubActions.cancel();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && githubActions.cancel()} />

{#if pending}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" role="presentation" onclick={backdrop}>
    <div class="modal" role="dialog" aria-modal="true">
      {#if pending.kind === "comment"}
        <h3>Comment on {pending.target === "pr" ? "PR" : "issue"} #{pending.number}</h3>
        <p class="sub">{pending.title}</p>
        <textarea bind:value={body} rows="5" placeholder="Leave a comment…"></textarea>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy || !body.trim()} onclick={() => githubActions.submit({ body })}>{githubActions.busy ? "Posting…" : "Comment"}</button>
        </div>
      {:else if pending.kind === "merge"}
        <h3>Merge PR #{pending.number}</h3>
        <p class="sub">{pending.title}</p>
        <div class="methods">
          {#each METHODS as m (m)}
            <label><input type="radio" name="merge-method" value={m} checked={method === m} onchange={() => (method = m)} /> {m}</label>
          {/each}
        </div>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy} onclick={() => githubActions.submit({ method })}>{githubActions.busy ? "Merging…" : `Merge (${method})`}</button>
        </div>
      {:else if pending.kind === "setState"}
        <h3>{pending.to === "closed" ? "Close" : "Reopen"} issue #{pending.number}</h3>
        <p class="sub">{pending.title}</p>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy} onclick={() => githubActions.submit({})}>{githubActions.busy ? "Working…" : pending.to === "closed" ? "Close issue" : "Reopen issue"}</button>
        </div>
      {:else if pending.kind === "create"}
        <h3>New issue</h3>
        <input class="title-in" bind:value={title} placeholder="Title" />
        <textarea bind:value={body} rows="6" placeholder="Describe the issue…"></textarea>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy || !title.trim()} onclick={() => githubActions.submit({ title, body })}>{githubActions.busy ? "Creating…" : "Create issue"}</button>
        </div>
      {/if}
      {#if githubActions.error}<p class="err">Failed: {githubActions.error}</p>{/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    width: min(460px, 92vw);
    background: var(--panel-bg, var(--popover-bg));
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  .sub {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  textarea,
  .title-in {
    width: 100%;
    box-sizing: border-box;
    background: var(--input-bg, var(--btn-bg));
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-dialog);
    padding: 8px 10px;
    font: inherit;
    font-size: 13px;
    resize: vertical;
  }
  .methods {
    display: flex;
    gap: 14px;
    font-size: 13px;
  }
  .methods label {
    display: flex;
    align-items: center;
    gap: 5px;
    text-transform: capitalize;
    cursor: pointer;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .actions button {
    padding: 6px 14px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  .actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .actions .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .err {
    margin: 0;
    color: var(--err, #c0392b);
    font-size: 12.5px;
  }
</style>
