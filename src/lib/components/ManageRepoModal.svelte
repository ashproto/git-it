<script lang="ts">
  // "Manage Repository" modal — houses the History & Recovery panels that used to
  // live in the sidebar (Remotes, History/reflog, Backups). Stashes intentionally
  // stay in the sidebar. Structure mirrors SettingsPanel (overlay + dialog + Escape
  // + scrim-close + deferred focus), but laid out as a two-column SECTIONED dialog:
  // a left vertical tab-nav switches the single visible panel on the right.
  import { manageRepo } from "../manageRepo.svelte";
  import BackupsPanel from "./BackupsPanel.svelte";
  import ReflogPanel from "./ReflogPanel.svelte";
  import RemotePanel from "./RemotePanel.svelte";

  type Section = "remotes" | "history" | "backups";

  const sections: { id: Section; label: string }[] = [
    { id: "remotes", label: "Remotes" },
    { id: "history", label: "History" },
    { id: "backups", label: "Backups" },
  ];

  let section = $state<Section>("remotes");

  let closeBtn = $state<HTMLButtonElement | undefined>();

  // Deferred focus on the close button when the modal opens (mirrors SettingsPanel).
  $effect(() => {
    if (manageRepo.open) {
      Promise.resolve().then(() => closeBtn?.focus());
    }
  });

  const activeLabel = $derived(
    sections.find((s) => s.id === section)?.label ?? "",
  );

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      manageRepo.close();
    }
  }

  // WAI-ARIA tablist keyboard nav: arrows / Home / End move selection + focus.
  function onTablistKeydown(e: KeyboardEvent) {
    const idx = sections.findIndex((s) => s.id === section);
    let next = idx;
    if (e.key === "ArrowDown" || e.key === "ArrowRight") next = (idx + 1) % sections.length;
    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") next = (idx - 1 + sections.length) % sections.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = sections.length - 1;
    else return;
    e.preventDefault();
    section = sections[next].id;
    document.getElementById(`manage-tab-${section}`)?.focus();
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

      <div class="body">
        <div class="nav" role="tablist" aria-label="Repository sections" aria-orientation="vertical">
          {#each sections as s (s.id)}
            <button
              type="button"
              role="tab"
              id="manage-tab-{s.id}"
              class="nav-item"
              class:active={section === s.id}
              aria-selected={section === s.id}
              aria-controls="manage-panel"
              tabindex={section === s.id ? 0 : -1}
              onclick={() => (section = s.id)}
            >{s.label}</button>
          {/each}
        </div>

        <div
          class="content"
          id="manage-panel"
          role="tabpanel"
          tabindex="0"
          aria-labelledby="manage-tab-{section}"
          aria-label={activeLabel}
        >
          {#if section === "remotes"}
            <RemotePanel bare />
          {:else if section === "history"}
            <ReflogPanel bare />
          {:else}
            <BackupsPanel bare />
          {/if}
        </div>
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
    width: 640px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 80px);
    overflow: hidden;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
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

  /* Two-column body: left tab-nav + right scrolling content. */
  .body {
    display: flex;
    gap: 14px;
    min-height: 0;
    flex: 1;
  }

  .nav {
    flex: 0 0 150px;
    width: 150px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text-muted);
    font-size: 12.5px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .nav-item:hover {
    background: var(--row-hover);
    color: var(--text);
  }

  .nav-item.active {
    background: var(--accent);
    color: var(--on-accent);
  }

  .nav-item:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
  }

  .content:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius-md);
  }
</style>
