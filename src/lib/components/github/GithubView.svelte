<script lang="ts">
  import { appState } from "../../store.svelte";
  import { dialogs } from "../../dialogs.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { githubState } from "../../githubState.svelte";
  import GithubHeader from "./GithubHeader.svelte";
  import GithubTabs from "./GithubTabs.svelte";
  import GithubOverview from "./GithubOverview.svelte";
  import GithubSetupCard from "./GithubSetupCard.svelte";
  import GithubPulls from "./GithubPulls.svelte";
  import GithubIssues from "./GithubIssues.svelte";
  import GithubReleases from "./GithubReleases.svelte";
  import GithubActions from "./GithubActions.svelte";
  import GithubInsights from "./GithubInsights.svelte";
  import GithubActionModal from "./GithubActionModal.svelte";
  import GithubSkeleton from "./GithubSkeleton.svelte";
  import { revealIn } from "../../github/motion";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  // (Re)load availability + stats whenever the GitHub screen is active for a repo.
  $effect(() => {
    const repo = appState.repo;
    if (repo && appState.activeView === "github") {
      void githubState.ensure(repo);
    }
  });

  const avail = $derived(githubState.availability);

  // ⌘F focuses the active tab's filter field. This component only mounts while
  // the GitHub screen is shown, so the graph screen's ⌘F handler (gated on
  // activeView === "timeline") never conflicts. Only one tab renders at a time,
  // so the fixed input id is unique; tabs without a filter (overview/actions/
  // insights) simply have no input and ⌘F is a no-op.
  function onWindowKeydown(e: KeyboardEvent) {
    // toLowerCase: with Caps Lock on, e.key reports "F".
    if (e.key.toLowerCase() !== "f" || !e.metaKey || e.shiftKey || e.altKey || e.ctrlKey) return;
    if (dialogs.state.kind !== "none") return;
    if (githubActions.pending !== null) return; // GithubActionModal is open
    // Don't steal focus from a text field the user is typing in.
    const t = e.target as HTMLElement | null;
    if (t?.closest?.('input, textarea, [contenteditable="true"]')) return;
    const input = document.getElementById("gh-filter-input");
    if (!(input instanceof HTMLInputElement)) return;
    e.preventDefault();
    input.focus();
    input.select();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<section class="gh">
  {#if !isTauri()}
    <p class="note">The GitHub screen is available in the desktop app only.</p>
  {:else if githubState.availLoading && !avail}
    <GithubSkeleton variant="header" />
  {:else if avail && avail.kind === "Ok"}
    <GithubHeader
      stats={githubState.stats}
      owner={avail.owner}
      repo={avail.repo}
      onrefresh={() => appState.repo && githubState.refresh(appState.repo)}
    />
    <GithubTabs />
    {#if githubState.activeTab === "overview"}
      {#if githubState.statsLoading && !githubState.stats}
        <GithubSkeleton variant="overview" />
      {:else if githubState.statsError}
        <p class="note err">Could not load repository data ({githubState.statsError.kind}).</p>
      {:else if githubState.stats}
        <div in:revealIn><GithubOverview stats={githubState.stats} /></div>
      {/if}
    {:else if githubState.activeTab === "pulls"}
      <GithubPulls />
    {:else if githubState.activeTab === "issues"}
      <GithubIssues />
    {:else if githubState.activeTab === "releases"}
      <GithubReleases />
    {:else if githubState.activeTab === "actions"}
      <GithubActions />
    {:else if githubState.activeTab === "insights"}
      <GithubInsights />
    {/if}
  {:else if avail}
    <GithubSetupCard availability={avail} onretry={() => appState.repo && githubState.refresh(appState.repo)} />
  {/if}
  <GithubActionModal />
</section>

<style>
  .gh {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .note {
    margin: 16px 2px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
