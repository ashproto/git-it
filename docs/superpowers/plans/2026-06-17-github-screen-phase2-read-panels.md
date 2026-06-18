# GitHub Screen — Phase 2 (Read Panels) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the four read-only data tabs — **Pull Requests · Issues · Releases · Actions** — to the GitHub screen: each fetches via `gh`, renders a filterable list with per-item "open on github.com" links, and is lazily loaded + cached per repo/filter.

**Architecture:** Four new Rust commands in the existing `src-tauri/src/github/mod.rs` (each: resolve owner/repo → `gh` → `serde` parse → camelCase DTO, via a pure mapping fn that's unit-tested against real captured payloads). The `githubState` controller gains a small generic per-panel cache (`makePanel<T>()`) with repo+filter keys and a refresh nonce. Four self-contained Svelte components load their panel on mount and render loading/error/empty/list states; `GithubTabs` and `GithubView` grow four entries/branches.

**Tech Stack:** Tauri 2 (Rust, `serde`/`serde_json`) · SvelteKit 5 / Svelte 5 runes · `gh` CLI · Vitest + `cargo test`.

**Spec:** `docs/superpowers/specs/2026-06-17-github-screen-design.md` §7 (Pull Requests / Issues / Releases / Actions). Builds on Phase 1 (`docs/superpowers/plans/2026-06-17-github-screen-phase1-foundation.md`, merged to `main`).

**Branch:** create `feat/github-screen-p2` off `main`.

> All `gh` commands/JSON fields below were verified against live gh 2.86. Reuse the Phase-1 helpers already in `github/mod.rs`: `resolve_owner_repo`, `run_gh`, `GithubError`. Scope (read-only): **no write actions** (merge/comment/close are Phase 4) — every item just deep-links out via an `<a href target="_blank">` (the pattern Phase 1's `GithubHeader` already uses).

---

## File Structure

**Modified (Rust):**
- `src-tauri/src/github/mod.rs` — add `GhLabel`, `GhPull`/`map_pull`/`pulls`, `GhIssue`/`map_issue`/`issues`, `GhRelease`/`map_release`/`releases`, `GhRun`/`map_run`/`runs`, plus a `validate_state` helper and their `#[cfg(test)]` mapping tests.
- `src-tauri/src/commands.rs` — four command wrappers.
- `src-tauri/src/lib.rs` — register the four commands.

**Modified (TS):**
- `src/lib/types.ts` — `GhLabel`, `GhPull`, `GhIssue`, `GhRelease`, `GhRun`, `PullStateFilter`, `IssueStateFilter`.
- `src/lib/api.ts` — `githubPulls`, `githubIssues`, `githubReleases`, `githubRuns`.
- `src/lib/githubState.svelte.ts` — `makePanel<T>()` + four panels + filters + loaders + reset-on-repo-switch + refresh nonce.

**New (Svelte):**
- `src/lib/components/github/GithubPulls.svelte`, `GithubIssues.svelte`, `GithubReleases.svelte`, `GithubActions.svelte`.

**Modified (Svelte):**
- `src/lib/components/github/GithubTabs.svelte` — add the four tab entries.
- `src/lib/components/github/GithubView.svelte` — add the four render branches.

---

## Task 1: Rust — Pull Requests

**Files:** Modify `src-tauri/src/github/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Add the `validate_state` helper, DTOs, mapping, and command fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
/// Validate a list-state filter against an allowlist before handing it to `gh`
/// (defense-in-depth; the frontend already sends a typed union). Returns the
/// matched `&'static str` so we never forward an arbitrary string.
fn validate_state(state: &str, allowed: &[&'static str]) -> Result<&'static str, GithubError> {
    allowed
        .iter()
        .find(|a| **a == state)
        .copied()
        .ok_or_else(|| GithubError::Other(format!("invalid state filter: {state}")))
}

#[derive(Deserialize)]
struct RawUser {
    #[serde(default)]
    login: String,
}

#[derive(Deserialize)]
struct RawLabel {
    name: String,
    #[serde(default)]
    color: String,
}

/// A label chip (name + 6-hex color, no leading '#'). Serialized camelCase.
#[derive(Debug, Clone, Serialize)]
pub struct GhLabel {
    pub name: String,
    pub color: String,
}

#[derive(Deserialize)]
struct RawPull {
    number: u64,
    title: String,
    author: Option<RawUser>,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "baseRefName")]
    base_ref_name: String,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(rename = "reviewDecision", default)]
    review_decision: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    state: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhPull {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub head_ref_name: String,
    pub base_ref_name: String,
    pub labels: Vec<GhLabel>,
    pub review_decision: String, // "" | REVIEW_REQUIRED | APPROVED | CHANGES_REQUESTED
    pub is_draft: bool,
    pub state: String, // OPEN | CLOSED | MERGED
    pub updated_at: String,
    pub url: String,
}

fn map_label(l: RawLabel) -> GhLabel {
    GhLabel { name: l.name, color: l.color }
}

fn map_pull(p: RawPull) -> GhPull {
    GhPull {
        number: p.number,
        title: p.title,
        author: p.author.map(|a| a.login).unwrap_or_default(),
        head_ref_name: p.head_ref_name,
        base_ref_name: p.base_ref_name,
        labels: p.labels.into_iter().map(map_label).collect(),
        review_decision: p.review_decision,
        is_draft: p.is_draft,
        state: p.state,
        updated_at: p.updated_at,
        url: p.url,
    }
}

/// List pull requests via `gh pr list`. `state` ∈ {open,closed,merged,all}.
pub fn pulls(repo: &Path, state: &str, limit: u32) -> Result<Vec<GhPull>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let st = validate_state(state, &["open", "closed", "merged", "all"])?;
    let slug = format!("{owner}/{name}");
    let limit_s = limit.clamp(1, 200).to_string();
    let json = run_gh(
        &[
            "pr", "list", "--repo", &slug, "--state", st, "--limit", &limit_s,
            "--json",
            "number,title,author,headRefName,baseRefName,labels,reviewDecision,isDraft,state,updatedAt,url",
        ],
        None,
    )?;
    let raw: Vec<RawPull> =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse pulls: {e}")))?;
    Ok(raw.into_iter().map(map_pull).collect())
}
```

- [ ] **Step 2: Add the mapping test (real cli/cli shape).**

Add inside the existing `#[cfg(test)] mod tests` block in `src-tauri/src/github/mod.rs`:

```rust
    #[test]
    fn map_pull_flattens_author_and_labels() {
        let json = r#"[{
            "number":13675,"title":"Fix flaky test",
            "author":{"login":"pdostal","is_bot":false},
            "headRefName":"fix/race","baseRefName":"trunk",
            "labels":[{"name":"bug","color":"D6393F","id":"x"}],
            "reviewDecision":"REVIEW_REQUIRED","isDraft":false,"state":"OPEN",
            "updatedAt":"2026-06-17T14:06:53Z","url":"https://github.com/cli/cli/pull/13675"
        }]"#;
        let raw: Vec<RawPull> = serde_json::from_str(json).unwrap();
        let out: Vec<GhPull> = raw.into_iter().map(map_pull).collect();
        assert_eq!(out.len(), 1);
        let p = &out[0];
        assert_eq!(p.number, 13675);
        assert_eq!(p.author, "pdostal");
        assert_eq!(p.base_ref_name, "trunk");
        assert_eq!(p.labels.len(), 1);
        assert_eq!(p.labels[0].name, "bug");
        assert_eq!(p.review_decision, "REVIEW_REQUIRED");
        assert!(!p.is_draft);
    }

    #[test]
    fn map_pull_tolerates_missing_author_and_empty_labels() {
        let json = r#"[{
            "number":1,"title":"t","author":null,"headRefName":"h","baseRefName":"main",
            "labels":[],"reviewDecision":"","isDraft":true,"state":"OPEN",
            "updatedAt":"2026-01-01T00:00:00Z","url":"u"
        }]"#;
        let raw: Vec<RawPull> = serde_json::from_str(json).unwrap();
        let p = &raw.into_iter().map(map_pull).collect::<Vec<_>>()[0];
        assert_eq!(p.author, "");
        assert!(p.labels.is_empty());
        assert_eq!(p.review_decision, "");
        assert!(p.is_draft);
    }

    #[test]
    fn validate_state_rejects_unknown() {
        assert!(validate_state("open", &["open", "all"]).is_ok());
        assert!(validate_state("bogus", &["open", "all"]).is_err());
    }
```

- [ ] **Step 3: Add the command + register it.**

In `src-tauri/src/commands.rs` add:

```rust
#[tauri::command]
pub fn github_pulls(
    repo: String,
    state: String,
    limit: u32,
) -> Result<Vec<github::GhPull>, github::GithubError> {
    github::pulls(&PathBuf::from(repo), &state, limit)
}
```

In `src-tauri/src/lib.rs` add to the `tauri::generate_handler![ … ]` list:

```rust
    commands::github_pulls,
```

- [ ] **Step 4: Build + test.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github`
Expected: builds clean; the three new tests + existing github tests PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_pulls command (gh pr list → GhPull)"
```

---

## Task 2: Rust — Issues

**Files:** Modify `src-tauri/src/github/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Add DTOs, mapping, command fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawIssue {
    number: u64,
    title: String,
    author: Option<RawUser>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    assignees: Vec<RawUser>,
    state: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub labels: Vec<GhLabel>,
    pub assignees: Vec<String>,
    pub state: String, // OPEN | CLOSED
    pub updated_at: String,
    pub url: String,
}

fn map_issue(i: RawIssue) -> GhIssue {
    GhIssue {
        number: i.number,
        title: i.title,
        author: i.author.map(|a| a.login).unwrap_or_default(),
        labels: i.labels.into_iter().map(map_label).collect(),
        assignees: i.assignees.into_iter().map(|a| a.login).collect(),
        state: i.state,
        updated_at: i.updated_at,
        url: i.url,
    }
}

/// List issues via `gh issue list` (which EXCLUDES PRs). `state` ∈ {open,closed,all}.
pub fn issues(repo: &Path, state: &str, limit: u32) -> Result<Vec<GhIssue>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let st = validate_state(state, &["open", "closed", "all"])?;
    let slug = format!("{owner}/{name}");
    let limit_s = limit.clamp(1, 200).to_string();
    let json = run_gh(
        &[
            "issue", "list", "--repo", &slug, "--state", st, "--limit", &limit_s,
            "--json", "number,title,author,labels,assignees,state,updatedAt,url",
        ],
        None,
    )?;
    let raw: Vec<RawIssue> =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse issues: {e}")))?;
    Ok(raw.into_iter().map(map_issue).collect())
}
```

- [ ] **Step 2: Add the mapping test.**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_issue_flattens_users() {
        let json = r#"[{
            "number":13676,"title":"honor SSL_CERT_FILE",
            "author":{"login":"davidlovas","name":"David Lovas"},
            "labels":[{"name":"needs-triage","color":"D6393F"}],
            "assignees":[{"login":"mislav"}],"state":"OPEN",
            "updatedAt":"2026-06-17T20:35:50Z","url":"https://github.com/cli/cli/issues/13676"
        }]"#;
        let raw: Vec<RawIssue> = serde_json::from_str(json).unwrap();
        let i = &raw.into_iter().map(map_issue).collect::<Vec<_>>()[0];
        assert_eq!(i.number, 13676);
        assert_eq!(i.author, "davidlovas");
        assert_eq!(i.labels[0].color, "D6393F");
        assert_eq!(i.assignees, vec!["mislav".to_string()]);
        assert_eq!(i.state, "OPEN");
    }
