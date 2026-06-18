# GitHub Screen — Phase 3 (Insights) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the **Insights** tab — four sections in one screen: Traffic (views/clones + popular paths/referrers, owner-only), Contributors, a commit-activity sparkline, and Milestones & Labels.

**Architecture:** Five new read-only Rust commands in `src-tauri/src/github/mod.rs`, each resolving owner/repo → `gh api` → `serde` → camelCase DTO via a pure mapping fn. Two real-world quirks are handled in the backend: the **Traffic** endpoints return HTTP 403 without push access (propagated as `GithubError::Forbidden`, which the UI turns into a "needs push access" state), and `stats/commit_activity` returns an **empty body (HTTP 202) while GitHub computes** (returned as `{ computing: true }`, retried client-side). The `githubState` controller gains five more `makePanel` instances + loaders; a single `GithubInsights.svelte` renders the four sections and a pure `sparklinePoints` helper draws the activity chart.

**Tech Stack:** Tauri 2 (Rust, `serde`/`serde_json`) · SvelteKit 5 / Svelte 5 runes · `gh` CLI · Vitest + `cargo test`.

**Spec:** `docs/superpowers/specs/2026-06-17-github-screen-design.md` §7 (Insights). Builds on Phases 1–2 (merged to `main`).

**Branch:** create `feat/github-screen-p3` off `main`.

> Verified `gh` facts (live gh 2.86): `/traffic/views` & `/traffic/clones` → `{count,uniques,views|clones:[{timestamp,count,uniques}]}`; `/traffic/popular/{paths,referrers}` → BARE arrays; all four need `repo` scope + push access (403 "Must have push access" otherwise; a 200 with `count:0` = no-traffic). `/contributors?per_page=N` → `[{login,contributions,avatar_url,html_url,type}]` pre-sorted desc. `/stats/commit_activity` → 52 weekly `{week(unix seconds),total,days[7]}`, may 202+empty-body while computing. `/milestones?state=open` → `[{title,number,state,open_issues,closed_issues,due_on,description,html_url}]`. `/labels` → `[{name,color,description,default}]`. Reuse Phase-1/2 helpers `resolve_owner_repo`, `run_gh`, `GithubError`, `RawLabel`/`map_label`, and the `GhLabel` TS type.

---

## File Structure

**Modified (Rust):** `src-tauri/src/github/mod.rs` (5 command fns + DTOs + maps + tests), `src-tauri/src/commands.rs` (5 wrappers), `src-tauri/src/lib.rs` (register 5).

**New (TS):** `src/lib/github/activity.ts` (+ `activity.test.ts`) — the pure `sparklinePoints` helper.

**Modified (TS):** `src/lib/types.ts` (insight types), `src/lib/api.ts` (5 wrappers), `src/lib/githubState.svelte.ts` (5 panels + loaders, activity-retry).

**New (Svelte):** `src/lib/components/github/GithubInsights.svelte`.

**Modified (Svelte):** `src/lib/components/github/GithubTabs.svelte` (+insights), `GithubView.svelte` (+insights branch).

---

## Task 1: Rust — Traffic

**Files:** Modify `src-tauri/src/github/mod.rs`, `commands.rs`, `lib.rs`.

