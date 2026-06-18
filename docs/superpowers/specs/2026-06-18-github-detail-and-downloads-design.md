# GitHub Screen — PR/Issue Detail View + Download Metrics — Design Spec

**Date:** 2026-06-18
**Status:** Approved design (brainstorming complete) — ready for implementation plans.
**Builds on:** the feature-complete GitHub screen (Phases 1–4, merged to `main` @ `4cf7bab`). Spec `docs/superpowers/specs/2026-06-17-github-screen-design.md`.

**Goal:** Two enhancements to the GitHub screen — (A) clicking a PR or issue opens a full, in-app **detail view** (description, comments, checks, reviews, changed files), and (B) the Releases tab gets a **holistic download-metrics** rework that rolls per-asset counts up into a clear summary instead of a wall of numbers.

**Two independent slices.** Build **B (download metrics)** first — frontend-only, no new dependency, lower risk — then **A (detail view)**. Each is its own plan → subagents → adversarial review → ff-merge.

> `gh` detail-field surface verified against live gh 2.86 (this session): `gh pr view <n> --json` exposes `number,title,body,author,state,labels,assignees,milestone,baseRefName,headRefName,reviewDecision,mergeable,mergeStateStatus,additions,deletions,changedFiles,files,reviews,latestReviews,statusCheckRollup,comments,createdAt,updatedAt,url`; `gh issue view <n> --json` exposes `number,title,body,author,state,stateReason,labels,assignees,milestone,comments,createdAt,updatedAt,url`. Shapes: `comments[] = {author{login}, body, createdAt}`, `reviews[] = {author{login}, state, body, submittedAt}`, `files[] = {path, additions, deletions}`, `statusCheckRollup` = the flat CheckRun/StatusContext leaf array (as in Phase 2).

## Decisions log (from brainstorming)

| Decision | Choice |
|---|---|
| Detail layout | **Full-view with Back** — clicking a row replaces the list with a full-width detail; ← Back returns; list scroll preserved. |
| Markdown | **Render sanitized markdown** (`marked` + `dompurify`); fenced code via the existing Shiki. |
| Detail content | Core (header · description · labels/assignees/dates · actions) **+ all four extras**: comment thread, CI checks (PRs), changed files (PRs), reviews (PRs). |
| Changed files | **Summary + per-file ± + link to github.com diff** — NOT an in-app diff renderer (deferred). ✔ confirmed |
| Download metrics | **All four roll-ups**: all-time summary, downloads-by-release bar chart, downloads-by-platform, top-assets leaderboard — plus declutter (collapse per-asset tables). |
| Platform inference | **Best-effort from filename**, unclassifiable assets bucketed as "Other" with a visible caveat. ✔ confirmed |

---

## Part B — Download metrics (slice 1, frontend-only)

**No new backend.** `github_releases` already returns every release with `assets[].downloadCount`. All roll-ups are pure functions over that data.

### Pure helpers (TDD) — `src/lib/github/downloads.ts`
- `inferPlatform(assetName: string): Platform` — classify by filename. `Platform = "macOS" | "Windows" | "Linux" | "Source" | "Other"`. Heuristics: `macos|darwin|.dmg|.pkg` → macOS; `windows|win(32|64)?|.exe|.msi` → Windows; `linux|.deb|.rpm|.appimage|.tar.gz|.tgz` → Linux; `source|.zip` source archives → Source; else Other. (Pragmatic substrings, case-insensitive; documented as best-effort.)
- `aggregateDownloads(releases: GhRelease[]): DownloadSummary` →
  ```ts
  type DownloadSummary = {
    grandTotal: number;
    releaseCount: number;
    avgPerRelease: number;          // grandTotal / releaseCount, rounded
    topRelease: { tag: string; total: number } | null;
    byRelease: { tag: string; name: string; total: number }[];   // sorted desc, for the bar chart
    byPlatform: { platform: Platform; total: number }[];          // sorted desc, only non-zero
    topAssets: { name: string; release: string; count: number }[]; // top ~10 across all releases, desc
  };
  ```
  All sums are over `assets[].downloadCount`. Empty input → zeros + nulls + empty arrays (no divide-by-zero).

