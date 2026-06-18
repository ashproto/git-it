# GitHub Screen — Design Spec

**Date:** 2026-06-17
**Status:** Approved design (brainstorming complete) — ready for an implementation plan.
**Branch:** `feat/github-screen`

**Goal:** Add a third top-level screen, **GitHub**, alongside *Local Changes* and *Commit Timeline* — a read-only dashboard of the current repo's GitHub presence (stats, PRs, issues, releases + download analytics, CI/Actions, traffic, contributors, milestones/labels) plus four guarded write actions, all powered by shelling out to the `gh` CLI.

**Architecture:** All GitHub I/O happens in Rust by spawning the `gh` CLI (no shell, args passed directly) and parsing JSON — the exact same "spawn a CLI, parse stdout" pattern the app already uses for every `git` call. Auth is delegated entirely to `gh` (we store no token). The Svelte frontend calls thin Tauri command wrappers; a per-repo, per-tab cache fetches lazily and refreshes on demand.

**Tech stack:** Tauri 2 (Rust) · SvelteKit 5 (Svelte 5 runes) · `gh` CLI ≥ 2.x · existing Tauri Store plugin for the one new persisted setting.

> All command/JSON facts below were verified against a live, authenticated `gh` 2.86 on 2026-06-17 (probe workflow `verify-gh-surface`). Field names, enums, and error strings are real, not assumed.

---

## 1. Scope

**In (v1):**
- One new top-level screen with a **sub-tabbed** layout: persistent stat header + six tabs (Overview · Pull Requests · Issues · Releases · Actions · Insights).
- Eight read-only data panels (see §7).
- Four guarded write actions: comment on PR/issue, close/reopen issue, merge PR, create issue (§8).
- "Open on github.com" deep-link on every item.
- `gh` install/auth detection with setup cards; per-panel loading/error/empty states (§9).

**Out (v1) — explicitly deferred:**
- GitHub **Projects** (v2) — needs the GraphQL surface + `read:project` scope; revisit later.
- In-app PR review (approve/request-changes), label/assignee editing, release authoring, workflow log viewing (deep-link out instead).
- **GitHub Enterprise** / custom hosts — `github.com` only in v1.
- SSH-only realtime; no websockets/polling — refresh is lazy + manual.

## 2. Decisions log (from brainstorming)

| Decision | Choice | Rationale |
|---|---|---|
| Data source | **`gh` CLI shell-out** | Matches the app's existing CLI-shell-out architecture; delegates auth to `gh` → no token storage. |
| Interactivity | **Read-only + a few actions** | Everything requested is viewing; a thin guarded write layer covers the rest. |
| Extra panels | **All four** (CI/Actions, Traffic, Contributors/activity, Milestones/labels) | User wants a comprehensive health dashboard. |
| Actions | comment · close/reopen · merge PR · create issue | Highest-value safe writes, one `gh` command each. |
| Projects | **Deferred** | Different API + extra scope; keeps v1 cohesive. |
| Layout | **Sub-tabbed (B)** | Scales to long PR/issue lists; mirrors GitHub's own IA; holds both overview content and browsable lists. |

**Confirmed assumptions (defaults):** github.com only · hide the GitHub nav entry on repos with no GitHub remote · PRs/Issues default to *Open*, capped ~50 with "Load more" · no new persisted settings beyond the last-active sub-tab.

## 3. Architecture & data flow

```
Svelte tab → api.ts wrapper → Tauri command → spawn `gh` (Command, no shell) → JSON → typed Rust struct → frontend
```

- **Rust owns all GitHub I/O.** A new `github` module spawns `gh` with args passed directly to `std::process::Command` (never a shell string). Free-text bodies are piped via **stdin** (`--body-file -`), never as args.
- **Lazy + cached.** A frontend controller `githubState.svelte.ts` holds, per active repo, a record keyed by tab: `{ status: 'idle'|'loading'|'ok'|'error', data, error, fetchedAt }`. A tab fetches on first activation; **Refresh** re-fetches the active tab; switching repos clears the cache. The Overview tab fetches a small summarized subset.
- **Every command returns `Result<T, GithubError>`** where `GithubError` is a typed enum (§6) so each panel renders the right state independently. One failing panel never blanks the screen.
- **Rate-limit aware.** The header shows a subtle meter from `gh api rate_limit` (`remaining`/`limit`, `reset`). Lazy fetch + cache keeps us far under the 5,000/hr authed budget.

