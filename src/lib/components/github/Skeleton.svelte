<script lang="ts">
  // A single placeholder block with an accent-tinted shimmer sweep.
  // Decorative only — hidden from assistive tech (the parent skeleton
  // carries the `role="status"` loading announcement).
  let {
    w = "100%",
    h = "12px",
    radius = "6px",
    circle = false,
  }: { w?: string; h?: string; radius?: string; circle?: boolean } = $props();
</script>

<span
  class="sk"
  aria-hidden="true"
  style="width:{w}; height:{h}; border-radius:{circle ? '50%' : radius};"
></span>

<style>
  .sk {
    position: relative;
    display: block;
    flex: none;
    overflow: hidden;
    /* Translucent grey so it reads over the frosted-glass background
       without punching an opaque hole in it. */
    background: color-mix(in srgb, var(--text-muted) 16%, transparent);
  }
  .sk::after {
    content: "";
    position: absolute;
    inset: 0;
    transform: translateX(-120%);
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, var(--accent) 32%, transparent),
      transparent
    );
    animation: gh-sweep 1.5s ease-in-out infinite;
  }
  @keyframes gh-sweep {
    to {
      transform: translateX(120%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sk::after {
      animation: none;
      opacity: 0;
    }
    .sk {
      opacity: 0.7;
    }
  }
</style>
