//! GitHub integration: spawns the `gh` CLI and parses JSON. Auth is delegated to
//! `gh` (no token is stored here). All owner/repo operands are validated before
//! reaching `gh`, and `gh` is spawned with args passed directly (never a shell).

use crate::ops_remote;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// True for a single `owner` or `repo` path segment we will hand to `gh`.
/// Beyond the charset, reject the degenerate/traversal cases — `.`/`..` (which
/// `gh api repos/{o}/{r}` would path-normalize into a different endpoint) and any
/// leading `-` (which later phases pass positionally to `gh`, where it would read
/// as a flag). GitHub owner/repo names never legitimately hit these, so nothing
/// real is rejected.
fn is_valid_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && !s.starts_with('-')
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
    // `gh --version` is offline; any failure means gh is unusable (missing, or
    // present-but-not-executable) — point the user at install instructions either way.
    if run_gh(&["--version"], None).is_err() {
        return GhAvailability::NotInstalled;
    }
    // `gh auth status` exits 0 when authed (writes to stderr even then). Only treat
    // it as NotAuthed when gh actually reports being logged out — a transient
    // network failure with a valid token must NOT block with a sign-in card; fall
    // through and let the data calls surface any real error instead.
    match run_gh(&["auth", "status"], None) {
        Ok(_) => {}
        Err(GithubError::NotAuthed) => return GhAvailability::NotAuthed,
        Err(_) => {}
    }
    match resolve_owner_repo(repo) {
        Some((owner, repo)) => GhAvailability::Ok { owner, repo },
        None => GhAvailability::NoRemote,
    }
}

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
        // Path-traversal / leading-dash segments must be rejected: `repos/../user`
        // would normalize to a different endpoint, and a leading `-` reads as a flag.
        assert_eq!(parse_github_remote("https://github.com/../user"), None);
        assert_eq!(parse_github_remote("https://github.com/./x"), None);
        assert_eq!(parse_github_remote("https://github.com/-rf/x"), None);
        assert_eq!(parse_github_remote("https://github.com/x/-rf"), None);
    }

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
}
