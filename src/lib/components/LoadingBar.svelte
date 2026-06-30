<script lang="ts">
  import { appState } from "../store.svelte";
  const busy = $derived(appState.repoLoading || appState.navBusy);
</script>

{#if busy}
  <div class="loadbar" role="progressbar" aria-busy="true" aria-label="Loading">
    <div class="seg"></div>
  </div>
{/if}

<style>
  .loadbar {
    position: relative;
    height: 3px;
    width: 100%;
    overflow: hidden;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .seg {
    position: absolute;
    top: 0;
    height: 100%;
    width: 35%;
    border-radius: 999px;
    background: var(--accent);
    animation: gi-sweep 1.1s ease-in-out infinite;
  }
  @keyframes gi-sweep {
    0% { left: -35%; }
    100% { left: 100%; }
  }
  @media (prefers-reduced-motion: reduce) {
    .seg { animation: none; left: 0; width: 100%; opacity: 0.4; }
  }
</style>
