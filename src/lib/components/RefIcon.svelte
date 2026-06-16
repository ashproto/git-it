<script lang="ts">
  // A small ref-kind glyph: branch (local), cloud (remote-tracking), tag, or
  // commit (detached HEAD). Drawn with `currentColor` so the parent tints it to
  // the ref's lane colour. Kind discriminates local/remote/tag; colour matches
  // the graph — the two together replace the old colour-only swatch.
  let {
    kind,
    size = 12,
  }: { kind: "local" | "remote" | "tag" | "head"; size?: number } = $props();
</script>

{#if kind === "tag"}
  <svg
    class="ref-icon" width={size} height={size} viewBox="0 0 24 24" fill="none"
    stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
    <line x1="7" y1="7" x2="7.01" y2="7" />
  </svg>
{:else if kind === "remote"}
  <svg
    class="ref-icon" width={size} height={size} viewBox="0 0 24 24" fill="none"
    stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z" />
  </svg>
{:else if kind === "head"}
  <svg
    class="ref-icon" width={size} height={size} viewBox="0 0 24 24" fill="none"
    stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="4" />
    <line x1="1.05" y1="12" x2="7" y2="12" />
    <line x1="17.01" y1="12" x2="22.96" y2="12" />
  </svg>
{:else}
  <svg
    class="ref-icon" width={size} height={size} viewBox="0 0 24 24" fill="none"
    stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true"
  >
    <line x1="6" y1="3" x2="6" y2="15" />
    <circle cx="18" cy="6" r="3" />
    <circle cx="6" cy="18" r="3" />
    <path d="M18 9a9 9 0 0 1-9 9" />
  </svg>
{/if}

<style>
  .ref-icon {
    flex-shrink: 0;
    vertical-align: middle;
  }
</style>
