<script lang="ts">
  let { size = 12 }: { size?: number } = $props();
</script>

<!-- Two linked working directories: intentionally distinct from the branch
     glyph while using the same rounded 2px outline language as RefIcon. -->
<svg
  class="worktree-icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <rect x="3" y="4" width="8" height="7" rx="1.5" />
  <rect x="13" y="13" width="8" height="7" rx="1.5" />
  <path d="M11 7.5h2a4 4 0 0 1 4 4V13" />
</svg>

<style>
  .worktree-icon {
    display: block;
    flex-shrink: 0;
  }
</style>
