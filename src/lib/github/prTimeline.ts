import type { GhPullDetail, PrTimelineEvent, GhReviewThread } from "../types";

/** Flatten a PR detail into a single chronological list of events (commits,
 * conversation comments, reviews with their inline threads, and CI runs),
 * sorted ascending by ISO timestamp. Review threads are attached to the
 * earliest review submitted at or after the thread's first comment; a thread
 * with no top-level review to attach to is surfaced as its own `reviewThread`
 * event so its inline comments are never dropped. */
export function buildPrTimeline(d: GhPullDetail): PrTimelineEvent[] {
  const events: PrTimelineEvent[] = [];
  for (const commit of d.commits) events.push({ kind: "commit", at: commit.committedDate, commit });
  for (const comment of d.comments) events.push({ kind: "comment", at: comment.createdAt, comment });
  for (const run of d.checkRuns) events.push({ kind: "ciRun", at: run.startedAt, run });

  const reviewsSorted = [...d.reviews].sort((a, b) => a.submittedAt.localeCompare(b.submittedAt));
  const threadsFor = new Map<number, GhReviewThread[]>();
  const threadTime = (t: GhReviewThread) => t.comments[0]?.createdAt ?? "";
  for (const t of d.reviewThreads) {
    // With no top-level reviews to attach to, surface each thread as its own
    // event — otherwise its inline comments would be silently dropped.
    if (reviewsSorted.length === 0) {
      events.push({ kind: "reviewThread", at: threadTime(t), thread: t });
      continue;
    }
    const tt = threadTime(t);
    let idx = reviewsSorted.length - 1;
    for (let i = 0; i < reviewsSorted.length; i++) {
      if (reviewsSorted[i].submittedAt >= tt) { idx = i; break; }
    }
    if (idx < 0) idx = 0;
    const list = threadsFor.get(idx) ?? [];
    list.push(t);
    threadsFor.set(idx, list);
  }
  reviewsSorted.forEach((review, i) => {
    events.push({ kind: "review", at: review.submittedAt, review, threads: threadsFor.get(i) ?? [] });
  });
  return events.sort((a, b) => a.at.localeCompare(b.at));
}