```

- [ ] **Step 3: Command + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_issues(
    repo: String,
    state: String,
    limit: u32,
) -> Result<Vec<github::GhIssue>, github::GithubError> {
    github::issues(&PathBuf::from(repo), &state, limit)
}
```

`src-tauri/src/lib.rs` handler list:

```rust
    commands::github_issues,
```

- [ ] **Step 4: Test.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github`
Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_issues command (gh issue list → GhIssue)"
```

---

## Task 3: Rust — Releases (with download analytics)

**Files:** Modify `src-tauri/src/github/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Add DTOs, mapping (computes total downloads), command fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawAsset {
    name: String,
    size: u64,
    download_count: u64,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhAsset {
    pub name: String,
    pub size: u64,
    pub download_count: u64,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhRelease {
    pub tag_name: String,
    pub name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub html_url: String,
    pub assets: Vec<GhAsset>,
    pub total_downloads: u64,
}

fn map_release(r: RawRelease) -> GhRelease {
    let assets: Vec<GhAsset> = r
        .assets
        .into_iter()
        .map(|a| GhAsset {
            name: a.name,
            size: a.size,
            download_count: a.download_count,
            download_url: a.browser_download_url,
        })
        .collect();
    let total_downloads = assets.iter().map(|a| a.download_count).sum();
    GhRelease {
        tag_name: r.tag_name,
        name: r.name.unwrap_or_default(),
        draft: r.draft,
        prerelease: r.prerelease,
        published_at: r.published_at,
        html_url: r.html_url,
        assets,
        total_downloads,
    }
}

/// List releases via the REST API (the only path exposing per-asset
/// `download_count` — there is no time series). Up to 30 most-recent.
pub fn releases(repo: &Path) -> Result<Vec<GhRelease>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("repos/{owner}/{name}/releases?per_page=30");
    let json = run_gh(&["api", &slug], None)?;
    let raw: Vec<RawRelease> = serde_json::from_str(&json)
        .map_err(|e| GithubError::Other(format!("parse releases: {e}")))?;
    Ok(raw.into_iter().map(map_release).collect())
}
```

