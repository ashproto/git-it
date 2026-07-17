import type { WorktreeInfo } from "./types";

export function worktreeForBranch(
  worktrees: readonly WorktreeInfo[],
  branch: string,
): WorktreeInfo | undefined {
  return worktrees.find((worktree) => worktree.branch === branch);
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
    return "Git's main worktree cannot be removed. Check out a different branch there before deleting this branch.";
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
  if (worktree.bare || worktree.detached || worktree.branch === null) {
    return "This worktree no longer has the branch checked out. Refresh the repository and try again.";
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
  if (worktreeIsDirty(worktree)) return "Modified";
  if (worktree.detached) return "Detached";
  if (worktree.bare) return "Bare";
  return null;
}