- [ ] **Step 1: DTOs + `map_traffic` + `traffic` fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawTrafficPoint {
    timestamp: String,
    count: u64,
    uniques: u64,
}
#[derive(Deserialize)]
struct RawViews {
    count: u64,
    uniques: u64,
    #[serde(default)]
    views: Vec<RawTrafficPoint>,
}
#[derive(Deserialize)]
struct RawClones {
    count: u64,
    uniques: u64,
    #[serde(default)]
    clones: Vec<RawTrafficPoint>,
}
#[derive(Deserialize)]
struct RawPopularPath {
    path: String,
    title: String,
    count: u64,
    uniques: u64,
}
#[derive(Deserialize)]
struct RawReferrer {
    referrer: String,
    count: u64,
    uniques: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhTrafficPoint {
    pub timestamp: String,
    pub count: u64,
    pub uniques: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhSeries {
    pub count: u64,
    pub uniques: u64,
    pub points: Vec<GhTrafficPoint>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhPopularPath {
    pub path: String,
    pub title: String,
    pub count: u64,
    pub uniques: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhReferrer {
    pub referrer: String,
    pub count: u64,
    pub uniques: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhTraffic {
    pub views: GhSeries,
    pub clones: GhSeries,
    pub paths: Vec<GhPopularPath>,
    pub referrers: Vec<GhReferrer>,
}

fn point(p: RawTrafficPoint) -> GhTrafficPoint {
    GhTrafficPoint { timestamp: p.timestamp, count: p.count, uniques: p.uniques }
}

fn map_traffic(
    v: RawViews,
    c: RawClones,
    paths: Vec<RawPopularPath>,
    refs: Vec<RawReferrer>,
) -> GhTraffic {
    GhTraffic {
        views: GhSeries {
            count: v.count,
            uniques: v.uniques,
            points: v.views.into_iter().map(point).collect(),
        },
        clones: GhSeries {
            count: c.count,
            uniques: c.uniques,
            points: c.clones.into_iter().map(point).collect(),
        },
        paths: paths
            .into_iter()
            .map(|p| GhPopularPath { path: p.path, title: p.title, count: p.count, uniques: p.uniques })
            .collect(),
        referrers: refs
            .into_iter()
            .map(|r| GhReferrer { referrer: r.referrer, count: r.count, uniques: r.uniques })
            .collect(),
    }
}

/// Repository traffic (owner-only). The first call (views) decides access: a 403
/// surfaces as `GithubError::Forbidden`, which the UI renders as "needs push
/// access". A 200 with `count: 0` is the distinct no-traffic case.
pub fn traffic(repo: &Path) -> Result<GhTraffic, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let base = format!("repos/{owner}/{name}");
    let v: RawViews = serde_json::from_str(&run_gh(&["api", &format!("{base}/traffic/views")], None)?)
        .map_err(|e| GithubError::Other(format!("parse views: {e}")))?;
    let c: RawClones =
        serde_json::from_str(&run_gh(&["api", &format!("{base}/traffic/clones")], None)?)
            .map_err(|e| GithubError::Other(format!("parse clones: {e}")))?;
    let paths: Vec<RawPopularPath> =
        serde_json::from_str(&run_gh(&["api", &format!("{base}/traffic/popular/paths")], None)?)
            .map_err(|e| GithubError::Other(format!("parse paths: {e}")))?;
    let refs: Vec<RawReferrer> =
        serde_json::from_str(&run_gh(&["api", &format!("{base}/traffic/popular/referrers")], None)?)
            .map_err(|e| GithubError::Other(format!("parse referrers: {e}")))?;
    Ok(map_traffic(v, c, paths, refs))
}
```

- [ ] **Step 2: Mapping test.**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_traffic_shapes_series_and_lists() {
        let v: RawViews = serde_json::from_str(
            r#"{"count":1450,"uniques":300,"views":[{"timestamp":"2026-06-01T00:00:00Z","count":50,"uniques":12}]}"#,
        )
        .unwrap();
        let c: RawClones =
            serde_json::from_str(r#"{"count":42,"uniques":20,"clones":[]}"#).unwrap();
        let paths: Vec<RawPopularPath> = serde_json::from_str(
            r#"[{"path":"/cli/cli","title":"cli/cli","count":300,"uniques":120}]"#,
        )
        .unwrap();
        let refs: Vec<RawReferrer> =
            serde_json::from_str(r#"[{"referrer":"google.com","count":80,"uniques":40}]"#).unwrap();
        let t = map_traffic(v, c, paths, refs);
        assert_eq!(t.views.count, 1450);
        assert_eq!(t.views.points.len(), 1);
        assert_eq!(t.views.points[0].uniques, 12);
        assert!(t.clones.points.is_empty());
        assert_eq!(t.paths[0].path, "/cli/cli");
        assert_eq!(t.referrers[0].referrer, "google.com");
    }
```

- [ ] **Step 3: Command + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_traffic(repo: String) -> Result<github::GhTraffic, github::GithubError> {
    github::traffic(&PathBuf::from(repo))
}
```

`src-tauri/src/lib.rs` handler list:

```rust
    commands::github_traffic,
```

- [ ] **Step 4: Test.** Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github` — PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_traffic command (views/clones/paths/referrers, owner-only)"
```

---

## Task 2: Rust — Contributors

**Files:** Modify `src-tauri/src/github/mod.rs`, `commands.rs`, `lib.rs`.

- [ ] **Step 1: DTOs + map + fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawContributor {
    login: String,
    contributions: u64,
    #[serde(default)]
    avatar_url: String,
    #[serde(default)]
    html_url: String,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhContributor {
    pub login: String,
    pub contributions: u64,
    pub avatar_url: String,
    pub html_url: String,
    pub is_bot: bool,
}

fn map_contributor(c: RawContributor) -> GhContributor {
    GhContributor {
        login: c.login,
        contributions: c.contributions,
        avatar_url: c.avatar_url,
        html_url: c.html_url,
        is_bot: c.kind == "Bot",
    }
}

/// Top contributors (pre-sorted desc by GitHub). An empty repo can return a 204
/// with an empty body — treat that as an empty list.
pub fn contributors(repo: &Path, limit: u32) -> Result<Vec<GhContributor>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let per = limit.clamp(1, 100).to_string();
    let json = run_gh(
        &["api", &format!("repos/{owner}/{name}/contributors?per_page={per}")],
        None,
    )?;
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<RawContributor> = serde_json::from_str(&json)
        .map_err(|e| GithubError::Other(format!("parse contributors: {e}")))?;
    Ok(raw.into_iter().map(map_contributor).collect())
}
```

- [ ] **Step 2: Test.**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_contributor_flags_bots() {
        let json = r#"[
            {"login":"mislav","contributions":2061,"avatar_url":"a","html_url":"h","type":"User"},
            {"login":"dependabot[bot]","contributions":5,"type":"Bot"}
        ]"#;
        let raw: Vec<RawContributor> = serde_json::from_str(json).unwrap();
        let out: Vec<GhContributor> = raw.into_iter().map(map_contributor).collect();
        assert_eq!(out[0].login, "mislav");
        assert_eq!(out[0].contributions, 2061);
        assert!(!out[0].is_bot);
        assert!(out[1].is_bot);
    }
```

- [ ] **Step 3: Command + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_contributors(
    repo: String,
    limit: u32,
) -> Result<Vec<github::GhContributor>, github::GithubError> {
    github::contributors(&PathBuf::from(repo), limit)
}
```

`src-tauri/src/lib.rs`:

```rust
    commands::github_contributors,
```

- [ ] **Step 4: Test.** `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github` — PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_contributors command"
```

---

## Task 3: Rust — Commit activity (with 202-computing handling)

**Files:** Modify `src-tauri/src/github/mod.rs`, `commands.rs`, `lib.rs`.

- [ ] **Step 1: DTOs + fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawWeek {
    week: i64,
    total: u64,
    #[serde(default)]
    days: Vec<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhWeek {
    pub week: i64, // unix SECONDS
    pub total: u64,
    pub days: Vec<u64>,
}

/// 52 weeks of commit activity. GitHub returns HTTP 202 with an EMPTY body the
/// first time while it computes the stats; we surface that as `computing: true`
/// so the client can retry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhActivity {
    pub computing: bool,
    pub weeks: Vec<GhWeek>,
}

pub fn commit_activity(repo: &Path) -> Result<GhActivity, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let json = run_gh(&["api", &format!("repos/{owner}/{name}/stats/commit_activity")], None)?;
    if json.trim().is_empty() {
        // 202 Accepted with empty body → still computing.
        return Ok(GhActivity { computing: true, weeks: Vec::new() });
    }
    let raw: Vec<RawWeek> = serde_json::from_str(&json)
        .map_err(|e| GithubError::Other(format!("parse activity: {e}")))?;
    Ok(GhActivity {
        computing: false,
        weeks: raw
            .into_iter()
            .map(|w| GhWeek { week: w.week, total: w.total, days: w.days })
            .collect(),
    })
}
```

- [ ] **Step 2: Test (normal + computing).**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn commit_activity_parses_weeks() {
        let json = r#"[{"week":1750550400,"total":23,"days":[1,11,10,0,0,1,0]}]"#;
        let raw: Vec<RawWeek> = serde_json::from_str(json).unwrap();
        let weeks: Vec<GhWeek> = raw
            .into_iter()
            .map(|w| GhWeek { week: w.week, total: w.total, days: w.days })
            .collect();
        assert_eq!(weeks.len(), 1);
        assert_eq!(weeks[0].week, 1750550400);
        assert_eq!(weeks[0].total, 23);
        assert_eq!(weeks[0].days.len(), 7);
    }
```

(The empty-body→`computing` branch is exercised by hand against a cold repo; the parse path is unit-tested above.)

- [ ] **Step 3: Command + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_activity(repo: String) -> Result<github::GhActivity, github::GithubError> {
    github::commit_activity(&PathBuf::from(repo))
}
```

`src-tauri/src/lib.rs`:

```rust
    commands::github_activity,
```

- [ ] **Step 4: Test.** `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github` — PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_activity command (commit_activity + 202-computing flag)"
```

---

## Task 4: Rust — Milestones + Labels

**Files:** Modify `src-tauri/src/github/mod.rs`, `commands.rs`, `lib.rs`.

- [ ] **Step 1: Milestone DTO + map + the two fns.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawMilestone {
    title: String,
    number: u64,
    state: String,
    open_issues: u64,
    closed_issues: u64,
    due_on: Option<String>,
    #[serde(default)]
    description: Option<String>,
    html_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhMilestone {
    pub title: String,
    pub number: u64,
    pub state: String,
    pub open_issues: u64,
    pub closed_issues: u64,
    pub due_on: Option<String>,
    pub description: Option<String>,
    pub html_url: String,
}

fn map_milestone(m: RawMilestone) -> GhMilestone {
    GhMilestone {
        title: m.title,
        number: m.number,
        state: m.state,
        open_issues: m.open_issues,
        closed_issues: m.closed_issues,
        due_on: m.due_on,
        description: m.description,
        html_url: m.html_url,
    }
}

/// Open milestones (issue subsystem). Empty `[]` when the repo uses none.
pub fn milestones(repo: &Path) -> Result<Vec<GhMilestone>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let json = run_gh(
        &["api", &format!("repos/{owner}/{name}/milestones?state=open&per_page=50")],
        None,
    )?;
    let raw: Vec<RawMilestone> = serde_json::from_str(&json)
        .map_err(|e| GithubError::Other(format!("parse milestones: {e}")))?;
    Ok(raw.into_iter().map(map_milestone).collect())
}

/// The repo's label catalog. Reuses `RawLabel`/`map_label` + the `GhLabel` type.
pub fn labels(repo: &Path) -> Result<Vec<GhLabel>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let json = run_gh(&["api", &format!("repos/{owner}/{name}/labels?per_page=100")], None)?;
    let raw: Vec<RawLabel> = serde_json::from_str(&json)
        .map_err(|e| GithubError::Other(format!("parse labels: {e}")))?;
    Ok(raw.into_iter().map(map_label).collect())
}
```

- [ ] **Step 2: Milestone test.**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_milestone_progress_fields() {
        let json = r#"[{
            "title":"next-candidate","number":17,"state":"open","open_issues":17,
            "closed_issues":606,"due_on":null,"description":"Candidates","html_url":"u"
        }]"#;
        let raw: Vec<RawMilestone> = serde_json::from_str(json).unwrap();
        let m = &raw.into_iter().map(map_milestone).collect::<Vec<_>>()[0];
        assert_eq!(m.title, "next-candidate");
        assert_eq!(m.open_issues, 17);
        assert_eq!(m.closed_issues, 606);
        assert_eq!(m.due_on, None);
    }
```

- [ ] **Step 3: Commands + register both.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_milestones(repo: String) -> Result<Vec<github::GhMilestone>, github::GithubError> {
    github::milestones(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_labels(repo: String) -> Result<Vec<github::GhLabel>, github::GithubError> {
    github::labels(&PathBuf::from(repo))
}
```

`src-tauri/src/lib.rs` handler list:

```rust
    commands::github_milestones,
    commands::github_labels,
```

- [ ] **Step 4: Full Rust test run.** `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test` — clean build; all pass (107 + new tests).

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_milestones + github_labels commands"
```

---

## Task 5: TS — types + api wrappers + sparkline helper

**Files:** Modify `src/lib/types.ts`, `src/lib/api.ts`; Create `src/lib/github/activity.ts`, `src/lib/github/activity.test.ts`.

- [ ] **Step 1: Write the sparkline test.**

Create `src/lib/github/activity.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { sparklinePoints } from "./activity";

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
```

- [ ] **Step 2: Run it — expect FAIL (module missing).**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github/activity`
Expected: FAIL — cannot resolve `./activity`.

- [ ] **Step 3: Implement the helper.**

Create `src/lib/github/activity.ts`:

```typescript
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
```

- [ ] **Step 4: Run it — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github/activity`
Expected: PASS.

- [ ] **Step 5: Add the types.**

Append to `src/lib/types.ts`:

```typescript
export type GhTrafficPoint = { timestamp: string; count: number; uniques: number };
export type GhSeries = { count: number; uniques: number; points: GhTrafficPoint[] };
export type GhPopularPath = { path: string; title: string; count: number; uniques: number };
export type GhReferrer = { referrer: string; count: number; uniques: number };
export type GhTraffic = {
  views: GhSeries;
  clones: GhSeries;
  paths: GhPopularPath[];
  referrers: GhReferrer[];
};

export type GhContributor = {
  login: string;
  contributions: number;
  avatarUrl: string;
  htmlUrl: string;
  isBot: boolean;
};

export type GhWeek = { week: number; total: number; days: number[] };
export type GhActivity = { computing: boolean; weeks: GhWeek[] };

export type GhMilestone = {
  title: string;
  number: number;
  state: string;
  openIssues: number;
  closedIssues: number;
  dueOn: string | null;
  description: string | null;
  htmlUrl: string;
};
```

- [ ] **Step 6: Add the api wrappers.**

In `src/lib/api.ts`, add `GhTraffic`, `GhContributor`, `GhActivity`, `GhMilestone`, `GhLabel` to the `import type { … } from "./types"` block (GhLabel already imported by Phase 2 — don't duplicate), then add near `githubRuns`:

```typescript
  githubTraffic: (repo: string) => invoke<GhTraffic>("github_traffic", { repo }),
  githubContributors: (repo: string, limit: number) =>
    invoke<GhContributor[]>("github_contributors", { repo, limit }),
  githubActivity: (repo: string) => invoke<GhActivity>("github_activity", { repo }),
  githubMilestones: (repo: string) => invoke<GhMilestone[]>("github_milestones", { repo }),
  githubLabels: (repo: string) => invoke<GhLabel[]>("github_labels", { repo }),
```

- [ ] **Step 7: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 8: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/github/activity.ts src/lib/github/activity.test.ts src/lib/types.ts src/lib/api.ts
git commit -m "feat(github): insight types, api wrappers, sparklinePoints helper"
```

---

## Task 6: TS — `githubState` insight panels

**Files:** Modify `src/lib/githubState.svelte.ts`.

- [ ] **Step 1: Extend the imports.**

In `src/lib/githubState.svelte.ts`, add the new types to the `import type { … } from "./types"` block:

```typescript
  GhTraffic,
  GhContributor,
  GhActivity,
  GhMilestone,
  GhLabel,
```

- [ ] **Step 2: Add the panels + loaders inside `makeGithubState`.**

After the existing `let reloadNonce = $state(0);` line, add:

```typescript
  const traffic = makePanel<GhTraffic>();
  const contributors = makePanel<GhContributor[]>();
  const activity = makePanel<GhActivity>();
  const milestones = makePanel<GhMilestone[]>();
  const labels = makePanel<GhLabel[]>();

  function loadTraffic(repo: string) {
    return traffic.load(`${repo}|${reloadNonce}`, () => api.githubTraffic(repo));
  }
  function loadContributors(repo: string) {
    return contributors.load(`${repo}|${reloadNonce}`, () => api.githubContributors(repo, 12));
  }
  function loadActivity(repo: string) {
    return activity.load(`${repo}|${reloadNonce}`, async () => {
      // /stats/commit_activity returns 202 + empty body while GitHub computes;
      // retry a few times before giving up and showing the "computing" state.
      let a = await api.githubActivity(repo);
      for (let tries = 0; a.computing && tries < 3; tries++) {
        await new Promise((r) => setTimeout(r, 1800));
        a = await api.githubActivity(repo);
      }
      return a;
    });
  }
  function loadMilestones(repo: string) {
    return milestones.load(`${repo}|${reloadNonce}`, () => api.githubMilestones(repo));
  }
  function loadLabels(repo: string) {
    return labels.load(`${repo}|${reloadNonce}`, () => api.githubLabels(repo));
  }
```

- [ ] **Step 3: Reset the new panels on repo switch.**

In `ensure(repo)`, extend the reset block (currently `pulls.reset(); issues.reset(); releases.reset(); runs.reset();`) to also reset the insight panels:

```typescript
    pulls.reset();
    issues.reset();
    releases.reset();
    runs.reset();
    traffic.reset();
    contributors.reset();
    activity.reset();
    milestones.reset();
    labels.reset();
```

- [ ] **Step 4: Expose the panels + loaders.**

In the returned object, after the `loadRuns,` line, add:

```typescript
    get traffic() {
      return traffic;
    },
    get contributors() {
      return contributors;
    },
    get activity() {
      return activity;
    },
    get milestones() {
      return milestones;
    },
    get labels() {
      return labels;
    },
    loadTraffic,
    loadContributors,
    loadActivity,
    loadMilestones,
    loadLabels,
```

- [ ] **Step 5: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 6: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/githubState.svelte.ts
git commit -m "feat(github): insight panels in githubState (traffic/contributors/activity/milestones/labels)"
```

---

## Task 7: Svelte — the Insights tab

**Files:** Create `src/lib/components/github/GithubInsights.svelte`.

- [ ] **Step 1: Build the component.**

Create `src/lib/components/github/GithubInsights.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { formatCompact } from "../../github/format";
  import { sparklinePoints } from "../../github/activity";

