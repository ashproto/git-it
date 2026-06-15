<script lang="ts">
  import RepoLoader from "$lib/components/RepoLoader.svelte";
  import GraphHistory from "$lib/components/GraphHistory.svelte";
  import CommitDetail from "$lib/components/CommitDetail.svelte";
  import WorkingCopyView from "$lib/components/WorkingCopyView.svelte";
  import ConflictView from "$lib/components/ConflictView.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import EditTabs from "$lib/components/EditTabs.svelte";
  import ApplyPanel from "$lib/components/ApplyPanel.svelte";
  import LogPanel from "$lib/components/LogPanel.svelte";
  import PrereqBanner from "$lib/components/PrereqBanner.svelte";
  import DateFormatMenu from "$lib/components/DateFormatMenu.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import Modal from "$lib/components/Modal.svelte";
  import UndoBar from "$lib/components/UndoBar.svelte";
  import AmendDialog from "$lib/components/AmendDialog.svelte";
  import RebaseTodo from "$lib/components/RebaseTodo.svelte";
  import RemoteProgress from "$lib/components/RemoteProgress.svelte";
  import { gitActions } from "$lib/gitActions";
  import { onWindowDragMouseDown } from "$lib/tauriDrag";
  import { onMount } from "svelte";
  import { appState } from "$lib/store.svelte";
  import { SAMPLE_GRAPH } from "$lib/graph/sample";

  const currentBranch = $derived(
    appState.refsByKind.local.find((r) => r.isHead)?.name ?? null,
  );
  const detachedHead = $derived(
    currentBranch === null && appState.refsByKind.head.length > 0,
  );
  const hasRemotes = $derived(appState.remotes.length > 0);
  const aheadBehind = $derived(
    appState.currentUpstream !== null && (appState.currentAhead > 0 || appState.currentBehind > 0)
      ? { ahead: appState.currentAhead, behind: appState.currentBehind }
      : null,
  );
  const noRemoteTitle = "Add a remote first (see Remotes panel in the sidebar)";

  onMount(() => {
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!inTauri && appState.graphCommits.length === 0) {
      appState.setGraphCommits(SAMPLE_GRAPH);
    }
  });
</script>

