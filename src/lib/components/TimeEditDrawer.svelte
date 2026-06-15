<script lang="ts">
  import { timeEditDrawer } from "../timeEditDrawer.svelte";
  import { appState } from "../store.svelte";
  import EditTabs from "./EditTabs.svelte";
  import ApplyPanel from "./ApplyPanel.svelte";

  let closeBtn = $state<HTMLButtonElement | undefined>();

  // Focus the close button when the drawer opens (a predictable, always-present
  // focus target; the editor fields live in a reused child component).
  $effect(() => {
    if (timeEditDrawer.open) {
      Promise.resolve().then(() => closeBtn?.focus());
    }
  });

  const selectedCount = $derived(appState.selected.size);

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
    <div class="drawer panel" role="dialog" aria-modal="true" aria-label="Edit timestamps">
      <header class="drawer-head">
        <div class="titles">
          <h3>Edit timestamps</h3>
          <span class="sel-count">
            {selectedCount}
            {selectedCount === 1 ? "commit" : "commits"} selected
          </span>
        </div>
        <button
          class="close"
          bind:this={closeBtn}
          onclick={() => timeEditDrawer.close()}
          aria-label="Close timestamp editor"
          title="Close"
        >✕</button>
      </header>

      {#if selectedCount === 0}
        <p class="empty-hint">
          Select one or more commits in the list, then choose an edit mode below.
        </p>
      {/if}

      <div class="body">
        <EditTabs />
        <ApplyPanel />
      </div>
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
    justify-content: flex-end;
  }
  .drawer {
    width: 560px;
    max-width: calc(100vw - 32px);
    height: 100%;
    background: var(--popover-bg, var(--panel-bg));
    border-left: 1px solid var(--border);
    box-shadow: -12px 0 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    box-sizing: border-box;
    overflow-y: auto;
    animation: slide-in 0.16s ease-out;
  }
  @keyframes slide-in {
    from { transform: translateX(16px); opacity: 0.4; }
    to   { transform: translateX(0);    opacity: 1; }
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
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
