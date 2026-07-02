import { appState } from "./store.svelte";
import { api } from "./api";
import { githubState } from "./githubState.svelte";
import { reviewDraft } from "./reviewDraft.svelte";
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

  // Submit a review as ONE atomic POST. Verdict + summary are passed in
  // explicitly by the caller (ReviewBar owns them per-PR) — never read from the
  // global draft, so a draft pending on another PR can't leak its summary here.
  // Same surfacing contract as commentInline: no shared busy/error state — the
  // caller owns its own and displays `error` verbatim.
  async function submitReview(
    number: number,
    verdict: string,
    summary: string,
  ): Promise<{ ok: boolean; error?: string }> {
    const repo = appState.repo;
    if (!repo) return { ok: false, error: "No repository open." };
    // Inline comments only ride along when the draft is bound to THIS PR — a
    // draft pending on another PR must never leak its comments into this review
    // (and must survive a summary-only submit here).
    const own = reviewDraft.belongsTo(repo, number);
    const comments = own ? [...reviewDraft.comments] : [];
    try {
      await api.githubPrSubmitReview(repo, number, verdict, summary, comments);
      // Only THIS PR's draft is consumed; a foreign draft stays untouched.
      // (A failure keeps the draft intact for retry.) An UNBOUND store (no
      // comments anywhere) is also cleared so a submitted summary-only review
      // can't re-seed another PR's bar.
      if (own || !reviewDraft.bound) reviewDraft.discard();
      githubState.bumpReload(); // re-fetch the detail/timeline so the review shows
      return { ok: true };
    } catch (e) {
      return { ok: false, error: errText(e) };
    }
  }

  // Reply to an inline review thread (targets the thread's first comment via
  // the REST replies endpoint). Same surfacing contract as commentInline: no
  // shared busy/error state — the caller owns its own; on success the reload
  // nonce bump re-fetches the detail so the new reply lands in the thread.
  async function replyThread(
    prNumber: number,
    commentId: number,
    body: string,
  ): Promise<{ ok: boolean; error?: string }> {
    const repo = appState.repo;
    if (!repo || !body.trim()) return { ok: false, error: "Empty comment." };
    try {
      await api.githubPrReplyThread(repo, prNumber, commentId, body);
      githubState.bumpReload();
      return { ok: true };
    } catch (e) {
      return { ok: false, error: errText(e) };
    }
  }

  // Resolve or unresolve an inline review thread (GraphQL mutation). Same
  // contract as replyThread; the reload re-fetch flips the badge.
  async function resolveThread(
    threadId: string,
    resolve: boolean,
  ): Promise<{ ok: boolean; error?: string }> {
    const repo = appState.repo;
    if (!repo) return { ok: false, error: "No repository open." };
    try {
      await api.githubPrResolveThread(repo, threadId, resolve);
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
    submitReview,
    replyThread,
    resolveThread,
  };
}

export const githubActions = makeGithubActions();
