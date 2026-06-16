<script lang="ts">
  // "Manage Repository" modal — houses the History & Recovery panels that used to
  // live in the sidebar (Backups, History/reflog, Remotes). Stashes intentionally
  // stay in the sidebar. Structure mirrors SettingsPanel (overlay + dialog + Escape
  // + scrim-close + deferred focus).
  import { manageRepo } from "../manageRepo.svelte";
  import BackupsPanel from "./BackupsPanel.svelte";
  import ReflogPanel from "./ReflogPanel.svelte";
  import RemotePanel from "./RemotePanel.svelte";

  let closeBtn = $state<HTMLButtonElement | undefined>();

  $effect(() => {
    if (manageRepo.open) {
      Promise.resolve().then(() => closeBtn?.focus());
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      manageRepo.close();
    }
  }
</script>

{#if manageRepo.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) manageRepo.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Manage Repository">
      <div class="header">
        <h3>Manage Repository</h3>
        <button
          type="button"
          class="close-btn"
          aria-label="Close Manage Repository"
          bind:this={closeBtn}
          onclick={() => manageRepo.close()}
        >✕</button>
      </div>

      <div class="panels">
        <BackupsPanel />
        <ReflogPanel />
        <RemotePanel />
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
    align-items: center;
    justify-content: center;
  }

  .dialog {
    width: 560px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    line-height: 1;
  }
  .close-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }
  .close-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .panels {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>
