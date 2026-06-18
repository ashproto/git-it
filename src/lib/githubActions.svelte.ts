import { appState } from "./store.svelte";
import { api } from "./api";
import { githubState } from "./githubState.svelte";
import type { GithubError, MergeMethod } from "./types";

export type PendingAction =
  | { kind: "comment"; target: "pr" | "issue"; number: number; title: string }
  | { kind: "merge"; number: number; title: string }
  | { kind: "setState"; number: number; title: string; to: "open" | "closed" }
  | { kind: "create" }
  | null;

function errText(e: unknown): string {
  const g = e as GithubError | undefined;
  if (g && typeof g === "object" && "kind" in g) {
    return g.kind === "Other" ? (g as { message: string }).message : g.kind;
  }
  return String(e);
}

function makeGithubActions() {
  let pending = $state<PendingAction>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  function open(a: Exclude<PendingAction, null>) {
    pending = a;
    busy = false;
    error = null;
  }
  function cancel() {
    if (busy) return; // don't close mid-request
    pending = null;
    error = null;
  }

  async function submit(fields: { body?: string; title?: string; method?: MergeMethod }) {
    const a = pending;
    const repo = appState.repo;
    if (!a || !repo) return;
    busy = true;
    error = null;
    try {
      if (a.kind === "comment" && a.target === "pr") {
        await api.githubPrComment(repo, a.number, fields.body ?? "");
      } else if (a.kind === "comment") {
        await api.githubIssueComment(repo, a.number, fields.body ?? "");
      } else if (a.kind === "merge") {
        await api.githubPrMerge(repo, a.number, fields.method ?? "squash");
      } else if (a.kind === "setState") {
        await api.githubIssueSetState(repo, a.number, a.to);
      } else if (a.kind === "create") {
        await api.githubIssueCreate(repo, fields.title ?? "", fields.body ?? "");
      }
      pending = null;
      githubState.bumpReload(); // refresh the affected list
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }

  return {
    get pending() {
      return pending;
    },
    get busy() {
      return busy;
    },
    get error() {
      return error;
    },
    open,
    cancel,
    submit,
  };
}

export const githubActions = makeGithubActions();
