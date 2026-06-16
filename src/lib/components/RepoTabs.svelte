<script lang="ts">
  import { appState } from "../store.svelte";
  import { pickRepoFolder, api } from "../api";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  function basename(path: string): string {
    return path.split("/").filter(Boolean).pop() ?? path;
  }

  // ── Recent-repos dropdown ────────────────────────────────────────────────────
  let dropdownOpen = $state(false);
  let dropdownRoot: HTMLDivElement | undefined = $state();
  let addBtnEl: HTMLButtonElement | undefined = $state();
  // The menu is position:fixed (anchored to the + button via getBoundingClientRect)
  // so it escapes the tab strip's overflow-x clipping — which previously hid it
  // "behind" the page content.
  let menuPos = $state<{ top: number; left: number } | null>(null);

  function openMenu() {
    if (!addBtnEl) return;
    const r = addBtnEl.getBoundingClientRect();
    const MENU_W = 260;
    const left = Math.max(8, Math.min(r.left, window.innerWidth - MENU_W - 8));
    menuPos = { top: r.bottom + 4, left };
    dropdownOpen = true;
  }

  function toggleDropdown(e: MouseEvent) {
    e.stopPropagation();
    if (dropdownOpen) closeDropdown();
    else openMenu();
  }

  function closeDropdown() {
    dropdownOpen = false;
  }

  function onDocPointerDown(e: PointerEvent) {
    if (dropdownRoot && !dropdownRoot.contains(e.target as Node)) closeDropdown();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closeDropdown();
  }

  // Register / unregister global listeners while the dropdown is open.
  $effect(() => {
    if (!dropdownOpen) return;
    const onBlur = () => closeDropdown();
    // The menu is position:fixed at coords captured on open, so close it if the
    // tab strip scrolls or the window resizes (it would otherwise float away).
    const onReflow = () => closeDropdown();
    document.addEventListener("pointerdown", onDocPointerDown, true);
    document.addEventListener("keydown", onKeydown);
    window.addEventListener("blur", onBlur);
    window.addEventListener("scroll", onReflow, true);
    window.addEventListener("resize", onReflow);
    return () => {
      document.removeEventListener("pointerdown", onDocPointerDown, true);
      document.removeEventListener("keydown", onKeydown);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("scroll", onReflow, true);
      window.removeEventListener("resize", onReflow);
    };
  });

  // Repos in the recent list that aren't already open.
  const recentNotOpen = $derived(
    appState.recentRepos.filter((p) => !appState.openRepos.includes(p)),
  );

  // ── Shared open flow ─────────────────────────────────────────────────────────
  async function openRepoFlow() {
    if (!isTauri()) {
      appState.status = "Opening a repo needs the desktop app.";
      return;
    }
    const p = await pickRepoFolder(appState.repo || undefined);
    if (!p) return;
    if (!(await api.isGitRepo(p))) {
      appState.status = `${p} is not a git repo.`;
      return;
    }
    appState.openRepo(p);
  }

  async function openRecent(path: string) {
    closeDropdown();
    if (!isTauri()) {
      appState.status = "Opening a repo needs the desktop app.";
      return;
    }
    if (!(await api.isGitRepo(path))) {
      appState.status = `${path} is not a git repo.`;
      return;
    }
    appState.openRepo(path);
  }
</script>

<!-- Tab strip — sits flush against --header-bg -->
<div class="tab-strip" data-no-drag>
  {#each appState.openRepos as path (path)}
    <button
      class="tab"
      class:active={path === appState.repo}
      title={path}
      onclick={() => appState.setActiveRepo(path)}
    >
      <span class="tab-name">{basename(path)}</span>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="tab-close"
        title="Close"
        onclick={(e) => { e.stopPropagation(); appState.closeRepo(path); }}
        role="button"
        tabindex="-1"
        aria-label="Close {basename(path)}"
      >×</span>
    </button>
  {/each}

  <!-- + button: opens a menu with "Open repository…" + recent repositories -->
  <div class="add-wrap" bind:this={dropdownRoot}>
    <button
      class="add-btn"
      bind:this={addBtnEl}
      title="Open or recent repositories"
      aria-haspopup="menu"
      aria-expanded={dropdownOpen}
      onclick={toggleDropdown}
      aria-label="Open or recent repositories"
    >+</button>

    {#if dropdownOpen && menuPos}
      <div
        class="dropdown"
        role="menu"
        aria-label="Open or recent repositories"
        style={`top:${menuPos.top}px; left:${menuPos.left}px`}
      >
        <button
          class="drop-item open-item"
          role="menuitem"
          onclick={() => { closeDropdown(); openRepoFlow(); }}
        >Open repository…</button>
        <div class="drop-sep" role="separator"></div>
        <p class="drop-head">Recent</p>
        {#if recentNotOpen.length === 0}
          <p class="empty">No recent repositories</p>
        {:else}
          {#each recentNotOpen as path (path)}
            <button
              class="drop-item"
              role="menuitem"
              title={path}
              onclick={() => openRecent(path)}
            >{basename(path)}</button>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .tab-strip {
    display: flex;
    align-items: stretch;
    gap: 2px;
    padding: 0 6px;
    background: var(--header-bg);
    border-bottom: 1px solid var(--border);
    min-height: 34px;
    overflow-x: auto;
    scrollbar-width: none; /* Firefox */
  }
  .tab-strip::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 10px;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--text-muted);
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
    border-radius: 0;
    transition: color 0.1s;
    flex-shrink: 0;
  }
  .tab:hover {
    color: var(--text);
    background: var(--row-hover);
  }
  /* No font-weight change on activate: the accent underline + brighter color mark
     the active tab. Bolding would widen the label and shift the whole tab strip
     every time you switch tabs. */
  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .tab-name {
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    font-size: 14px;
    line-height: 1;
    color: var(--text-muted);
    opacity: 0;
    transition: opacity 0.1s, background 0.1s;
    cursor: pointer;
  }
  .tab:hover .tab-close,
  .tab.active .tab-close {
    opacity: 1;
  }
  .tab-close:hover {
    background: var(--btn-hover);
    color: var(--text);
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    align-self: center;
    flex-shrink: 0;
    margin-left: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
  }
  .add-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }

  /* + button + its open/recent menu */
  .add-wrap {
    position: relative;
    display: flex;
    align-items: center;
    margin-left: 2px;
    flex-shrink: 0;
  }

  .dropdown {
    /* Fixed (anchored to the + button) so the tab strip's overflow-x can't clip
       it; coordinates are set inline from getBoundingClientRect. */
    position: fixed;
    z-index: 3000;
    min-width: 200px;
    max-width: 320px;
    max-height: 280px;
    overflow-y: auto;
    padding: 4px;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }

  .drop-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    border: none;
    background: none;
    color: var(--text);
    font-size: 12.5px;
    border-radius: 5px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .drop-item:hover {
    background: var(--row-hover);
  }

  .empty {
    margin: 4px 10px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  .open-item {
    font-weight: 500;
  }
  .drop-sep {
    height: 1px;
    background: var(--border-subtle);
    margin: 4px 0;
  }
  .drop-head {
    margin: 2px 10px;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
</style>
