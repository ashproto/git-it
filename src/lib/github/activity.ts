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

/** Only a transient cold-cache failure is worth the short fixed backoff. GitHub computes
 *  /stats/commit_activity lazily (HTTP 202 the first time), which can surface as an
 *  unparseable placeholder body — an `Other`/parse error — or a non-GithubError network
 *  blip. Explicit terminal states are NOT retried: `RateLimited` won't clear inside the
 *  backoff window (and re-hitting it prolongs the throttle), and Forbidden / NotFound /
 *  NotAuthed / NoRemote / NotInstalled are permanent. Reserve the retries for the
 *  computing/parse case; a real rate limit should surface (and wait for its reset) instead. */
export function isRetryableActivityError(e: unknown): boolean {
  if (e && typeof e === "object" && "kind" in e) {
    return (e as GithubError).kind === "Other";
  }
  return true; // non-GithubError throw → treat as a transient blip
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
