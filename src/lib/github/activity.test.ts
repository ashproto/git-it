import { describe, it, expect, vi } from "vitest";
import { sparklinePoints, fetchActivityWithRetry, isRetryableActivityError } from "./activity";
import type { GhActivity } from "../types";

describe("sparklinePoints", () => {
  it("maps values to a polyline string, max at the top (y=0)", () => {
    expect(sparklinePoints([0, 5, 10], 100, 20)).toBe("0.0,20.0 50.0,10.0 100.0,0.0");
  });
  it("returns empty for no values", () => {
    expect(sparklinePoints([], 100, 20)).toBe("");
  });
  it("flat series sits on the baseline", () => {
    expect(sparklinePoints([3, 3], 10, 10)).toBe("0.0,0.0 10.0,0.0");
  });
});

describe("isRetryableActivityError", () => {
  it("retries cold-stats / transient failures", () => {
    for (const kind of ["Other", "RateLimited", "Forbidden"]) {
      expect(isRetryableActivityError({ kind })).toBe(true);
    }
    expect(isRetryableActivityError("network blip")).toBe(true); // non-GithubError → retry
  });
  it("does not retry permanent failures", () => {
    for (const kind of ["NotFound", "NotAuthed", "NoRemote", "NotInstalled"]) {
      expect(isRetryableActivityError({ kind })).toBe(false);
    }
  });
});

describe("fetchActivityWithRetry", () => {
  const ready: GhActivity = { computing: false, weeks: [{ week: 1, total: 2, days: [] }] };
  const computing: GhActivity = { computing: true, weeks: [] };
  const noSleep = () => Promise.resolve();

  it("returns data on the first try when stats are ready", async () => {
    const fetchOnce = vi.fn().mockResolvedValue(ready);
    const out = await fetchActivityWithRetry(fetchOnce, noSleep);
    expect(out).toEqual(ready);
    expect(fetchOnce).toHaveBeenCalledTimes(1);
  });

  it("retries while computing, then returns the data once it's ready", async () => {
    const fetchOnce = vi
      .fn()
      .mockResolvedValueOnce(computing)
      .mockResolvedValueOnce(computing)
      .mockResolvedValueOnce(ready);
    const out = await fetchActivityWithRetry(fetchOnce, noSleep);
    expect(out).toEqual(ready);
    expect(fetchOnce).toHaveBeenCalledTimes(3);
  });

  it("retries through a TRANSIENT THROW (the regression) and recovers", async () => {
    // The old code retried only `computing:true`; a thrown cold-cache error hard-errored
    // the panel. This must now retry the throw and still resolve to data.
    const fetchOnce = vi
      .fn()
      .mockRejectedValueOnce({ kind: "Other", message: "parse activity: expected array" })
      .mockResolvedValueOnce(ready);
    const out = await fetchActivityWithRetry(fetchOnce, noSleep);
    expect(out).toEqual(ready);
    expect(fetchOnce).toHaveBeenCalledTimes(2);
  });

  it("resolves to the computing marker (soft note, not error) when attempts run out", async () => {
    const fetchOnce = vi.fn().mockResolvedValue(computing);
    const out = await fetchActivityWithRetry(fetchOnce, noSleep, 3);
    expect(out).toEqual({ computing: true, weeks: [] });
    expect(fetchOnce).toHaveBeenCalledTimes(3);
  });

  it("aborts immediately on a PERMANENT error without burning retries", async () => {
    const fetchOnce = vi.fn().mockRejectedValue({ kind: "NotFound" });
    await expect(fetchActivityWithRetry(fetchOnce, noSleep, 5)).rejects.toEqual({ kind: "NotFound" });
    expect(fetchOnce).toHaveBeenCalledTimes(1);
  });

  it("propagates the last error when a transient failure never clears", async () => {
    const fetchOnce = vi.fn().mockRejectedValue({ kind: "RateLimited" });
    await expect(fetchActivityWithRetry(fetchOnce, noSleep, 3)).rejects.toEqual({ kind: "RateLimited" });
    expect(fetchOnce).toHaveBeenCalledTimes(3);
  });
});
