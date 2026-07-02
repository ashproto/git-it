<script lang="ts">
  import { appState } from "../store.svelte";
  import { contextMenu, type MenuItem } from "../contextMenu.svelte";
  import { dialogs } from "../dialogs.svelte";
  import { gitActions, jumpToRefWithLoad } from "../gitActions";
  import { branchColorDialog } from "../branchColorDialog.svelte";
  import { buildRefTree } from "../refTree";
  import type { RefEntry } from "../types";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import RefTree from "./RefTree.svelte";
  import StashPanel from "./StashPanel.svelte";
  import { githubState } from "../githubState.svelte";
  import { prForBranch } from "../github/branchPr";

  const refs = $derived(appState.refsByKind);
  // Folderize slashed ref names (feature/x → feature ▸ x), Fork/SourceTree-style.
  const localTree = $derived(buildRefTree(refs.local));
  const remoteTree = $derived(buildRefTree(refs.remote));
  const tagTree = $derived(buildRefTree(refs.tags));

  // Uncommitted-change count for the pinned "Working Copy" entry — mirrors the
  // count used by the synthetic graph row.
  const wcCount = $derived(
    Math.max(
      appState.workingChanges.length,
      appState.repoStatus
        ? appState.repoStatus.staged +
            appState.repoStatus.unstaged +
            appState.repoStatus.untracked +
            appState.repoStatus.conflicted
        : 0,
    ),
  );

  // Ref sections (Local/Remotes/Tags) now use CollapsiblePanel, which manages its
  // own collapse state — no per-section open flags needed here.

  // Visual "selected row" state for the branch/remote/tag lists — a composite
  // `${kind}:${name}` key (kinds can share names, e.g. local "main" vs a tag
  // "main"), so it must include the kind to disambiguate.
  let selectedRefKey = $state<string | null>(null);
  function selectRef(kind: "local" | "remote" | "tag", name: string) {
    selectedRefKey = `${kind}:${name}`;
  }

  // Display-only navigation: focus + select the ref's commit and scroll to it.
  // (Checkout etc. arrive in the operations phases.)
  function jumpTo(sha: string) {
    appState.setCurrent(sha);
    appState.selected = new Set([sha]);
    // The graph virtualizes rows AND pages history lazily, so the target may not
    // be loaded yet (e.g. a tag far down). jumpToRefWithLoad pages in more history
    // until the commit is present, then scrolls the graph to it.
    jumpToRefWithLoad(sha);
  }

  function onRefCheckout(r: RefEntry, kind: "local" | "remote" | "tag") {
    if (kind === "local") {
      gitActions.checkout(r.name);
    } else if (kind === "remote") {
      // origin/feature → local "feature" tracking branch (create if missing, else switch).
      const short = r.name.replace(/^[^/]+\//, "");
      const exists = appState.refsByKind.local.some((b) => b.name === short);
      if (exists) gitActions.checkout(short);
      else gitActions.createBranch(short, r.name).then((ok) => { if (ok) gitActions.checkout(short); });
    }
    // tag → no-op (double-click only jumps).
  }

  async function confirmDeleteBranch(name: string) {
    const detail = appState.refsDetailed.find((d) => d.kind === "local" && d.name === name);
    const upstream = detail?.upstream ?? null; // "origin/feature" | null
    const res = await dialogs.confirmBranchDelete({ branch: name, upstream });
    if (!res.confirmed) return;
    let remote: string | undefined;
    let remoteBranch: string | undefined;
    if (res.deleteRemote && upstream) {
      const slash = upstream.indexOf("/");
      remote = upstream.slice(0, slash);
      remoteBranch = upstream.slice(slash + 1);
    }
    await gitActions.deleteBranch(name, res.force, res.deleteRemote && !!upstream, remote, remoteBranch);
  }

  function onRefContext(event: MouseEvent, r: RefEntry, kind: "local" | "remote" | "tag") {
    event.preventDefault();
    jumpTo(r.sha);
    selectRef(kind, r.name);
    const items: MenuItem[] = [];
    if (kind === "local") {
      items.push({ label: `Checkout ${r.name}`, action: () => gitActions.checkout(r.name) });
      if (!r.isHead) {
        const detail = appState.refsDetailed.find((d) => d.kind === "local" && d.name === r.name);
        const upstream = detail?.upstream ?? null; // e.g. "origin/main"
        if (upstream) {
          const remote = upstream.slice(0, upstream.indexOf("/"));
          items.push({
            label: `Fast-forward to ${remote}`,
            action: () => gitActions.fastForwardBranch(r.name, remote),
          });
        }
      }
      // Branch ↔ GitHub bridge: jump to the branch's open PR, or offer to
      // create one for the checked-out branch. No gh call is made from the
      // menu itself — only the already-cached pulls list is consulted.
      if (githubState.hasGithubRemote) {
        // Trust the cached pulls list only when it belongs to THIS repo (the
        // GitHub screen caches one repo at a time) — else treat it as unknown.
        const pr =
          githubState.loadedRepoPath === appState.repo
            ? prForBranch(githubState.pulls.data, r.name)
            : null;
        if (pr) {
          items.push({
            label: `Open pull request #${pr.number}`,
            action: () => gitActions.openPrInApp(pr.number),
          });
        } else if (r.isHead) {
          items.push({
            label: "Create pull request…",
            action: () => gitActions.createPullRequest(),
          });
        }
      }
      items.push({
        label: "Rename…",
        action: async () => {
          const n = await dialogs.prompt({ title: "Rename branch", label: "New name", value: r.name });
          if (n && n !== r.name) gitActions.renameBranch(r.name, n);
        },
      });
      items.push({
        label: "Create tag here…",
        action: async () => {
          const n = await dialogs.prompt({ title: "New tag", label: "Tag name", placeholder: "v1.0.0" });
          if (n) gitActions.createTag(n, r.sha);
        },
      });
      items.push({ separator: true });
      items.push({ label: "Delete branch", danger: true, action: () => confirmDeleteBranch(r.name) });
    } else if (kind === "remote") {
      items.push({ label: `Checkout ${r.name} (detached)`, action: () => gitActions.checkout(r.name) });
      items.push({
        label: "Create local branch…",
        action: async () => {
          const n = await dialogs.prompt({ title: `New branch from ${r.name}`, label: "Branch name" });
          if (n) gitActions.createBranch(n, r.sha);
        },
      });
    } else {
      items.push({ label: `Checkout ${r.name} (detached)`, action: () => gitActions.checkout(r.name) });
      items.push({ separator: true });
      items.push({
        label: "Delete tag",
        danger: true,
        action: async () => {
          const ok = await dialogs.confirm({
            title: "Delete tag",
            message: `Delete tag "${r.name}"?`,
            confirmLabel: "Delete",
            danger: true,
          });
          if (ok) gitActions.deleteTag(r.name);
        },
      });
    }

    // Per-ref colour override (applies to any kind).
    items.push({ separator: true });
    items.push({ label: "Set branch colour…", action: () => branchColorDialog.openFor(r.name) });

    // Merge this ref into the current branch (contract e: ff/no-ff/squash are
    // distinct items, never combined). Skip when this IS the checked-out branch.
    // Squash is now wired: a clean squash stages changes and opens the commit
    // composer with a prefilled message; a conflicting squash surfaces conflicts
    // in the working-copy panel (no MERGE_HEAD, so the user resolves then commits).
    if (!(kind === "local" && r.isHead)) {
      items.push({ separator: true });
      items.push({ label: `Merge ${r.name} into current`, action: () => gitActions.merge(r.name) });
      items.push({ label: `Merge ${r.name} (no-ff)`, action: () => gitActions.merge(r.name, { noFf: true }) });
      items.push({ label: `Squash merge ${r.name} into current`, action: () => gitActions.squashMerge(r.name) });
    }
    contextMenu.openAt(event.clientX, event.clientY, items);
  }
</script>

<aside class="sidebar">
  <nav class="view-nav" aria-label="Main view">
    <button
      class="wc-entry"
      class:active={appState.activeView === "changes"}
      aria-current={appState.activeView === "changes" ? "page" : undefined}
      onclick={() => appState.setActiveView("changes")}
      title="Local changes — uncommitted work"
    >
      <span class="wc-dot" aria-hidden="true"></span>
      <span class="wc-label">Local Changes</span>
      {#if wcCount > 0}<span class="wc-count">{wcCount}</span>{/if}
    </button>
    <button
      class="wc-entry"
      class:active={appState.activeView === "timeline"}
      aria-current={appState.activeView === "timeline" ? "page" : undefined}
      onclick={() => appState.setActiveView("timeline")}
      title="Commit timeline — the graph of commits"
    >
      <span class="tl-dot" aria-hidden="true"></span>
      <span class="wc-label">Commit Timeline</span>
    </button>
    {#if githubState.hasGithubRemote}
      <button
        class="wc-entry"
        class:active={appState.activeView === "github"}
        aria-current={appState.activeView === "github" ? "page" : undefined}
        onclick={() => appState.setActiveView("github")}
        title="GitHub — pull requests, issues, releases and more"
      >
        <span class="gh-dot" aria-hidden="true"></span>
        <span class="wc-label">GitHub</span>
      </button>
    {/if}
  </nav>

  {#if refs.head.length}
    <button class="ref detached" onclick={() => jumpTo(refs.head[0].sha)} title="Detached HEAD">
      <span class="dot head" aria-hidden="true"></span>
      <span class="rn">HEAD (detached)</span>
    </button>
  {/if}
  <CollapsiblePanel title="Local">
    {#snippet headerActions()}<span class="ref-count">{refs.local.length}</span>{/snippet}
    <RefTree nodes={localTree} kind="local" onJump={jumpTo} onContext={onRefContext} colorOf={(ref) => appState.colorForRef(ref.name, ref.sha)} onCheckout={onRefCheckout} selectedKey={selectedRefKey} onSelect={selectRef} />
    {#if refs.local.length === 0}<p class="none">No local branches</p>{/if}
  </CollapsiblePanel>

  <CollapsiblePanel title="Remotes">
    {#snippet headerActions()}<span class="ref-count">{refs.remote.length}</span>{/snippet}
    <RefTree nodes={remoteTree} kind="remote" onJump={jumpTo} onContext={onRefContext} colorOf={(ref) => appState.colorForRef(ref.name, ref.sha)} onCheckout={onRefCheckout} selectedKey={selectedRefKey} onSelect={selectRef} />
    {#if refs.remote.length === 0}<p class="none">No remotes</p>{/if}
  </CollapsiblePanel>

  <CollapsiblePanel title="Tags">
    {#snippet headerActions()}<span class="ref-count">{refs.tags.length}</span>{/snippet}
    <RefTree nodes={tagTree} kind="tag" onJump={jumpTo} onContext={onRefContext} colorOf={(ref) => appState.colorForRef(ref.name, ref.sha)} onCheckout={onRefCheckout} selectedKey={selectedRefKey} onSelect={selectRef} />
    {#if refs.tags.length === 0}<p class="none">No tags</p>{/if}
  </CollapsiblePanel>

  <StashPanel />
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
  }
  /* Pinned view switcher — Local Changes / Commit Timeline entries. */
  .wc-entry {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    margin-bottom: 2px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
  }
  .wc-entry:hover {
    background: var(--row-hover);
  }
  .wc-entry.active {
    background: var(--row-selected);
  }
  .wc-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }
  .wc-label {
    flex: 1;
  }
  .wc-count {
    font-size: 11px;
    color: var(--text-muted);
    background: var(--btn-bg);
    border-radius: 999px;
    padding: 0 7px;
    min-width: 18px;
    text-align: center;
  }
  /* Two-item view switcher: Local Changes / Commit Timeline. */
  .view-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  /* Hollow node glyph for Commit Timeline — distinct from the filled accent dot. */
  .tl-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted);
    box-sizing: border-box;
    flex-shrink: 0;
  }
  .gh-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent);
    flex-shrink: 0;
  }
  /* Count shown in each ref panel's header (via CollapsiblePanel headerActions). */
  .ref-count {
    font-size: 11px;
    color: var(--text-muted);
    font-weight: 500;
  }
  .ref {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px 5px 18px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .ref:hover {
    background: var(--row-hover);
  }
  .rn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot.head {
    background: var(--err);
  }
  .ref.detached .rn {
    color: var(--err);
    font-weight: 600;
  }
  .none {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
