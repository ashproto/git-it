<script lang="ts">
  import type { WorktreeInfo } from "../types";
  import { worktreeStatusLabel } from "../worktrees";
  import WorktreeIcon from "./WorktreeIcon.svelte";

  let { worktrees }: { worktrees: WorktreeInfo[] } = $props();

  const ordered = $derived(
    [...worktrees].sort((a, b) =>
      Number(b.isCurrent) - Number(a.isCurrent) ||
      Number(b.isMain) - Number(a.isMain) ||
      (a.branch ?? a.path).localeCompare(b.branch ?? b.path),
    ),
  );

  function label(worktree: WorktreeInfo): string {
    if (worktree.branch) return worktree.branch;
    if (worktree.detached) return `Detached @ ${worktree.head?.slice(0, 7) ?? "unknown"}`;
    if (worktree.bare) return "Bare repository";
    return "Unborn worktree";
  }
</script>

<div class="worktrees" role="list" aria-label="Repository worktrees">
  {#each ordered as worktree (worktree.path)}
    <div
      class="worktree"
      class:current={worktree.isCurrent}
      role="listitem"
      title={`${label(worktree)} — ${worktree.path}`}
    >
      <span class="icon" aria-hidden="true"><WorktreeIcon size={14} /></span>
      <span class="identity">
        <span class="branch">{label(worktree)}</span>
        <span class="path">{worktree.path}</span>
      </span>
      {#if worktreeStatusLabel(worktree)}
        <span class="state">{worktreeStatusLabel(worktree)}</span>
      {/if}
    </div>
  {/each}
</div>

<style>
  .worktrees {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .worktree {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    padding: 6px 8px;
    border-radius: var(--radius-md);
    color: var(--text-muted);
  }
  .worktree:hover {
    background: var(--row-hover);
  }
  .worktree.current {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 75%, transparent);
  }
  .icon {
    display: inline-flex;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .current .icon,
  .current .branch {
    color: var(--accent);
  }
  .identity {
    display: flex;
    flex: 1;
    min-width: 0;
    flex-direction: column;
    gap: 1px;
  }
  .branch {
    overflow: hidden;
    color: var(--text);
    font-size: 12px;
    font-weight: 600;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    flex-shrink: 0;
    color: var(--text-muted);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
</style>