- [ ] **Step 2: Add the mapping test (real cli/cli asset shape).**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_release_sums_downloads() {
        let json = r#"[{
            "tag_name":"v2.95.0","name":"GitHub CLI 2.95.0","draft":false,"prerelease":false,
            "published_at":"2026-06-17T19:55:07Z","html_url":"https://github.com/cli/cli/releases/tag/v2.95.0",
            "assets":[
              {"name":"a.zip","size":1950,"download_count":467,"browser_download_url":"https://x/a.zip"},
              {"name":"b.zip","size":10,"download_count":33,"browser_download_url":"https://x/b.zip"}
            ]
        }]"#;
        let raw: Vec<RawRelease> = serde_json::from_str(json).unwrap();
        let r = &raw.into_iter().map(map_release).collect::<Vec<_>>()[0];
        assert_eq!(r.tag_name, "v2.95.0");
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.assets[0].download_count, 467);
        assert_eq!(r.total_downloads, 500);
    }

    #[test]
    fn map_release_handles_no_assets_and_missing_name() {
        let json = r#"[{
            "tag_name":"v1","draft":false,"prerelease":true,"published_at":null,
            "html_url":"u","assets":[]
        }]"#;
        let raw: Vec<RawRelease> = serde_json::from_str(json).unwrap();
        let r = &raw.into_iter().map(map_release).collect::<Vec<_>>()[0];
        assert_eq!(r.name, "");
        assert_eq!(r.total_downloads, 0);
        assert!(r.prerelease);
        assert_eq!(r.published_at, None);
    }
