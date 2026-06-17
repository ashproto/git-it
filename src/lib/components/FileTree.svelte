<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import type { FileTreeNode } from "../fileTree";

  // Presentational folder tree. Folders are collapsible <button>s; each file leaf is
  // rendered by the caller-supplied `fileRow` snippet, positioned at the given indent
  // so selection / click / context-menu all live in the caller. Mirrors RefTree.svelte's
  // visual conventions (chevron, indent = 8 + depth*14, in-memory collapsed set).
  let {
    nodes,
    fileRow,
    // animate: slide folders in/out as they appear/empty (Fork-style), so a folder
    // doesn't hard-pop when its last file is staged away. Opt-in — the commit file
    // list leaves it off so switching commits doesn't churn the whole tree.
    animate = false,
  }: {
    nodes: FileTreeNode<any>[];
    fileRow: Snippet<[any, number]>; // (item, indentPx) → renders one file leaf
    animate?: boolean;
  } = $props();

  // Suppress the folder INTRO on the very first render (opening the view / first data)
  // so the whole tree doesn't slide in at once — only folders that appear LATER (a
  // stage/unstage move) slide. Matches how the file rows appear instantly on load and
  // only fly on a genuine move. The out-transition is never gated (removal only fires
  // post-mount), so an emptied folder always collapses smoothly.
  let mounted = $state(false);
  onMount(() => {
    mounted = true;
  });
  // A folder's slide height-animation also reflows the rows below it, so siblings glide
  // up/down to follow — no animate:flip needed (and flip isn't usable here: the button
  // lives inside an {#if}, not as the each block's immediate child).
  const inDur = $derived(animate && mounted ? 200 : 0);
  const outDur = $derived(animate ? 200 : 0);

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

{#snippet tree(items: FileTreeNode<any>[], depth: number)}
  {#each items as node (node.kind === "folder" ? "d:" + node.path : "f:" + node.path)}
    {#if node.kind === "folder"}
      <button
        type="button"
        class="folder"
        style={`padding-left:${indent(depth)}px`}
        in:slide={{ duration: inDur, easing: quintOut }}
        out:slide={{ duration: outDur, easing: quintOut }}
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
      {@render fileRow(node.item, indent(depth + 1))}
    {/if}
  {/each}
{/snippet}

{@render tree(nodes, 0)}

<style>
  .folder {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding-top: 5px;
    padding-bottom: 5px;
    padding-right: 8px;
    background: none;
    border: none;
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .folder:hover {
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
</style>
