# GitHub Screen — Phase 1 (Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the new top-level **GitHub** screen end-to-end — a navigable, sub-tab shell that detects `gh`, resolves the repo's owner/repo, fetches live repository stats via `gh`, and renders them in a header + Overview tab — proving the whole `gh → Rust → Svelte` pipeline.

**Architecture:** A new Rust `github` module spawns the `gh` CLI (args passed directly to `std::process::Command`, no shell) and parses JSON into `serde` DTOs; a typed `GithubError` enum classifies failures from `gh`'s real stderr strings. The Svelte side adds a `"github"` value to `activeView`, a conditional sidebar entry, a `githubState.svelte.ts` controller, and a `GithubView` shell (header + data-driven tab bar + Overview). Auth is delegated entirely to `gh` (no token stored).

**Tech Stack:** Tauri 2 (Rust) · SvelteKit 5 / Svelte 5 runes · `gh` CLI ≥ 2.x · `serde`/`serde_json` (already deps) · Vitest + `cargo test`.

**Spec:** `docs/superpowers/specs/2026-06-17-github-screen-design.md` (all `gh` commands/fields verified against live gh 2.86).

**Branch:** `feat/github-screen` (already checked out, off `main` `9009560`).

---

## File Structure

**New (Rust):**
- `src-tauri/src/github/mod.rs` — the whole Phase-1 backend: `parse_github_remote`, `resolve_owner_repo`, `GithubError` + `classify_gh_error`, `run_gh`, `GhAvailability` + `availability`, `GhRepoStats` + `map_rest` + `repo_stats`, with `#[cfg(test)]` tests. (One focused module; split later phases into submodules if it grows.)

**Modified (Rust):**
- `src-tauri/src/lib.rs` — add `mod github;` and register the two new commands.
- `src-tauri/src/commands.rs` — add `github_availability` + `github_repo_stats` command wrappers.

**New (TS/Svelte):**
- `src/lib/github/remote.ts` — `parseGithubRemote` (pure TS mirror of the Rust parser, for cheap nav-visibility).
- `src/lib/github/format.ts` — `formatCompact` (1.2k/3.4M number formatting).
- `src/lib/github/remote.test.ts`, `src/lib/github/format.test.ts` — vitest.
- `src/lib/githubState.svelte.ts` — runes controller (availability + stats + active tab).
- `src/lib/components/github/StatTile.svelte`, `GithubSetupCard.svelte`, `GithubHeader.svelte`, `GithubOverview.svelte`, `GithubTabs.svelte`, `GithubView.svelte`.

**Modified (TS/Svelte):**
- `src/lib/types.ts` — add `GhAvailability`, `GhRepoStats`, `GithubError`.
- `src/lib/api.ts` — add `githubAvailability`, `githubRepoStats` wrappers.
- `src/lib/store.svelte.ts` — widen `activeView` union to include `"github"` (3 sites).
- `src/lib/components/Sidebar.svelte` — add the conditional GitHub nav entry.
- `src/routes/+page.svelte` — add the `"github"` branch in `.view-swap`.

---

## Task 1: Rust — GitHub remote URL parser

**Files:**
- Create: `src-tauri/src/github/mod.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod github;`)

- [ ] **Step 1: Create the module with the parser + tests (write tests first conceptually, but Rust compiles them together — add both).**

Create `src-tauri/src/github/mod.rs`:

```rust
//! GitHub integration: spawns the `gh` CLI and parses JSON. Auth is delegated to
//! `gh` (no token is stored here). All owner/repo operands are validated before
//! reaching `gh`, and `gh` is spawned with args passed directly (never a shell).

use crate::ops_remote;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// True for a single `owner` or `repo` path segment we will hand to `gh`.
fn is_valid_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Parse a git remote URL into `(owner, repo)` when it points at github.com.
/// Handles `https://`, `http://`, scp-style `git@github.com:`, and `ssh://`
/// forms, with or without a trailing `.git`. Returns `None` for non-github.com
/// hosts, extra path segments, or segments with invalid characters.
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let u = url.trim();
    let rest = u
        .strip_prefix("https://github.com/")
        .or_else(|| u.strip_prefix("http://github.com/"))
        .or_else(|| u.strip_prefix("git@github.com:"))
        .or_else(|| u.strip_prefix("ssh://git@github.com/"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repo) = rest.split_once('/')?;
    let owner = owner.trim();
    let repo = repo.trim();
    if repo.contains('/') || !is_valid_segment(owner) || !is_valid_segment(repo) {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// Resolve the GitHub owner/repo for a local repo path from its remotes,
/// preferring `origin`, else the first github.com remote.
pub fn resolve_owner_repo(repo: &Path) -> Option<(String, String)> {
    let remotes = ops_remote::remotes(repo).ok()?;
    if let Some(origin) = remotes.iter().find(|r| r.name == "origin") {
        if let Some(or) = parse_github_remote(&origin.url) {
            return Some(or);
        }
    }
    remotes.iter().find_map(|r| parse_github_remote(&r.url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_github_url_forms() {
        let cases = [
            "https://github.com/cli/cli.git",
            "https://github.com/cli/cli",
            "http://github.com/cli/cli.git",
            "git@github.com:cli/cli.git",
            "git@github.com:cli/cli",
            "ssh://git@github.com/cli/cli.git",
        ];
        for c in cases {
            assert_eq!(
                parse_github_remote(c),
                Some(("cli".to_string(), "cli".to_string())),
                "failed for {c}"
            );
        }
    }

    #[test]
    fn rejects_non_github_and_malformed() {
        assert_eq!(parse_github_remote("https://gitlab.com/x/y.git"), None);
        assert_eq!(parse_github_remote("git@bitbucket.org:x/y.git"), None);
        assert_eq!(parse_github_remote("https://github.com/onlyowner"), None);
        assert_eq!(parse_github_remote("https://github.com/cli/cli/extra.git"), None);
        assert_eq!(parse_github_remote("https://github.com/cli/c li"), None);
        assert_eq!(parse_github_remote(""), None);
    }
}
```

Add `mod github;` to `src-tauri/src/lib.rs` in the `mod` block near the top (alphabetical with the others, after `mod fswatch;`):

```rust
mod commands;
mod fswatch;
mod git_ops;
mod github;
mod graph;
```

- [ ] **Step 2: Run the tests — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github::tests`
Expected: `parses_all_github_url_forms` and `rejects_non_github_and_malformed` PASS. (If `cargo` errors that `github` is unused elsewhere, that's fine for now — the module is registered.)

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/lib.rs
git commit -m "feat(github): remote URL parser + owner/repo resolver"
```

---

## Task 2: Rust — typed error model + classifier

**Files:**
- Modify: `src-tauri/src/github/mod.rs`

- [ ] **Step 1: Add the error enum + classifier + tests.**

Append to `src-tauri/src/github/mod.rs` (before the `#[cfg(test)]` module, or anywhere in the module body):

```rust
/// Typed result of a failed `gh` invocation. Serialized adjacently-tagged so the
/// frontend sees `{ "kind": "NotFound" }` or `{ "kind": "Other", "message": "…" }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum GithubError {
    NotInstalled,
    NotAuthed,
    NoRemote,
    NotFound,
    Forbidden,
    RateLimited,
    Other(String),
}

/// Classify a failed `gh` call from its stdout (API JSON body) + stderr. `gh`
/// writes the human "gh: … (HTTP 4xx)" line to stderr and the raw API JSON to
/// stdout, so we inspect both. (Strings verified against live gh 2.86.)
pub fn classify_gh_error(stdout: &str, stderr: &str) -> GithubError {
    let err = stderr.to_lowercase();
    if err.contains("no git remotes found") {
        return GithubError::NoRemote;
    }
    if err.contains("not logged into any github") {
        return GithubError::NotAuthed;
    }
    let combined = format!("{stdout}\n{stderr}").to_lowercase();
    if combined.contains("rate limit exceeded") {
        return GithubError::RateLimited;
    }
    if stderr.contains("HTTP 404") {
        return GithubError::NotFound;
    }
    if stderr.contains("HTTP 403") {
        return GithubError::Forbidden;
    }
    GithubError::Other(stderr.trim().to_string())
}
```

Add these tests inside the existing `#[cfg(test)] mod tests` block (after the parser tests):

```rust
    #[test]
    fn classifies_gh_errors_from_real_strings() {
        assert_eq!(
            classify_gh_error("", "gh: Not Found (HTTP 404)"),
            GithubError::NotFound
        );
        assert_eq!(
            classify_gh_error(
                "{\"message\":\"Must have push access to repository\",\"status\":\"403\"}",
                "gh: Must have push access to repository (HTTP 403)"
            ),
            GithubError::Forbidden
        );
        assert_eq!(
            classify_gh_error(
                "{\"message\":\"API rate limit exceeded for user\"}",
                "gh: API rate limit exceeded (HTTP 403)"
            ),
            GithubError::RateLimited
        );
        assert_eq!(
            classify_gh_error("", "no git remotes found"),
            GithubError::NoRemote
        );
        assert_eq!(
            classify_gh_error(
                "",
                "You are not logged into any GitHub hosts. Run gh auth login to authenticate."
            ),
            GithubError::NotAuthed
        );
        match classify_gh_error("", "some other failure") {
            GithubError::Other(m) => assert_eq!(m, "some other failure"),
            other => panic!("expected Other, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Run the tests — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github::tests::classifies`
Expected: `classifies_gh_errors_from_real_strings` PASSES.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs
git commit -m "feat(github): typed GithubError + stderr classifier"
```

---

## Task 3: Rust — the `gh` runner

**Files:**
- Modify: `src-tauri/src/github/mod.rs`

- [ ] **Step 1: Add `run_gh` + a smoke test.**

Add the imports at the top of `src-tauri/src/github/mod.rs` (extend the existing `use` block):

```rust
use std::io::Write;
use std::process::{Command, Stdio};
```

Append `run_gh` to the module body:

```rust
/// Run `gh` with the given args, optionally writing `stdin` (used later for
/// comment/issue bodies via `--body-file -`). On success returns stdout; on
/// failure returns a classified `GithubError`. `gh` is spawned with args passed
/// directly — never via a shell.
pub fn run_gh(args: &[&str], stdin: Option<&str>) -> Result<String, GithubError> {
    let mut cmd = Command::new("gh");
    cmd.args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(GithubError::NotInstalled)
        }
        Err(e) => return Err(GithubError::Other(format!("failed to spawn gh: {e}"))),
    };
    if let Some(body) = stdin {
        if let Some(mut sink) = child.stdin.take() {
            let _ = sink.write_all(body.as_bytes());
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| GithubError::Other(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(classify_gh_error(&stdout, &stderr))
    }
}
```

Add this test inside `#[cfg(test)] mod tests` (it requires `gh` to be installed — it is in this dev env; it makes no network call):

```rust
    #[test]
    fn run_gh_version_succeeds() {
        // `gh --version` is offline and proves the spawn+capture path.
        match run_gh(&["--version"], None) {
            Ok(out) => assert!(out.contains("gh version"), "unexpected: {out}"),
            Err(GithubError::NotInstalled) => {
                eprintln!("gh not installed — skipping run_gh smoke test");
            }
            Err(e) => panic!("run_gh --version failed: {e:?}"),
        }
    }
```

- [ ] **Step 2: Run the test — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github::tests::run_gh_version`
Expected: PASS (prints `gh version …` internally; passes). If `gh` is absent it prints a skip note and still passes.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs
git commit -m "feat(github): run_gh subprocess runner"
```

---

## Task 4: Rust — availability detection + command

**Files:**
- Modify: `src-tauri/src/github/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `GhAvailability` + `availability()`.**

Append to `src-tauri/src/github/mod.rs`:

```rust
/// Whether the GitHub screen can operate for a given local repo. The `Ok`
/// variant carries the resolved owner/repo for the header. Serialized as
/// `{ "kind": "Ok", "owner": "…", "repo": "…" }` etc.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum GhAvailability {
    Ok { owner: String, repo: String },
    NotInstalled,
    NotAuthed,
    NoRemote,
}

/// Probe, in order: is `gh` installed? authenticated? does this repo have a
/// github.com remote? Returns the first failing state, else `Ok` with owner/repo.
pub fn availability(repo: &Path) -> GhAvailability {
    match run_gh(&["--version"], None) {
        Ok(_) => {}
        Err(GithubError::NotInstalled) => return GhAvailability::NotInstalled,
        Err(_) => return GhAvailability::NotInstalled,
    }
    // `gh auth status` exits 0 when authed (writes to stderr even then), 1 otherwise.
    if run_gh(&["auth", "status"], None).is_err() {
        return GhAvailability::NotAuthed;
    }
    match resolve_owner_repo(repo) {
        Some((owner, repo)) => GhAvailability::Ok { owner, repo },
        None => GhAvailability::NoRemote,
    }
}
```

- [ ] **Step 2: Add the command wrapper.**

In `src-tauri/src/commands.rs`, add `use crate::github;` to the import block at the top (after `use crate::git_ops;`), then add this command (anywhere among the other `#[tauri::command]` fns):

```rust
#[tauri::command]
pub fn github_availability(repo: String) -> github::GhAvailability {
    github::availability(&PathBuf::from(repo))
}
```

- [ ] **Step 3: Register the command.**

In `src-tauri/src/lib.rs`, add to the `tauri::generate_handler![ … ]` list (next to the other `commands::…` entries):

```rust
    commands::github_availability,
```

- [ ] **Step 4: Verify it compiles + tests still pass.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test`
Expected: builds clean; all existing + new github tests PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): availability probe + github_availability command"
```

---

## Task 5: Rust — repo stats command

**Files:**
- Modify: `src-tauri/src/github/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the DTOs, the pure `map_rest`, and a fixture test.**

Append to `src-tauri/src/github/mod.rs`:

```rust
/// Subset of `GET /repos/{owner}/{repo}` we consume.
#[derive(Deserialize)]
struct RestRepo {
    full_name: String,
    description: Option<String>,
    html_url: String,
    visibility: String,
    default_branch: String,
    language: Option<String>,
    license: Option<RestLicense>,
    #[serde(default)]
    topics: Vec<String>,
    stargazers_count: u64,
    subscribers_count: u64,
    forks_count: u64,
    pushed_at: String,
    archived: bool,
    fork: bool,
}

#[derive(Deserialize)]
struct RestLicense {
    spdx_id: Option<String>,
}

/// Repository stats for the header + Overview. Serialized camelCase for TS.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhRepoStats {
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub visibility: String,
    pub default_branch: String,
    pub language: Option<String>,
    pub license_spdx_id: Option<String>,
    pub topics: Vec<String>,
    pub stars: u64,
    pub watchers: u64,
    pub forks: u64,
    pub open_issues: u64,
    pub open_pulls: u64,
    pub pushed_at: String,
    pub archived: bool,
    pub is_fork: bool,
}

/// Map the REST repo object + the GraphQL open counts into `GhRepoStats`.
/// CRITICAL: `watchers` is `subscribers_count` (the true Watch count), NOT
/// `watchers_count`/`watchers`, which GitHub aliases to the star count.
fn map_rest(r: RestRepo, open_issues: u64, open_pulls: u64) -> GhRepoStats {
    GhRepoStats {
        full_name: r.full_name,
        description: r.description,
        html_url: r.html_url,
        visibility: r.visibility,
        default_branch: r.default_branch,
        language: r.language,
        license_spdx_id: r.license.and_then(|l| l.spdx_id),
        topics: r.topics,
        stars: r.stargazers_count,
        watchers: r.subscribers_count,
        forks: r.forks_count,
        open_issues,
        open_pulls,
        pushed_at: r.pushed_at,
        archived: r.archived,
        is_fork: r.fork,
    }
}
```

Add this test inside `#[cfg(test)] mod tests` (the JSON is the real cli/cli payload shape captured from live `gh`):

```rust
    #[test]
    fn map_rest_uses_subscribers_for_watchers() {
        // Real cli/cli shape: stargazers_count == watchers_count == watchers == 44882,
        // subscribers_count == 1040. The Watch stat must read 1040, not 44882.
        let json = r#"{
            "full_name":"cli/cli","description":"GitHub's official command line tool",
            "html_url":"https://github.com/cli/cli","visibility":"public",
            "default_branch":"trunk","language":"Go","license":{"spdx_id":"MIT"},
            "topics":["cli","git"],"stargazers_count":44882,"watchers_count":44882,
            "watchers":44882,"subscribers_count":1040,"forks_count":8571,
            "pushed_at":"2026-06-17T19:55:07Z","archived":false,"fork":false
        }"#;
        let rest: RestRepo = serde_json::from_str(json).unwrap();
        let stats = map_rest(rest, 5, 3);
        assert_eq!(stats.stars, 44882);
        assert_eq!(stats.watchers, 1040, "watchers must be subscribers_count");
        assert_eq!(stats.forks, 8571);
        assert_eq!(stats.open_issues, 5);
        assert_eq!(stats.open_pulls, 3);
        assert_eq!(stats.license_spdx_id.as_deref(), Some("MIT"));
        assert_eq!(stats.default_branch, "trunk");
    }

    #[test]
    fn map_rest_tolerates_null_license_and_missing_topics() {
        let json = r#"{
            "full_name":"a/b","description":null,"html_url":"https://github.com/a/b",
            "visibility":"private","default_branch":"main","language":null,"license":null,
            "stargazers_count":0,"watchers_count":0,"watchers":0,"subscribers_count":0,
            "forks_count":0,"pushed_at":"2026-01-01T00:00:00Z","archived":false,"fork":true
        }"#;
        let rest: RestRepo = serde_json::from_str(json).unwrap();
        let stats = map_rest(rest, 0, 0);
        assert_eq!(stats.license_spdx_id, None);
        assert!(stats.topics.is_empty());
        assert!(stats.is_fork);
        assert_eq!(stats.description, None);
    }
```

- [ ] **Step 2: Run the mapping tests — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test github::tests::map_rest`
Expected: both `map_rest_*` tests PASS.

- [ ] **Step 3: Add `repo_stats()` (the two `gh` calls + mapping).**

Append to `src-tauri/src/github/mod.rs`:

```rust
#[derive(Deserialize)]
struct GqlEnvelope {
    data: GqlData,
}
#[derive(Deserialize)]
struct GqlData {
    repository: GqlRepo,
}
#[derive(Deserialize)]
struct GqlRepo {
    issues: GqlCount,
    #[serde(rename = "pullRequests")]
    pull_requests: GqlCount,
}
#[derive(Deserialize)]
struct GqlCount {
    #[serde(rename = "totalCount")]
    total_count: u64,
}

/// Fetch repository stats: `GET /repos/{o}/{r}` for identity/stars/forks/watchers,
/// plus a small GraphQL query for the accurate open issue/PR counts (REST
/// `open_issues_count` conflates issues + PRs).
pub fn repo_stats(repo: &Path) -> Result<GhRepoStats, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("repos/{owner}/{name}");
    let rest_json = run_gh(&["api", &slug], None)?;
    let rest: RestRepo = serde_json::from_str(&rest_json)
        .map_err(|e| GithubError::Other(format!("parse repo: {e}")))?;

    let query = "query($o:String!,$n:String!){repository(owner:$o,name:$n){issues(states:OPEN){totalCount} pullRequests(states:OPEN){totalCount}}}";
    let q_arg = format!("query={query}");
    let o_arg = format!("o={owner}");
    let n_arg = format!("n={name}");
    let gql_json = run_gh(
        &["api", "graphql", "-f", &q_arg, "-f", &o_arg, "-f", &n_arg],
        None,
    )?;
    let gql: GqlEnvelope = serde_json::from_str(&gql_json)
        .map_err(|e| GithubError::Other(format!("parse counts: {e}")))?;

    Ok(map_rest(
        rest,
        gql.data.repository.issues.total_count,
        gql.data.repository.pull_requests.total_count,
    ))
}
```

- [ ] **Step 4: Add the command + register it.**

In `src-tauri/src/commands.rs`, add:

```rust
#[tauri::command]
pub fn github_repo_stats(repo: String) -> Result<github::GhRepoStats, github::GithubError> {
    github::repo_stats(&PathBuf::from(repo))
}
```

In `src-tauri/src/lib.rs`, add to the handler list:

```rust
    commands::github_repo_stats,
```

- [ ] **Step 5: Build + run all Rust tests.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test`
Expected: clean build; all tests PASS (93 prior + the new github tests).

- [ ] **Step 6: (Optional) sanity-check the real endpoints by hand (no code change).**

Run: `gh api repos/cli/cli --jq '{stars:.stargazers_count, watchers:.subscribers_count, forks:.forks_count}'`
Expected: real numbers print, with `watchers` far smaller than `stars` — the exact mapping `repo_stats` relies on. The full Rust path is exercised in the app during Task 12.

- [ ] **Step 7: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): github_repo_stats command (REST stats + GraphQL open counts)"
```

---

## Task 6: TS — pure helpers (parser mirror + number format)

**Files:**
- Create: `src/lib/github/remote.ts`, `src/lib/github/remote.test.ts`, `src/lib/github/format.ts`, `src/lib/github/format.test.ts`

- [ ] **Step 1: Write the failing tests.**

Create `src/lib/github/remote.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { parseGithubRemote } from "./remote";

describe("parseGithubRemote", () => {
  it("parses all github url forms to owner/repo", () => {
    for (const url of [
      "https://github.com/cli/cli.git",
      "https://github.com/cli/cli",
      "git@github.com:cli/cli.git",
      "ssh://git@github.com/cli/cli.git",
    ]) {
      expect(parseGithubRemote(url)).toEqual({ owner: "cli", repo: "cli" });
    }
  });

  it("returns null for non-github or malformed urls", () => {
    expect(parseGithubRemote("https://gitlab.com/x/y.git")).toBeNull();
    expect(parseGithubRemote("https://github.com/onlyowner")).toBeNull();
    expect(parseGithubRemote("https://github.com/cli/cli/extra")).toBeNull();
    expect(parseGithubRemote("")).toBeNull();
  });
});
```

Create `src/lib/github/format.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { formatCompact } from "./format";

describe("formatCompact", () => {
  it("leaves sub-thousands as-is", () => {
    expect(formatCompact(0)).toBe("0");
    expect(formatCompact(999)).toBe("999");
  });
  it("compacts thousands and millions", () => {
    expect(formatCompact(1000)).toBe("1k");
    expect(formatCompact(1234)).toBe("1.2k");
    expect(formatCompact(44882)).toBe("44.9k");
    expect(formatCompact(1_000_000)).toBe("1M");
    expect(formatCompact(2_500_000)).toBe("2.5M");
  });
  it("is defensive about non-finite input", () => {
    expect(formatCompact(NaN)).toBe("0");
  });
});
```

- [ ] **Step 2: Run them — expect FAIL (modules not found).**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github`
Expected: FAIL — cannot resolve `./remote` / `./format`.

- [ ] **Step 3: Implement the helpers.**

Create `src/lib/github/remote.ts`:

```typescript
const SEGMENT = /^[A-Za-z0-9._-]+$/;
const PREFIXES = [
  "https://github.com/",
  "http://github.com/",
  "git@github.com:",
  "ssh://git@github.com/",
];

/** Pure TS mirror of the Rust `parse_github_remote` — used for cheap nav
 *  visibility (no `gh` call needed to know a repo has a github.com remote). */
export function parseGithubRemote(url: string): { owner: string; repo: string } | null {
  const u = (url ?? "").trim();
  const prefix = PREFIXES.find((p) => u.startsWith(p));
  if (!prefix) return null;
  let rest = u.slice(prefix.length);
  if (rest.endsWith(".git")) rest = rest.slice(0, -4);
  const slash = rest.indexOf("/");
  if (slash < 0) return null;
  const owner = rest.slice(0, slash).trim();
  const repo = rest.slice(slash + 1).trim();
  if (repo.includes("/") || !SEGMENT.test(owner) || !SEGMENT.test(repo)) return null;
  return { owner, repo };
}
```

Create `src/lib/github/format.ts`:

```typescript
/** Compact a count for a stat tile: 1234 → "1.2k", 44882 → "44.9k", 2.5e6 → "2.5M". */
export function formatCompact(n: number): string {
  if (!Number.isFinite(n)) return "0";
  const abs = Math.abs(n);
  if (abs < 1000) return String(n);
  if (abs < 1_000_000) return trimOne(n / 1000) + "k";
  return trimOne(n / 1_000_000) + "M";
}

function trimOne(x: number): string {
  return (Math.round(x * 10) / 10).toString();
}
```

- [ ] **Step 4: Run them — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github`
Expected: both files PASS.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/github/remote.ts src/lib/github/remote.test.ts src/lib/github/format.ts src/lib/github/format.test.ts
git commit -m "feat(github): pure TS helpers (remote parser mirror + formatCompact)"
```

---

## Task 7: TS — types + api wrappers + activeView

**Files:**
- Modify: `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/store.svelte.ts`

- [ ] **Step 1: Add the types.**

Append to `src/lib/types.ts`:

```typescript
export type GhAvailability =
  | { kind: "Ok"; owner: string; repo: string }
  | { kind: "NotInstalled" }
  | { kind: "NotAuthed" }
  | { kind: "NoRemote" };

export type GhRepoStats = {
  fullName: string;
  description: string | null;
  htmlUrl: string;
  visibility: string;
  defaultBranch: string;
  language: string | null;
  licenseSpdxId: string | null;
  topics: string[];
  stars: number;
  watchers: number;
  forks: number;
  openIssues: number;
  openPulls: number;
  pushedAt: string;
  archived: boolean;
  isFork: boolean;
};

export type GithubError =
  | { kind: "NotInstalled" }
  | { kind: "NotAuthed" }
  | { kind: "NoRemote" }
  | { kind: "NotFound" }
  | { kind: "Forbidden" }
  | { kind: "RateLimited" }
  | { kind: "Other"; message: string };
```

- [ ] **Step 2: Add the api wrappers.**

In `src/lib/api.ts`, add `GhAvailability` and `GhRepoStats` to the type-import block (the `import type { … } from "./types";` list), then add these two wrappers to the returned `api` object (next to `remotes`):

```typescript
  githubAvailability: (repo: string) => invoke<GhAvailability>("github_availability", { repo }),
  githubRepoStats: (repo: string) => invoke<GhRepoStats>("github_repo_stats", { repo }),
```

- [ ] **Step 3: Widen the `activeView` union (3 sites in `src/lib/store.svelte.ts`).**

Site 1 — the declaration (line ~664):

```typescript
let activeView = $state<"timeline" | "changes" | "github">("timeline");
```

Site 2 — the setter signature (line ~1338):

```typescript
setActiveView(v: "timeline" | "changes" | "github") {
  activeView = v;
  if (v === "changes") {
    currentSha = null;
    selected = new Set();
  }
},
```

(The getter at line ~1335 returns the variable and needs no type change.)

- [ ] **Step 4: Verify types compile.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/types.ts src/lib/api.ts src/lib/store.svelte.ts
git commit -m "feat(github): TS types, api wrappers, activeView 'github'"
```

---

## Task 8: TS — the `githubState` controller

**Files:**
- Create: `src/lib/githubState.svelte.ts`

- [ ] **Step 1: Create the controller.**

Create `src/lib/githubState.svelte.ts`:

```typescript
import { appState } from "./store.svelte";
import { api } from "./api";
import { parseGithubRemote } from "./github/remote";
import type { GhAvailability, GhRepoStats, GithubError } from "./types";

export type GithubTab = "overview" | "pulls" | "issues" | "releases" | "actions" | "insights";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function makeGithubState() {
  let availability = $state<GhAvailability | null>(null);
  let availLoading = $state(false);
  let stats = $state<GhRepoStats | null>(null);
  let statsError = $state<GithubError | null>(null);
  let statsLoading = $state(false);
  let activeTab = $state<GithubTab>("overview");
  let loadedRepo: string | null = null;

  async function loadStats(repo: string) {
    statsLoading = true;
    statsError = null;
    stats = null;
    try {
      stats = await api.githubRepoStats(repo);
    } catch (e) {
      statsError = e as GithubError;
    } finally {
      statsLoading = false;
    }
  }

  async function ensure(repo: string) {
    if (!isTauri()) return; // gh paths are desktop-only
    if (loadedRepo === repo && availability) return;
    loadedRepo = repo;
    availability = null;
    stats = null;
    statsError = null;
    availLoading = true;
    try {
      availability = await api.githubAvailability(repo);
    } catch {
      availability = { kind: "NotInstalled" };
    } finally {
      availLoading = false;
    }
    if (availability && availability.kind === "Ok") {
      void loadStats(repo);
    }
  }

  return {
    get availability() {
      return availability;
    },
    get availLoading() {
      return availLoading;
    },
    get stats() {
      return stats;
    },
    get statsError() {
      return statsError;
    },
    get statsLoading() {
      return statsLoading;
    },
    get activeTab() {
      return activeTab;
    },
    /** Cheap, no-`gh` check used for sidebar nav visibility. */
    get hasGithubRemote(): boolean {
      return appState.remotes.some((r) => parseGithubRemote(r.url) !== null);
    },
    setActiveTab(t: GithubTab) {
      activeTab = t;
    },
    ensure,
    refresh(repo: string) {
      loadedRepo = null;
      return ensure(repo);
    },
  };
}

export const githubState = makeGithubState();
```

- [ ] **Step 2: Verify it type-checks.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings. (If it complains that `appState.remotes` is missing, confirm the getter name in `store.svelte.ts` and use the correct one.)

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/githubState.svelte.ts
git commit -m "feat(github): githubState controller (availability + stats + tab)"
```

---

## Task 9: Svelte — presentational pieces (StatTile, SetupCard, Header)

**Files:**
- Create: `src/lib/components/github/StatTile.svelte`, `GithubSetupCard.svelte`, `GithubHeader.svelte`

- [ ] **Step 1: StatTile.**

Create `src/lib/components/github/StatTile.svelte`:

```svelte
<script lang="ts">
  import { formatCompact } from "../../github/format";
  let {
    label,
    value,
    onclick,
  }: { label: string; value: number; onclick?: () => void } = $props();
</script>

<button class="tile" class:clickable={!!onclick} type="button" disabled={!onclick} {onclick}>
  <span class="num">{formatCompact(value)}</span>
  <span class="lbl">{label}</span>
</button>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 14px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel-bg);
    color: var(--text);
    text-align: left;
    cursor: default;
  }
  .tile.clickable {
    cursor: pointer;
  }
  .tile.clickable:hover {
    border-color: var(--accent);
  }
  .num {
    font-size: 17px;
    font-weight: 600;
  }
  .lbl {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
```

- [ ] **Step 2: GithubSetupCard.**

Create `src/lib/components/github/GithubSetupCard.svelte`:

```svelte
<script lang="ts">
  import type { GhAvailability } from "../../types";
  let { availability, onretry }: { availability: GhAvailability; onretry: () => void } = $props();
</script>

<div class="setup">
  {#if availability.kind === "NotInstalled"}
    <h2>GitHub CLI not found</h2>
    <p>The GitHub screen uses the <code>gh</code> command-line tool. Install it, then retry.</p>
    <pre>brew install gh</pre>
    <a href="https://cli.github.com" target="_blank" rel="noreferrer">cli.github.com ↗</a>
  {:else if availability.kind === "NotAuthed"}
    <h2>Sign in to GitHub</h2>
    <p>Authenticate the <code>gh</code> CLI once in your terminal, then retry.</p>
    <pre>gh auth login</pre>
  {:else if availability.kind === "NoRemote"}
    <h2>No GitHub remote</h2>
    <p>This repository has no <code>github.com</code> remote, so there's nothing to show here.</p>
  {/if}
  <button class="retry" type="button" onclick={onretry}>Retry</button>
</div>

<style>
  .setup {
    max-width: 460px;
    margin: 48px auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--text);
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  p {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  pre {
    margin: 4px 0;
    padding: 8px 14px;
    background: var(--btn-bg);
    border-radius: 6px;
    font-size: 12.5px;
  }
  a {
    color: var(--accent);
    font-size: 12.5px;
  }
  .retry {
    margin-top: 8px;
    padding: 6px 16px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--btn-bg);
    color: var(--text);
    cursor: pointer;
  }
</style>
```

- [ ] **Step 3: GithubHeader.**

Create `src/lib/components/github/GithubHeader.svelte`:

```svelte
<script lang="ts">
  import type { GhRepoStats } from "../../types";
  import StatTile from "./StatTile.svelte";
  import { githubState } from "../../githubState.svelte";

  let {
    stats,
    owner,
    repo,
    onrefresh,
  }: { stats: GhRepoStats | null; owner: string; repo: string; onrefresh: () => void } = $props();
</script>

<header class="gh-header">
  <div class="id">
    <a class="slug" href={stats?.htmlUrl ?? `https://github.com/${owner}/${repo}`} target="_blank" rel="noreferrer">
      {owner}/{repo} ↗
    </a>
    {#if stats?.description}<p class="desc">{stats.description}</p>{/if}
  </div>
  <div class="tiles">
    {#if stats}
      <StatTile label="Stars" value={stats.stars} />
      <StatTile label="Forks" value={stats.forks} />
      <StatTile label="Watchers" value={stats.watchers} />
      <StatTile label="Open PRs" value={stats.openPulls} onclick={() => githubState.setActiveTab("pulls")} />
      <StatTile label="Open Issues" value={stats.openIssues} onclick={() => githubState.setActiveTab("issues")} />
    {/if}
    <button class="refresh" type="button" title="Refresh" onclick={onrefresh}>↻</button>
  </div>
</header>

<style>
  .gh-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 2px 12px 2px;
  }
  .slug {
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
    text-decoration: none;
  }
  .slug:hover {
    color: var(--accent);
  }
  .desc {
    margin: 4px 0 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  .tiles {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .refresh {
    margin-left: auto;
    width: 30px;
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel-bg);
    color: var(--text);
    cursor: pointer;
  }
</style>
```

- [ ] **Step 4: Verify they compile.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/StatTile.svelte src/lib/components/github/GithubSetupCard.svelte src/lib/components/github/GithubHeader.svelte
git commit -m "feat(github): StatTile, GithubSetupCard, GithubHeader components"
```

---

## Task 10: Svelte — Overview tab, tab bar, and the screen shell

**Files:**
- Create: `src/lib/components/github/GithubOverview.svelte`, `GithubTabs.svelte`, `GithubView.svelte`

- [ ] **Step 1: GithubOverview (the "repository at a glance" card).**

Create `src/lib/components/github/GithubOverview.svelte`:

```svelte
<script lang="ts">
  import type { GhRepoStats } from "../../types";
  import { appState } from "../../store.svelte";
  import { parseISO, formatCommitDate } from "../../dates";

  let { stats }: { stats: GhRepoStats } = $props();

  function lastPush(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
</script>

<div class="overview">
  <dl class="meta">
    <dt>Default branch</dt>
    <dd>{stats.defaultBranch}</dd>
    <dt>Visibility</dt>
    <dd>{stats.visibility}{stats.isFork ? " · fork" : ""}{stats.archived ? " · archived" : ""}</dd>
    <dt>Language</dt>
    <dd>{stats.language ?? "—"}</dd>
    <dt>License</dt>
    <dd>{stats.licenseSpdxId ?? "—"}</dd>
    <dt>Last push</dt>
    <dd>{lastPush(stats.pushedAt)}</dd>
  </dl>
  {#if stats.topics.length}
    <div class="topics">
      {#each stats.topics as t (t)}<span class="topic">{t}</span>{/each}
    </div>
  {/if}
  <p class="note">Pull requests, issues, releases and CI summaries appear in their tabs.</p>
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .meta {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 18px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    color: var(--text);
  }
  .topics {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .topic {
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
```

- [ ] **Step 2: GithubTabs (data-driven sub-tab bar).**

Create `src/lib/components/github/GithubTabs.svelte`:

```svelte
<script lang="ts">
  import { githubState, type GithubTab } from "../../githubState.svelte";

  // Phase 1 ships only Overview; later phases push more entries here.
  const TABS: { id: GithubTab; label: string }[] = [{ id: "overview", label: "Overview" }];
</script>

<nav class="tabs" aria-label="GitHub sections">
  {#each TABS as t (t.id)}
    <button
      class="tab"
      class:active={githubState.activeTab === t.id}
      aria-current={githubState.activeTab === t.id ? "page" : undefined}
      type="button"
      onclick={() => githubState.setActiveTab(t.id)}
    >
      {t.label}
    </button>
  {/each}
</nav>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border-subtle, var(--border));
    margin-bottom: 12px;
  }
  .tab {
    padding: 7px 14px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
  }
  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
</style>
```

- [ ] **Step 3: GithubView (the shell — availability gating + header + tabs + active tab).**

Create `src/lib/components/github/GithubView.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import GithubHeader from "./GithubHeader.svelte";
  import GithubTabs from "./GithubTabs.svelte";
  import GithubOverview from "./GithubOverview.svelte";
  import GithubSetupCard from "./GithubSetupCard.svelte";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  // (Re)load availability + stats whenever the GitHub screen is active for a repo.
  $effect(() => {
    const repo = appState.repo;
    if (repo && appState.activeView === "github") {
      void githubState.ensure(repo);
    }
  });

  const avail = $derived(githubState.availability);
</script>

<section class="gh">
  {#if !isTauri()}
    <p class="note">The GitHub screen is available in the desktop app only.</p>
  {:else if githubState.availLoading && !avail}
    <p class="note">Checking GitHub…</p>
  {:else if avail && avail.kind === "Ok"}
    <GithubHeader
      stats={githubState.stats}
      owner={avail.owner}
      repo={avail.repo}
      onrefresh={() => appState.repo && githubState.refresh(appState.repo)}
    />
    <GithubTabs />
    {#if githubState.activeTab === "overview"}
      {#if githubState.statsLoading && !githubState.stats}
        <p class="note">Loading…</p>
      {:else if githubState.statsError}
        <p class="note err">Could not load repository data ({githubState.statsError.kind}).</p>
      {:else if githubState.stats}
        <GithubOverview stats={githubState.stats} />
      {/if}
    {/if}
  {:else if avail}
    <GithubSetupCard availability={avail} onretry={() => appState.repo && githubState.refresh(appState.repo)} />
  {/if}
</section>

<style>
  .gh {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .note {
    margin: 16px 2px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }
</style>
```

- [ ] **Step 4: Verify they compile.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubOverview.svelte src/lib/components/github/GithubTabs.svelte src/lib/components/github/GithubView.svelte
git commit -m "feat(github): Overview tab, sub-tab bar, and GithubView shell"
```

---

## Task 11: Svelte — wire into sidebar + page

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`, `src/routes/+page.svelte`

- [ ] **Step 1: Add the sidebar nav entry (conditional on a GitHub remote).**

In `src/lib/components/Sidebar.svelte`, add the import (with the other imports near the top of `<script>`):

```typescript
import { githubState } from "../githubState.svelte";
```

Then, inside the existing `<nav class="view-nav" …>` block, add a third button after the "Commit Timeline" button (just before `</nav>`):

```svelte
  {#if githubState.hasGithubRemote}
    <button
      class="wc-entry"
      class:active={appState.activeView === "github"}
      aria-current={appState.activeView === "github" ? "page" : undefined}
      onclick={() => appState.setActiveView("github")}
      title="GitHub — pull requests, issues, releases and more"
    >
      <span class="gh-dot" aria-hidden="true"></span>
      <span class="wc-label">GitHub</span>
    </button>
  {/if}
```

Add a `.gh-dot` rule to the component's `<style>` (mirroring `.tl-dot`):

```css
  .gh-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent);
    flex-shrink: 0;
  }
```

- [ ] **Step 2: Add the `"github"` branch to the page's view-swap.**

In `src/routes/+page.svelte`, add the import (with the other component imports, matching this file's existing style — relative `../lib/...` or `$lib/...`; both resolve):

```typescript
import GithubView from "../lib/components/github/GithubView.svelte";
```

Then, inside `<div class="view-swap" …>`, extend the conditional. Change the existing:

```svelte
  {#if appState.activeView === "changes"}
    <WorkingCopyView />
  {:else if appState.selectedCommit}
```

to add a `github` branch first:

```svelte
  {#if appState.activeView === "github"}
    <GithubView />
  {:else if appState.activeView === "changes"}
    <WorkingCopyView />
  {:else if appState.selectedCommit}
```

Also hide the always-mounted timeline-stack on the GitHub screen — change the `timeline-stack` wrapper's `class:hidden` to also hide on `"github"`:

```svelte
  <div class="timeline-stack" class:hidden={appState.activeView === "changes" || appState.activeView === "github"}>
```

- [ ] **Step 3: Verify it compiles.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/Sidebar.svelte src/routes/+page.svelte
git commit -m "feat(github): wire GitHub screen into sidebar + page view-swap"
```

---

## Task 12: Full verification + manual smoke

**Files:** none (verification only)

- [ ] **Step 1: Run all gates.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run check
npx vitest run
cd src-tauri && cargo test
```
Expected: svelte-check 0 errors / 0 warnings · vitest all pass (170 prior + new remote/format suites) · cargo all pass (93 prior + new github tests).

- [ ] **Step 2: Build the desktop app and smoke-test against a real GitHub repo.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run tauri build
```
Then open the built app, open a repo that has a github.com `origin` (e.g. clone `https://github.com/cli/cli`), and confirm:
- The **GitHub** entry appears in the sidebar (and is absent for a repo with no GitHub remote).
- Clicking it shows the header with `owner/repo`, description, and five stat tiles with real numbers — **Watchers must be the small "watch" count (e.g. ~1k for cli/cli), not the star count.**
- The Overview tab shows default branch, visibility, language, license, last push, and topic chips.
- With `gh` logged out (`gh auth logout`), the screen shows the "Sign in to GitHub" card; after `gh auth login`, Retry loads it. (Re-login when done.)

- [ ] **Step 3: Update the spec's phase checklist (optional bookkeeping).** Mark Phase 1 complete in `docs/superpowers/specs/2026-06-17-github-screen-design.md` §12 if you keep a running status there.

- [ ] **Step 4: Commit any smoke-fix follow-ups, then proceed to the review + merge step below.**

---

## Done criteria (Phase 1)

- The GitHub screen is reachable from the sidebar (only when the repo has a GitHub remote), detects `gh` install/auth state with helpful setup cards, resolves owner/repo from the origin remote, and renders **live** repository stats (correct stars/forks/watchers, with the `subscribers_count` watcher gotcha handled) plus an Overview card — entirely through `gh`.
- All gates green; new pure logic (URL parser ×2, error classifier, stat mapping, number format) is unit-tested.
- After merge, the next plan covers **Phase 2 (Read panels: PRs · Issues · Releases · Actions)**.

## After all tasks

Run an adversarial review of the branch diff (`superpowers:code-reviewer` or `feature-dev:code-reviewer`), fix any Critical/Important findings, then use **superpowers:finishing-a-development-branch** to ff-merge `feat/github-screen` → `main` (this project's standing rhythm: gates → review → ff-merge → memory).