```

- [ ] **Step 3: Command + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_releases(repo: String) -> Result<Vec<github::GhRelease>, github::GithubError> {
    github::releases(&PathBuf::from(repo))
}
```

`src-tauri/src/lib.rs` handler list:

```rust
    commands::github_releases,
```

- [ ] **Step 4: Test.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github`
Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_releases command (REST releases + per-asset downloads)"
```

---

## Task 4: Rust — Actions (workflow runs)

**Files:** Modify `src-tauri/src/github/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Add DTOs, mapping, command fn.**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct RawRun {
    #[serde(rename = "databaseId")]
    database_id: u64,
    #[serde(rename = "displayTitle")]
    display_title: String,
    #[serde(rename = "workflowName")]
    workflow_name: String,
    #[serde(rename = "headBranch")]
    head_branch: String,
    event: String,
    status: String,
    conclusion: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhRun {
    pub id: u64,
    pub title: String,
    pub workflow_name: String,
    pub head_branch: String,
    pub event: String,
    pub status: String,     // queued | in_progress | completed
    pub conclusion: String, // "" until completed: success | failure | cancelled | …
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

fn map_run(r: RawRun) -> GhRun {
    GhRun {
        id: r.database_id,
        title: r.display_title,
        workflow_name: r.workflow_name,
        head_branch: r.head_branch,
        event: r.event,
        status: r.status,
        conclusion: r.conclusion,
        created_at: r.created_at,
        updated_at: r.updated_at,
        url: r.url,
    }
}

/// List recent workflow runs via `gh run list` (newest first).
pub fn runs(repo: &Path, limit: u32) -> Result<Vec<GhRun>, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let limit_s = limit.clamp(1, 100).to_string();
    let json = run_gh(
        &[
            "run", "list", "--repo", &slug, "--limit", &limit_s,
            "--json",
            "databaseId,displayTitle,workflowName,headBranch,event,status,conclusion,createdAt,updatedAt,url",
        ],
        None,
    )?;
    let raw: Vec<RawRun> =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse runs: {e}")))?;
    Ok(raw.into_iter().map(map_run).collect())
}
```

- [ ] **Step 2: Add the mapping test (real cli/cli run shape).**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_run_renames_database_id_and_keeps_conclusion() {
        let json = r#"[{
            "databaseId":27736972379,"displayTitle":"Bump Go","workflowName":"Bump Go",
            "headBranch":"trunk","event":"schedule","status":"completed","conclusion":"success",
            "createdAt":"2026-06-18T04:35:20Z","updatedAt":"2026-06-18T04:35:44Z",
            "url":"https://github.com/cli/cli/actions/runs/27736972379"
        }]"#;
        let raw: Vec<RawRun> = serde_json::from_str(json).unwrap();
        let r = &raw.into_iter().map(map_run).collect::<Vec<_>>()[0];
        assert_eq!(r.id, 27736972379);
        assert_eq!(r.title, "Bump Go");
        assert_eq!(r.status, "completed");
        assert_eq!(r.conclusion, "success");
    }

    #[test]
    fn map_run_running_has_empty_conclusion() {
        let json = r#"[{
            "databaseId":1,"displayTitle":"t","workflowName":"CI","headBranch":"main",
            "event":"push","status":"in_progress","conclusion":"",
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","url":"u"
        }]"#;
        let raw: Vec<RawRun> = serde_json::from_str(json).unwrap();
        let r = &raw.into_iter().map(map_run).collect::<Vec<_>>()[0];
        assert_eq!(r.status, "in_progress");
        assert_eq!(r.conclusion, "");
    }
```

- [ ] **Step 3: Command + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_runs(repo: String, limit: u32) -> Result<Vec<github::GhRun>, github::GithubError> {
    github::runs(&PathBuf::from(repo), limit)
}
```

`src-tauri/src/lib.rs` handler list:

```rust
    commands::github_runs,
```

- [ ] **Step 4: Build + full Rust test run.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test`
Expected: clean build; all tests PASS (99 from Phase 1 + the new mapping tests).

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_runs command (gh run list → GhRun)"
```

---

## Task 5: TS — types + api wrappers

**Files:** Modify `src/lib/types.ts`, `src/lib/api.ts`.

- [ ] **Step 1: Add the types.**

Append to `src/lib/types.ts`:

