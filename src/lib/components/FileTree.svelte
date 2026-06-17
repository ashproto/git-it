<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
  import type { TransitionConfig } from "svelte/transition";
  import { flattenFileTree, type FileTreeNode, type FlatFileRow } from "../fileTree";

  // A crossfade send/receive transition (from svelte's crossfade()). Passing it in lets
  // a file leaf FLY between two FileTree instances (e.g. Unstaged → Staged) — the shared
  // crossfade pairs send & receive by key across both trees.
  type CrossfadeFn = (
    node: Element,
    params: { key: unknown },
  ) => TransitionConfig | (() => TransitionConfig);

  // Presentational folder tree, rendered as a SINGLE flat keyed list of rows (folders +
  // file leaves) so `animate:flip` can animate the WHOLE tree: when a file is staged out
  // of a folder, the surviving rows flip into their new positions and any emptied folder
  // slides away — instead of the structure holding its space and popping at the end.
  // Each file leaf's interactive markup is supplied by the caller's `fileRow` snippet
  // (FileTree owns only the <li> wrapper that carries the flip + fly/slide transitions).
  let {
    nodes,
    fileRow,
    // animate: enable the move animation (Local Changes). Off (default) → an instant
    // static tree (the commit file list, which shouldn't churn when you switch commits).
    animate = false,
    // Shared crossfade pair so a file flies between sections. Omitted by the commit file
    // list (no cross-section movement there).
    send,
    receive,
  }: {
    nodes: FileTreeNode<any>[];
    fileRow: Snippet<[any, number]>; // (item, indentPx) → renders one file leaf's inner content
    animate?: boolean;
    send?: CrossfadeFn;
    receive?: CrossfadeFn;
  } = $props();

  // Suppress the folder slide-INTRO on the first render (opening the view / first data)
  // so the whole tree doesn't slide in at once — only folders that appear LATER (a
  // stage/unstage move) slide in. flip never runs on first render (nothing repositions),
  // and a file leaf with no crossfade partner falls back to instant, so only the folder
  // intro needs gating. The out-transition is never gated (removal only happens later).
  let mounted = $state(false);
  onMount(() => {
    mounted = true;
  });

  // Collapsed folder paths (in-memory; folders default expanded).
  let collapsed = $state<Set<string>>(new Set());
  function toggle(path: string) {
    const next = new Set(collapsed);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    collapsed = next;
  }
  const indent = (depth: number) => 8 + depth * 14;

  // The whole tree as one ordered, keyed list of rows (collapsed folders hide children).
  const rows = $derived(flattenFileTree(nodes, collapsed));

  const FLIP = $derived(animate ? { duration: 220, easing: quintOut } : { duration: 0 });
  const inDur = $derived(animate && mounted ? 200 : 0);
  const outDur = $derived(animate ? 200 : 0);

  // Per-row enter/leave dispatcher. Files use the shared crossfade (fly between sections)
  // when one is provided; folders animate their height (slide) so an emptied folder
  // collapses and a new folder expands. Wrapping the crossfade fn is safe: we return its
  // (possibly deferred) result unchanged, so its cross-tree pairing still works.
  function rowIn(node: Element, row: FlatFileRow<any>) {
    if (row.kind === "folder") return slide(node, { duration: inDur, easing: quintOut });
    if (animate && receive) return receive(node, { key: row.path });
    return { duration: 0 };
  }
  function rowOut(node: Element, row: FlatFileRow<any>) {
    if (row.kind === "folder") return slide(node, { duration: outDur, easing: quintOut });
    if (animate && send) return send(node, { key: row.path });
    return { duration: 0 };
  }
</script>

{#each rows as row (row.kind === "folder" ? "d:" + row.path : "f:" + row.path)}
  <li class="ft-row" animate:flip={FLIP} in:rowIn={row} out:rowOut={row}>
    {#if row.kind === "folder"}
      <button
        type="button"
        class="folder"
        style={`padding-left:${indent(row.depth)}px`}
        onclick={() => toggle(row.path)}
        aria-expanded={!collapsed.has(row.path)}
        title={row.path}
      >
        <span class="chev" class:open={!collapsed.has(row.path)} aria-hidden="true">▶</span>
        <span class="fn">{row.name}</span>
      </button>
    {:else}
      {@render fileRow(row.item, indent(row.depth + 1))}
    {/if}
  </li>
{/each}

<style>
  /* Wrapper <li> for every row; carries the flip + enter/leave transitions. Visuals live
     on the inner content (the folder <button> below, or the caller's file row). */
  .ft-row {
    list-style: none;
    display: block;
  }
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
