import { invoke, Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BundleInfo,
  Commit,
  DateMapping,
  PrerequisiteCheck,
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