```typescript
export type GhLabel = { name: string; color: string };

export type PullStateFilter = "open" | "closed" | "merged" | "all";
export type IssueStateFilter = "open" | "closed" | "all";

export type GhPull = {
  number: number;
  title: string;
  author: string;
  headRefName: string;
  baseRefName: string;
  labels: GhLabel[];
  reviewDecision: "" | "REVIEW_REQUIRED" | "APPROVED" | "CHANGES_REQUESTED";
  isDraft: boolean;
  state: "OPEN" | "CLOSED" | "MERGED";
  updatedAt: string;
  url: string;
};

export type GhIssue = {
  number: number;
  title: string;
  author: string;
  labels: GhLabel[];
  assignees: string[];
  state: "OPEN" | "CLOSED";
  updatedAt: string;
  url: string;
};

export type GhAsset = { name: string; size: number; downloadCount: number; downloadUrl: string };

export type GhRelease = {
  tagName: string;
  name: string;
  draft: boolean;
  prerelease: boolean;
  publishedAt: string | null;
  htmlUrl: string;
  assets: GhAsset[];
  totalDownloads: number;
};

export type GhRun = {
  id: number;
  title: string;
  workflowName: string;
  headBranch: string;
  event: string;
  status: string;
  conclusion: string;
  createdAt: string;
  updatedAt: string;
  url: string;
};
```

- [ ] **Step 2: Add the api wrappers.**

In `src/lib/api.ts`, add `GhPull`, `GhIssue`, `GhRelease`, `GhRun` (and `PullStateFilter`, `IssueStateFilter`) to the `import type { … } from "./types"` block, then add these wrappers near `githubRepoStats`:

```typescript
  githubPulls: (repo: string, state: PullStateFilter, limit: number) =>
    invoke<GhPull[]>("github_pulls", { repo, state, limit }),
  githubIssues: (repo: string, state: IssueStateFilter, limit: number) =>
    invoke<GhIssue[]>("github_issues", { repo, state, limit }),
  githubReleases: (repo: string) => invoke<GhRelease[]>("github_releases", { repo }),
  githubRuns: (repo: string, limit: number) => invoke<GhRun[]>("github_runs", { repo, limit }),
```

- [ ] **Step 3: Type-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/types.ts src/lib/api.ts
git commit -m "feat(github): TS types + api wrappers for pulls/issues/releases/runs"
```

---

## Task 6: TS — `githubState` per-panel cache

**Files:** Modify `src/lib/githubState.svelte.ts`.

- [ ] **Step 1: Add the `makePanel` factory + panel state + loaders.**

In `src/lib/githubState.svelte.ts`, update the imports at the top to add the new types:

```typescript
import type {
  GhAvailability,
  GhRepoStats,
  GithubError,
  GhPull,
  GhIssue,
  GhRelease,
  GhRun,
  PullStateFilter,
  IssueStateFilter,
} from "./types";
```

Add this factory ABOVE `function makeGithubState() {`:

```typescript
export type PanelStatus = "idle" | "loading" | "ok" | "error";

/** A reactive cache for one lazily-loaded panel. `load(key, fetcher)` is a no-op
 *  when the same `key` (repo + filter + refresh-nonce) is already loaded/loading,
 *  and discards a stale in-flight result if a newer `key` superseded it. */
function makePanel<T>() {
  let status = $state<PanelStatus>("idle");
  let data = $state<T | null>(null);
  let error = $state<GithubError | null>(null);
  let key: string | null = null;
  return {
    get status() {
      return status;
    },
    get data() {
      return data;
    },
    get error() {
      return error;
    },
    reset() {
      key = null;
      status = "idle";
      data = null;
      error = null;
    },
    async load(k: string, fetcher: () => Promise<T>) {
      if (key === k && (status === "ok" || status === "loading")) return;
      key = k;
      status = "loading";
      error = null;
      try {
        const d = await fetcher();
        if (key !== k) return; // superseded by a newer load
        data = d;
        status = "ok";
      } catch (e) {
        if (key !== k) return;
        error = e as GithubError;
        status = "error";
      }
    },
  };
}
```

- [ ] **Step 2: Wire the panels into `makeGithubState`.**

Inside `makeGithubState()`, after the existing `let loadedRepo: string | null = null;` line, add:

```typescript
  const pulls = makePanel<GhPull[]>();
  const issues = makePanel<GhIssue[]>();
  const releases = makePanel<GhRelease[]>();
  const runs = makePanel<GhRun[]>();
  let pullState = $state<PullStateFilter>("open");
  let issueState = $state<IssueStateFilter>("open");
  let reloadNonce = $state(0);

  function loadPulls(repo: string) {
    return pulls.load(`${repo}|${pullState}|${reloadNonce}`, () =>
      api.githubPulls(repo, pullState, 50),
    );
  }
  function loadIssues(repo: string) {
    return issues.load(`${repo}|${issueState}|${reloadNonce}`, () =>
      api.githubIssues(repo, issueState, 50),
    );
  }
  function loadReleases(repo: string) {
    return releases.load(`${repo}|${reloadNonce}`, () => api.githubReleases(repo));
  }
  function loadRuns(repo: string) {
    return runs.load(`${repo}|${reloadNonce}`, () => api.githubRuns(repo, 30));
  }
```

- [ ] **Step 3: Reset panels on repo switch.**

In `ensure(repo)`, the block that clears state on a new repo currently reads:

```typescript
    loadedRepo = repo;
    availability = null;
    stats = null;
    statsError = null;
    availLoading = true;
```

Add panel resets right after `loadedRepo = repo;`:

```typescript
    loadedRepo = repo;
    pulls.reset();
    issues.reset();
    releases.reset();
    runs.reset();
    availability = null;
    stats = null;
    statsError = null;
    availLoading = true;
```

- [ ] **Step 4: Bump the nonce on refresh + expose everything.**

Change the `refresh` method and add the new getters/setters to the returned object. Replace the existing:

```typescript
    setActiveTab(t: GithubTab) {
      activeTab = t;
    },
    ensure,
    refresh(repo: string) {
      loadedRepo = null;
      return ensure(repo);
    },
  };
