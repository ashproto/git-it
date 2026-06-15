<script lang="ts">
  import {
    laneX,
    curvedEdgePath,
    angularEdgePath,
    laneColor,
    type RowLayout,
    type Edge,
    type GeomConfig,
  } from "../graph";

  let {
    rows,
    heads,
    rowHeight = 30,
    lineStyle = "curved",
  }: {
    rows: RowLayout[];
    heads: boolean[];
    rowHeight?: number;
    lineStyle?: "curved" | "angular";
  } = $props();

  const laneWidth = 16;
  const offsetX = 12;
  const g: GeomConfig = $derived({ laneWidth, rowHeight, offsetX });
  const maxLanes = $derived(rows.reduce((m, r) => Math.max(m, r.width), 1));
  const width = $derived(offsetX + maxLanes * laneWidth);
  const height = $derived(Math.max(rows.length * rowHeight, rowHeight));

  const dotY = (i: number) => i * rowHeight + rowHeight / 2;
  const pathFor = (edge: Edge, topY: number) =>
    lineStyle === "angular" ? angularEdgePath(edge, topY, g) : curvedEdgePath(edge, topY, g);
</script>

<svg
  class="gutter"
  width={width}
  height={height}
  viewBox={`0 0 ${width} ${height}`}
  aria-hidden="true"
>
  {#each rows as row, i}
    {#each row.edges as edge}
      <path
        d={pathFor(edge, dotY(i))}
        stroke={laneColor(edge.colorIndex, null, {})}
        stroke-width="2"
        fill="none"
      />
    {/each}
  {/each}
  {#each rows as row, i}
    {#if heads[i]}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="7.5"
        fill="none"
        stroke="var(--accent)"
        stroke-width="1.5"
      />
    {/if}
    {#if row.isMerge}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="4.5"
        fill="var(--panel-bg)"
        stroke={laneColor(row.colorIndex, null, {})}
        stroke-width="2"
      />
    {:else}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="4.5"
        fill={laneColor(row.colorIndex, null, {})}
        stroke="var(--panel-bg)"
        stroke-width="1.5"
      />
    {/if}
  {/each}
</svg>

<style>
  .gutter {
    display: block;
    pointer-events: none;
  }
</style>
