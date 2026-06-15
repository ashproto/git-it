<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { PrerequisiteCheck } from "../types";

  let check = $state<PrerequisiteCheck | null>(null);

  onMount(async () => {
    try {
      check = await api.checkPrerequisites();
    } catch {
      check = null;
    }
  });

  let ok = $derived(check ? check.git && check.filterRepo : true);
</script>

{#if check && !ok}
  <div class="banner">
    <strong>⚠ Missing prerequisites</strong>
    {#if !check.git}
      <p>
        <code>git</code> was not found on your PATH. Install Xcode Command Line Tools or
        <code>brew install git</code>.
      </p>
    {/if}
    {#if !check.filterRepo}
      <p>
        <code>git-filter-repo</code> was not found on your PATH. Install with:
        <code>brew install git-filter-repo</code>
      </p>
    {/if}
  </div>
{/if}

<style>
  .banner {
    background: #fde68a22;
    border: 1px solid #fde68a;
    color: var(--text);
    border-radius: 8px;
    padding: 10px 14px;
    margin-bottom: 8px;
    font-size: 13px;
  }
  .banner strong {
    color: #b45309;
  }
  .banner p {
    margin: 6px 0 0 0;
  }
  code {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    background: #00000010;
    padding: 1px 5px;
    border-radius: 4px;
  }
</style>
