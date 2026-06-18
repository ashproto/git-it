# GitHub Screen — Download Metrics (Slice 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework the Releases tab to lead with **holistic download roll-ups** (all-time summary · by-release bars · by-platform bars · top-assets leaderboard) and collapse the per-asset tables, replacing today's wall of raw per-asset numbers.

**Architecture:** Frontend-only — `github_releases` already returns every asset's `downloadCount`, so all roll-ups are pure functions over data we have. A new tested `downloads.ts` provides `inferPlatform` (filename → platform, best-effort) and `aggregateDownloads` (the summary); `GithubReleases.svelte` is reworked to render the roll-ups + a decluttered per-release list. No new backend, no new dependency.

**Tech Stack:** SvelteKit 5 / Svelte 5 runes · Vitest.

**Spec:** `docs/superpowers/specs/2026-06-18-github-detail-and-downloads-design.md` (Part B). **Branch:** `feat/github-downloads` (already checked out; the spec is committed on it).

---

## File Structure

- **Create:** `src/lib/github/downloads.ts` (+ `src/lib/github/downloads.test.ts`) — `Platform`, `inferPlatform`, `DownloadSummary`, `aggregateDownloads`. Pure, no Svelte/Tauri deps.
- **Modify:** `src/lib/components/github/GithubReleases.svelte` — render the roll-ups + collapse the per-asset tables.

---

## Task 1: Pure download-aggregation helpers (TDD)

**Files:** Create `src/lib/github/downloads.ts`, `src/lib/github/downloads.test.ts`.

- [ ] **Step 1: Write the failing tests.**

Create `src/lib/github/downloads.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { inferPlatform, aggregateDownloads } from "./downloads";
import type { GhRelease } from "../types";

function rel(tag: string, name: string, assets: [string, number][]): GhRelease {
  return {
    tagName: tag,
    name,
    draft: false,
    prerelease: false,
    publishedAt: null,
    htmlUrl: "",
    totalDownloads: 0,
    assets: assets.map(([n, c]) => ({ name: n, size: 1, downloadCount: c, downloadUrl: "" })),
  };
}

describe("inferPlatform", () => {
  it("classifies by filename markers", () => {
    expect(inferPlatform("gh_2.95.0_macOS_arm64.zip")).toBe("macOS");
    expect(inferPlatform("app-darwin-amd64.tar.gz")).toBe("macOS");
    expect(inferPlatform("tool_1.0_windows_amd64.msi")).toBe("Windows");
    expect(inferPlatform("setup.exe")).toBe("Windows");
    expect(inferPlatform("gh_2.95.0_linux_amd64.deb")).toBe("Linux");
    expect(inferPlatform("app.AppImage")).toBe("Linux");
    expect(inferPlatform("myproject-1.0-source.tar.gz")).toBe("Source");
    expect(inferPlatform("gh_2.95.0_checksums.txt")).toBe("Other");
  });
});

describe("aggregateDownloads", () => {
  it("returns zeros/nulls/empties for no releases", () => {
    const s = aggregateDownloads([]);
    expect(s.grandTotal).toBe(0);
    expect(s.releaseCount).toBe(0);
    expect(s.avgPerRelease).toBe(0);
    expect(s.topRelease).toBeNull();
    expect(s.byRelease).toEqual([]);
    expect(s.byPlatform).toEqual([]);
    expect(s.topAssets).toEqual([]);
  });

  it("rolls up totals, releases, platforms, and top assets", () => {
    const s = aggregateDownloads([
      rel("v2", "Two", [["app_macOS_arm64.dmg", 100], ["app_windows.exe", 50]]),
      rel("v1", "One", [["app_linux.deb", 30]]),
    ]);
    expect(s.grandTotal).toBe(180);
    expect(s.releaseCount).toBe(2);
    expect(s.avgPerRelease).toBe(90);
    expect(s.topRelease).toEqual({ tag: "v2", total: 150 });
    expect(s.byRelease).toEqual([
      { tag: "v2", name: "Two", total: 150 },
      { tag: "v1", name: "One", total: 30 },
    ]);
    expect(s.byPlatform).toEqual([
      { platform: "macOS", total: 100 },
      { platform: "Windows", total: 50 },
      { platform: "Linux", total: 30 },
    ]);
    expect(s.topAssets[0]).toEqual({ name: "app_macOS_arm64.dmg", release: "v2", count: 100 });
    expect(s.topAssets).toHaveLength(3);
  });

  it("excludes zero-download assets from top assets and platform totals", () => {
    const s = aggregateDownloads([rel("v1", "One", [["a.dmg", 0], ["b.exe", 5]])]);
    expect(s.topAssets).toEqual([{ name: "b.exe", release: "v1", count: 5 }]);
    expect(s.byPlatform).toEqual([{ platform: "Windows", total: 5 }]);
  });
});
```