## 4. Repo resolution

Derive `owner/repo` from the repo's remotes (reuse `ops_remote::remotes()`): prefer `origin` if it points at `github.com`, else the first `github.com` remote. Parse all three URL forms:
- `https://github.com/OWNER/REPO(.git)`
- `git@github.com:OWNER/REPO(.git)`
- `ssh://git@github.com/OWNER/REPO(.git)`

Validate `OWNER` and `REPO` against `^[A-Za-z0-9._-]+$` (reject anything else → treated as "no GitHub remote"). A pure parser function `parse_github_remote(url) -> Option<(owner, repo)>` is unit-tested in isolation. The sidebar entry is **hidden** when resolution yields nothing.

> Note: we do **not** call `gh`'s implicit "current repo" resolution (it depends on cwd and picks a remote heuristically). We always pass an explicit `--repo OWNER/REPO` (or `gh api repos/{owner}/{repo}`) computed from our own parser, so behavior is deterministic and testable.

## 5. The `gh` runner

A single helper `run_gh(args: &[&str], stdin: Option<&str>) -> Result<String, GithubError>`:
- Spawns `gh` via `Command`, captures stdout + stderr + exit code; writes `stdin` if provided.
- On non-zero exit, classifies stderr/stdout into `GithubError` (§6).
- Returns stdout (JSON) on success for the caller to `serde_json`-parse into DTOs.
- `gh` itself is located on `PATH`; spawn `ENOENT` (or exit 127) ⇒ `NotInstalled`.

## 6. Error model (verified strings)

```rust
enum GithubError {
  NotInstalled,                 // spawn ENOENT / exit 127
  NotAuthed,                    // stderr: "You are not logged into any GitHub hosts"
  NoRemote,                     // stderr: "no git remotes found" (no HTTP code)
  NotFound,                     // stderr matches "HTTP 404"
  Forbidden,                    // stderr matches "HTTP 403" (push-access / perms)
  RateLimited,                  // 403 whose body/message mentions "rate limit exceeded"
  Other(String),               // anything else; carry trimmed stderr
}
```

Classification rules (from the live probe):
- `gh` writes diagnostics to **stderr**, raw API JSON to **stdout**. Capture both.
- HTTP status appears in stderr as `... (HTTP 404)` / `(HTTP 403)` → regex `HTTP (\d{3})` is the reliable classifier.
- `403` + message containing "rate limit exceeded" ⇒ `RateLimited`, else `Forbidden`.
- `gh auth status` prints to stderr **even on success** — key on **exit code**, then parse text.
- `NoRemote` carries no HTTP code — match the exact string `no git remotes found`.

The traffic panel specifically maps `Forbidden` → a "needs push access to this repo" empty state (not a hard error).

## 7. Data contracts (verified) & the screen

### Persistent header (all tabs)
`owner/repo` (→ `htmlUrl`), default branch, language dot, license, visibility, description, topic chips, and **stat tiles**: ★ Stars · ⑂ Forks · 👁 Watchers · Open PRs · Open Issues. Plus Refresh + "updated 2m ago" + rate-limit meter. Clicking the PR/Issue tiles jumps to that tab.

**Commands:** `gh api repos/{owner}/{repo}` for identity/stats, **plus** one small `gh api graphql` query for the accurate open counts (REST conflates issues + PRs):
```graphql
query($o:String!,$n:String!){ repository(owner:$o,name:$n){
  issues(states:OPEN){ totalCount } pullRequests(states:OPEN){ totalCount } } }
```
**TS type:**
```ts
type GhRepoStats = {
  fullName: string; description: string | null; htmlUrl: string;
  visibility: string;                 // "public" | "private"
  defaultBranch: string;              // may be "trunk" etc — never hardcode
  language: string | null;
  licenseSpdxId: string | null;       // license.spdx_id; license object null when unlicensed
  topics: string[];                   // [] when none
  stars: number;                      // stargazers_count
  watchers: number;                   // ⚠ subscribers_count — NOT watchers/watchers_count
  forks: number;                      // forks_count
  openIssues: number;                 // GraphQL issues(states:OPEN).totalCount
  openPulls: number;                  // GraphQL pullRequests(states:OPEN).totalCount
  pushedAt: string;                   // ISO-8601
  archived: boolean; isFork: boolean;
};
```
**Gotchas:** `watchers_count` and `watchers` both equal **stars** — the true watch count is `subscribers_count` (REST-only; `gh repo view --json` does not expose it). REST `open_issues_count` **includes open PRs**, so the **Open Issues** and **Open PRs** tiles use the GraphQL `totalCount`s above (accurate and uncapped — the issues/PRs *lists* are `--limit`-capped, so list length is not a reliable total). `default_branch` is not always `main`. Guard `license` for null.

