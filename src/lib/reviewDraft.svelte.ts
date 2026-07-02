import type { DraftComment } from "./types";

export type ReviewVerdict = "COMMENT" | "APPROVE" | "REQUEST_CHANGES";

// Pending PR-review draft: inline comments accumulated from the Files tab plus
// the summary/verdict picked in the ReviewBar, submitted as ONE atomic review
// via github_pr_submit_review. A draft is bound to a single (repo, prNumber)
// from the first addComment until discard() — removing the last comment keeps
// the binding (and the verdict/summary) so an in-progress review survives; only
// discard() (user action or post-submit) resets everything.
function makeReviewDraft() {
  let repo = $state<string | null>(null);
  let prNumber = $state<number | null>(null);
  let comments = $state<DraftComment[]>([]);
  let summary = $state("");
  let verdict = $state<ReviewVerdict>("COMMENT");

  return {
    get count() {
      return comments.length;
    },
    get comments() {
      return comments;
    },
    get summary() {
      return summary;
    },
    get verdict() {
      return verdict;
    },
    /** True while the draft is bound to some (repo, PR) — from the first
     *  addComment until discard(), surviving removal of the last comment. */
    get bound(): boolean {
      return repo !== null;
    },
    belongsTo(r: string, n: number): boolean {
      return repo === r && prNumber === n;
    },
    addComment(r: string, n: number, c: DraftComment) {
      if (repo !== null && prNumber !== null && (repo !== r || prNumber !== n)) {
        throw new Error("draft-belongs-to-other-pr");
      }
      repo = r;
      prNumber = n;
      comments = [...comments, c];
    },
    updateComment(index: number, body: string) {
      if (index < 0 || index >= comments.length) return;
      comments = comments.map((c, i) => (i === index ? { ...c, body } : c));
    },
    removeComment(index: number) {
      if (index < 0 || index >= comments.length) return;
      comments = comments.filter((_, i) => i !== index);
      // Keep the (repo, prNumber) binding + verdict/summary — only discard() resets.
    },
    commentAt(path: string, line: number, side: "LEFT" | "RIGHT"): DraftComment | undefined {
      return comments.find((c) => c.path === path && c.line === line && c.side === side);
    },
    /** Index of the draft comment at (path, line, side), or -1. For edit/remove. */
    indexAt(path: string, line: number, side: "LEFT" | "RIGHT"): number {
      return comments.findIndex((c) => c.path === path && c.line === line && c.side === side);
    },
    setSummary(s: string) {
      summary = s;
    },
    setVerdict(v: ReviewVerdict) {
      verdict = v;
    },
    discard() {
      repo = null;
      prNumber = null;
      comments = [];
      summary = "";
      verdict = "COMMENT";
    },
  };
}

export const reviewDraft = makeReviewDraft();