```

with:

```typescript
    setActiveTab(t: GithubTab) {
      activeTab = t;
    },
    get pulls() {
      return pulls;
    },
    get issues() {
      return issues;
    },
    get releases() {
      return releases;
    },
    get runs() {
      return runs;
    },
    get pullState() {
      return pullState;
    },
    setPullState(s: PullStateFilter) {
      pullState = s;
    },
    get issueState() {
      return issueState;
    },
    setIssueState(s: IssueStateFilter) {
      issueState = s;
    },
    /** Read by each tab's load-effect so a Refresh re-runs them. */
    get reloadNonce() {
      return reloadNonce;
    },
    loadPulls,
    loadIssues,
    loadReleases,
    loadRuns,
    ensure,
    refresh(repo: string) {
      loadedRepo = null;
      reloadNonce++;
      return ensure(repo);
    },
  };
```

- [ ] **Step 5: Type-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 6: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/githubState.svelte.ts
git commit -m "feat(github): per-panel cache in githubState (pulls/issues/releases/runs)"
```

---

## Task 7: Svelte — Pull Requests + Issues tabs

**Files:** Create `src/lib/components/github/GithubPulls.svelte`, `src/lib/components/github/GithubIssues.svelte`.

- [ ] **Step 1: GithubPulls.**

Create `src/lib/components/github/GithubPulls.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { PullStateFilter } from "../../types";

  const FILTERS: PullStateFilter[] = ["open", "closed", "merged", "all"];

  // Load when the tab mounts, the filter changes, or Refresh bumps the nonce.
  $effect(() => {
    const repo = appState.repo;
    void githubState.pullState; // track
    void githubState.reloadNonce; // track (Refresh)
    if (repo) void githubState.loadPulls(repo);
  });

  const panel = $derived(githubState.pulls);
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.pullState === f} type="button" onclick={() => githubState.setPullState(f)}>{f}</button>
  {/each}
</div>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading pull requests…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load pull requests ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No {githubState.pullState === "all" ? "" : githubState.pullState} pull requests.</p>
{:else if panel.data}
  <ul class="list">
    {#each panel.data as pr (pr.number)}
      <li class="row">
        <a class="title" href={pr.url} target="_blank" rel="noreferrer">
          <span class="num">#{pr.number}</span>{pr.title}
          {#if pr.isDraft}<span class="badge">draft</span>{/if}
        </a>
        <div class="meta">
          <span>{pr.author}</span>
          <span class="mono">{pr.headRefName} → {pr.baseRefName}</span>
          {#if pr.reviewDecision}<span class="rev {pr.reviewDecision}">{pr.reviewDecision.replace(/_/g, " ").toLowerCase()}</span>{/if}
          <span class="when">{rel(pr.updatedAt)}</span>
        </div>
        {#if pr.labels.length}
          <div class="labels">
            {#each pr.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .filters {
    display: flex;
    gap: 6px;
    margin-bottom: 10px;
  }
  .chip {
    padding: 3px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel-bg);
    color: var(--text-muted);
    font-size: 12px;
    text-transform: capitalize;
    cursor: pointer;
  }
  .chip.active {
    color: #fff;
    background: var(--accent);
    border-color: var(--accent);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    padding: 10px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title {
    color: var(--text);
    text-decoration: none;
    font-weight: 500;
    font-size: 13.5px;
  }
  .title:hover {
    color: var(--accent);
  }
  .num {
    color: var(--text-muted);
    margin-right: 6px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
  }
  .badge {
    font-size: 10px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-left: 6px;
    text-transform: uppercase;
  }
  .rev {
    text-transform: capitalize;
  }
  .rev.APPROVED {
    color: var(--status-add, #2ea043);
  }
  .rev.CHANGES_REQUESTED {
    color: var(--err, #c0392b);
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
    margin: 14px 2px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
```

- [ ] **Step 2: GithubIssues.**