### Overview tab
A compact at-a-glance dashboard composed from small slices of the other panels: recent PRs (5), recent issues (5), latest release (name + summed downloads), default-branch CI status (latest run ✅/❌), top contributors (5), commits/week sparkline. Each card header links to its full tab. Implemented by calling the same commands below with small `--limit`s (no new endpoints).

### Pull Requests tab
**List command:** `gh pr list --repo {o}/{r} --state {open|closed|merged|all} --limit N --json number,title,author,headRefName,baseRefName,labels,reviewDecision,isDraft,state,updatedAt,url`
**Detail (on select) adds the slow fields:** `--json ...,statusCheckRollup,mergeable,mergeStateStatus` (or `gh pr view`).
```ts
type GhPull = {
  number: number; title: string; author: string;     // author.login
  headRefName: string; baseRefName: string;
  labels: { name: string; color: string }[];          // color = 6-hex, no '#', mixed case
  reviewDecision: "" | "REVIEW_REQUIRED" | "APPROVED" | "CHANGES_REQUESTED"; // "" not null
  isDraft: boolean;
  state: "OPEN" | "CLOSED" | "MERGED";                 // uppercase
  updatedAt: string; url: string;
  // detail-only:
  mergeable?: "MERGEABLE" | "CONFLICTING" | "UNKNOWN";
  mergeStateStatus?: string;                            // BEHIND|BLOCKED|CLEAN|DIRTY|...
  checks?: GhCheck[];                                   // flattened from statusCheckRollup
};
type GhCheck = { name: string; bucket: "pass"|"fail"|"pending"|"neutral"; detailsUrl?: string };
```
**Gotchas:** `statusCheckRollup` is a **flat array of per-check leaves**, two `__typename` variants — `CheckRun` `{name,status,conclusion,workflowName,detailsUrl}` and `StatusContext` `{context,state,targetUrl}`. We normalize both into `GhCheck` (CheckRun: `conclusion`; StatusContext: `state`; empty `conclusion` ⇒ pending). `mergeable`/`mergeStateStatus`/`statusCheckRollup` force extra GraphQL round-trips and are slow — **omit them from the list**, fetch on detail only. Empty list ⇒ `[]` (exit 0); detect emptiness by length.

### Issues tab
**List command:** `gh issue list --repo {o}/{r} --state {open|closed|all} --limit N --json number,title,author,labels,assignees,state,updatedAt,url`
```ts
type GhIssue = {
  number: number; title: string; author: string;       // author.login
  labels: { name: string; color: string }[];
  assignees: string[];                                  // login[]
  state: "OPEN" | "CLOSED"; updatedAt: string; url: string;
};
```
**Gotchas:** `gh issue list` **excludes PRs** (uses the GraphQL issues connection) — no post-filtering needed. Default `--limit` is 30 → always pass an explicit limit. Label `color` has no `#` and inconsistent case → normalize. State value is uppercase though the `--state` flag is lowercase. (`gh issue view` does *not* filter PRs, but we only use `list`.)

### Releases tab (download analytics)
**Command:** `gh api repos/{o}/{r}/releases` (project with `--jq` to drop the verbose `uploader` subobject).
```ts
type GhRelease = {
  tagName: string; name: string; draft: boolean; prerelease: boolean;
  publishedAt: string | null; htmlUrl: string;
  assets: { name: string; size: number; downloadCount: number; downloadUrl: string }[];
  totalDownloads: number;                                // sum(assets[].download_count) — we compute
};
```
UI: per release, an assets table (name, size, **download count**) + a per-release total + a downloads-by-asset bar; aggregate across releases (grand total, most-downloaded asset). **Gotchas:** `download_count` is the **only** download metric GitHub exposes — a single cumulative integer per asset, **no time series**. No pre-summed total — we sum. Source-code archives are not in `assets[]`. `gh release list` carries no download data.

