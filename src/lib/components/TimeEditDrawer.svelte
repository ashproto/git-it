<script lang="ts">
  import { timeEditDrawer } from "../timeEditDrawer.svelte";
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";
  import EditTabs from "./EditTabs.svelte";
  import ApplyPanel from "./ApplyPanel.svelte";

  let closeBtn = $state<HTMLButtonElement | undefined>();

  // Focus the close button when the modal opens (a predictable, always-present
  // focus target; the editor fields live in a reused child component).
  $effect(() => {
    if (timeEditDrawer.open) {
      Promise.resolve().then(() => closeBtn?.focus());
    }
  });

  // Commits currently selected in the graph, in graph order.
  const selectedCommits = $derived(
    appState.graphCommits.filter((c) => appState.selected.has(c.sha)),
  );
  const selectedCount = $derived(selectedCommits.length);

  // ── Single-commit message editing ──────────────────────────────────────────
  // Only when exactly one commit is selected.
  const only = $derived(selectedCommits.length === 1 ? (selectedCommits[0] ?? null) : null);
  const isHead = $derived(!!only && only.refs.some((r) => r.is_head));
  const isMerge = $derived(!!only && only.parents.length > 1);
  const isRoot = $derived(!!only && only.parents.length === 0);
  const rewordable = $derived(!!only && (isHead || (!isMerge && !isRoot)));

  let message = $state("");
  let originalMessage = $state("");
  let loadingMessage = $state(false);
  // The sha whose message we last seeded into the textarea — so we only re-seed
  // when the selected commit actually changes (never clobber an in-progress edit).
  let seededSha = $state<string | null>(null);

  $effect(() => {
    // Reset the seed when the modal closes so reopening always refetches.
    if (!timeEditDrawer.open) {
      seededSha = null;
      return;
    }
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
    timeEditDrawer.close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      timeEditDrawer.close();
    }
  }
</script>

{#if timeEditDrawer.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) timeEditDrawer.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="drawer panel" role="dialog" aria-modal="true" aria-label="Edit commits">
      <header class="drawer-head">
        <div class="titles">
          <h3>Edit commit(s)</h3>
          <span class="sel-count">
            {selectedCount}
            {selectedCount === 1 ? "commit" : "commits"} selected
          </span>
        </div>
        <button
          class="close"
          bind:this={closeBtn}
          onclick={() => timeEditDrawer.close()}
          aria-label="Close commit editor"
          title="Close commit editor"
        >✕</button>
      </header>

      {#if selectedCount === 0}
        <p class="empty-hint">
          Select one or more commits in the list, then choose an edit mode below.
        </p>
      {:else}
        <section class="commits">
          <ul class="commit-list">
            {#each selectedCommits as c (c.sha)}
              <li class="commit-row">
                <code class="sha">{c.sha.slice(0, 9)}</code>
                <span class="subject" title={c.subject}>{c.subject}</span>
                {#if c.refs.some((r) => r.is_head)}
                  <span class="head-badge">HEAD</span>
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}

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

      <section class="timestamps-section">
        <h4>Timestamps</h4>
        <div class="body">
          <EditTabs />
          <ApplyPanel />
        </div>
      </section>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 3000;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    justify-content: center;
    align-items: center;
  }
  .drawer {
    width: min(720px, calc(100vw - 32px));
    max-height: 85vh;
    height: auto;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.34);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    box-sizing: border-box;
    overflow-y: auto;
    animation: pop-in 0.16s ease-out;
  }
  @keyframes pop-in {
    from { transform: scale(0.98); opacity: 0.4; }
    to   { transform: scale(1);    opacity: 1; }
  }
  .drawer-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  h4 {
    margin: 0 0 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .sel-count {
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .close {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
    line-height: 1;
  }
  .close:hover {
    background: var(--btn-hover);
  }
  .empty-hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .commit-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 168px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    font-size: 12px;
  }
  .commit-row + .commit-row {
    border-top: 1px solid var(--border);
  }
  .commit-row .sha {
    flex-shrink: 0;
    font-family: var(--mono, ui-monospace, monospace);
    color: var(--text-muted);
  }
  .commit-row .subject {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .head-badge {
    flex-shrink: 0;
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    padding: 1px 5px;
    border-radius: 999px;
    background: var(--accent, #2563eb);
    color: #fff;
  }
  .message-section,
  .timestamps-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .section-hint {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  .section-hint.muted {
    font-style: italic;
  }
  .message-input {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 84px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
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
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