  // Load all four insight panels when the tab mounts (and on Refresh).
  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) {
      void githubState.loadTraffic(repo);
      void githubState.loadContributors(repo);
      void githubState.loadActivity(repo);
      void githubState.loadMilestones(repo);
      void githubState.loadLabels(repo);
    }
  });

  const traffic = $derived(githubState.traffic);
  const contributors = $derived(githubState.contributors);
  const activity = $derived(githubState.activity);
  const milestones = $derived(githubState.milestones);
  const labels = $derived(githubState.labels);

  const weekTotals = $derived(activity.data ? activity.data.weeks.map((w) => w.total) : []);
  const peakWeek = $derived(weekTotals.length ? Math.max(...weekTotals) : 0);
  const yearTotal = $derived(weekTotals.reduce((s, n) => s + n, 0));
  function initials(login: string): string {
    return login.slice(0, 2).toUpperCase();
  }
</script>

<div class="insights">
  <!-- Traffic (owner-only) -->
  <section class="card">
    <h3>Traffic <span class="hint">last 14 days</span></h3>
    {#if traffic.status === "loading" && !traffic.data}
      <p class="note">Loading…</p>
    {:else if traffic.status === "error"}
      {#if traffic.error?.kind === "Forbidden"}
        <p class="note">Traffic is only available for repositories you have push access to.</p>
      {:else}
        <p class="note err">Could not load traffic ({traffic.error?.kind}).</p>
      {/if}
    {:else if traffic.data}
      <div class="metrics">
        <div class="metric"><span class="big">{formatCompact(traffic.data.views.count)}</span><span class="lbl">Views · {formatCompact(traffic.data.views.uniques)} unique</span></div>
        <div class="metric"><span class="big">{formatCompact(traffic.data.clones.count)}</span><span class="lbl">Clones · {formatCompact(traffic.data.clones.uniques)} unique</span></div>
      </div>
      {#if traffic.data.views.count === 0 && traffic.data.clones.count === 0}
        <p class="note">No traffic in the last 14 days.</p>
      {/if}
      {#if traffic.data.paths.length}
        <h4>Popular content</h4>
        <ul class="mini">
          {#each traffic.data.paths.slice(0, 6) as p (p.path)}
            <li><span class="t">{p.title || p.path}</span><span class="n">{formatCompact(p.count)}</span></li>
          {/each}
        </ul>
      {/if}
      {#if traffic.data.referrers.length}
        <h4>Referrers</h4>
        <ul class="mini">
          {#each traffic.data.referrers.slice(0, 6) as r (r.referrer)}
            <li><span class="t">{r.referrer}</span><span class="n">{formatCompact(r.count)}</span></li>
          {/each}
        </ul>
      {/if}
    {/if}
  </section>

  <!-- Commit activity -->
  <section class="card">
    <h3>Commit activity <span class="hint">52 weeks</span></h3>
    {#if activity.status === "loading" && !activity.data}
      <p class="note">Loading…</p>
    {:else if activity.status === "error"}
      <p class="note err">Could not load activity ({activity.error?.kind}).</p>
    {:else if activity.data?.computing}
      <p class="note">GitHub is still computing these stats — try Refresh in a moment.</p>
    {:else if activity.data}
      <svg class="spark" viewBox="0 0 300 48" preserveAspectRatio="none" role="img" aria-label="Commits per week">
        <polyline points={sparklinePoints(weekTotals, 300, 44)} fill="none" stroke="var(--accent)" stroke-width="1.5" />
      </svg>
      <p class="note">{yearTotal} commits in the last year · peak {peakWeek}/week</p>
    {/if}
  </section>

  <!-- Contributors -->
  <section class="card">
    <h3>Top contributors</h3>
    {#if contributors.status === "loading" && !contributors.data}
      <p class="note">Loading…</p>
    {:else if contributors.status === "error"}
      <p class="note err">Could not load contributors ({contributors.error?.kind}).</p>
    {:else if contributors.data && contributors.data.length === 0}
      <p class="note">No contributors.</p>
    {:else if contributors.data}
      <ul class="contribs">
        {#each contributors.data as c (c.login)}
          <li>
            <a href={c.htmlUrl} target="_blank" rel="noreferrer">
              <span class="av" aria-hidden="true">{initials(c.login)}</span>
              <span class="login">{c.login}</span>
            </a>
            <span class="count">{formatCompact(c.contributions)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- Milestones & labels -->
  <section class="card">
    <h3>Milestones &amp; labels</h3>
    {#if milestones.status === "ok" || labels.status === "ok"}
      {#if milestones.data && milestones.data.length}
        <ul class="miles">
          {#each milestones.data as m (m.number)}
            {@const total = m.openIssues + m.closedIssues}
            <li>
              <a href={m.htmlUrl} target="_blank" rel="noreferrer">{m.title}</a>
              <div class="bar"><div class="fill" style={`width:${total ? (m.closedIssues / total) * 100 : 0}%`}></div></div>
              <span class="prog">{m.closedIssues}/{total}</span>
            </li>
          {/each}
        </ul>
      {:else if milestones.status === "ok"}
        <p class="note">No open milestones.</p>
      {/if}
      {#if labels.data && labels.data.length}
        <div class="labels">
          {#each labels.data as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
        </div>
      {/if}
    {:else if milestones.status === "error" || labels.status === "error"}
      <p class="note err">Could not load milestones/labels.</p>
    {:else}
      <p class="note">Loading…</p>
    {/if}
  </section>
</div>

<style>
  .insights {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 14px;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px;
    background: var(--panel-bg);
  }
  h3 {
    margin: 0 0 10px 0;
    font-size: 13px;
  }
  h4 {
    margin: 12px 0 4px 0;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hint {
    font-weight: 400;
    color: var(--text-muted);
    font-size: 11px;
  }
  .metrics {
    display: flex;
    gap: 18px;
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
  .spark {
    width: 100%;
    height: 48px;
  }
  .mini {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .mini li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    padding: 2px 0;
  }
  .mini .t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .mini .n {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .contribs {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .contribs li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 0;
  }
  .contribs a {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
    text-decoration: none;
  }
  .contribs a:hover .login {
    color: var(--accent);
  }
  .av {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--row-selected);
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 600;
  }
  .login {
    font-size: 12.5px;
  }
  .count {
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .miles {
    list-style: none;
    margin: 0 0 10px 0;
    padding: 0;
  }
  .miles li {
    display: grid;
    grid-template-columns: 1fr 80px auto;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
    font-size: 12.5px;
  }
  .miles a {
    color: var(--text);
    text-decoration: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .miles a:hover {
    color: var(--accent);
  }
  .bar {
    height: 6px;
    background: var(--btn-bg);
    border-radius: 999px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
  }
  .prog {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .label {
    font-size: 10.5px;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid var(--lc);
    color: var(--lc);
  }
  .note {
    margin: 8px 2px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
```

- [ ] **Step 2: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubInsights.svelte
git commit -m "feat(github): Insights tab (traffic, activity sparkline, contributors, milestones/labels)"
```

---

## Task 8: Svelte — wire Insights into the bar + view

**Files:** Modify `src/lib/components/github/GithubTabs.svelte`, `GithubView.svelte`.

- [ ] **Step 1: Add the tab entry.**

In `src/lib/components/github/GithubTabs.svelte`, append `{ id: "insights", label: "Insights" }` to the `TABS` array so it reads:

```typescript
  const TABS: { id: GithubTab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "pulls", label: "Pull Requests" },
    { id: "issues", label: "Issues" },
    { id: "releases", label: "Releases" },
    { id: "actions", label: "Actions" },
    { id: "insights", label: "Insights" },
  ];
```

- [ ] **Step 2: Add the render branch.**

In `src/lib/components/github/GithubView.svelte`, add the import (with the other tab imports):

```typescript
  import GithubInsights from "./GithubInsights.svelte";
```

Then add a branch after the `actions` one (before the closing `{/if}` of the active-tab block):

```svelte
    {:else if githubState.activeTab === "insights"}
      <GithubInsights />
```

- [ ] **Step 3: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubTabs.svelte src/lib/components/github/GithubView.svelte
git commit -m "feat(github): wire the Insights tab into the screen"
```

---

## Task 9: Full verification + smoke

**Files:** none.

- [ ] **Step 1: Gates.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run check
npx vitest run
cd src-tauri && cargo test
```
Expected: svelte-check 0/0 · vitest all pass (+ the new `activity` suite) · cargo all pass (+ the new mapping tests).

- [ ] **Step 2: Build + smoke.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run tauri build
```
Open the app, open the **Insights** tab:
- On a repo you **own**: Traffic shows view/clone counts + popular paths/referrers; on a repo you only cloned, Traffic shows "only available for repositories you have push access to."
- Commit activity draws a sparkline with the 1-year total + peak; on a never-fetched big repo it may briefly show "still computing" then fill in on Refresh.
- Top contributors list with counts; clicking opens the profile.
- Open milestones (if any) with progress bars; the label catalog as colored chips.

- [ ] **Step 3: Proceed to review + merge.**

---

## Done criteria (Phase 3)

- The Insights tab renders all four sections from live `gh`, with the traffic-403 "needs push access" state and the commit-activity "computing" retry handled.
- Gates green; new pure logic (`map_traffic`, `map_contributor`, `map_milestone`, commit-activity parse, `sparklinePoints`) is unit-tested.
- Next: **Phase 4 (the four guarded write actions)** — the final phase.

## After all tasks

Adversarial review of the branch diff (`superpowers:code-reviewer`), fix Critical/Important, then **superpowers:finishing-a-development-branch** to ff-merge `feat/github-screen-p3` → `main`.
