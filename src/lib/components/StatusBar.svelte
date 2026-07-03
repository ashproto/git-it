<script lang="ts">
  import { appState } from "../store.svelte";

  // ── Branch label ─────────────────────────────────────────────────────────────
  const branchLabel = $derived.by(() => {
    if (!appState.repo) return "no repository";
    const local = appState.refsByKind.local.find((r) => r.isHead);
    if (local) return local.name;
    if (appState.refsByKind.head.length > 0) return "detached HEAD";
    return "no branch";
  });

  const isDetached = $derived(
    !!appState.repo &&
    appState.refsByKind.local.find((r) => r.isHead) == null &&
    appState.refsByKind.head.length > 0,
  );

  // ── Ahead / behind ───────────────────────────────────────────────────────────
  const showAheadBehind = $derived(
    appState.currentUpstream !== null &&
    (appState.currentAhead > 0 || appState.currentBehind > 0),
  );

  // ── Working-tree summary ─────────────────────────────────────────────────────
  const treeSummary = $derived.by(() => {
    const rs = appState.repoStatus;
    if (!rs) return null; // browser / no repo
    const { staged, unstaged, untracked, conflicted } = rs;
    if (staged === 0 && unstaged === 0 && untracked === 0 && conflicted === 0) {
      return "clean";
    }
    const parts: string[] = [];
    if (staged > 0) parts.push(`${staged} staged`);
    if (unstaged > 0) parts.push(`${unstaged} unstaged`);
    if (untracked > 0) parts.push(`${untracked} untracked`);
    if (conflicted > 0) parts.push(`${conflicted} conflict${conflicted > 1 ? "s" : ""}`);
    return parts.join(" · ");
  });

  // ── Spinner visibility ────────────────────────────────────────────────────────
  // Spin for ANY in-flight work — remote ops, history rewrites, AND the general
  // navigation/git-action busy state (checkout, delete, fetch, fast-forward, repo
  // load). Without navBusy/repoLoading the bar showed only text during a delete, so
  // the app looked frozen.
  const spinning = $derived(
    appState.remoteOpActive || appState.isRewriting || appState.navBusy || appState.repoLoading,
  );
</script>

<div class="status-bar" role="status" aria-live="polite">
  <!-- Left: branch + ahead/behind -->
  <span class="section left" class:detached={isDetached} title={branchLabel}>
    <span class="branch-icon" aria-hidden="true">⎇</span>
    {branchLabel}
    {#if showAheadBehind}
      <span class="ahead-behind" aria-label="{appState.currentAhead} ahead, {appState.currentBehind} behind">
        {#if appState.currentAhead > 0}↑{appState.currentAhead}{/if}{#if appState.currentAhead > 0 && appState.currentBehind > 0}&thinsp;{/if}{#if appState.currentBehind > 0}↓{appState.currentBehind}{/if}
      </span>
    {/if}
  </span>

  <!-- Middle: working-tree summary -->
  {#if treeSummary !== null}
    <span class="section mid" class:clean={treeSummary === "clean"}>
      {treeSummary}
    </span>
  {/if}

  <!-- Right: status text + optional spinner -->
  <span class="section right">
    {#if spinning}
      {@const opLabel = appState.busyOp ? `${appState.busyOp} in progress` : "Operation in progress"}
      <span class="spinner" aria-label={opLabel} title={opLabel}></span>
    {/if}
    <span class="status-text" title={appState.status}>{appState.status}</span>
  </span>
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: 0;
    width: 100%;
    height: 24px;
    min-height: 24px;
    background: var(--header-bg);
    border-top: 1px solid var(--border-subtle);
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    box-sizing: border-box;
    /* Full-width bottom chrome: a direct flex child of the app-shell <main> (which has no
       padding), so it spans edge-to-edge with no margin hacks. A little inner padding
       keeps content off the window edges; sections carry their own padding too. */
    padding: 0 12px;
    flex-shrink: 0;
  }

  .section {
    display: flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
    overflow: hidden;
    flex-shrink: 0;
    padding: 0 10px;
  }

  .left {
    min-width: 0;
    max-width: 280px;
    flex-shrink: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    padding-left: 0;
    border-right: 1px solid var(--border-subtle);
  }
  .left.detached {
    color: var(--err);
  }

  .branch-icon {
    font-size: 12px;
    flex-shrink: 0;
    opacity: 0.7;
  }

  .ahead-behind {
    font-size: 11px;
    opacity: 0.8;
    flex-shrink: 0;
  }

  .mid {
    flex: 1 1 0;
    justify-content: center;
    overflow: hidden;
    text-overflow: ellipsis;
    border-right: 1px solid var(--border-subtle);
  }
  .mid.clean {
    opacity: 0.55;
  }

  .right {
    flex-shrink: 1;
    min-width: 0;
    max-width: 380px;
    overflow: hidden;
    justify-content: flex-end;
    gap: 6px;
    padding-right: 0;
  }

  .status-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  /* Pulsing dot — signals an in-flight operation (remote or rewrite) */
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.3; }
  }
  .spinner {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
    animation: pulse 1.2s ease-in-out infinite;
  }
</style>
