<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import GithubHeader from "./GithubHeader.svelte";
  import GithubTabs from "./GithubTabs.svelte";
  import GithubOverview from "./GithubOverview.svelte";
  import GithubSetupCard from "./GithubSetupCard.svelte";

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
</script>

<section class="gh">
  {#if !isTauri()}
    <p class="note">The GitHub screen is available in the desktop app only.</p>
  {:else if githubState.availLoading && !avail}
    <p class="note">Checking GitHub…</p>
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
        <p class="note">Loading…</p>
      {:else if githubState.statsError}
        <p class="note err">Could not load repository data ({githubState.statsError.kind}).</p>
      {:else if githubState.stats}
        <GithubOverview stats={githubState.stats} />
      {/if}
    {/if}
  {:else if avail}
    <GithubSetupCard availability={avail} onretry={() => appState.repo && githubState.refresh(appState.repo)} />
  {/if}
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
