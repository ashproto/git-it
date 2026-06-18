<script lang="ts">
  import { githubState, type GithubTab } from "../../githubState.svelte";

  // Phase 1 ships only Overview; later phases push more entries here.
  const TABS: { id: GithubTab; label: string }[] = [{ id: "overview", label: "Overview" }];
</script>

<nav class="tabs" aria-label="GitHub sections">
  {#each TABS as t (t.id)}
    <button
      class="tab"
      class:active={githubState.activeTab === t.id}
      aria-current={githubState.activeTab === t.id ? "page" : undefined}
      type="button"
      onclick={() => githubState.setActiveTab(t.id)}
    >
      {t.label}
    </button>
  {/each}
</nav>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    margin-bottom: 12px;
  }
  .tab {
    padding: 7px 14px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
  }
  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
</style>
