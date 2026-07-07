<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import type { PrerequisiteCheck } from "../types";

  let check = $state<PrerequisiteCheck | null>(null);
  let installing = $state(false);
  let installStatus = $state<string | null>(null);

  onMount(async () => {
    try {
      check = await api.checkPrerequisites();
    } catch {
      check = null;
    }
  });

  let ok = $derived(check ? check.git && check.python3 : true);

  async function installTools() {
    installing = true;
    installStatus = null;
    try {
      installStatus = await api.installCommandLineTools();
    } catch (e) {
      installStatus = String(e);
    } finally {
      installing = false;
    }
  }
</script>

{#if check && !ok}
  <div class="banner">
    <strong>⚠ Missing prerequisites</strong>
    <p>
      Git It needs <code>git</code> and <code>python3</code>, both from Apple's Xcode
      Command Line Tools.
    </p>
    <div class="actions">
      <button onclick={installTools} disabled={installing}>
        {installing ? "Installing…" : "Install Command Line Tools"}
      </button>
      {#if installStatus}
        <span class="status">{installStatus}</span>
      {/if}
    </div>
  </div>
{:else if check && check.python3 && !check.filterRepo}
  <div class="banner">
    <strong>⚠ Commit-time editing unavailable</strong>
    <p>
      The bundled <code>git-filter-repo</code> script could not be run. Commit-time editing
      is disabled.
    </p>
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
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
  }
  .actions button {
    font: inherit;
    color: #fff;
    background: #b45309;
    border: none;
    border-radius: 6px;
    padding: 5px 12px;
    cursor: pointer;
  }
  .actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .status {
    color: var(--text);
    opacity: 0.85;
  }
  code {
    font-family: var(--font-mono);
    background: #00000010;
    padding: 1px 5px;
    border-radius: 4px;
  }
</style>