- [ ] **Step 2: Run them — expect FAIL (module missing).**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github/downloads`
Expected: FAIL — cannot resolve `./downloads`.

- [ ] **Step 3: Implement the helpers.**

Create `src/lib/github/downloads.ts`:

```typescript
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
  const byRelease: { tag: string; name: string; total: number }[] = [];
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
    byRelease.push({ tag: r.tagName, name: r.name || r.tagName, total: relTotal });
  }

  const releaseCount = releases.length;
  const avgPerRelease = releaseCount ? Math.round(grandTotal / releaseCount) : 0;
  byRelease.sort((a, b) => b.total - a.total);
  const topRelease =
    byRelease.length && byRelease[0].total > 0
      ? { tag: byRelease[0].tag, total: byRelease[0].total }
      : null;
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
```

- [ ] **Step 4: Run them — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github/downloads`
Expected: all PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/github/downloads.ts src/lib/github/downloads.test.ts
git commit -m "feat(github): download aggregation helpers (inferPlatform, aggregateDownloads)"
```

---

## Task 2: Rework the Releases tab

**Files:** Modify `src/lib/components/github/GithubReleases.svelte` (full rewrite).

- [ ] **Step 1: Replace the component with the roll-up layout + collapsed per-release tables.**

Overwrite `src/lib/components/github/GithubReleases.svelte` with:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import { formatCompact } from "../../github/format";
  import { aggregateDownloads } from "../../github/downloads";

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) void githubState.loadReleases(repo);
  });

  const panel = $derived(githubState.releases);
  const summary = $derived(panel.data ? aggregateDownloads(panel.data) : null);
  const releaseMax = $derived(summary && summary.byRelease.length ? Math.max(1, summary.byRelease[0].total) : 1);
  const platformMax = $derived(summary && summary.byPlatform.length ? Math.max(1, summary.byPlatform[0].total) : 1);

  // Which per-release asset tables are expanded (collapsed by default).
  let expanded = $state(new Set<string>());
  function toggle(tag: string) {
    const next = new Set(expanded);
    if (next.has(tag)) next.delete(tag);
    else next.add(tag);
    expanded = next;
  }

  function rel(iso: string | null): string {
    if (!iso) return "—";
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  function kb(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
</script>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading releases…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load releases ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No releases.</p>
{:else if panel.data && summary}
  <!-- All-time summary -->
  <div class="metrics">
    <div class="metric"><span class="big">{formatCompact(summary.grandTotal)}</span><span class="lbl">total downloads</span></div>
    <div class="metric"><span class="big">{summary.releaseCount}</span><span class="lbl">releases</span></div>
    <div class="metric"><span class="big">{formatCompact(summary.avgPerRelease)}</span><span class="lbl">avg / release</span></div>
    {#if summary.topRelease}
      <div class="metric"><span class="big mono">{summary.topRelease.tag}</span><span class="lbl">top · {formatCompact(summary.topRelease.total)} ↓</span></div>
    {/if}
  </div>

  {#if summary.grandTotal > 0}
    <div class="charts">
      {#if summary.byRelease.length}
        <section class="chart">
          <h4>Downloads by release</h4>
          {#each summary.byRelease.slice(0, 8) as b (b.tag)}
            <div class="bar-row">
              <span class="bl mono" title={b.name}>{b.tag}</span>
              <div class="track"><div class="fill" style={`width:${(b.total / releaseMax) * 100}%`}></div></div>
              <span class="bn">{formatCompact(b.total)}</span>
            </div>
          {/each}
        </section>
      {/if}
      {#if summary.byPlatform.length}
        <section class="chart">
          <h4>Downloads by platform <span class="hint">inferred from filenames</span></h4>
          {#each summary.byPlatform as p (p.platform)}
            <div class="bar-row">
              <span class="bl">{p.platform}</span>
              <div class="track"><div class="fill alt" style={`width:${(p.total / platformMax) * 100}%`}></div></div>
              <span class="bn">{formatCompact(p.total)}</span>
            </div>
          {/each}
        </section>
      {/if}
    </div>

    {#if summary.topAssets.length}
      <section class="chart">
        <h4>Top assets</h4>
        <ul class="top">
          {#each summary.topAssets as a (a.name + a.release)}
            <li>
              <span class="an" title={a.name}>{a.name}</span>
              <span class="ar mono">{a.release}</span>
              <span class="ac">{formatCompact(a.count)} ↓</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}

  <!-- Per-release list (asset tables collapsed) -->
  <h4 class="rels-h">All releases</h4>
  <div class="rels">
    {#each panel.data as r (r.tagName)}
      <section class="rel">
        <header>
          <a class="rtitle" href={r.htmlUrl} target="_blank" rel="noreferrer">{r.name || r.tagName}</a>
          <span class="tag mono">{r.tagName}</span>
          {#if r.prerelease}<span class="badge">pre-release</span>{/if}
          {#if r.draft}<span class="badge">draft</span>{/if}
          <span class="when">{rel(r.publishedAt)}</span>
          <span class="total">{formatCompact(r.totalDownloads)} ↓</span>
          {#if r.assets.length}
            <button class="exp" type="button" onclick={() => toggle(r.tagName)} aria-expanded={expanded.has(r.tagName)}>
              {expanded.has(r.tagName) ? "▾" : "▸"} {r.assets.length} {r.assets.length === 1 ? "asset" : "assets"}
            </button>
          {/if}
        </header>
        {#if r.assets.length && expanded.has(r.tagName)}
          <table class="assets">
            <tbody>
              {#each r.assets as a (a.name)}
                <tr>
                  <td><a href={a.downloadUrl} target="_blank" rel="noreferrer">{a.name}</a></td>
                  <td class="num">{kb(a.size)}</td>
                  <td class="num">{formatCompact(a.downloadCount)} ↓</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </section>
    {/each}
  </div>
{/if}

<style>
  .metrics {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    margin-bottom: 16px;
  }
  .metric {
    display: flex;
    flex-direction: column;
  }
  .big {
    font-size: 20px;
    font-weight: 600;
  }
  .lbl {
    font-size: 11px;
    color: var(--text-muted);
  }
  .charts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 16px;
    margin-bottom: 16px;
  }
  .chart {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    background: var(--panel-bg);
  }
  h4 {
    margin: 0 0 10px 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .hint {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    font-size: 10.5px;
  }
  .bar-row {
    display: grid;
    grid-template-columns: 90px 1fr 54px;
    align-items: center;
    gap: 8px;
    margin-bottom: 5px;
    font-size: 12px;
  }
  .bl {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .track {
    height: 8px;
    background: var(--btn-bg);
    border-radius: 999px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
  }
  .fill.alt {
    background: color-mix(in srgb, var(--accent) 65%, var(--text-muted));
  }
  .bn {
    text-align: right;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .top {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .top li {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 10px;
    padding: 2px 0;
    font-size: 12px;
  }
  .an {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ar {
    color: var(--text-muted);
    font-size: 11px;
  }
  .ac {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .rels-h {
    margin-top: 4px;
  }
  .rels {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .rel header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .rtitle {
    color: var(--text);
    text-decoration: none;
    font-weight: 600;
    font-size: 13.5px;
  }
  .rtitle:hover {
    color: var(--accent);
  }
  .tag {
    color: var(--text-muted);
    font-size: 11.5px;
  }
  .badge {
    font-size: 10px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    text-transform: uppercase;
  }
  .when {
    font-size: 12px;
    color: var(--text-muted);
  }
  .total {
    margin-left: auto;
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
  }
  .exp {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 11.5px;
    cursor: pointer;
    padding: 0;
  }
  .exp:hover {
    color: var(--accent);
  }
  .assets {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    margin-top: 6px;
  }
  .assets td {
    padding: 3px 8px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
  }
  .assets a {
    color: var(--text);
    text-decoration: none;
  }
  .assets a:hover {
    color: var(--accent);
  }
  .num {
    text-align: right;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .note {
    margin: 14px 2px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
```

