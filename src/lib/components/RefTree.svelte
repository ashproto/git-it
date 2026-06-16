<script lang="ts">
  import type { RefEntry } from "../types";
  import type { RefTreeNode } from "../refTree";

  let {
    nodes,
    kind,
    onJump,
    onContext,
    colorOf,
  }: {
    nodes: RefTreeNode[];
    kind: "local" | "remote" | "tag";
    onJump: (sha: string) => void;
    onContext: (e: MouseEvent, ref: RefEntry, kind: "local" | "remote" | "tag") => void;
    // Resolves a ref's swatch colour — its graph lane colour (or manual override).
    colorOf: (ref: RefEntry) => string;
  } = $props();

  // Collapsed folder paths (in-memory; folders default expanded).
  let collapsed = $state<Set<string>>(new Set());
  function toggle(path: string) {
    const next = new Set(collapsed);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    collapsed = next;
  }
  const indent = (depth: number) => 8 + depth * 14;
</script>

{#snippet tree(items: RefTreeNode[], depth: number)}
  {#each items as node (node.kind === "folder" ? "d:" + node.path : "l:" + node.ref.name)}
    {#if node.kind === "folder"}
      <button
        type="button"
        class="folder"
        style={`padding-left:${indent(depth)}px`}
        onclick={() => toggle(node.path)}
        aria-expanded={!collapsed.has(node.path)}
        title={node.path}
      >
        <span class="chev" class:open={!collapsed.has(node.path)} aria-hidden="true">▶</span>
        <span class="fn">{node.name}</span>
      </button>
      {#if !collapsed.has(node.path)}
        {@render tree(node.children, depth + 1)}
      {/if}
    {:else}
      <button
        type="button"
        class="ref"
        class:head={kind === "local" && node.ref.isHead}
        class:muted={kind === "remote"}
        style={`padding-left:${indent(depth) + 12}px`}
        onclick={() => onJump(node.ref.sha)}
        oncontextmenu={(e) => onContext(e, node.ref, kind)}
        title={node.ref.name}
      >
        <span class="dot" style={`background:${colorOf(node.ref)}`} aria-hidden="true"></span>
        <span class="rn">{node.name}</span>
      </button>
    {/if}
  {/each}
{/snippet}

{@render tree(nodes, 0)}

<style>
  .folder,
  .ref {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding-top: 5px;
    padding-bottom: 5px;
    padding-right: 8px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .folder:hover,
  .ref:hover {
    background: var(--row-hover);
  }
  .chev {
    display: inline-block;
    transition: transform 0.12s ease;
    font-size: 10px;
    color: var(--text-muted);
    width: 12px;
    text-align: center;
    flex-shrink: 0;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .fn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
    font-weight: 500;
  }
  .ref.head .rn {
    color: var(--accent);
    font-weight: 600;
  }
  .ref.muted .rn {
    color: var(--text-muted);
  }
  .rn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    /* Colour is set inline to the ref's graph lane colour (or manual override). */
  }
</style>
