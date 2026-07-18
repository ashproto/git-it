<script lang="ts">
  import { openPath } from "@tauri-apps/plugin-opener";
  import type { Ref, WorktreeInfo } from "../types";
  import {
    worktreeRemovalBlocker,
    worktreeStatusDetail,
    worktreeStatusLabel,
    worktreeTrackingDetail,
  } from "../worktrees";
  import { appState } from "../store.svelte";
  import { contextMenu, type MenuItem } from "../contextMenu.svelte";
  import { dialogs } from "../dialogs.svelte";
  import { gitActions } from "../gitActions";
  import WorktreeIcon from "./WorktreeIcon.svelte";

  let {
    worktrees,
    refs = [],
    onDeleteBranch,
  }: {
    worktrees: WorktreeInfo[];
    refs?: Ref[];
    onDeleteBranch?: (branch: string) => void;
  } = $props();

  const ordered = $derived(
    worktrees.filter((worktree) => !worktree.isMain).sort((a, b) =>
      Number(b.isCurrent) - Number(a.isCurrent) ||
      (a.branch ?? a.path).localeCompare(b.branch ?? b.path),
    ),
  );

  function label(worktree: WorktreeInfo): string {
    if (worktree.branch) return worktree.branch;
    if (worktree.detached) return `Detached @ ${worktree.head?.slice(0, 7) ?? "unknown"}`;
    if (worktree.bare) return "Bare repository";
    return "Unborn worktree";
  }

  function rowState(worktree: WorktreeInfo): string | null {
    const status = worktreeStatusLabel(worktree);
    const tracking = worktreeTrackingDetail(worktree, refs);
    const compactTracking = tracking?.match(/^([↑↓]\d+(?: to (?:push|pull))?)(?: · ([↑↓]\d+).*)?$/);
    const pushPull = compactTracking
      ? [compactTracking[1]?.split(" ")[0], compactTracking[2]].filter(Boolean).join(" ")
      : null;
    return [status, pushPull].filter(Boolean).join(" · ") || null;
  }

  function openInGitIt(worktree: WorktreeInfo, changes = false) {
    appState.openRepo(worktree.path);
    if (changes) appState.setActiveView("changes");
  }

  async function confirmRemove(worktree: WorktreeInfo) {
    const branch = worktree.branch ? ` The branch “${worktree.branch}” will be kept.` : "";
    const confirmed = await dialogs.confirm({
      title: `Remove worktree ${label(worktree)}?`,
      message: `Remove the linked worktree at ${worktree.path}? Its folder, including ignored files, will be deleted.${branch}`,
      confirmLabel: "Remove Worktree",
      danger: true,
    });
    if (confirmed) await gitActions.removeWorktree(worktree.path);
  }

  function openContextMenu(event: MouseEvent, worktree: WorktreeInfo) {
    event.preventDefault();
    event.stopPropagation();
    const blocker = worktreeRemovalBlocker(worktree);
    const tracking = worktreeTrackingDetail(worktree, refs);
    const items: MenuItem[] = [
      { label: label(worktree), detail: true },
      { label: worktreeStatusDetail(worktree), detail: true },
    ];
    if (tracking) items.push({ label: tracking, detail: true });
    items.push(
      { separator: true },
      { label: "Open Worktree in Git It", action: () => openInGitIt(worktree) },
      { label: "View Local Changes", action: () => openInGitIt(worktree, true) },
      { label: "Open Worktree Folder", action: () => void openPath(worktree.path) },
      { separator: true },
    );
    if (blocker) items.push({ label: blocker, detail: true });
    items.push({
      label: "Remove Worktree…",
      danger: true,
      disabled: !!blocker,
      action: () => void confirmRemove(worktree),
    });
    if (worktree.branch && onDeleteBranch) {
      items.push({
        label: "Remove Worktree & Delete Branch…",
        danger: true,
        disabled: !!blocker,
        action: () => onDeleteBranch(worktree.branch!),
      });
    }
    contextMenu.openAt(event.clientX, event.clientY, items);
  }
</script>

<div class="worktrees" role="list" aria-label="Repository worktrees">
  {#each ordered as worktree (worktree.path)}
    <div
      class="worktree"
      class:current={worktree.isCurrent}
      role="listitem"
      title={`${label(worktree)} — ${worktree.path}\n${worktreeStatusDetail(worktree)}${worktreeTrackingDetail(worktree, refs) ? `\n${worktreeTrackingDetail(worktree, refs)}` : ""}`}
      oncontextmenu={(event) => openContextMenu(event, worktree)}
    >
      <span class="icon" aria-hidden="true"><WorktreeIcon size={14} /></span>
      <span class="identity">
        <span class="branch">{label(worktree)}</span>
        <span class="path">{worktree.path}</span>
      </span>
      {#if rowState(worktree)}
        <span class="state">{rowState(worktree)}</span>
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