Create `src/lib/components/github/GithubIssues.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { IssueStateFilter } from "../../types";

  const FILTERS: IssueStateFilter[] = ["open", "closed", "all"];

  $effect(() => {
    const repo = appState.repo;
    void githubState.issueState;
    void githubState.reloadNonce;
    if (repo) void githubState.loadIssues(repo);
  });

  const panel = $derived(githubState.issues);
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.issueState === f} type="button" onclick={() => githubState.setIssueState(f)}>{f}</button>
  {/each}
</div>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading issues…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load issues ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No {githubState.issueState === "all" ? "" : githubState.issueState} issues.</p>
{:else if panel.data}
  <ul class="list">
    {#each panel.data as it (it.number)}
      <li class="row">
        <a class="title" href={it.url} target="_blank" rel="noreferrer">
          <span class="num">#{it.number}</span>{it.title}
        </a>
        <div class="meta">
          <span>{it.author}</span>
          {#if it.assignees.length}<span>→ {it.assignees.join(", ")}</span>{/if}
          <span class="when">{rel(it.updatedAt)}</span>
        </div>
        {#if it.labels.length}
          <div class="labels">
            {#each it.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .filters {
    display: flex;
    gap: 6px;
    margin-bottom: 10px;
  }
  .chip {
    padding: 3px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel-bg);
    color: var(--text-muted);
    font-size: 12px;
    text-transform: capitalize;
    cursor: pointer;
  }
  .chip.active {
    color: #fff;
    background: var(--accent);
    border-color: var(--accent);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    padding: 10px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title {
    color: var(--text);
    text-decoration: none;
    font-weight: 500;
    font-size: 13.5px;
  }
  .title:hover {
    color: var(--accent);
  }
  .num {
    color: var(--text-muted);
    margin-right: 6px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 12px;
    color: var(--text-muted);
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
    margin: 14px 2px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
```

