import { invoke, Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BundleInfo,
  Commit,
  DateMapping,
  GraphCommit,
  OpOutcome,
  PrerequisiteCheck,
  Ref,
  RepoStatus,
  RewriteOptions,
  SafetyRef,
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
};
