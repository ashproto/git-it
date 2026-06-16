<script lang="ts">
  import { timeEditDrawer } from "../timeEditDrawer.svelte";
  import { appState } from "../store.svelte";
  import CommitMessageEdit from "./CommitMessageEdit.svelte";
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

      <CommitMessageEdit onDone={() => timeEditDrawer.close()} />

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
  .timestamps-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
