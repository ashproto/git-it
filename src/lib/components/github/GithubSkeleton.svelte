<script lang="ts">
  import Skeleton from "./Skeleton.svelte";

  let {
    variant,
    rows = 6,
  }: {
    variant: "header" | "overview" | "list" | "releases" | "actions" | "card" | "detail";
    rows?: number;
  } = $props();

  const rowArr = $derived(Array.from({ length: Math.max(1, rows) }));
  const listW = ["72%", "60%", "80%", "55%", "68%", "64%"];
  const metaW = ["44%", "52%", "38%", "46%", "40%", "50%"];
  // Stagger rows in; capped so long lists don't drift too far.
  const delay = (i: number) => `${Math.min(i, 7) * 70}ms`;
</script>

<div class="gh-skel {variant}" role="status" aria-label="Loading">
  <span class="sr-only">Loading…</span>

  {#if variant === "header"}
    <div class="rise-in hd">
      <Skeleton w="180px" h="18px" />
      <Skeleton w="320px" h="13px" />
    </div>
    <div class="rise-in tiles" style="animation-delay:70ms">
      {#each Array.from({ length: 5 }) as _, i (i)}
        <Skeleton w="66px" h="38px" radius="8px" />
      {/each}
    </div>
  {:else if variant === "overview"}
    {#each Array.from({ length: 5 }) as _, i (i)}
      <div class="rise-in meta-row" style="animation-delay:{delay(i)}">
        <Skeleton w="92px" h="12px" />
        <Skeleton w={["60%", "45%", "38%", "30%", "52%"][i]} h="12px" />
      </div>
    {/each}
    <div class="rise-in topics" style="animation-delay:{delay(5)}">
      {#each Array.from({ length: 3 }) as _, i (i)}
        <Skeleton w="62px" h="18px" radius="999px" />
      {/each}
    </div>
  {:else if variant === "list"}
    {#each rowArr as _, i (i)}
      <div class="rise-in row" style="animation-delay:{delay(i)}">
        <Skeleton w={listW[i % 6]} h="13px" />
        <Skeleton w={metaW[i % 6]} h="10px" />
      </div>
    {/each}
  {:else if variant === "actions"}
    {#each rowArr as _, i (i)}
      <div class="rise-in arow" style="animation-delay:{delay(i)}">
        <Skeleton w="14px" h="14px" circle />
        <Skeleton w={["52%", "40%", "60%", "46%"][i % 4]} h="12px" />
        <Skeleton w="70px" h="10px" />
        <Skeleton w="54px" h="10px" />
      </div>
    {/each}
  {:else if variant === "card"}
    <Skeleton w="40%" h="13px" />
    <div class="card-bars">
      {#each ["90%", "75%", "60%"] as w (w)}
        <Skeleton {w} h="11px" />
      {/each}
    </div>
  {:else if variant === "releases"}
    <div class="rise-in metrics">
      {#each Array.from({ length: 4 }) as _, i (i)}
        <div class="metric">
          <Skeleton w="56px" h="20px" />
          <Skeleton w="72px" h="11px" />
        </div>
      {/each}
    </div>
    <div class="charts">
      {#each Array.from({ length: 2 }) as _, c (c)}
        <div class="rise-in chart" style="animation-delay:{delay(c + 1)}">
          <Skeleton w="140px" h="12px" />
          <div class="chart-rows">
            {#each Array.from({ length: 4 }) as _, i (i)}
              <div class="bar-row">
                <Skeleton w="80px" h="11px" />
                <Skeleton w="100%" h="8px" radius="999px" />
                <Skeleton w="40px" h="11px" />
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {:else if variant === "detail"}
    <div class="rise-in dhd">
      <Skeleton w="58%" h="18px" />
      <div class="dsub">
        {#each ["64px", "80px", "120px", "90px"] as w (w)}
          <Skeleton {w} h="11px" />
        {/each}
      </div>
    </div>
    <div class="rise-in" style="animation-delay:140ms"><Skeleton w="100%" h="120px" radius="10px" /></div>
    {#each Array.from({ length: 2 }) as _, i (i)}
      <div class="rise-in" style="animation-delay:{delay(i + 3)}"><Skeleton w="100%" h="64px" radius="10px" /></div>
    {/each}
  {/if}
</div>

<style>
  .gh-skel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .gh-skel.overview,
  .gh-skel.releases {
    gap: 16px;
  }
  .gh-skel.card {
    gap: 8px;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  .hd {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .tiles {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .meta-row {
    display: grid;
    grid-template-columns: 92px 1fr;
    align-items: center;
    gap: 6px 18px;
  }
  .topics {
    display: flex;
    gap: 6px;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
  }
  .arow {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
  }
  .card-bars {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .metrics {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
  }
  .metric {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .charts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 16px;
  }
  .chart {
    display: flex;
    flex-direction: column;
    gap: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    background: var(--panel-bg);
  }
  .chart-rows {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .bar-row {
    display: grid;
    grid-template-columns: 80px 1fr 40px;
    align-items: center;
    gap: 8px;
  }
  .dhd {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .dsub {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .rise-in {
    animation: gh-rise 0.5s ease both;
  }
  @keyframes gh-rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .rise-in {
      animation: none;
    }
  }
</style>