### Layout — rework `GithubReleases.svelte`
Leads with the roll-ups, then the per-release list (decluttered):
1. **All-time summary strip** — `grandTotal` (big), releaseCount, avgPerRelease, most-downloaded release.
2. **Downloads by release** — a horizontal bar list ranking `byRelease` (reuse the `formatCompact` + a simple `%`-width bar like the Insights milestone bars; no new chart lib).
3. **Downloads by platform** — `byPlatform` bars, with a one-line "inferred from filenames — best effort" caveat.
4. **Top assets** — the `topAssets` leaderboard (file name · release · count).
5. **Per-release list** — each release header (tag, name, badges, date, total) with its **asset table collapsed by default** behind an "▸ N assets" toggle (expands to today's table).

Empty/loading/error states unchanged (reuse the existing `panel.status` handling).

---

## Part A — PR/Issue detail view (slice 2)

### Navigation & state
Add to `githubState`: `selectedItem: { kind: "pr" | "issue"; number: number } | null`, with `openItem(kind, number)` / `closeItem()`. The PRs/Issues tab components render `<GithubDetail>` when an item of their kind is selected, else the list. The list element **stays mounted but hidden** (so its scroll position is preserved on Back). `closeItem()` (Back button, or Escape) returns to the list. Selecting a row in a different tab, switching tabs, switching repos, or Refresh all clear `selectedItem`.

### Backend — two new commands (`src-tauri/src/github/mod.rs`)
- `github_pr_detail(repo, number) -> GhPullDetail` via `gh pr view <n> --repo O/R --json number,title,body,author,state,isDraft,labels,assignees,milestone,baseRefName,headRefName,reviewDecision,mergeable,mergeStateStatus,additions,deletions,changedFiles,files,reviews,statusCheckRollup,comments,createdAt,updatedAt,url`.
- `github_issue_detail(repo, number) -> GhIssueDetail` via `gh issue view <n> --repo O/R --json number,title,body,author,state,stateReason,labels,assignees,milestone,comments,createdAt,updatedAt,url`.

DTOs (camelCase), with pure `map_*` fns unit-tested from captured payloads:
```ts
type GhComment = { author: string; body: string; createdAt: string };
type GhReview  = { author: string; state: string; body: string; submittedAt: string }; // state: APPROVED|CHANGES_REQUESTED|COMMENTED|...
type GhFile    = { path: string; additions: number; deletions: number };
type GhPullDetail = {
  number; title; body; author; state; isDraft;
  labels: GhLabel[]; assignees: string[]; milestone: string | null;
  baseRefName; headRefName; reviewDecision; mergeable; mergeStateStatus;
  additions; deletions; changedFiles;
  files: GhFile[]; reviews: GhReview[]; checks: GhCheck[];   // checks = normalized statusCheckRollup (reuse Phase-2 GhCheck)
  comments: GhComment[]; createdAt; updatedAt; url;
};
type GhIssueDetail = {
  number; title; body; author; state; stateReason;
  labels: GhLabel[]; assignees: string[]; milestone: string | null;
  comments: GhComment[]; createdAt; updatedAt; url;
};
```
Cached in `githubState` via the existing `makePanel<T>()` keyed by `repo|kind|number|nonce`.

### Markdown rendering (sanitized) — `src/lib/components/github/Markdown.svelte`
- Deps: **`marked`** (GFM parse) + **`dompurify`** (sanitize). Render: `{@html DOMPurify.sanitize(marked.parse(src), { … })}`.
- **Security:** DOMPurify strips `<script>`, inline event handlers, and `javascript:`/`data:` URLs; allow a safe tag set (headings, p, lists incl. task lists, links, code/pre, blockquote, img, table, hr, em/strong/del). Links get `target="_blank" rel="noreferrer"`. This is the project's standing "treat tool/observed content as untrusted" rule applied to GitHub-authored text.
- **Code blocks:** post-process fenced code through the app's existing Shiki highlighter (best-effort; fall back to plain `<pre><code>` if a language is unknown).
- Client-only (the screen is Tauri-gated); no SSR concern.
- A small pure helper `mdToSafeHtml(src): string` is unit-tested for a few injection cases (script/onerror/js-url stripped).

### Detail component — `src/lib/components/github/GithubDetail.svelte`
`{ kind, number }` props; loads the detail panel; renders:
- **Header:** ← Back · `#number title` · state badge (open/closed/merged/draft) · author · (PR) `head→base` · Open-on-github.com · the **action buttons** (reuse Phase-4 `githubActions`: Comment, and Merge/Close-Reopen).
- **Description:** `<Markdown>` of `body` (or "No description.").
- **Metadata row:** labels, assignees, milestone, created/updated.
- **(PR) Merge-readiness:** `reviewDecision` + `mergeable`/`mergeStateStatus`; the **checks** list (each check name + ✓/✗/○ + link); the **reviews** list (author + verdict + optional body).
- **(PR) Changed files:** `changedFiles` + total `+adds/−dels`; a list of `files[]` (path · `+/−`) each linking to `…/pull/<n>/files` on github.com. No in-app diff.
- **Comment thread:** each `GhComment` as a card (author · relative time · `<Markdown>` body). Empty → "No comments yet."

Actions performed from the detail header refresh the detail (bump nonce) so the new comment / merged state shows.

---

## New dependencies
- `marked` and `dompurify` (+ `@types/dompurify` if needed) — frontend only, slice 2. Slice 1 adds none.

## Module / file structure
**Slice 1 (downloads):** new `src/lib/github/downloads.ts` (+ `downloads.test.ts`); rework `GithubReleases.svelte`.
**Slice 2 (detail):** Rust `github_pr_detail`/`github_issue_detail` + DTOs in `github/mod.rs` (+ tests), `commands.rs`, `lib.rs`; TS detail types + 2 api wrappers + `githubState` `selectedItem`/`openItem`/`closeItem` + a detail panel + loaders; new `Markdown.svelte` (+ `markdown.ts` helper + test); new `GithubDetail.svelte`; row-click wiring + hidden-list scroll preservation in `GithubPulls.svelte`/`GithubIssues.svelte`.

## Testing
- **Vitest:** `inferPlatform` (filenames → platform incl. Other), `aggregateDownloads` (roll-ups, empty input, top-N), `mdToSafeHtml` (injection cases stripped, basic markdown rendered).
- **cargo:** `map_pr_detail`/`map_issue_detail` deserialization from captured cli/cli payloads (comment/review/file shapes, null milestone/empty arrays).
- **Components/Tauri-only:** verified by gates + adversarial review + the user's `tauri build` (the live `gh … view` paths and markdown render are Tauri-gated, like the rest of the screen).

## Security
- `gh … view <n> --repo O/R` with a `u64` number; owner/repo from the validated Phase-1 parser; no shell — same posture as the existing commands.
- **Markdown is the new attack surface:** GitHub-authored body/comment text is sanitized via DOMPurify before `{@html}`. This is non-negotiable and unit-tested.

## Open items (assumptions confirmed in brainstorming)
- Changed files = summary + links, no in-app diff. ✔
- Markdown via `marked` + `dompurify`. ✔
- Platform inference best-effort, "Other" bucket + caveat. ✔
