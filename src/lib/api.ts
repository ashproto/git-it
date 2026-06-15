import { invoke, Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BundleInfo,
  Commit,
  ConflictEntry,
  DateMapping,
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
  isGitRepo: (repo: string) => invoke<boolean>("is_git_repo", { repo }),
  loadCommits: (repo: string, count: number, range?: string) =>
    invoke<Commit[]>("load_commits", { repo, count, range: range ?? null }),
  loadGraph: (repo: string, count: number, skip = 0) =>
    invoke<GraphCommit[]>("load_graph", { repo, count, skip }),
  listRefs: (repo: string) => invoke<Ref[]>("list_refs", { repo }),
  repoStatus: (repo: string) => invoke<RepoStatus>("repo_status", { repo }),
  checkout: (repo: string, target: string) => invoke<string>("checkout", { repo, target }),
  createBranch: (repo: string, name: string, startPoint: string) =>
    invoke<void>("create_branch", { repo, name, startPoint }),
  renameBranch: (repo: string, oldName: string, newName: string) =>
    invoke<void>("rename_branch", { repo, old: oldName, new: newName }),
  deleteBranch: (repo: string, name: string, force: boolean) =>
    invoke<void>("delete_branch", { repo, name, force }),
  createTag: (repo: string, name: string, target: string, message?: string) =>
    invoke<void>("create_tag", { repo, name, target, message: message ?? null }),
  deleteTag: (repo: string, name: string) => invoke<void>("delete_tag", { repo, name }),
  fetch: (repo: string, remote?: string) =>
    invoke<string>("fetch", { repo, remote: remote ?? null }),
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
  diff: (repo: string, path: string | null, staged: boolean) =>
    invoke<string>("diff", { repo, path, staged }),
  commitDiff: (repo: string, sha: string, path: string | null) =>
    invoke<string>("commit_diff", { repo, sha, path }),
  stageHunk: (repo: string, path: string, hunkIndex: number) =>
    invoke<void>("stage_hunk", { repo, path, hunkIndex }),
  unstageHunk: (repo: string, path: string, hunkIndex: number) =>
    invoke<void>("unstage_hunk", { repo, path, hunkIndex }),
  stashPush: (repo: string, message: string | null) =>
    invoke<void>("stash_push", { repo, message }),
  stashList: (repo: string) => invoke<StashEntry[]>("stash_list", { repo }),
  stashApply: (repo: string, index: number) => invoke<void>("stash_apply", { repo, index }),
  stashPop: (repo: string, index: number) => invoke<void>("stash_pop", { repo, index }),
  stashDrop: (repo: string, index: number) => invoke<void>("stash_drop", { repo, index }),
  remotes: (repo: string) => invoke<RemoteInfo[]>("remotes", { repo }),
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
};
