<script lang="ts">
  import type { RefEntry } from "../types";
  import type { RefTreeNode } from "../refTree";
  import RefIcon from "./RefIcon.svelte";

  let {
    nodes,
    kind,
    onJump,
    onContext,
    colorOf,
    onCheckout,
    selectedKey,
    onSelect,
  }: {
    nodes: RefTreeNode[];
    kind: "local" | "remote" | "tag";
    onJump: (sha: string) => void;
    onContext: (e: MouseEvent, ref: RefEntry, kind: "local" | "remote" | "tag") => void;
    // Resolves a ref's swatch colour — its graph lane colour (or manual override).
    colorOf: (ref: RefEntry) => string;
    onCheckout: (ref: RefEntry, kind: "local" | "remote" | "tag") => void;
    // Composite `${kind}:${name}` key of the currently-selected ref row (across
    // all three trees), or null if none selected yet.
    selectedKey: string | null;
    onSelect: (kind: "local" | "remote" | "tag", name: string) => void;
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
        class:selected={`${kind}:${node.ref.name}` === selectedKey}
        style={`padding-left:${indent(depth) + 12}px`}
        onclick={() => {
          onJump(node.ref.sha);
          onSelect(kind, node.ref.name);
        }}
        ondblclick={() => onCheckout(node.ref, kind)}
        oncontextmenu={(e) => {
          onContext(e, node.ref, kind);
          onSelect(kind, node.ref.name);
        }}
        title={node.ref.name}
      >
        <span class="ref-ic" style={`color:${colorOf(node.ref)}`} aria-hidden="true">
          <RefIcon {kind} size={13} />
        </span>
        <span class="rn">{node.name}</span>
        <!-- Per-branch ahead/behind vs upstream (local branches only), like
             Fork/SourceTree: ↑ commits to push, ↓ commits to pull. -->
        {#if kind === "local" && ((node.ref.ahead ?? 0) > 0 || (node.ref.behind ?? 0) > 0)}
          <span
            class="track"
            aria-label={`${node.ref.ahead ?? 0} ahead, ${node.ref.behind ?? 0} behind upstream`}
          >
            {#if (node.ref.ahead ?? 0) > 0}<span class="ahead">↑{node.ref.ahead}</span>{/if}
            {#if (node.ref.behind ?? 0) > 0}<span class="behind">↓{node.ref.behind}</span>{/if}
          </span>
        {/if}
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
    border-radius: var(--radius-md);
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .folder:hover,
  .ref:hover {
    background: color-mix(in srgb, var(--accent) 10%, var(--row-hover));
  }
  /* Selected ref row: stronger accent tint + a left accent bar, so it reads
     as clearly distinct from the (lighter) hover state. Applies to local,
     remote (.muted) and tag rows alike. */
  .ref.selected {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .ref.selected:hover {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
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
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Ahead/behind counts, right-aligned at the end of the row. */
  .track {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
  }
  .track .ahead {
    color: var(--accent);
  }
  .track .behind {
    color: var(--text-muted);
  }
  .ref-ic {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    /* The glyph (currentColor) is tinted to the ref's graph lane colour (or
       manual override); its shape conveys local / remote / tag. */
  }
</style>
