<script lang="ts">
  // Compact client-side filter field shared by the Pulls/Issues/Releases tabs.
  // Only one tab is mounted at a time, so the fixed id is unique in the DOM —
  // GithubView's ⌘F handler focuses it via getElementById.
  let {
    query = $bindable(),
    placeholder,
    shown,
    total,
  }: { query: string; placeholder: string; shown: number; total: number } = $props();

  const active = $derived(query.trim().length > 0);
</script>

<div class="filter">
  <div class="box">
    <input id="gh-filter-input" type="text" bind:value={query} {placeholder} aria-label={placeholder} />
    {#if active}
      <button class="clear" type="button" aria-label="Clear filter" onclick={() => (query = "")}>✕</button>
    {/if}
  </div>
  {#if active}
    <span class="count">{shown} of {total}</span>
  {/if}
</div>

<style>
  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .box {
    position: relative;
  }
  input {
    width: 200px;
    padding: 3px 22px 3px 10px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel-bg);
    color: var(--text);
    font-size: 12px;
  }
  input::placeholder {
    color: var(--text-muted);
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .clear {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    background: none;
    border: none;
    padding: 0 4px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
  }
  .clear:hover {
    color: var(--text);
  }
  .count {
    font-size: 11.5px;
    color: var(--text-muted);
    white-space: nowrap;
  }
</style>
