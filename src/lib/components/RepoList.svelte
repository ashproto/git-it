<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { createRepositoryFlow, openRepositoryFlow } from "../repositoryFlows";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  function basename(path: string): string {
    return path.split("/").filter(Boolean).pop() ?? path;
  }

  let openRecent = $state(true);

  // Repos in the recent list that aren't already open.
  const recentNotOpen = $derived(
    appState.recentRepos.filter((p) => !appState.openRepos.includes(p)),
  );

  // ── Shared open flow ─────────────────────────────────────────────────────────
  async function openRepoFlow() {
    await openRepositoryFlow(appState.repo || undefined);
  }

  async function openRecentRepo(path: string) {
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

<!-- Workspace sidebar block — matches Sidebar.svelte section styling -->
<div class="workspace">
  <!-- WORKSPACE section header -->
  <div class="sec-header">
    <span class="sec-label">Workspace</span>
  </div>

  <!-- Open repos list -->
  {#if appState.openRepos.length === 0}
    <p class="none">No open repos</p>
  {:else}
    {#each appState.openRepos as path (path)}
      <div class="repo-row" class:active={path === appState.repo}>
        <button
          class="repo-btn"
          class:active={path === appState.repo}
          title={path}
          onclick={() => appState.setActiveRepo(path)}
        >
          <span class="dot" class:active-dot={path === appState.repo} aria-hidden="true"></span>
          <span class="repo-name">{basename(path)}</span>
        </button>
        <button
          class="remove-btn"
          title="Close {basename(path)}"
          onclick={() => appState.closeRepo(path)}
          aria-label="Close {basename(path)}"
        >×</button>
      </div>
    {/each}
  {/if}

  <button class="action-row" onclick={() => createRepositoryFlow()} title="Create a new repository">
    <span class="action-icon" aria-hidden="true">+</span>
    <span>New Repository…</span>
  </button>

  <!-- Open… row -->
  <button class="action-row" onclick={openRepoFlow} title="Open a repository folder">
    <span class="action-icon" aria-hidden="true">+</span>
    <span>Open…</span>
  </button>

  <!-- Recent subsection (only when there are repos not already open) -->
  {#if recentNotOpen.length > 0}
    <button
      class="sub-sec"
      onclick={() => (openRecent = !openRecent)}
      aria-expanded={openRecent}
    >
      <span class="chev" class:open={openRecent} aria-hidden="true">▸</span>
      <span class="sub-label">Recent</span>
      <span class="n">{recentNotOpen.length}</span>
    </button>

    {#if openRecent}
      {#each recentNotOpen as path (path)}
        <button
          class="repo-btn recent"
          title={path}
          onclick={() => openRecentRepo(path)}
        >
          <span class="dot recent-dot" aria-hidden="true"></span>
          <span class="repo-name">{basename(path)}</span>
        </button>
      {/each}
    {/if}
  {/if}
</div>

<style>
  .workspace {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
    padding-bottom: 6px;
  }

  /* Section header — uppercase muted label, matches Sidebar.svelte .sec style */
  .sec-header {
    display: flex;
    align-items: center;
    padding: 6px 6px;
    margin-top: 4px;
  }
  .sec-label {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    flex: 1;
  }

  /* Each open-repo row: button + remove affordance */
  .repo-row {
    display: flex;
    align-items: center;
    border-radius: var(--radius-md);
  }
  .repo-row:hover .remove-btn {
    opacity: 1;
  }
  .repo-row.active {
    background: var(--row-hover);
  }

  .repo-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 5px 8px 5px 18px;
    background: none;
    border: none;
    border-radius: var(--radius-md);
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .repo-btn:hover {
    background: var(--row-hover);
  }
  .repo-btn.active {
    background: none; /* row-row handles the bg */
  }
  .repo-btn.active .repo-name {
    color: var(--accent);
    font-weight: 600;
  }

  /* Recent items are slightly muted */
  .repo-btn.recent .repo-name {
    color: var(--text-muted);
  }
  .repo-btn.recent {
    padding-left: 26px; /* extra indent under the sub-section header */
  }

  .repo-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  /* Colored dot indicators */
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #888780; /* muted by default */
  }
  .dot.active-dot {
    background: var(--accent);
  }
  .dot.recent-dot {
    background: #555;
    opacity: 0.5;
  }

  /* Remove (×) button — only visible on row hover */
  .remove-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    margin-right: 4px;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s, background 0.1s;
  }
  .remove-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
    opacity: 1 !important;
  }

  /* "Open…" action row */
  .action-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px 5px 16px;
    background: none;
    border: none;
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .action-row:hover {
    background: var(--row-hover);
    color: var(--text);
  }
  .action-icon {
    font-size: 15px;
    line-height: 1;
    width: 14px;
    text-align: center;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  /* Sub-section (Recent) collapsible header — matches Sidebar.svelte .sec */
  .sub-sec {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 5px 6px 5px 10px;
    margin-top: 2px;
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .sub-sec:hover {
    color: var(--text);
  }
  .chev {
    display: inline-block;
    transition: transform 0.12s ease;
    font-size: 10px;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .sub-label {
    flex: 1;
    text-align: left;
  }
  .n {
    color: var(--text-muted);
    font-weight: 500;
  }

  .none {
    margin: 0 0 4px 18px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
