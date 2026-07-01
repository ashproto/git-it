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

  // Post a comment inline (no modal) — used by the composer at the bottom of a
  // PR/issue detail. Manages no shared busy/error state (the caller owns its own);
  // on success it bumps the reload nonce so the open detail re-fetches and the new
  // comment lands in the timeline.
  async function commentInline(
    target: "pr" | "issue",
    number: number,
    body: string,
  ): Promise<{ ok: boolean; error?: string }> {
    const repo = appState.repo;
    if (!repo || !body.trim()) return { ok: false, error: "Empty comment." };
    try {
      if (target === "pr") await api.githubPrComment(repo, number, body);
      else await api.githubIssueComment(repo, number, body);
      githubState.bumpReload();
      return { ok: true };
    } catch (e) {
      return { ok: false, error: errText(e) };
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
    commentInline,
  };
}

export const githubActions = makeGithubActions();
