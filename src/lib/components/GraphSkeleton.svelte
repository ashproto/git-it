<script lang="ts">
  const rows = Array.from({ length: 8 }, (_, i) => i);
  const widths = [54, 42, 62, 48, 58, 38, 66, 50];
</script>

<div class="gs" aria-hidden="true">
  {#each rows as i}
    <div class="row">
      <span class="dot" style={`margin-left:${(i % 3) * 14}px`}></span>
      <span class="bar" style={`width:${widths[i]}%`}></span>
      <span class="meta"></span>
      <span class="glow" style={`animation-delay:${i * 60}ms`}></span>
    </div>
  {/each}
</div>

<style>
  .gs { padding: 10px 16px; display: flex; flex-direction: column; gap: 12px; }
  .row { position: relative; overflow: hidden; display: flex; align-items: center; gap: 12px; height: 16px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: color-mix(in srgb, var(--text-muted) 16%, transparent); flex: none; }
  .bar { height: 9px; border-radius: 999px; background: color-mix(in srgb, var(--text-muted) 16%, transparent); }
  .meta { height: 9px; width: 12%; border-radius: 999px; background: color-mix(in srgb, var(--text-muted) 16%, transparent); margin-left: auto; }
  .glow { position: absolute; inset: 0; background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 22%, transparent), transparent); animation: gs-sweep 1.5s ease-in-out infinite; }
  @keyframes gs-sweep { 0% { transform: translateX(-100%); } 100% { transform: translateX(280%); } }
  @media (prefers-reduced-motion: reduce) { .glow { animation: none; opacity: 0.25; } }
</style>