- [ ] **Step 2: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0 errors, 0 warnings.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubReleases.svelte
git commit -m "feat(github): holistic download metrics in the Releases tab"
```

---

## Task 3: Full verification

**Files:** none.

- [ ] **Step 1: Gates.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run check
npx vitest run
cd src-tauri && cargo test
```
Expected: svelte-check 0/0 · vitest all pass (+ the new `downloads` suite) · cargo unchanged (no Rust change).

- [ ] **Step 2: Build + smoke.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run tauri build
```
Open a repo with releases (e.g. `cli/cli`) → Releases tab: confirm the summary strip (total/releases/avg/top), the by-release and by-platform bars, the top-assets list, and that each release's asset table is collapsed until you click "▸ N assets". On a repo with no releases, the empty state is unchanged.

- [ ] **Step 3: Proceed to review + merge.**

---

## Done criteria (Slice 1)
- The Releases tab leads with the four roll-ups and collapses per-asset tables; the aggregation logic is unit-tested (`inferPlatform`, `aggregateDownloads`, including empty input + zero-download exclusion).
- Gates green; no new dependency, no backend change.
- Next: **Slice 2 (PR/issue detail view)**.

## After all tasks
Adversarial review of the branch diff (`superpowers:code-reviewer`), fix Critical/Important, then **superpowers:finishing-a-development-branch** to ff-merge `feat/github-downloads` → `main`.
