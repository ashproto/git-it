import type { GhRelease } from "../types";

export type Platform = "macOS" | "Windows" | "Linux" | "Source" | "Other";

/** Best-effort platform classification from a release-asset filename. Documented
 *  as heuristic: excellent for conventionally-named assets (CLIs/installers),
 *  falls back to "Other" otherwise. "source" in the name wins (it's unambiguous). */
export function inferPlatform(assetName: string): Platform {
  const n = assetName.toLowerCase();
  if (n.includes("source")) return "Source";
  if (/darwin|macos|osx|apple|\.dmg|\.pkg/.test(n)) return "macOS";
  if (/windows|win32|win64|win-|_win|\.exe|\.msi/.test(n)) return "Windows";
  if (/linux|musl|\.deb|\.rpm|\.appimage/.test(n)) return "Linux";
  return "Other";
}

export type DownloadSummary = {
  grandTotal: number;
  releaseCount: number;
  avgPerRelease: number;
  topRelease: { tag: string; total: number } | null;
  byRelease: { tag: string; name: string; total: number }[]; // sorted desc
  byPlatform: { platform: Platform; total: number }[]; // sorted desc, non-zero only
  topAssets: { name: string; release: string; count: number }[]; // top 10 desc, non-zero
};

/** Roll the per-asset download counts up across releases. GitHub exposes only a
 *  cumulative count per asset (no time series), so these are the holistic views. */
export function aggregateDownloads(releases: GhRelease[]): DownloadSummary {
  let grandTotal = 0;
  const byReleaseMap = new Map<string, { tag: string; name: string; total: number }>();
  const platformTotals = new Map<Platform, number>();
  const allAssets: { name: string; release: string; count: number }[] = [];

  for (const r of releases) {
    let relTotal = 0;
    for (const a of r.assets) {
      grandTotal += a.downloadCount;
      relTotal += a.downloadCount;
      const p = inferPlatform(a.name);
      platformTotals.set(p, (platformTotals.get(p) ?? 0) + a.downloadCount);
      allAssets.push({ name: a.name, release: r.tagName, count: a.downloadCount });
    }
    // Dedupe by tag (GitHub tags are unique, but don't assume it) — merge totals.
    const acc = byReleaseMap.get(r.tagName);
    if (acc) acc.total += relTotal;
    else byReleaseMap.set(r.tagName, { tag: r.tagName, name: r.name || r.tagName, total: relTotal });
  }

  const releaseCount = releases.length;
  const avgPerRelease = releaseCount ? Math.round(grandTotal / releaseCount) : 0;
  // Only releases that actually have downloads appear (no empty labeled bars).
  const byRelease = [...byReleaseMap.values()]
    .filter((r) => r.total > 0)
    .sort((a, b) => b.total - a.total);
  const topRelease = byRelease.length ? { tag: byRelease[0].tag, total: byRelease[0].total } : null;
  const byPlatform = [...platformTotals.entries()]
    .filter(([, t]) => t > 0)
    .map(([platform, total]) => ({ platform, total }))
    .sort((a, b) => b.total - a.total);
  const topAssets = allAssets
    .filter((a) => a.count > 0)
    .sort((a, b) => b.count - a.count)
    .slice(0, 10);

  return { grandTotal, releaseCount, avgPerRelease, topRelease, byRelease, byPlatform, topAssets };
}