### Actions tab (CI)
**Command:** `gh run list --repo {o}/{r} --limit N --json databaseId,name,displayTitle,workflowName,headBranch,event,status,conclusion,createdAt,updatedAt,url` (filters: `--branch`, `--workflow`, `--event`, `--status`).
```ts
type GhRun = {
  id: number;                          // databaseId
  title: string;                       // displayTitle
  workflowName: string; headBranch: string; event: string;
  status: "queued" | "in_progress" | "completed" | string;
  conclusion: "" | "success" | "failure" | "cancelled" | "skipped" | "timed_out" | "neutral" | "action_required" | "stale";
  createdAt: string; updatedAt: string; url: string;     // duration ≈ updatedAt - createdAt
};
```
**Gotcha:** check **pass/fail via `conclusion`, but only once `status === "completed"`** — `conclusion` is `""` while running. Newest-first; default limit 20. Run detail/logs deep-link to `url` (not rendered in-app).

### Insights tab (Traffic · Contributors · Activity · Milestones/labels)

**Traffic** (owner-only): `gh api repos/{o}/{r}/traffic/views`, `.../traffic/clones` (top-level `{count, uniques, views|clones:[{timestamp,count,uniques}]}`), `.../traffic/popular/paths` (bare array `{path,title,count,uniques}`), `.../traffic/popular/referrers` (bare array `{referrer,count,uniques}`). **Requires `repo` scope AND push access** → `403 "Must have push access to repository"` otherwise → render the "needs push access" empty state. A `200` with `count:0` + empty array is the distinct "no traffic" empty state. `?per=day|week` for granularity. (Note shape difference: views/clones are wrapped objects; popular/* are bare arrays.)

**Contributors:** `gh api 'repos/{o}/{r}/contributors?per_page=N'` → `[{login, contributions, type, avatarUrl, htmlUrl}]`, pre-sorted desc. Filter `type === "Bot"` optionally.

**Activity:** `gh api repos/{o}/{r}/stats/commit_activity` → exactly 52 weekly buckets `{week, total, days[7]}`, oldest-first. Rendered as a commits/week sparkline. **Gotcha:** `/stats/*` may return **HTTP 202 with an empty body** while GitHub computes — the runner must detect 202/empty and the panel polls/retries (e.g. 3 tries, 1.5s apart) before showing data or an "still computing" note. `week` is **unix seconds** (×1000 for JS Date).

**Milestones & labels:** `gh api 'repos/{o}/{r}/milestones?state=open'` → `[{title, number, state, openIssues, closedIssues, dueOn, description, htmlUrl}]` (progress = `closed/(open+closed)`); `gh api 'repos/{o}/{r}/labels'` → `[{name, color, description, default}]`. Default page size 30 → paginate / `--paginate` for repos with more. Empty milestones ⇒ `[]` (many repos have none — show an empty state, not an error).

## 8. Write actions (guarded)

Each is one `gh` command behind a confirm dialog (reusing the app's existing confirm + `OpOutcome` patterns); on success the affected panel refreshes. **Free-text bodies are passed via stdin (`--body-file -`)**, never as args.

| Action | Command |
|---|---|
| Comment on PR | `gh pr comment <n> --repo {o}/{r} --body-file -` |
| Comment on issue | `gh issue comment <n> --repo {o}/{r} --body-file -` |
| Close issue | `gh issue close <n> --repo {o}/{r} [--reason completed\|"not planned"]` |
| Reopen issue | `gh issue reopen <n> --repo {o}/{r}` *(no `--reason`)* |
| Merge PR | `gh pr merge <n> --repo {o}/{r} --merge\|--squash\|--rebase [--delete-branch]` |
| Create issue | `gh issue create --repo {o}/{r} --title "…" --body-file - [--label …] [--assignee …]` |

**Gotchas:** always pass a body/title flag or `gh` drops into an interactive editor and hangs. `issue create` has no positional number (always creates new). Merge dialog shows the target branch + method picker; `--admin`/`--auto` are out of scope for v1.

## 9. Setup, auth & empty states (screen-level)

- **`gh` not installed** (`NotInstalled`) → setup card: "Install the GitHub CLI" + `brew install gh` + link + Recheck.
- **`gh` not authenticated** (`NotAuthed`) → card: "Run `gh auth login`" + Recheck.
- **No GitHub remote** (`NoRemote` / parser miss) → the sidebar entry is hidden; if reached directly, an empty state.
- **Per-panel:** independent `loading` (spinner), `error` (message + Retry), and `empty` (panel-appropriate copy) states.
- Detection at screen mount: `gh --version` (installed?) + `gh auth status` (authed?) via one `github_availability` command.

## 10. Module / file structure

**Rust (`src-tauri/src/`):**
- `github/mod.rs` — `parse_github_remote` (unit-tested), `run_gh` + error classifier (unit-tested), `serde` DTOs, one function per endpoint.
- `commands.rs` — read: `github_availability`, `github_context`, `github_repo_stats`, `github_pulls`, `github_pull_detail`, `github_issues`, `github_releases`, `github_runs`, `github_traffic`, `github_contributors`, `github_commit_activity`, `github_milestones`, `github_labels`; write: `github_comment`, `github_issue_set_state`, `github_pr_merge`, `github_issue_create`. Registered in `lib.rs`.

**TS (`src/lib/`):**
- `types.ts` — the `Gh*` types above.
- `api.ts` — one wrapper per command.
- `githubState.svelte.ts` — controller + per-repo/per-tab cache + active-tab state + rate-limit + availability.
- `githubActions.ts` — the four write actions (confirm → command → refresh).
- `components/github/` — `GithubView.svelte` (shell+tabs+header), `GithubHeader.svelte`, `GithubTabs.svelte`, `GithubOverview.svelte`, `GithubPulls.svelte`, `GithubIssues.svelte`, `GithubReleases.svelte`, `GithubActions.svelte`, `GithubInsights.svelte`, `GithubSetupCard.svelte`, and small shared `StatTile.svelte` / `Sparkline.svelte` / `MiniBar.svelte` + a reused confirm dialog.

**Wiring:** `store.svelte.ts` `activeView` union gains `"github"` + `setActiveView` accepts it; `Sidebar.svelte` gains a (conditionally-rendered) nav entry; `+page.svelte` adds the `"github"` branch in the `.view-swap` block; persist the last-active sub-tab via the existing Store pattern.

## 11. Testing strategy

- **Rust unit tests** (`#[cfg(test)]`): `parse_github_remote` (all three URL forms, non-GitHub rejection, `.git` trimming, validation rejects bad chars); error classifier (stderr strings → `NotAuthed`/`NoRemote`/`NotFound`/`Forbidden`/`RateLimited`); DTO `serde` deserialization from the captured real-payload samples (kept as fixtures).
- **Vitest** (pure helpers): download-count aggregation/totals; `statusCheckRollup` → `GhCheck` normalization (both `__typename` variants, empty conclusion ⇒ pending); run status/conclusion → pass/fail bucket; commit-activity week (sec→ms) + sparkline shaping; number formatting (`1.2k`); owner/repo validation mirror; Overview health-summary derivation.
- **Components:** verified in the browser preview with sample fixtures. Live `gh` paths are Tauri-only/manual (need `gh` installed + authed), like the project's other Tauri-only flows — manual smoke after each phase.

## 12. Phasing (one spec → four shippable phases)

1. **Foundation** — `activeView` + sidebar entry + `+page` branch; repo resolution; `run_gh` + error model; `github_availability` + setup cards; `GithubView` shell + header + stat tiles; **Overview** tab. (Proves the pipeline end-to-end.)
2. **Read panels** — Pull Requests, Issues, Releases, Actions tabs (lists + detail, read-only, open-in-browser).
3. **Insights** — Traffic (owner-only state + 202 retry), Contributors, Activity sparkline, Milestones/labels.
4. **Actions** — the four writes + confirm dialogs + refresh-after-action; `githubActions.ts`.

Each phase ends working, gated (svelte-check 0/0 · vitest green · cargo green · build), preview-verified, adversarially reviewed, and merged.

## 13. Security

- `gh` is spawned via `Command` with args passed **directly** — never interpolated into a shell string.
- `owner`, `repo`, issue/PR `number`, release `tag` validated against strict allowlists before reaching `gh`; numbers are typed integers.
- Free-text (comment/issue bodies) flows through **stdin** (`--body-file -`), avoiding arg-injection and arg-length limits.
- We never read, store, or log the `gh` token; auth is entirely `gh`'s.
- This is the `gh` analogue of the project's standing rule (always `--`/`--end-of-options` before user operands for git shell-outs).

## 14. Open items (defaults chosen; flag if you disagree)
- github.com only (Enterprise later). ✔ default
- Hide nav entry when no GitHub remote. ✔ default
- PRs/Issues default Open, ~50 cap + Load more. ✔ default
- One new persisted setting (last-active sub-tab). ✔ default