- [ ] **Step 3: Type-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubPulls.svelte src/lib/components/github/GithubIssues.svelte
git commit -m "feat(github): Pull Requests + Issues tab components"
```

---

## Task 8: Svelte — Releases + Actions tabs

**Files:** Create `src/lib/components/github/GithubReleases.svelte`, `src/lib/components/github/GithubActions.svelte`.

- [ ] **Step 1: GithubReleases.**

Create `src/lib/components/github/GithubReleases.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import { formatCompact } from "../../github/format";

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) void githubState.loadReleases(repo);
  });

  const panel = $derived(githubState.releases);
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
{:else if panel.data}
  <div class="rels">
    {#each panel.data as r (r.tagName)}
      <section class="rel">
        <header>
          <a class="rtitle" href={r.htmlUrl} target="_blank" rel="noreferrer">{r.name || r.tagName}</a>
          <span class="tag mono">{r.tagName}</span>
          {#if r.prerelease}<span class="badge">pre-release</span>{/if}
          {#if r.draft}<span class="badge">draft</span>{/if}
          <span class="when">{rel(r.publishedAt)}</span>
          <span class="total">{formatCompact(r.totalDownloads)} downloads</span>
        </header>
        {#if r.assets.length}
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
  .rels {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .rel header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .rtitle {
    color: var(--text);
    text-decoration: none;
    font-weight: 600;
    font-size: 14px;
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
  .assets {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
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

- [ ] **Step 2: GithubActions.**

Create `src/lib/components/github/GithubActions.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { parseISO, formatCommitDate } from "../../dates";

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) void githubState.loadRuns(repo);
  });

  const panel = $derived(githubState.runs);
  function rel(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  // A run's pass/fail is only meaningful once status === "completed".
  function glyph(status: string, conclusion: string): string {
    if (status !== "completed") return "○";
    if (conclusion === "success") return "✓";
    if (conclusion === "failure" || conclusion === "timed_out") return "✗";
    return "–";
  }
  function cls(status: string, conclusion: string): string {
    if (status !== "completed") return "pending";
    if (conclusion === "success") return "ok";
    if (conclusion === "failure" || conclusion === "timed_out") return "fail";
    return "other";
  }
</script>

{#if panel.status === "loading" && !panel.data}
  <p class="note">Loading workflow runs…</p>
{:else if panel.status === "error"}
  <p class="note err">Could not load runs ({panel.error?.kind}).</p>
{:else if panel.data && panel.data.length === 0}
  <p class="note">No workflow runs.</p>
{:else if panel.data}
  <ul class="list">
    {#each panel.data as run (run.id)}
      <li class="row">
        <span class="glyph {cls(run.status, run.conclusion)}" aria-hidden="true">{glyph(run.status, run.conclusion)}</span>
        <a class="title" href={run.url} target="_blank" rel="noreferrer">{run.title}</a>
        <span class="wf">{run.workflowName}</span>
        <span class="branch mono">{run.headBranch}</span>
        <span class="ev">{run.event}</span>
        <span class="when">{rel(run.updatedAt)}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    font-size: 12.5px;
  }
  .glyph {
    width: 16px;
    text-align: center;
    font-weight: 700;
  }
  .glyph.ok {
    color: var(--status-add, #2ea043);
  }
  .glyph.fail {
    color: var(--err, #c0392b);
  }
  .glyph.pending {
    color: var(--status-mod, #d29922);
  }
  .title {
    color: var(--text);
    text-decoration: none;
    font-weight: 500;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title:hover {
    color: var(--accent);
  }
  .wf,
  .ev,
  .when {
    color: var(--text-muted);
  }
  .branch {
    color: var(--text-muted);
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
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

- [ ] **Step 3: Type-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubReleases.svelte src/lib/components/github/GithubActions.svelte
git commit -m "feat(github): Releases (download analytics) + Actions tab components"
```

---

## Task 9: Svelte — wire the tabs into the bar + the view

**Files:** Modify `src/lib/components/github/GithubTabs.svelte`, `src/lib/components/github/GithubView.svelte`.

- [ ] **Step 1: Add the four tab entries.**

In `src/lib/components/github/GithubTabs.svelte`, replace the `TABS` array:

```typescript
  const TABS: { id: GithubTab; label: string }[] = [{ id: "overview", label: "Overview" }];
```

with:

```typescript
  const TABS: { id: GithubTab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "pulls", label: "Pull Requests" },
    { id: "issues", label: "Issues" },
    { id: "releases", label: "Releases" },
    { id: "actions", label: "Actions" },
  ];
```

- [ ] **Step 2: Add the render branches.**

In `src/lib/components/github/GithubView.svelte`, add the four imports (with the others):

```typescript
  import GithubPulls from "./GithubPulls.svelte";
  import GithubIssues from "./GithubIssues.svelte";
  import GithubReleases from "./GithubReleases.svelte";
  import GithubActions from "./GithubActions.svelte";
```

Then extend the active-tab block. The current block is:

```svelte
    {#if githubState.activeTab === "overview"}
      {#if githubState.statsLoading && !githubState.stats}
        <p class="note">Loading…</p>
      {:else if githubState.statsError}
        <p class="note err">Could not load repository data ({githubState.statsError.kind}).</p>
      {:else if githubState.stats}
        <GithubOverview stats={githubState.stats} />
      {/if}
    {/if}
```

Add the four `{:else if}` branches before its closing `{/if}` so it reads:

```svelte
    {#if githubState.activeTab === "overview"}
      {#if githubState.statsLoading && !githubState.stats}
        <p class="note">Loading…</p>
      {:else if githubState.statsError}
        <p class="note err">Could not load repository data ({githubState.statsError.kind}).</p>
      {:else if githubState.stats}
        <GithubOverview stats={githubState.stats} />
      {/if}
    {:else if githubState.activeTab === "pulls"}
      <GithubPulls />
    {:else if githubState.activeTab === "issues"}
      <GithubIssues />
    {:else if githubState.activeTab === "releases"}
      <GithubReleases />
    {:else if githubState.activeTab === "actions"}
      <GithubActions />
    {/if}
```

- [ ] **Step 3: Type-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubTabs.svelte src/lib/components/github/GithubView.svelte
git commit -m "feat(github): wire Pull Requests/Issues/Releases/Actions tabs into the screen"
```

---

## Task 10: Full verification + manual smoke

**Files:** none.

- [ ] **Step 1: Gates.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run check
npx vitest run
cd src-tauri && cargo test
```
Expected: svelte-check 0/0 · vitest all pass (no new vitest files this phase — count unchanged) · cargo all pass (99 + new mapping tests).

- [ ] **Step 2: Build + smoke against a real repo.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run tauri build
```
Open a repo with a github.com origin (e.g. `cli/cli`), open **GitHub**, and confirm each tab:
- **Pull Requests:** open PRs list; filter chips (open/closed/merged/all) re-fetch; clicking a row opens the PR on github.com; labels show with their colors; draft/review badges appear.
- **Issues:** open issues list (no PRs mixed in); filter works; assignees + labels render.
- **Releases:** each release shows total downloads and a per-asset table with download counts; clicking opens the release/asset.
- **Actions:** recent runs with ✓/✗/○ glyphs (green/red/amber); a still-running workflow shows the pending glyph; clicking opens the run.
- **Refresh** (header ↻) re-fetches the active tab.

- [ ] **Step 3: Proceed to review + merge (below).**

---

## Done criteria (Phase 2)

- All four read tabs work against live `gh`, lazily loaded and cached per repo+filter, with loading/error/empty states and out-links. Releases surface the per-asset + total download analytics. Actions distinguishes `status` from `conclusion` correctly.
- Gates green; the four new mapping fns are unit-tested against real captured payloads.
- Next plan: **Phase 3 (Insights: traffic + contributors + activity + milestones/labels)**.

## After all tasks

Run an adversarial review of the branch diff (`superpowers:code-reviewer`), fix any Critical/Important findings, then use **superpowers:finishing-a-development-branch** to ff-merge `feat/github-screen-p2` → `main` (gates → review → ff-merge → memory).
