<script lang="ts">
  import GraphHistory from "$lib/components/GraphHistory.svelte";
  import CommitDetail from "$lib/components/CommitDetail.svelte";
  import InlineEditCommit from "$lib/components/InlineEditCommit.svelte";
  import WorkingCopyView from "$lib/components/WorkingCopyView.svelte";
  import ConflictView from "$lib/components/ConflictView.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import LogPanel from "$lib/components/LogPanel.svelte";
  import PrereqBanner from "$lib/components/PrereqBanner.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import { settingsPanel } from "$lib/settingsPanel.svelte";
  import ManageRepoModal from "$lib/components/ManageRepoModal.svelte";
  import { manageRepo } from "$lib/manageRepo.svelte";
  import BranchColorDialog from "$lib/components/BranchColorDialog.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import Modal from "$lib/components/Modal.svelte";
  import UndoBar from "$lib/components/UndoBar.svelte";
  import AmendDialog from "$lib/components/AmendDialog.svelte";
  import RebaseTodo from "$lib/components/RebaseTodo.svelte";
  import RemoteProgress from "$lib/components/RemoteProgress.svelte";
  import RepoTabs from "$lib/components/RepoTabs.svelte";
  import RepoList from "$lib/components/RepoList.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import TimeEditDrawer from "$lib/components/TimeEditDrawer.svelte";
  import { gitActions, reloadGraph } from "$lib/gitActions";
  import { pickRepoFolder, api } from "$lib/api";
  import { onWindowDragMouseDown } from "$lib/tauriDrag";
  import { onMount } from "svelte";
  import { appState } from "$lib/store.svelte";
  import { SAMPLE_GRAPH } from "$lib/graph/sample";

  const currentBranch = $derived(
    appState.refsByKind.local.find((r) => r.isHead)?.name ?? null,
  );
  // The project name shown centered in the title bar — the active repo's folder
  // name (Fork-style), falling back to the app name in the empty state.
  const repoName = $derived(
    appState.repo ? (appState.repo.split("/").filter(Boolean).pop() ?? "Git It") : "Git It",
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
  const noRemoteTitle = "Add a remote first (Manage Repository ▸ Remotes)";

  // ── Reload-on-switch effect (Tauri only) ─────────────────────────────────────
  // Fires once per actual repo change; the lastLoaded guard prevents re-running
  // on unrelated reactive state changes. The browser keeps the sample graph.
  let lastLoaded = "";
  $effect(() => {
    const r = appState.repo;
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (inTauri && r && r !== lastLoaded) { lastLoaded = r; reloadGraph(); }
    // Closing the last repo (r === "") resets the guard so re-opening the same
    // path triggers a fresh reload instead of showing a stale-empty graph.
    else if (!r) lastLoaded = "";
  });

  // ── Empty-state open flow ─────────────────────────────────────────────────────
  async function openRepoFlow() {
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!inTauri) {
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

  onMount(() => {
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!inTauri && appState.graphCommits.length === 0) {
      appState.setGraphCommits(SAMPLE_GRAPH);
    }
  });

  // ── Sidebar resize ────────────────────────────────────────────────────────
  // Drag the divider between the sidebar and main column; width persists. The
  // store clamps to a sane range. Double-click the handle to reset to default.
  function startSidebarResize(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = appState.sidebarWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    const onMove = (ev: PointerEvent) => appState.setSidebarWidth(startW + (ev.clientX - startX));
    const onUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  // Derived: nothing to show — no active repo, no open repos, and no loaded commits.
  // (The graphCommits check keeps the browser preview's sample graph visible, since
  // its onMount loads commits without setting a repo.)
  const isEmpty = $derived(
    appState.openRepos.length === 0 && !appState.repo && appState.graphCommits.length === 0,
  );
</script>

<main>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="app-header" onmousedown={onWindowDragMouseDown}>
    <div class="tl-inset" aria-hidden="true"></div>
    <div class="header-center">
      <h1 title={appState.repo || "Git It"}>{repoName}</h1>
      {#if currentBranch}
        <span class="branch-chip" title="Current branch">
          {currentBranch}{#if aheadBehind}&nbsp;<span class="ahead-behind" aria-label="{aheadBehind.ahead} ahead, {aheadBehind.behind} behind">↑{aheadBehind.ahead} ↓{aheadBehind.behind}</span>{/if}
        </span>
      {:else if detachedHead}
        <span class="branch-chip detached" title="Detached HEAD">detached HEAD</span>
      {/if}
    </div>
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
    <div class="gear" data-no-drag>
      <button
        type="button"
        class="gear-btn"
        aria-label="Manage repository"
        title="Manage repository — backups, history, remotes"
        onclick={() => manageRepo.openPanel()}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <line x1="4" y1="8" x2="20" y2="8"></line>
          <circle cx="10" cy="8" r="2.4"></circle>
          <line x1="4" y1="16" x2="20" y2="16"></line>
          <circle cx="15" cy="16" r="2.4"></circle>
        </svg>
      </button>
      <button
        type="button"
        class="gear-btn"
        aria-label="Settings"
        title="Settings"
        onclick={() => settingsPanel.openPanel()}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="3"></circle>
          <path
            d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
          ></path>
        </svg>
      </button>
    </div>
  </header>

  <!-- Repo tab strip (tabs mode only) — sits between header and shell -->
  {#if appState.repoSwitcherMode === "tabs"}
    <RepoTabs />
  {/if}

  <RemoteProgress />

  <PrereqBanner />

  <!-- Scrolling content area: the shell (sidebar + main columns) scrolls here;
       the header/tabs/RemoteProgress/PrereqBanner above and StatusBar below are
       fixed chrome that never scroll. -->
  <div class="scroll-area">
    <div class="scroll-inner">
      <div class="shell">
        <aside class="side-col" style={`--sidebar-w:${appState.sidebarWidth}px`}>
          {#if appState.repoSwitcherMode === "sidebar"}
            <RepoList />
          {/if}
          <Sidebar />
        </aside>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize sidebar"
          title="Drag to resize · double-click to reset"
          onpointerdown={startSidebarResize}
          ondblclick={() => appState.setSidebarWidth(240)}
        ></div>
        {#if isEmpty}
          <!-- Empty state: no repos open yet -->
          <div class="empty-state main-col">
            <p class="empty-prompt">Open a repository to get started</p>
            <button class="open-btn" onclick={openRepoFlow}>Open Repository…</button>
          </div>
        {:else}
          <div class="main-col">
            <!-- Timeline stays MOUNTED (just hidden) in Local Changes view so the
                 graph's scroll-to-commit keeps working when a sidebar ref is clicked
                 from the changes screen. display:contents → no layout box when shown. -->
            <div class="timeline-stack" class:hidden={appState.activeView === "changes"}>
              <UndoBar />
              <GraphHistory />
            </div>
            <ConflictView />
            {#if appState.activeView === "changes"}
              <WorkingCopyView />
            {:else}
              <CommitDetail />
              {#if appState.autoShowEditTools && appState.selectedCommit}
                <InlineEditCommit />
              {/if}
            {/if}
            <LogPanel />
          </div>
        {/if}
      </div>
    </div>
  </div>

  <ContextMenu />
  <Modal />
  <AmendDialog />
  <RebaseTodo />
  <TimeEditDrawer />
  <SettingsPanel />
  <ManageRepoModal />
  <BranchColorDialog />
  <StatusBar />
</main>

<style>
  :global(:root) {
    --bg: #eceef1;
    --panel-bg: #ffffff;
    --header-bg: #f1f3f5;
    --input-bg: #f7f8fa;
    --btn-bg: #e9ecef;
    --btn-hover: #dfe3e8;
    --border: #d6d9de;
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
      --panel-bg: #1b1f26;
      --header-bg: #1a1e24;
      --input-bg: #1a1e24;
      --btn-bg: #2a313b;
      --btn-hover: #353d48;
      --border: #353c46;
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
    --panel-bg: rgba(255, 255, 255, 0.16);
    /* The settings popover is small + interactive, so it stays much more opaque
       than the panels to guarantee text contrast over any wallpaper. */
    --popover-bg: rgba(250, 250, 252, 0.85);
    --header-bg: rgba(241, 243, 245, 0.16);
    --input-bg: rgba(247, 248, 250, 0.22);
    /* Buttons need to read clearly against the translucent panels, so their fill
       is more opaque than other glass surfaces. */
    --btn-bg: rgba(241, 243, 245, 0.62);
    --btn-hover: rgba(231, 233, 236, 0.78);
    --border: rgba(255, 255, 255, 0.4);
    --border-subtle: rgba(255, 255, 255, 0.22);
    --row-hover: rgba(245, 247, 250, 0.22);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root[data-tauri="true"]) {
      --bg: transparent;
      --panel-bg: rgba(28, 32, 39, 0.24);
      --popover-bg: rgba(24, 27, 32, 0.88);
      --header-bg: rgba(26, 30, 36, 0.2);
      --input-bg: rgba(26, 30, 36, 0.24);
      /* Lighter + more opaque than the dark panels so buttons separate. */
      --btn-bg: rgba(58, 66, 76, 0.66);
      --btn-hover: rgba(72, 82, 94, 0.8);
      --border: rgba(120, 130, 142, 0.34);
      --border-subtle: rgba(90, 100, 112, 0.22);
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
    /* App-shell: fills the full viewport as a flex column so fixed chrome
       (header, tabs, status bar) never scrolls and the content area takes
       all remaining height. */
    height: 100dvh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  /* Timeline group: renders inline (display:contents → no extra box, identical
     layout) when the commit timeline is active, and fully unrenders in Local
     Changes view — while keeping GraphHistory mounted so scroll-to-commit works. */
  .timeline-stack {
    display: contents;
  }
  .timeline-stack.hidden {
    display: none;
  }
  /* In glass mode the very top of the window must be a draggable element so
     the user can grab the strip around the traffic-light buttons. We move the
     top spacing from <main>'s padding (which is empty space — no element to
     receive mousedown) INTO .app-header (which has the drag handler). */
  :global(:root[data-tauri="true"]) main {
    padding-top: 0;
  }
  :global(:root[data-tauri="true"]) .app-header {
    /* Sit the row BESIDE the traffic lights (not below them): a left inset clears
       the lights and the shrunk vertical padding reclaims the ~36px band. */
    --tl-inset: 78px;
    padding-top: 6px;
    padding-bottom: 6px;
    min-height: 28px;
  }
  /* Light frosted-glass on top of OS NSVisualEffect vibrancy. Kept light on the
     blur side so the wallpaper detail comes through clearly — the OS material
     already provides plenty of blur underneath. */
  :global(:root[data-tauri="true"] .panel) {
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }

  /* Left zone (title + branch chip) absorbs all the variable width, so the
     Fetch/Pull/Push group and the gear stay anchored on the right and never shift
     when the branch name changes (was: two competing margin-left:auto). */
  /* Centered project + branch in the title-bar strip. The header is a grid:
     [traffic-light inset | centered title | remote buttons | gear]. The inset is
     0 in the browser and ~78px in glass mode (to clear the macOS lights). */
  .tl-inset {
    width: var(--tl-inset, 0px);
    flex: 0 0 auto;
  }
  .header-center {
    /* True window-centre: absolutely positioned at 50% of the header (= window)
       width, so the title doesn't shift when the side button widths change. */
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    max-width: 52%;
    overflow: hidden;
    /* Never intercept clicks meant for the action buttons (or the window drag),
       even if a long title visually approaches them. */
    pointer-events: none;
  }
  .app-header {
    /* position:relative anchors the absolutely-centred title. */
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    /* Native macOS titlebars are non-selectable so click-drag doesn't start
       a text selection that would preempt the window drag. */
    user-select: none;
    -webkit-user-select: none;
    /* Horizontal padding matches the former main padding so the header content
       lines up with the scrolling content area below it. */
    padding: 16px 18px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  h1 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .branch-chip {
    align-self: center;
    font-size: 12px;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 1px 10px;
    white-space: nowrap;
    /* Truncate a very long branch name instead of pushing the toolbar buttons. */
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .branch-chip.detached {
    color: var(--err);
    border-color: var(--err);
  }
  .remote-btns {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    /* The centred title is out of flow (absolute), so push the action groups
       (Fetch/Pull/Push + gears) to the right edge. */
    margin-left: auto;
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

  /* Gear button — opens the Settings panel */
  .gear {
    position: relative;
    display: inline-flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .gear-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    cursor: pointer;
  }
  .gear-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }
  .gear-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* Scrolling content area: takes all remaining height and provides the single
     vertical scroll container for the shell (sidebar + main columns). */
  .scroll-area {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }
  /* Inner wrapper restores the former main padding and fills the full window
     width (no max-width cap) so wide/fullscreen windows use all the space. */
  .scroll-inner {
    padding: 16px 18px;
    width: 100%;
    box-sizing: border-box;
  }

  /* Empty state — centered "Open a repository" prompt in the main column area */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    min-height: 320px;
  }
  .empty-prompt {
    margin: 0;
    font-size: 15px;
    color: var(--text-muted);
  }
  .open-btn {
    padding: 8px 20px;
    border-radius: 7px;
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s;
  }
  .open-btn:hover {
    background: var(--accent-hover);
  }

  .shell {
    display: flex;
    /* No gap: the resize handle IS the gutter between the columns. */
    gap: 0;
    align-items: flex-start;
  }
  .side-col {
    /* Width is the persisted --sidebar-w (set inline); the @media below overrides
       flex for the stacked column layout. */
    flex: 0 0 var(--sidebar-w, 240px);
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  /* Drag handle between sidebar and main column; doubles as the visual gutter. */
  .resize-handle {
    flex: 0 0 14px;
    align-self: stretch;
    cursor: col-resize;
    position: relative;
    touch-action: none;
  }
  .resize-handle::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 1px;
    background: var(--border);
    transform: translateX(-50%);
    transition: background 0.1s, width 0.1s;
  }
  .resize-handle:hover::before {
    background: var(--accent);
    width: 2px;
  }
  .main-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  @media (max-width: 900px) {
    .shell {
      flex-direction: column;
      gap: 12px;
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
    .resize-handle {
      display: none;
    }
  }
</style>
