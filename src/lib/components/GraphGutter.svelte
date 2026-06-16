<script lang="ts">
  import {
    laneX,
    curvedEdgePath,
    angularEdgePath,
    laneColor,
    LANE_WIDTH,
    OFFSET_X,
    type RowLayout,
    type Edge,
    type GeomConfig,
  } from "../graph";

  let {
    rows,
    heads,
    rowHeight = 30,
    lineStyle = "curved",
    renderStart = 0,
    renderEnd = undefined,
    colorOf,
  }: {
    rows: RowLayout[];
    heads: boolean[];
    rowHeight?: number;
    lineStyle?: "curved" | "angular";
    /** First commit index whose dot/edges to draw (inclusive). */
    renderStart?: number;
    /** One past the last commit index to draw (exclusive); undefined ⇒ all rows. */
    renderEnd?: number;
    /** Resolve a lane colour from its colorIndex (override-aware); defaults to the palette. */
    colorOf?: (colorIndex: number) => string;
  } = $props();

  // Lane-colour resolver: a manual override wins where present, else the palette.
  const resolveColor = (idx: number) => (colorOf ? colorOf(idx) : laneColor(idx, null, {}));

  const g: GeomConfig = $derived({ laneWidth: LANE_WIDTH, rowHeight, offsetX: OFFSET_X });
  const maxLanes = $derived(rows.reduce((m, r) => Math.max(m, r.width), 1));
  const width = $derived(OFFSET_X + maxLanes * LANE_WIDTH);
  // SVG stays full height so every dot keeps its absolute y (dotY uses the global
  // commit index) — only the *child elements* are windowed, never the coordinate
  // system. That keeps the lanes pixel-aligned with the table rows for free.
  const height = $derived(Math.max(rows.length * rowHeight, rowHeight));

  // Windowed render range. Dots cover [winStart, winEnd); edges start one row
  // earlier because row i's edges descend from dot i into dot i+1, so the row
  // just above the window paints the band crossing the window's top border.
  const winStart = $derived(Math.max(0, Math.min(renderStart, rows.length)));
  const winEnd = $derived(Math.min(rows.length, Math.max(winStart, renderEnd ?? rows.length)));
  const edgeStart = $derived(Math.max(0, winStart - 1));
  const edgeRows = $derived(rows.slice(edgeStart, winEnd));
  const dotRows = $derived(rows.slice(winStart, winEnd));

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
  {#each edgeRows as row, k}
    {@const i = edgeStart + k}
    {#each row.edges as edge}
      <path
        d={pathFor(edge, dotY(i))}
        stroke={resolveColor(edge.colorIndex)}
        stroke-width="2"
        fill="none"
      />
    {/each}
  {/each}
  {#each dotRows as row, k}
    {@const i = winStart + k}
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
        stroke={resolveColor(row.colorIndex)}
        stroke-width="2"
      />
    {:else}
      <circle
        cx={laneX(row.lane, g)}
        cy={dotY(i)}
        r="4.5"
        fill={resolveColor(row.colorIndex)}
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
