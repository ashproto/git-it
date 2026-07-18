import type { Ref, WorktreeInfo } from "./types";

export function linkedWorktrees(worktrees: readonly WorktreeInfo[]): WorktreeInfo[] {
  return worktrees.filter((worktree) => !worktree.isMain);
}

export function worktreeForBranch(
  worktrees: readonly WorktreeInfo[],
  branch: string,
): WorktreeInfo | undefined {
  return worktrees.find((worktree) => worktree.branch === branch);
}

export function linkedWorktreeForBranch(
  worktrees: readonly WorktreeInfo[],
  branch: string,
): WorktreeInfo | undefined {
  return worktrees.find((worktree) => !worktree.isMain && worktree.branch === branch);
}

export function worktreeIsDirty(worktree: WorktreeInfo): boolean {
  const status = worktree.status;
  return !!status && (
    status.staged > 0 ||
    status.unstaged > 0 ||
    status.untracked > 0 ||
    status.conflicted > 0 ||
    status.operation !== null
  );
}

/** A non-null result means the app must not offer removal. The backend repeats
 * every check immediately before executing; this is the explanatory UI gate. */
export function worktreeRemovalBlocker(worktree: WorktreeInfo): string | null {
  if (worktree.isMain) {
    return "The repository's primary checkout cannot be removed as a linked worktree.";
  }
  if (worktree.isCurrent) {
    return "This is the worktree currently open in Git It. Open the repository from another worktree before removing it.";
  }
  if (worktree.locked) {
    return worktree.lockedReason
      ? `This worktree is locked: ${worktree.lockedReason}`
      : "This worktree is locked. Unlock it before removing it.";
  }
  if (worktree.prunable) {
    return worktree.prunableReason
      ? `This worktree is stale or missing: ${worktree.prunableReason}`
      : "This worktree is stale or missing. Prune its Git metadata before deleting the branch.";
  }
  if (worktree.detached) {
    return "This detached worktree cannot be removed here because its commits may not be referenced by a branch.";
  }
  if (worktree.bare || worktree.branch === null) {
    return "This worktree does not have a branch checked out. Refresh the repository and try again.";
  }
  if (worktree.status === null) {
    return "Git It could not verify that this worktree is clean, so it will not remove it.";
  }
  if (worktreeIsDirty(worktree)) {
    return "This worktree has uncommitted changes or an operation in progress. Commit, stash, or discard them before removing it.";
  }
  return null;
}

export function worktreeStatusLabel(worktree: WorktreeInfo): string | null {
  if (worktree.isCurrent) return "Current";
  if (worktree.locked) return "Locked";
  if (worktree.prunable) return "Missing";
  if (worktree.status === null && !worktree.bare) return "Unavailable";
  if (worktree.status?.operation) {
    const operation = worktree.status.operation;
    return operation.charAt(0).toUpperCase() + operation.slice(1);
  }
  if (worktreeIsDirty(worktree)) return "Modified";
  if (worktree.detached) return "Detached";
  if (worktree.bare) return "Bare";
  return null;
}

export function worktreeStatusDetail(worktree: WorktreeInfo): string {
  if (worktree.locked) return worktree.lockedReason
    ? `Locked — ${worktree.lockedReason}`
    : "Locked";
  if (worktree.prunable) return worktree.prunableReason
    ? `Missing — ${worktree.prunableReason}`
    : "Missing from disk";
  if (!worktree.status) return "Local state unavailable";
  const parts: string[] = [];
  if (worktree.status.operation) parts.push(`${worktree.status.operation} in progress`);
  if (worktree.status.staged) parts.push(`${worktree.status.staged} staged`);
  if (worktree.status.unstaged) parts.push(`${worktree.status.unstaged} unstaged`);
  if (worktree.status.untracked) parts.push(`${worktree.status.untracked} untracked`);
  if (worktree.status.conflicted) parts.push(`${worktree.status.conflicted} conflicted`);
  return parts.length ? parts.join(" · ") : "Working tree clean";
}

export function worktreeTrackingDetail(
  worktree: WorktreeInfo,
  refs: readonly Ref[],
): string | null {
  if (!worktree.branch) return null;
  const local = refs.find((ref) => ref.kind === "local" && ref.name === worktree.branch);
  if (!local) return null;
  if (!local.upstream) return "No upstream branch";
  const upstreamExists = refs.some(
    (ref) => ref.kind === "remote" && ref.name === local.upstream,
  );
  if (!upstreamExists) return `Upstream missing — ${local.upstream}`;
  const counts: string[] = [];
  if (local.ahead) counts.push(`↑${local.ahead} to push`);
  if (local.behind) counts.push(`↓${local.behind} to pull`);
  return counts.length ? counts.join(" · ") : `Up to date with ${local.upstream}`;
}
