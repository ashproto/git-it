<script lang="ts">
  import { appState } from "../store.svelte";
  const busy = $derived(appState.repoLoading || appState.navBusy);
</script>

{#if busy}
  <!-- Zero-height wrapper: the bar itself is absolutely positioned so toggling it
       on/off never reflows the page. Previously the bar took 3px of flow height, so
       every appearance shoved all content below it down 3px then snapped back — a
       visible jank on every repo switch / nav. -->
  <div class="loadbar-wrap">
    <div class="loadbar" role="progressbar" aria-busy="true" aria-label="Loading">
      <div class="seg"></div>
    </div>
  </div>
{/if}

<style>
  .loadbar-wrap {
    position: relative;
    height: 0; /* no flow height — the bar overlays the top edge of the content below */
  }
  .loadbar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 40;
    height: 3px;
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
