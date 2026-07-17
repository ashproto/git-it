import { invoke, Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BundleInfo,
  Commit,
  ConflictEntry,
  DateMapping,
  GhAvailability,
  GhRepoStats,
  GhPull,
  GhIssue,
  GhRelease,
  GhRun,
  GhTraffic,
  GhContributor,
  GhActivity,
  GhMilestone,
  GhLabel,
  MergeMethod,
  PullStateFilter,
  IssueStateFilter,
  GhPullDetail,
  GhIssueDetail,
  GhCommentKind,
  DraftComment,
  GraphCommit,
  OpOutcome,
  PrerequisiteCheck,
  RebaseOutcome,
  RebaseStep,
  Ref,
  ReflogEntry,
  RemoteInfo,
  RemoteOutcome,
  RepoStatus,
  RewriteOptions,
  RewriteResult,
  SafetyRef,
  StashEntry,
  WorkingFile,
  UpdateInfo,
  DownloadEvent,
  WorktreeInfo,
} from "./types";

export async function pickRepoFolder(initial?: string): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    defaultPath: initial,
    title: "Select git repository root",
  });
  return typeof result === "string" ? result : null;
}

export const api = {
  checkPrerequisites: () => invoke<PrerequisiteCheck>("check_prerequisites"),
  installCommandLineTools: () => invoke<string>("install_command_line_tools"),
  isGitRepo: (repo: string) => invoke<boolean>("is_git_repo", { repo }),
  // ── auto-updater (desktop only) ──────────────────────────────────────────
  checkUpdateOnChannel: (channel: "stable" | "beta") =>
    invoke<UpdateInfo | null>("check_update_on_channel", { channel }),
  installPendingUpdate: (onEvent: (e: DownloadEvent) => void) => {
    const ch = new Channel<DownloadEvent>();
    ch.onmessage = onEvent;
    return invoke<void>("install_pending_update", { onEvent: ch });
  },
  loadCommits: (repo: string, count: number, range?: string) =>
    invoke<Commit[]>("load_commits", { repo, count, range: range ?? null }),
  loadGraph: (repo: string, count: number, skip = 0) =>
    invoke<GraphCommit[]>("load_graph", { repo, count, skip }),
  listRefs: (repo: string) => invoke<Ref[]>("list_refs", { repo }),
  listWorktrees: (repo: string) => invoke<WorktreeInfo[]>("list_worktrees", { repo }),
  repoStatus: (repo: string) => invoke<RepoStatus>("repo_status", { repo }),
  checkout: (repo: string, target: string) => invoke<string>("checkout", { repo, target }),
  createBranch: (repo: string, name: string, startPoint: string) =>
    invoke<void>("create_branch", { repo, name, startPoint }),
  renameBranch: (repo: string, oldName: string, newName: string) =>
    invoke<void>("rename_branch", { repo, old: oldName, new: newName }),
  deleteBranch: (
    repo: string,
    name: string,
    force: boolean,
    deleteRemote?: boolean,
    remote?: string,
    remoteBranch?: string,
    worktreePath?: string,
  ) =>
    invoke<void>("delete_branch", {
      repo, name, force,
      deleteRemote: deleteRemote ?? false,
      remote: remote ?? null,
      remoteBranch: remoteBranch ?? null,
      worktreePath: worktreePath ?? null,
    }),
  createTag: (repo: string, name: string, target: string, message?: string) =>
    invoke<void>("create_tag", { repo, name, target, message: message ?? null }),
  deleteTag: (repo: string, name: string) => invoke<void>("delete_tag", { repo, name }),
  fetch: (repo: string, remote?: string) =>
    invoke<string>("fetch", { repo, remote: remote ?? null }),
  fastForwardBranch: (repo: string, branch: string, remote: string, remoteBranch: string) =>
    invoke<string>("fast_forward_branch", { repo, branch, remote, remoteBranch }),
  appsForFile: (path: string) =>
    invoke<{ name: string; path: string }[]>("apps_for_file", { path }),
  deleteRemoteBranch: (repo: string, remote: string, branch: string) =>
    invoke<string>("delete_remote_branch", { repo, remote, branch }),
  merge: (repo: string, reference: string, noFf = false, squash = false) =>
    invoke<OpOutcome>("merge", { repo, reference, noFf, squash }),
  cherryPick: (repo: string, shas: string[]) =>
    invoke<OpOutcome>("cherry_pick", { repo, shas }),
  revert: (repo: string, shas: string[]) => invoke<OpOutcome>("revert", { repo, shas }),
  opAbort: (repo: string, kind: string) => invoke<void>("op_abort", { repo, kind }),
  opContinue: (repo: string, kind: string) => invoke<OpOutcome>("op_continue", { repo, kind }),
  resolveConflict: (repo: string, path: string, ours: boolean) =>
    invoke<void>("resolve_conflict", { repo, path, ours }),
  conflictedFiles: (repo: string) => invoke<string[]>("conflicted_files", { repo }),
  conflictDetails: (repo: string) =>
    invoke<ConflictEntry[]>("conflict_details", { repo }),
  resolveKeep: (repo: string, path: string) =>
    invoke<void>("resolve_keep", { repo, path }),
  resolveRemove: (repo: string, path: string) =>
    invoke<void>("resolve_remove", { repo, path }),
  opSkip: (repo: string, kind: string) => invoke<OpOutcome>("op_skip", { repo, kind }),
  createBundle: (repo: string) => invoke<string>("create_bundle", { repo }),
  listBundles: (repo: string) => invoke<BundleInfo[]>("list_bundles", { repo }),
  deleteBundle: (path: string) => invoke<void>("delete_bundle", { path }),
  fetchBundle: (repo: string, bundlePath: string) =>
    invoke<string>("fetch_bundle", { repo, bundlePath }),
  listSafetyRefs: (repo: string) => invoke<SafetyRef[]>("list_safety_refs", { repo }),
  deleteRefs: (repo: string, refs: string[]) =>
    invoke<void>("delete_refs", { repo, refs }),
  previewCallback: (mappings: DateMapping[], updateAuthor: boolean) =>
    invoke<string>("preview_callback", { mappings, updateAuthor }),
  rewriteHistory: (
    repo: string,
    mappings: DateMapping[],
    options: RewriteOptions,
    onEvent: (line: string) => void,
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onEvent;
    return invoke<void>("rewrite_history", {
      repo,
      mappings,
      options,
      onEvent: channel,
    });
  },
  reset: (repo: string, target: string, mode: "soft" | "mixed" | "hard", autoBackup: boolean) =>
    invoke<RewriteResult>("reset", { repo, target, mode, autoBackup }),
  amend: (repo: string, message: string | null, resetAuthorDate: boolean, resetCommitterDate: boolean, autoBackup: boolean) =>
    invoke<RewriteResult>("amend", { repo, message, resetAuthorDate, resetCommitterDate, autoBackup }),
  undoOp: (repo: string, sha: string) => invoke<void>("undo_op", { repo, sha }),
  rebase: (repo: string, onto: string, autoBackup: boolean) =>
    invoke<RebaseOutcome>("rebase", { repo, onto, autoBackup }),
  rebaseTodoPreview: (repo: string, base: string) =>
    invoke<ReflogEntry[]>("rebase_todo_preview", { repo, base }),
  commitMessage: (repo: string, sha: string) =>
    invoke<string>("commit_message", { repo, sha }),
  countMergesInRange: (repo: string, base: string) =>
    invoke<number>("count_merges_in_range", { repo, base }),
  rebaseInteractive: (repo: string, base: string, steps: RebaseStep[], autoBackup: boolean) =>
    invoke<RebaseOutcome>("rebase_interactive", { repo, base, steps, autoBackup }),
  reflog: (repo: string, limit = 50) => invoke<ReflogEntry[]>("reflog", { repo, limit }),
  workingChanges: (repo: string) => invoke<WorkingFile[]>("working_changes", { repo }),
  stage: (repo: string, paths: string[]) => invoke<void>("stage", { repo, paths }),
  unstage: (repo: string, paths: string[]) => invoke<void>("unstage", { repo, paths }),
  discard: (repo: string, paths: string[]) => invoke<void>("discard", { repo, paths }),
  clean: (repo: string, paths: string[]) => invoke<void>("clean", { repo, paths }),
  commit: (repo: string, message: string, signoff = false) =>
    invoke<void>("commit", { repo, message, signoff }),
  diff: (repo: string, path: string | null, staged: boolean, untracked = false, context = 3) =>
    invoke<string>("diff", { repo, path, staged, untracked, context }),
  commitDiff: (repo: string, sha: string, path: string | null, context = 3) =>
    invoke<string>("commit_diff", { repo, sha, path, context }),
  stageHunk: (repo: string, path: string, hunkIndex: number, context = 3) =>
    invoke<void>("stage_hunk", { repo, path, hunkIndex, context }),
  unstageHunk: (repo: string, path: string, hunkIndex: number, context = 3) =>
    invoke<void>("unstage_hunk", { repo, path, hunkIndex, context }),
  stageLines: (repo: string, path: string, hunkIndex: number, selected: number[], context = 3) =>
    invoke<void>("stage_lines", { repo, path, hunkIndex, selected, context }),
  unstageLines: (repo: string, path: string, hunkIndex: number, selected: number[], context = 3) =>
    invoke<void>("unstage_lines", { repo, path, hunkIndex, selected, context }),
  stashPush: (repo: string, message: string | null) =>
    invoke<void>("stash_push", { repo, message }),
  stashList: (repo: string) => invoke<StashEntry[]>("stash_list", { repo }),
  stashApply: (repo: string, index: number) => invoke<void>("stash_apply", { repo, index }),
  stashPop: (repo: string, index: number) => invoke<void>("stash_pop", { repo, index }),
  stashDrop: (repo: string, index: number) => invoke<void>("stash_drop", { repo, index }),
  remotes: (repo: string) => invoke<RemoteInfo[]>("remotes", { repo }),
  githubAvailability: (repo: string) => invoke<GhAvailability>("github_availability", { repo }),
  githubRepoStats: (repo: string) => invoke<GhRepoStats>("github_repo_stats", { repo }),
  githubPulls: (repo: string, state: PullStateFilter, limit: number) =>
    invoke<GhPull[]>("github_pulls", { repo, state, limit }),
  githubIssues: (repo: string, state: IssueStateFilter, limit: number) =>
    invoke<GhIssue[]>("github_issues", { repo, state, limit }),
  githubReleases: (repo: string) => invoke<GhRelease[]>("github_releases", { repo }),
  githubRuns: (repo: string, limit: number) => invoke<GhRun[]>("github_runs", { repo, limit }),
  githubTraffic: (repo: string) => invoke<GhTraffic>("github_traffic", { repo }),
  githubContributors: (repo: string, limit: number) =>
    invoke<GhContributor[]>("github_contributors", { repo, limit }),
  githubActivity: (repo: string) => invoke<GhActivity>("github_activity", { repo }),
  githubMilestones: (repo: string) => invoke<GhMilestone[]>("github_milestones", { repo }),
  githubLabels: (repo: string) => invoke<GhLabel[]>("github_labels", { repo }),
  githubPrComment: (repo: string, number: number, body: string) =>
    invoke<void>("github_pr_comment", { repo, number, body }),
  githubIssueComment: (repo: string, number: number, body: string) =>
    invoke<void>("github_issue_comment", { repo, number, body }),
  githubIssueSetState: (repo: string, number: number, state: "open" | "closed") =>
    invoke<void>("github_issue_set_state", { repo, number, state }),
  githubPrMerge: (repo: string, number: number, method: MergeMethod) =>
    invoke<void>("github_pr_merge", { repo, number, method }),
  githubIssueCreate: (repo: string, title: string, body: string) =>
    invoke<string>("github_issue_create", { repo, title, body }),
  githubPrDetail: (repo: string, number: number) =>
    invoke<GhPullDetail>("github_pr_detail", { repo, number }),
  githubPrDiff: (repo: string, number: number) =>
    invoke<string>("github_pr_diff", { repo, number }),
  githubPrSubmitReview: (repo: string, number: number, event: string, body: string, comments: DraftComment[]) =>
    invoke<void>("github_pr_submit_review", { repo, number, event, body, comments }),
  githubPrReplyThread: (repo: string, prNumber: number, commentId: number, body: string) =>
    invoke<void>("github_pr_reply_thread", { repo, prNumber, commentId, body }),
  githubPrResolveThread: (repo: string, threadId: string, resolve: boolean) =>
    invoke<void>("github_pr_resolve_thread", { repo, threadId, resolve }),
  githubCurrentLogin: () => invoke<string>("github_current_login"),
  githubToggleReaction: (repo: string, kind: GhCommentKind, target: number, content: string) =>
    invoke<boolean>("github_toggle_reaction", { repo, kind, target, content }),
  githubEditComment: (repo: string, kind: GhCommentKind, target: number, body: string) =>
    invoke<void>("github_edit_comment", { repo, kind, target, body }),
  githubDeleteComment: (repo: string, kind: GhCommentKind, target: number) =>
    invoke<void>("github_delete_comment", { repo, kind, target }),
  githubIssueDetail: (repo: string, number: number) =>
    invoke<GhIssueDetail>("github_issue_detail", { repo, number }),
  githubReadme: (repo: string) => invoke<string>("github_readme", { repo }),
  githubCreateRepo: (repo: string, name: string, isPrivate: boolean, description: string) =>
    invoke<string>("github_create_repo", { repo, name, private: isPrivate, description }),
  githubPrCreate: (repo: string, title: string, body: string, base: string, draft: boolean) =>
    invoke<number>("github_pr_create", { repo, title, body, base, draft }),
  githubPrCheckout: (repo: string, number: number) =>
    invoke<void>("github_pr_checkout", { repo, number }),
  branchSubjects: (repo: string, base: string, limit: number) =>
    invoke<string[]>("branch_subjects", { repo, base, limit }),
  remoteAdd: (repo: string, name: string, url: string) =>
    invoke<void>("remote_add", { repo, name, url }),
  remoteRemove: (repo: string, name: string) => invoke<void>("remote_remove", { repo, name }),
  remoteSetUrl: (repo: string, name: string, url: string) =>
    invoke<void>("remote_set_url", { repo, name, url }),
  pull: (
    repo: string,
    rebase: boolean,
    onEvent: (line: string) => void,
    creds?: { username: string; password: string },
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onEvent;
    return invoke<RemoteOutcome>("pull", {
      repo,
      rebase,
      username: creds?.username ?? null,
      password: creds?.password ?? null,
      onEvent: channel,
    });
  },
  push: (
    repo: string,
    remote: string,
    refspec: string | null,
    forceWithLease: boolean,
    setUpstream: boolean,
    onEvent: (line: string) => void,
    creds?: { username: string; password: string },
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onEvent;
    return invoke<RemoteOutcome>("push", {
      repo,
      remote,
      refspec,
      forceWithLease,
      setUpstream,
      username: creds?.username ?? null,
      password: creds?.password ?? null,
      onEvent: channel,
    });
  },
  cancelRemote: () => invoke<void>("cancel_remote", {}),

  // ── Filesystem watcher (live Local Changes) ────────────────────────────────
  // Start watching `repo`'s worktree; `onChange` fires (debounced in Rust) when a
  // non-.git path changes. Replaces any existing watcher.
  startWatch: (repo: string, onChange: (kind: string) => void) => {
    const channel = new Channel<string>();
    channel.onmessage = (msg) => onChange(msg);
    return invoke<void>("start_watch", { repo, onChange: channel });
  },
  stopWatch: () => invoke<void>("stop_watch", {}),
};
