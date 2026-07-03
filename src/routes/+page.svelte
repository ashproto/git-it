<script lang="ts">
  import GraphHistory from "$lib/components/GraphHistory.svelte";
  import GithubView from "$lib/components/github/GithubView.svelte";
  import CommitDetail from "$lib/components/CommitDetail.svelte";
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
  import LoadingBar from "$lib/components/LoadingBar.svelte";
  import GraphSkeleton from "$lib/components/GraphSkeleton.svelte";
  import RepoTabs from "$lib/components/RepoTabs.svelte";
  import RepoList from "$lib/components/RepoList.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import GraphSearchBar from "$lib/components/GraphSearchBar.svelte";
  import {
    gitActions,
    reloadGraph,
    loadMoreGraph,
    startWatchingRepo,
    stopWatchingRepo,
    refreshActiveRepo,
  } from "$lib/gitActions";
  import { matchCommits } from "$lib/graph/commitSearch";
  import { graphView } from "$lib/graphView.svelte";
  import { dialogs } from "$lib/dialogs.svelte";
  import { pickRepoFolder, api } from "$lib/api";
  import { onWindowDragMouseDown } from "$lib/tauriDrag";
  import { onMount, untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { appState } from "$lib/store.svelte";
  import { githubState } from "$lib/githubState.svelte";
  import { prForBranch } from "$lib/github/branchPr";
  import { pullStateBadge } from "$lib/github/itemState";
  import { SAMPLE_GRAPH } from "$lib/graph/sample";

  const currentBranch = $derived(
    appState.refsByKind.local.find((r) => r.isHead)?.name ?? null,
  );
  // The Fetch button swaps to a spinner + "Fetching…" while its run() op is in
  // flight (run() labels the op "Fetch"). Feedback lands where the user clicked.
  const fetching = $derived(appState.busyOp === "Fetch");
  // Title-bar PR chip: the current branch's OPEN pull request, from the cached
  // pulls list. Shown only on the git screens (the GitHub screen shows PRs
  // itself), only when the cache belongs to THIS repo, and renders nothing in
  // every unknown state (no remote, list not loaded, no match).
  const currentBranchPr = $derived(
    appState.activeView !== "github" &&
      currentBranch &&
      githubState.hasGithubRemote &&
      githubState.loadedRepoPath === appState.repo
      ? prForBranch(githubState.pulls.data, currentBranch)
      : null,
  );
  // Opportunistic pulls load for the chip: ONLY when GitHub availability is
  // already known-Ok for this repo (the GitHub screen ran its ensure()) — the
  // toolbar never triggers availability probes or a full ensure. Lazy and
  // cheap: makePanel.load() dedupes by key, and no polling is involved. The
  // effect re-runs when availability lands (ensure resolving) or repo changes.
  $effect(() => {
    const repo = appState.repo;
    if (!repo || !githubState.hasGithubRemote) return;
    if (githubState.availability?.kind !== "Ok") return;
    if (githubState.loadedRepoPath !== repo) return;
    void githubState.loadPulls(repo);
  });
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
    if (inTauri && r && r !== lastLoaded) {
      lastLoaded = r;
      reloadGraph();
      // (Re)point the filesystem watcher at the new repo for live Local Changes.
      startWatchingRepo();
    }
    // Closing the last repo (r === "") resets the guard so re-opening the same
    // path triggers a fresh reload instead of showing a stale-empty graph.
    else if (!r) {
      lastLoaded = "";
      stopWatchingRepo();
    }
  });

  // ── Restore last-active repo on launch (Tauri only) ──────────────────────────
  // The active repo isn't part of synchronous boot state — it arrives via the
  // async store hydrate. This one-shot effect waits for that value, then (unless
  // the user already opened a repo) validates it's still a git repo and activates
  // it; the reload-on-switch effect above then loads it. A missing/moved repo is
  // simply skipped (its tab still appears from the openRepos hydrate).
  let restoreTried = false;
  $effect(() => {
    const last = appState.lastActiveRepo; // re-run when the hydrate resolves
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!inTauri || restoreTried) return;
    if (appState.repo) {
      restoreTried = true; // user already opened one — nothing to restore
      return;
    }
    if (!last) return; // first run, or hydrate hasn't resolved yet — wait
    restoreTried = true;
    (async () => {
      try {
        if (await api.isGitRepo(last)) appState.setActiveRepo(last);
      } catch (e) {
        console.warn("[gte] restore last repo failed", e);
      }
    })();
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

  // Collapse state of the two timeline panels, lifted here so the layout can give the
  // freed space to whichever panel is still open: collapsing the graph lets the details
  // pane fill (it otherwise stays stuck at detailsHeight), and collapsing the details
  // pane lets the graph fill (already handled by the graph's flex:1).
  let graphCollapsed = $state(false);
  let detailsCollapsed = $state(false);
  // When the graph is collapsed and the details are open, the details pane grows to fill
  // (and CommitDetail switches from a fixed height to fill mode); the boundary drag is
  // hidden then since there's no fixed height to drag.
  const detailsFills = $derived(graphCollapsed && !detailsCollapsed);

  // Selecting a NEW commit re-expands the details pane, so a stale collapse from a prior
  // commit never hides the details the user just asked to see. (Graph collapse is a
  // deliberate layout choice and is intentionally NOT reset here.) prevDetailsSha is a
  // plain non-reactive cursor of the last focused sha.
  let prevDetailsSha: string | null = null;
  $effect(() => {
    const sha = appState.currentSha;
    if (sha !== prevDetailsSha) {
      prevDetailsSha = sha;
      if (sha) detailsCollapsed = false;
    }
  });

  // ── Screen / repo switch animation ───────────────────────────────────────────
  // A quick zoom-and-fade-in played on the main content whenever the active screen
  // changes (Commit Timeline ⇄ Local Changes) or the active repository switches. It
  // animates ONLY opacity + transform (the two GPU-composited properties), so it stays
  // smooth, and it runs imperatively via the Web Animations API on a wrapper element —
  // nothing remounts. That matters: the commit graph must stay mounted across screen
  // switches (jump-to-ref) and the repo swap is deliberately flicker-free (old data is
  // kept until the new graph loads); a keyed remount would break both.
  let viewSwapEl = $state<HTMLElement>();
  let prevView = appState.activeView;
  let prevRepo = appState.repo;
  let firstSwap = true;
  $effect(() => {
    const view = appState.activeView; // tracked
    const repo = appState.repo; // tracked
    untrack(() => {
      const changed = view !== prevView || repo !== prevRepo;
      prevView = view;
      prevRepo = repo;
      if (firstSwap) {
        firstSwap = false; // don't animate the initial render
        return;
      }
      if (!changed) return;
      const el = viewSwapEl;
      if (!el || typeof el.animate !== "function") return;
      if (window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches) return;
      el.animate(
        [
          { opacity: 0, transform: "scale(0.985)" },
          { opacity: 1, transform: "scale(1)" },
        ],
        { duration: 260, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
      );
    });
  });

  // ── Commit-details panel resize (drag the boundary between the graph and the
  // slide-up details panel). Dragging UP grows the details panel (graph shrinks).
  // Keep at least this much height for the graph so dragging the details pane tall
  // can't hide it entirely (the pane is flex:0 0 auto, i.e. non-shrinking).
  const GRAPH_FLOOR = 140;
  function startDetailsResize(e: PointerEvent) {
    e.preventDefault();
    const startY = e.clientY;
    const startH = appState.detailsHeight;
    const mainCol = (e.currentTarget as HTMLElement).closest(".main-col") as HTMLElement | null;
    document.body.style.cursor = "ns-resize";
    document.body.style.userSelect = "none";
    const onMove = (ev: PointerEvent) => {
      let next = startH + (startY - ev.clientY);
      // Cap against the live container so the graph keeps GRAPH_FLOOR px (setDetailsHeight
      // still applies the absolute 140–1200 clamp). Adapts as the window is resized.
      if (mainCol) next = Math.min(next, mainCol.clientHeight - GRAPH_FLOOR);
      appState.setDetailsHeight(next);
    };
    const onUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  onMount(() => {
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!inTauri && appState.graphCommits.length === 0) {
      appState.setGraphCommits(SAMPLE_GRAPH);
    }
    // Reload the graph + working copy + status when the window regains focus /
    // becomes visible — the user may have committed or edited in another app.
    // Complements the filesystem watcher (which handles changes while the window
    // is already active). refreshActiveRepo no-ops outside Tauri / with no repo.
    const onActivate = () => {
      if (document.visibilityState === "visible") refreshActiveRepo();
    };
    window.addEventListener("focus", onActivate);
    document.addEventListener("visibilitychange", onActivate);
    return () => {
      window.removeEventListener("focus", onActivate);
      document.removeEventListener("visibilitychange", onActivate);
    };
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

  // ── ⌘F commit search (graph screen) ─────────────────────────────────────────
  // Jump-and-highlight over the LOADED commits: matches are indices into
  // appState.graphCommits (recomputed as pages load in), highlighting is passed
  // down to the (virtualized) GraphHistory as data, and jumps go through the
  // graphView controller so off-window rows scroll into view.
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchActive = $state(0); // position within searchMatches
  let searchBar = $state<{ focusInput: () => void }>();
  const searchMatches = $derived(
    searchOpen ? matchCommits(appState.graphCommits, searchQuery) : [],
  );
  // Clamp: a shrinking match list (query edit, repo refresh) must not strand the pointer.
  const searchActiveClamped = $derived(
    searchMatches.length ? Math.min(searchActive, searchMatches.length - 1) : 0,
  );
  const searchHits = $derived(searchMatches.length ? new Set(searchMatches) : null);
  const searchActiveRow = $derived(
    searchMatches.length ? searchMatches[searchActiveClamped] : -1,
  );
  const searchCanLoadMore = $derived(
    searchQuery.trim() !== "" && searchMatches.length === 0 && appState.graphHasMore,
  );

  function jumpToSearchMatch(pos: number) {
    const sha = appState.graphCommits[searchMatches[pos]]?.sha;
    if (sha) graphView.scrollToCommit(sha);
  }
  function onSearchQuery(q: string) {
    searchQuery = q;
    searchActive = 0;
    if (searchMatches.length) jumpToSearchMatch(0);
  }
  function searchStep(dir: 1 | -1) {
    const n = searchMatches.length;
    if (n === 0) return;
    searchActive = (searchActiveClamped + dir + n) % n;
    jumpToSearchMatch(searchActive);
  }
  function closeSearch() {
    searchOpen = false;
    searchQuery = "";
    searchActive = 0;
  }
  async function searchLoadMore() {
    await loadMoreGraph();
    if (searchMatches.length) {
      searchActive = 0;
      jumpToSearchMatch(0);
    }
  }
  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key !== "f" || !e.metaKey || e.shiftKey || e.altKey || e.ctrlKey) return;
    // Graph screen only (the GitHub screen gets its own ⌘F handling) and never
    // over a dialog.
    if (appState.activeView !== "timeline") return;
    if (dialogs.state.kind !== "none") return;
    // Don't steal focus from a text field the user is typing in (inline commit
    // message editing, modal inputs not tracked by `dialogs`, the search field
    // itself — where focus is already in the right place).
    const t = e.target as HTMLElement | null;
    if (t?.closest?.('input, textarea, [contenteditable="true"]')) return;
    e.preventDefault();
    if (searchOpen) searchBar?.focusInput();
    else searchOpen = true;
  }

  // Derived: nothing to show — no active repo, no open repos, and no loaded commits.
  // (The graphCommits check keeps the browser preview's sample graph visible, since
  // its onMount loads commits without setting a repo.)
  const isEmpty = $derived(
    appState.openRepos.length === 0 && !appState.repo && appState.graphCommits.length === 0,
  );
</script>

<svelte:window onkeydown={onWindowKeydown} />

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
      {#if currentBranchPr}
        {@const prBadge = pullStateBadge(currentBranchPr)}
        <button
          class="pr-chip"
          data-no-drag
          style="--pr-color:{prBadge.color}"
          title={currentBranchPr.title}
          onclick={() => gitActions.openPrInApp(currentBranchPr.number)}
        >
          <span class="pr-dot" aria-hidden="true"></span>#{currentBranchPr.number}
        </button>
      {/if}
    </div>
    <div class="remote-btns" data-no-drag>
      <button
        class="fetch-btn"
        class:busy={fetching}
        disabled={fetching}
        onclick={() => gitActions.fetch()}
        title="Fetch all remotes"
      >{#if fetching}<span class="spin" aria-hidden="true">⟳</span> Fetching…{:else}Fetch{/if}</button>
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
  <LoadingBar />

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
            <!-- view-swap is the element the screen/repo-switch zoom-fade ($effect above)
                 animates; it holds everything that swaps between the timeline and Local
                 Changes. The debug Output panel stays outside it. -->
            <div class="view-swap" bind:this={viewSwapEl}>
            <!-- Timeline stays MOUNTED (just hidden) in Local Changes view so the
                 graph's scroll-to-commit keeps working when a sidebar ref is clicked
                 from the changes screen. display:contents → no layout box when shown. -->
            <div class="timeline-stack" class:hidden={appState.activeView === "changes" || appState.activeView === "github"}>
              <UndoBar />
              <GraphSearchBar
                bind:this={searchBar}
                open={searchOpen}
                count={searchMatches.length}
                active={searchActiveClamped}
                canLoadMore={searchCanLoadMore}
                loadingMore={appState.graphLoadingMore}
                onQuery={onSearchQuery}
                onNext={() => searchStep(1)}
                onPrev={() => searchStep(-1)}
                onClose={closeSearch}
                onLoadMore={searchLoadMore}
              />
              {#if appState.repoLoading && (appState.graphCommits.length === 0 || appState.graphCommitsRepo !== appState.repo)}
                <GraphSkeleton />
              {:else}
                <GraphHistory bind:collapsed={graphCollapsed} {searchHits} {searchActiveRow} />
              {/if}
            </div>
            <ConflictView />
            {#if appState.activeView === "github"}
              <GithubView />
            {:else if appState.activeView === "changes"}
              <WorkingCopyView />
            {:else if appState.selectedCommit}
              <!-- Commit details slide up below the (full-height) graph only when a
                   commit is selected; drag the top edge to resize, collapse via the
                   panel's own chevron. The panel takes a FIXED height (detailsHeight)
                   and scrolls inside, so its height is stable while the diff loads —
                   the pane no longer shrinks-then-grows (jitter on open / jump on
                   commit-switch). Collapsing the panel drops it to auto (header only),
                   so a collapsed panel has no dead space. -->
              <div class="details-pane" class:fill-details={detailsFills} transition:slide={{ duration: 200 }}>
                {#if !detailsFills}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="details-resize"
                    role="separator"
                    aria-orientation="horizontal"
                    aria-label="Resize commit details"
                    title="Drag to resize · double-click to reset"
                    onpointerdown={startDetailsResize}
                    ondblclick={() => appState.setDetailsHeight(320)}
                  ></div>
                {/if}
                <CommitDetail
                  bind:collapsed={detailsCollapsed}
                  fill={detailsFills}
                  height={detailsFills ? undefined : appState.detailsHeight}
                />
              </div>
            {/if}
            </div>
            {#if appState.showOutput}
              <LogPanel />
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>

  <ContextMenu />
  <Modal />
  <AmendDialog />
  <RebaseTodo />
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
    /* File-status glyph colours (A/M/D…), keyed by what the change MEANS:
       add = green, modify = yellow/amber (a legible gold on white), remove = red. */
    --status-add: #2da44e;
    --status-mod: #bf8700;
    --status-del: #cf222e;
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
      --status-add: #3fb950;
      --status-mod: #e3b341;
      --status-del: #f85149;
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

  /* Native-app feel: clicking or right-clicking UI chrome (branch rows, menus,
     headers, labels, commit rows) must not start a text selection. We set a
     no-select baseline on the body and opt BACK IN only where copying text is
     genuinely useful — form fields, diff code, rendered markdown (PR/issue/
     comment bodies), commit messages/SHAs, and anything tagged `.selectable`.
     (DiffView's line-number gutter/sigils keep their own `user-select: none`
     so gutter-drag line staging never selects text.) */
  :global(body) {
    user-select: none;
    -webkit-user-select: none;
  }
  :global(input),
  :global(textarea),
  :global([contenteditable="true"]),
  :global(.diff-cell),
  :global(.md),
  :global(.body-msg),
  :global(.sha),
  :global(.selectable),
  :global(.selectable *) {
    user-select: text;
    -webkit-user-select: text;
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
  /* Panels are deliberately NOT given a CSS backdrop-filter blur. The window already
     carries a NATIVE macOS NSVisualEffect material (tauri.conf windowEffects → "popover")
     that blurs the wallpaper behind it. A second, CSS-level blur on every always-on panel
     duplicated that work AND — because backdrop-filter is not GPU-composited and forces a
     full repaint + re-blur of its area every frame over a transparent WKWebView — it capped
     the whole app's animation framerate (every animation re-blurred ~12 large surfaces per
     frame). Panels are now plain translucent tints (var(--panel-bg)) layered over the native
     material. Only transient overlays (modals, menus, popovers) keep a CSS blur: they overlap
     opaque content, are short-lived, and aren't part of the steady-state per-frame cost. */

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
  /* Current-branch PR chip — styled like the branch chip, tinted with the PR's
     GitHub state colour (green open / gray draft via itemState). The centred
     header strip is pointer-events:none, so the chip re-enables them itself
     (and opts out of the window drag via data-no-drag). */
  .pr-chip {
    pointer-events: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: center;
    flex-shrink: 0;
    font-size: 12px;
    color: var(--pr-color);
    border: 1px solid var(--pr-color);
    border-radius: 999px;
    padding: 1px 10px;
    background: none;
    cursor: pointer;
    white-space: nowrap;
  }
  .pr-chip:hover {
    background: color-mix(in srgb, var(--pr-color) 15%, transparent);
  }
  .pr-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--pr-color);
    flex-shrink: 0;
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
  /* Fetching state: keep the button legible (it IS the progress indicator) even
     though it's disabled, and spin the glyph. */
  .fetch-btn.busy {
    opacity: 0.9;
  }
  .fetch-btn .spin {
    display: inline-block;
    animation: b9spin 0.9s linear infinite;
  }
  @keyframes b9spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .fetch-btn .spin {
      animation: none;
      opacity: 0.6;
    }
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
    /* Fixed to the scroll viewport height (not min-height, which would grow with
       content) so the sidebar and main column are each BOUNDED and scroll their
       own overflow independently — a tall sidebar no longer stretches the main
       column. A view that opts into flex:1 (Local Changes) still fills to the
       bottom. In the narrow @media layout the columns switch to overflow:visible
       and the whole page scrolls via .scroll-area instead. */
    display: flex;
    flex-direction: column;
    height: 100%;
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
    /* Fill the scroll-inner column and let the columns stretch to full height so
       a full-height main view (Local Changes) reaches the bottom. */
    flex: 1;
    min-height: 0;
    align-items: stretch;
  }
  .side-col {
    /* Width is the persisted --sidebar-w (set inline); the @media below overrides
       flex for the stacked column layout. */
    flex: 0 0 var(--sidebar-w, 240px);
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
    /* Scroll the sidebar independently of the main column: min-height:0 lets it
       shrink to the shell height (instead of forcing the shell — and thus the main
       column — taller), and overflow-y:auto scrolls its own overflow. */
    min-height: 0;
    overflow: hidden auto;
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
  /* Wrapper around the swappable views (timeline / Local Changes). Reproduces the main
     column's flex context so the graph still fills and the details pane still pins, while
     giving the screen/repo-switch zoom-fade a single element to transform. The transform
     is only applied for ~260ms by the WAAPI pulse, so it doesn't affect resting layout. */
  .view-swap {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    transform-origin: center top;
  }
  .main-col {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    /* The graph panel (CollapsiblePanel fill) grows to occupy the timeline height;
       the slide-up details pane caps itself (max-height) and the Local Changes view
       fills. Each region scrolls internally, so the column itself doesn't scroll
       (overflow:hidden clips any overshoot on very short windows instead of adding a
       second scrollbar). The narrow @media below restores page scrolling. */
    overflow: hidden;
  }

  /* Slide-up commit-details pane (timeline view): a non-growing column holding the
     drag handle + the CommitDetail panel, which takes a FIXED height (detailsHeight)
     and scrolls internally, so the pane's height is stable while the diff loads.
     flex:0 0 auto so the pane keeps its requested height and the GRAPH (flex:1, scrolls
     internally) absorbs any shortage — otherwise a short window would shrink the pane
     and the fixed-height panel inside would spill over the content below it. */
  .details-pane {
    flex: 0 0 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* When the graph is collapsed, the details pane grows to fill the freed space
     (CommitDetail switches to fill mode so its body scrolls). */
  .details-pane.fill-details {
    flex: 1 1 auto;
  }
  /* Horizontal drag bar on the graph↔details boundary; mirrors the sidebar handle. */
  .details-resize {
    flex: 0 0 10px;
    height: 10px;
    margin: -6px 0 -2px;
    cursor: ns-resize;
    position: relative;
    touch-action: none;
  }
  .details-resize::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 1px;
    background: var(--border);
    transform: translateY(-50%);
    transition: background 0.1s, height 0.1s;
  }
  .details-resize:hover::before {
    background: var(--accent);
    height: 2px;
  }
  @media (max-width: 900px) {
    .shell {
      /* Stacked layout: revert to content height so the sidebar and main column
         don't split the viewport height (they stack and the page scrolls). */
      flex: 0 1 auto;
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
    /* Stacked layout scrolls as one page (via .scroll-area), so the columns must
       not trap their own scroll here — undo the wide-mode independent scrolling. */
    .side-col,
    .main-col {
      overflow: visible;
    }
    .resize-handle {
      display: none;
    }
  }
</style>
