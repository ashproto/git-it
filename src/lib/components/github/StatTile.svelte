<script lang="ts">
  import { formatCompact } from "../../github/format";
  let {
    label,
    value,
    onclick,
  }: { label: string; value: number; onclick?: () => void } = $props();
</script>

<button class="tile" class:clickable={!!onclick} type="button" disabled={!onclick} {onclick}>
  <span class="num">{formatCompact(value)}</span>
  <span class="lbl">{label}</span>
</button>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 14px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel-bg);
    color: var(--text);
    text-align: left;
    cursor: default;
  }
  .tile.clickable {
    cursor: pointer;
  }
  .tile.clickable:hover {
    border-color: var(--accent);
  }
  .num {
    font-size: 17px;
    font-weight: 600;
  }
  .lbl {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