<main>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="app-header" onmousedown={onWindowDragMouseDown}>
    <h1>Git It</h1>
    <span class="sub">Batch-edit commit timestamps via git-filter-repo</span>
    {#if currentBranch}
      <span class="branch-chip" title="Current branch">
        {currentBranch}{#if aheadBehind}&nbsp;<span class="ahead-behind" aria-label="{aheadBehind.ahead} ahead, {aheadBehind.behind} behind">↑{aheadBehind.ahead} ↓{aheadBehind.behind}</span>{/if}
      </span>
    {:else if detachedHead}
      <span class="branch-chip detached" title="Detached HEAD">detached HEAD</span>
    {/if}
    <div class="remote-btns" data-no-drag>
      <button
        class="fetch-btn"
        onclick={() => gitActions.fetch()}
        title="Fetch all remotes"
      >Fetch</button>
      <button
        class="fetch-btn"
        disabled={!hasRemotes || appState.remoteOpActive}
        title={hasRemotes ? "Pull changes from remote" : noRemoteTitle}
        onclick={() => gitActions.pull()}
      >Pull</button>
      <div class="push-group">
        <button
          class="fetch-btn push-main"
          disabled={!hasRemotes || appState.remoteOpActive}
          title={hasRemotes ? "Push to remote" : noRemoteTitle}
          onclick={() => gitActions.push()}
        >Push</button><button
          class="fetch-btn push-arrow"
          disabled={!hasRemotes || appState.remoteOpActive}
          title={hasRemotes ? "Force push with lease" : noRemoteTitle}
          onclick={async () => {
            if (!hasRemotes) return;
            await gitActions.push(true);
          }}
          aria-label="Force push with lease"
        >▾</button>
      </div>
    </div>
    <DateFormatMenu />
  </header>

  <RemoteProgress />

  <PrereqBanner />

  <div class="shell">
    <aside class="side-col">
      <RepoLoader />
      <Sidebar />
    </aside>
    <div class="main-col">
      <UndoBar />
      <GraphHistory />
      <ConflictView />
      {#if appState.workingCopySelected}
        <WorkingCopyView />
      {:else}
        <CommitDetail />
      {/if}
      <div class="two-col">
        <EditTabs />
        <ApplyPanel />
      </div>
      <LogPanel />
    </div>
  </div>

  <ContextMenu />
  <Modal />
  <AmendDialog />
  <RebaseTodo />
</main>

<style>
  :global(:root) {
    --bg: #f6f7f9;
    --panel-bg: #ffffff;
    --header-bg: #f1f3f5;
    --input-bg: #f7f8fa;
    --btn-bg: #f1f3f5;
    --btn-hover: #e7e9ec;
    --border: #e4e6ea;
    --border-subtle: #eef0f3;
    --text: #1a1d20;
    --text-muted: #6b7280;
    --row-hover: #f5f7fa;
    --row-selected: #bfdbfe;
    --row-selected-border: #2563eb;
    --accent: #2563eb;
    --accent-hover: #1d4ed8;
    --danger: #dc2626;
    --danger-hover: #b91c1c;
    --err: #b45309;
    /* tells native form controls (spinners, scrollbars, etc.) to follow light/dark */
    color-scheme: light;
    font-family:
      -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", Roboto, "Helvetica Neue",
      sans-serif;
    font-size: 14px;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #0f1115;
      --panel-bg: #15181d;
      --header-bg: #1a1e24;
      --input-bg: #1a1e24;
      --btn-bg: #232931;
      --btn-hover: #2c343d;
      --border: #2a3038;
      --border-subtle: #20252c;
      --text: #e5e7eb;
      --text-muted: #9ca3af;
      --row-hover: #1c2027;
      --row-selected: #1e40af66;
      --row-selected-border: #60a5fa;
      --accent: #3b82f6;
      --accent-hover: #2563eb;
      --danger: #ef4444;
      --danger-hover: #dc2626;
      --err: #fbbf24;
      color-scheme: dark;
    }
  }

  /* Glass mode — opt-in when running inside Tauri (set by src/lib/tauriMode.ts).
     We override the same variable names with rgba() so every var(--panel-bg) etc.
     downstream picks up the translucent values without touching component CSS. */
  :global(:root[data-tauri="true"]) {
    --bg: transparent;
    --panel-bg: rgba(255, 255, 255, 0.1);
    /* The settings popover is small + interactive, so it stays much more opaque
       than the panels to guarantee text contrast over any wallpaper. */
    --popover-bg: rgba(250, 250, 252, 0.85);
    --header-bg: rgba(241, 243, 245, 0.08);
    --input-bg: rgba(247, 248, 250, 0.22);
    /* Buttons need to read clearly against the very translucent panels
       (0.10), so their fill is more opaque than other glass surfaces. */
    --btn-bg: rgba(241, 243, 245, 0.62);
    --btn-hover: rgba(231, 233, 236, 0.78);
    --border: rgba(228, 230, 234, 0.22);
    --border-subtle: rgba(238, 240, 243, 0.12);
    --row-hover: rgba(245, 247, 250, 0.22);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root[data-tauri="true"]) {
      --bg: transparent;
      --panel-bg: rgba(21, 24, 29, 0.14);
      --popover-bg: rgba(24, 27, 32, 0.88);
      --header-bg: rgba(26, 30, 36, 0.1);
      --input-bg: rgba(26, 30, 36, 0.24);
      /* Lighter + more opaque than the dark panels (0.14) so buttons separate. */
      --btn-bg: rgba(58, 66, 76, 0.66);
      --btn-hover: rgba(72, 82, 94, 0.8);
      --border: rgba(42, 48, 56, 0.26);
      --border-subtle: rgba(32, 37, 44, 0.14);
      --row-hover: rgba(28, 32, 39, 0.22);
    }
  }

  :global(html, body) {
    margin: 0;
    padding: 0;
    background: var(--bg);
    color: var(--text);
    height: 100%;
  }
  /* Keep the document root paint-through transparent in glass mode so the
     OS NSVisualEffect material is what shows behind the panels. */
  :global(:root[data-tauri="true"]),
  :global(:root[data-tauri="true"] body) {
    background: transparent;
  }

  main {
    padding: 16px 18px;
    max-width: 1400px;
    margin: 0 auto;
  }
  /* In glass mode the very top of the window must be a draggable element so
     the user can grab the strip around the traffic-light buttons. We move the
     top spacing from <main>'s padding (which is empty space — no element to
     receive mousedown) INTO .app-header (which has the drag handler). */
  :global(:root[data-tauri="true"]) main {
    padding-top: 0;
  }
  :global(:root[data-tauri="true"]) .app-header {
    /* Reserve room for traffic lights + visually separate the title strip. */
    padding-top: 2.25rem;
    padding-left: 4px;
  }
  /* Light frosted-glass on top of OS NSVisualEffect vibrancy. Kept light on the
     blur side so the wallpaper detail comes through clearly — the OS material
     already provides plenty of blur underneath. */
  :global(:root[data-tauri="true"] .panel) {
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }

  .app-header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    /* Native macOS titlebars are non-selectable so click-drag doesn't start
       a text selection that would preempt the window drag. */
    user-select: none;
    -webkit-user-select: none;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 14px;
  }
  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .sub {
    color: var(--text-muted);
    font-size: 13px;
  }

  .branch-chip {
    margin-left: 4px;
    align-self: center;
    font-size: 12px;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 1px 10px;
    white-space: nowrap;
  }
  .branch-chip.detached {
    color: var(--err);
    border-color: var(--err);
  }
  .remote-btns {
    margin-left: auto;
    align-self: center;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .fetch-btn {
    align-self: center;
    padding: 4px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  .fetch-btn:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  .fetch-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* Push button split: left part is "Push", right part is the ▾ for force-push */
  .push-group {
    display: flex;
    align-items: center;
  }
  .push-main {
    border-radius: 6px 0 0 6px;
    border-right: none;
  }
  .push-arrow {
    padding: 4px 7px;
    border-radius: 0 6px 6px 0;
    font-size: 10px;
  }

  .ahead-behind {
    font-size: 11px;
    opacity: 0.75;
    font-weight: 400;
  }
  .shell {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .side-col {
    flex: 0 0 240px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .main-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  @media (max-width: 900px) {
    .shell {
      flex-direction: column;
      /* Column mode: stretch children to the viewport width (not max-content) so a
         wide child (the commits table) scrolls inside its own overflow:auto box
         instead of forcing page-level horizontal scroll that pushes panel controls
         (e.g. the conflict-resolution buttons) off-screen. */
      align-items: stretch;
    }
    .side-col {
      flex: 1 1 auto;
      width: 100%;
    }
    .two-col {
      grid-template-columns: 1fr;
    }
  }
</style>
