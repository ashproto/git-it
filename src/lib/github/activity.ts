import type { GhActivity, GithubError } from "../types";

/** Map a numeric series to an SVG polyline `points` string in a `w`×`h` box.
 *  The max value sits at the top (y=0); an all-equal series sits on y=0. */
export function sparklinePoints(values: number[], w: number, h: number): string {
  if (values.length === 0) return "";
  const max = Math.max(1, ...values);
  const dx = values.length > 1 ? w / (values.length - 1) : 0;
  return values
    .map((v, i) => `${(i * dx).toFixed(1)},${(h - (v / max) * h).toFixed(1)}`)
    .join(" ");
}

/** A cold-stats failure is worth retrying; a missing repo / auth / remote problem is not.
 *  GitHub computes /stats/commit_activity lazily (HTTP 202 the first time), so the initial
 *  request can fail transiently — a rate limit, a 202 surfaced as an error, an unparseable
 *  placeholder body. Those clear on their own; NotFound/NotAuthed/NoRemote/NotInstalled won't. */
export function isRetryableActivityError(e: unknown): boolean {
  const kind =
    e && typeof e === "object" && "kind" in e ? (e as GithubError).kind : "Other";
  return (
    kind !== "NotFound" &&
    kind !== "NotAuthed" &&
    kind !== "NoRemote" &&
    kind !== "NotInstalled"
  );
}

/** Fetch commit activity, retrying through GitHub's lazy-computation window.
 *
 *  The first request on a cold cache returns HTTP 202 while GitHub builds the 52-week
 *  series; depending on timing that reaches us as `computing: true`, an unparseable body,
 *  or a transient error. This retries ALL of those a few times with a fixed backoff —
 *  the previous logic retried only the `computing` flag, so a *thrown* cold-cache error
 *  hard-errored the Insights panel until the user manually switched tabs and back (which
 *  forced a re-fetch). A clearly-permanent error aborts early; if the stats are still
 *  computing when attempts run out, we resolve to the `computing` marker so the UI shows
 *  the soft "still computing…" note rather than a hard error.
 *
 *  Dependencies (`fetchOnce`, `sleep`) are injected so the retry policy is unit-testable. */
export async function fetchActivityWithRetry(
  fetchOnce: () => Promise<GhActivity>,
  sleep: (ms: number) => Promise<void>,
  attempts = 5,
  delayMs = 1500,
): Promise<GhActivity> {
  for (let i = 0; i < attempts; i++) {
    if (i > 0) await sleep(delayMs);
    try {
      const a = await fetchOnce();
      if (!a.computing) return a; // stats are ready
      // else: still computing → fall through to the next attempt
    } catch (e) {
      if (i === attempts - 1 || !isRetryableActivityError(e)) throw e;
    }
  }
  return { computing: true, weeks: [] };
}
