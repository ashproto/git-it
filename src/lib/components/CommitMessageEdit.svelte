<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";

  // Called after a successful message update — e.g. the modal closes itself.
  let { onDone }: { onDone?: () => void } = $props();

  // Commits currently selected in the graph, in graph order.
  const selectedCommits = $derived(
    appState.graphCommits.filter((c) => appState.selected.has(c.sha)),
  );

  // ── Single-commit message editing ──────────────────────────────────────────
  // Only when exactly one commit is selected.
  const only = $derived(selectedCommits.length === 1 ? (selectedCommits[0] ?? null) : null);
  const isHead = $derived(!!only && only.refs.some((r) => r.is_head));
  const isMerge = $derived(!!only && only.parents.length > 1);
  const isRoot = $derived(!!only && only.parents.length === 0);
  const rewordable = $derived(!!only && (isHead || (!isMerge && !isRoot)));
  // Amending HEAD folds any staged changes into the commit — warn so a user who
  // came here to "edit the message" isn't surprised.
  const headHasStaged = $derived(isHead && (appState.repoStatus?.staged ?? 0) > 0);

  let message = $state("");
  let originalMessage = $state("");
  let loadingMessage = $state(false);
  // The sha whose message we last seeded into the textarea — so we only re-seed
  // when the selected commit actually changes (never clobber an in-progress edit).
  let seededSha = $state<string | null>(null);

  $effect(() => {
    const sha = only?.sha ?? null;
    if (sha === null) {
      // 0 or >1 selected: nothing to edit. Clear so a later single-select reseeds.
      seededSha = null;
      message = "";
      originalMessage = "";
      return;
    }
    if (sha === seededSha) return; // already seeded this commit — leave edits alone
    seededSha = sha;
    loadingMessage = true;
    message = "";
    originalMessage = "";
    gitActions
      .getCommitMessage(sha)
      .then((m) => {
        // Guard against a race: only apply if this is still the seeded commit.
        if (seededSha === sha) {
          message = m;
          originalMessage = m;
        }
      })
      .catch(() => {
        if (seededSha === sha) appState.status = "Couldn't load the commit message.";
      })
      .finally(() => {
        if (seededSha === sha) loadingMessage = false;
      });
  });

  const messageChanged = $derived(message.trim() !== originalMessage.trim());
  const canUpdateMessage = $derived(
    rewordable && message.trim().length > 0 && messageChanged && !loadingMessage,
  );

  async function updateMessage() {
    if (!only || !canUpdateMessage) return;
    await gitActions.setCommitMessage(only, message);
    onDone?.();
  }
</script>

{#if only}
  <section class="message-section">
    <h4>Message</h4>
    <p class="section-hint">
      Editing the message of a commit other than HEAD rewrites it and the
      commits after it.
    </p>
    <textarea
      class="message-input"
      bind:value={message}
      disabled={!rewordable || loadingMessage}
      placeholder={loadingMessage ? "Loading message…" : "Commit message"}
      rows="5"
      spellcheck="true"
    ></textarea>
    {#if isMerge || isRoot}
      <p class="section-hint muted">
        Rewording a merge or root commit isn't supported here.
      </p>
    {:else if headHasStaged}
      <p class="section-hint warn">
        You have staged changes — updating this message also commits them
        into HEAD.
      </p>
    {/if}
    <div class="message-actions">
      <button
        type="button"
        class="primary"
        disabled={!canUpdateMessage}
        onclick={updateMessage}
      >Update message</button>
    </div>
  </section>
{/if}

<style>
  .message-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  h4 {
    margin: 0 0 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .section-hint {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  .section-hint.muted {
    font-style: italic;
  }
  .section-hint.warn {
    color: var(--err);
  }
  .message-input {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 84px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-dialog);
    background: var(--input-bg, var(--btn-bg));
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    line-height: 1.45;
  }
  .message-input:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .message-actions {
    display: flex;
    justify-content: flex-end;
  }
  .message-actions .primary {
    padding: 5px 12px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--accent, #2563eb);
    color: #fff;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .message-actions .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
