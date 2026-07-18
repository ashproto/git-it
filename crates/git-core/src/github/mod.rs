//! GitHub integration: spawns the `gh` CLI and parses JSON. Auth is delegated to
//! `gh` (no token is stored here). All owner/repo operands are validated before
//! reaching `gh`, and `gh` is spawned with args passed directly (never a shell).

pub mod review;

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
    run_gh_impl(None, args, stdin)
}

/// `run_gh`, but spawned with the repo as the working directory. Required for
/// gh subcommands that read the LOCAL git context (`gh pr create` infers the
/// head branch from the current directory; `gh pr checkout` operates on it).
pub fn run_gh_in(repo: &Path, args: &[&str], stdin: Option<&str>) -> Result<String, GithubError> {
    run_gh_impl(Some(repo), args, stdin)
}

fn run_gh_impl(cwd: Option<&Path>, args: &[&str], stdin: Option<&str>) -> Result<String, GithubError> {
    let mut cmd = Command::new("gh");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
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
    #[serde(rename = "stateReason", default)]
    state_reason: Option<String>,
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
    pub state_reason: Option<String>, // COMPLETED | NOT_PLANNED | null (closed reason)
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
        state_reason: i.state_reason,
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
            "--json", "number,title,author,labels,assignees,state,stateReason,updatedAt,url",
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

// ── Insight commands ─────────────────────────────────────────────────────────

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

/// Raw README markdown for the repo (empty string when there is no README or
/// the request fails — callers render nothing in that case).
pub fn readme(repo: &Path) -> Result<String, GithubError> {
    let (owner, name) = match resolve_owner_repo(repo) {
        Some(x) => x,
        None => return Ok(String::new()),
    };
    let slug = format!("repos/{owner}/{name}/readme");
    match run_gh(&["api", &slug, "-H", "Accept: application/vnd.github.raw"], None) {
        Ok(md) => Ok(md),
        Err(_) => Ok(String::new()), // 404 (no README) / transient → empty
    }
}

/// Map a merge-method name to the `gh pr merge` flag. None for an unknown method.
fn merge_flag(method: &str) -> Option<&'static str> {
    match method {
        "merge" => Some("--merge"),
        "squash" => Some("--squash"),
        "rebase" => Some("--rebase"),
        _ => None,
    }
}

/// Comment on a PR. Body is piped via stdin (`--body-file -`), never an arg.
pub fn pr_comment(repo: &Path, number: u64, body: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    run_gh(&["pr", "comment", &num, "--repo", &slug, "--body-file", "-"], Some(body))?;
    Ok(())
}

/// Comment on an issue.
pub fn issue_comment(repo: &Path, number: u64, body: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    run_gh(&["issue", "comment", &num, "--repo", &slug, "--body-file", "-"], Some(body))?;
    Ok(())
}

/// Close or reopen an issue. `state` ∈ {"closed","open"}.
pub fn issue_set_state(repo: &Path, number: u64, state: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    let verb = match state {
        "closed" => "close",
        "open" => "reopen",
        _ => return Err(GithubError::Other(format!("invalid issue state: {state}"))),
    };
    run_gh(&["issue", verb, &num, "--repo", &slug], None)?;
    Ok(())
}

/// Merge a PR with the given method ∈ {"merge","squash","rebase"}.
pub fn pr_merge(repo: &Path, number: u64, method: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let flag = merge_flag(method)
        .ok_or_else(|| GithubError::Other(format!("invalid merge method: {method}")))?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    run_gh(&["pr", "merge", &num, "--repo", &slug, flag], None)?;
    Ok(())
}

/// Create an issue. Title via `--title=` (single arg, dash-safe); body via stdin.
/// Returns the new issue's URL (gh prints it to stdout).
pub fn issue_create(repo: &Path, title: &str, body: &str) -> Result<String, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let title_arg = format!("--title={title}");
    let out = run_gh(
        &["issue", "create", "--repo", &slug, &title_arg, "--body-file", "-"],
        Some(body),
    )?;
    Ok(out.trim().to_string())
}

fn has_committed_head(repo: &Path) -> Result<bool, GithubError> {
    let head = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .output()
        .map_err(|e| GithubError::Other(format!("could not inspect HEAD: {e}")))?;
    Ok(head.status.success())
}

/// Create a GitHub repo from this local repo, wire it as `origin`, and push the
/// current branch (setting upstream). TWO steps, deliberately not gh's `--push`:
/// (1) `gh repo create <name> --source=<repo> --private|--public
/// [--description=<d>] --remote=origin` creates the repo + adds the remote using
/// gh's API token — always works. (2) We run `git push --set-upstream origin HEAD`
/// ourselves. gh's own `--push` shells out to a plain `git push` that relies on the
/// user's GLOBAL git credential helper, which is frequently unconfigured or broken
/// (its push then silently fails, leaving the repo created but empty — the reported
/// bug). We instead reset inherited helpers and force gh's own credential helper for
/// just this command, so the push authenticates with the gh token regardless of the
/// user's git config. Returns gh's stdout (includes the new repo URL). `description`
/// uses `--flag=value` form; `name` is a positional arg, so we reject a leading-dash
/// name up front (GitHub repository names can't start with `-` anyway).
pub fn create_repo(
    repo: &Path,
    name: &str,
    private: bool,
    description: &str,
) -> Result<String, GithubError> {
    if name.starts_with('-') {
        return Err(GithubError::Other(
            "Repository name can't start with '-'.".into(),
        ));
    }
    let repo_str = repo.to_string_lossy().to_string();
    let source_arg = format!("--source={repo_str}");
    let visibility_arg = if private { "--private" } else { "--public" };
    let desc_arg = format!("--description={description}");
    let mut args: Vec<&str> = vec![
        "repo",
        "create",
        name,
        &source_arg,
        visibility_arg,
        "--remote=origin",
    ];
    if !description.trim().is_empty() {
        args.push(&desc_arg);
    }
    // Step 1 — create the repo + add `origin` (gh API token; reliable).
    let created = run_gh(&args, None)?;

    // A freshly initialized repository has an unborn HEAD. The remote and
    // origin are already fully created in that state, but there is no ref Git
    // can push yet (`src refspec HEAD does not match any`). Treat that as a
    // successful empty-repository setup and let the first normal push establish
    // the upstream after the user creates a commit.
    if !has_committed_head(repo)? {
        return Ok(format!(
            "{}\nRemote added. Create the first commit to push this repository.",
            created.trim()
        ));
    }

    // Step 2 — push the current branch + set upstream. The empty `credential.helper=`
    // resets any inherited (possibly broken) global helpers; the second entry forces
    // gh's own credential helper so auth uses the gh token. GIT_TERMINAL_PROMPT=0 so a
    // missing credential fails fast instead of hanging on a prompt.
    let out = Command::new("git")
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "credential.helper=",
            "-c",
            "credential.helper=!gh auth git-credential",
            "push",
            "--set-upstream",
            "origin",
            "HEAD",
        ])
        .output()
        .map_err(|e| GithubError::Other(format!("git push failed to spawn: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(GithubError::Other(format!(
            "Repository created and remote added, but the push failed: {}",
            stderr.trim()
        )));
    }
    Ok(created)
}

// ---- statusCheckRollup normalization (CheckRun + StatusContext leaves) ----
#[derive(Deserialize)]
struct RawCheck {
    #[serde(rename = "__typename", default)]
    typename: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    conclusion: String,
    #[serde(rename = "detailsUrl", default)]
    details_url: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    state: String,
    #[serde(rename = "targetUrl", default)]
    target_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhCheck {
    pub name: String,
    pub bucket: String, // pass | fail | pending | neutral
    pub url: String,
}

fn map_check(c: RawCheck) -> GhCheck {
    if c.typename == "StatusContext" {
        let bucket = match c.state.as_str() {
            "SUCCESS" => "pass",
            "FAILURE" | "ERROR" => "fail",
            "PENDING" | "EXPECTED" => "pending",
            _ => "neutral",
        };
        GhCheck { name: c.context, bucket: bucket.to_string(), url: c.target_url }
    } else {
        // CheckRun: pass/fail only once completed; otherwise pending.
        let bucket = match (c.status.as_str(), c.conclusion.as_str()) {
            ("COMPLETED", "SUCCESS") => "pass",
            ("COMPLETED", "FAILURE") | ("COMPLETED", "TIMED_OUT") => "fail",
            ("COMPLETED", _) => "neutral",
            _ => "pending",
        };
        GhCheck { name: c.name, bucket: bucket.to_string(), url: c.details_url }
    }
}

// ---- reactions ----

/// One emoji-reaction group on a comment/body, normalized to the REST content
/// name (`+1`, `-1`, `laugh`, `confused`, `heart`, `hooray`, `rocket`, `eyes`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhReactionGroup {
    pub content: String,
    pub count: u64,
    pub viewer_reacted: bool,
}

/// Normalize a reaction content name to its REST form. GraphQL (and gh's JSON
/// export) emits UPPER_SNAKE (`THUMBS_UP`); REST uses `+1` etc. Already-REST
/// names pass through; unknown names → `None` (the group is dropped).
pub fn normalize_reaction(content: &str) -> Option<String> {
    let rest = match content {
        "THUMBS_UP" | "+1" => "+1",
        "THUMBS_DOWN" | "-1" => "-1",
        "LAUGH" | "laugh" => "laugh",
        "CONFUSED" | "confused" => "confused",
        "HEART" | "heart" => "heart",
        "HOORAY" | "hooray" => "hooray",
        "ROCKET" | "rocket" => "rocket",
        "EYES" | "eyes" => "eyes",
        _ => return None,
    };
    Some(rest.to_string())
}

/// Map a `reactionGroups` JSON array (gh export or GraphQL, both shaped
/// `{content, users:{totalCount}, viewerHasReacted?}`) into normalized groups.
/// Zero-count and unknown-content groups are dropped; anything malformed maps
/// to an empty list — never an error. gh's `pr/issue view` export omits
/// `viewerHasReacted`, which then defaults to `false`.
fn parse_reaction_groups(v: Option<&serde_json::Value>) -> Vec<GhReactionGroup> {
    let Some(arr) = v.and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|g| {
            let content = normalize_reaction(g.get("content")?.as_str()?)?;
            let count = g
                .pointer("/users/totalCount")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            if count == 0 {
                return None;
            }
            let viewer_reacted = g
                .get("viewerHasReacted")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            Some(GhReactionGroup { content, count, viewer_reacted })
        })
        .collect()
}

/// Extract the numeric REST id from a timeline-comment URL's
/// `#issuecomment-<digits>` fragment (gh's JSON export only carries the
/// GraphQL node id, so this is the only source of the REST id).
pub fn issue_comment_id_from_url(url: &str) -> Option<u64> {
    let (_, frag) = url.rsplit_once("#issuecomment-")?;
    if frag.is_empty() || !frag.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    frag.parse().ok()
}

/// The authenticated gh user's login (`gh api user --jq .login`), trimmed.
/// Repo-independent — used by the frontend to recognize "own" comments and by
/// the reaction-removal path to find the viewer's reaction.
pub fn current_login() -> Result<String, GithubError> {
    let out = run_gh(&["api", "user", "--jq", ".login"], None)?;
    Ok(out.trim().to_string())
}

// ---- shared detail sub-DTOs ----
#[derive(Deserialize)]
struct RawMilestoneRef {
    title: String,
}
#[derive(Deserialize)]
struct RawComment {
    author: Option<RawUser>,
    #[serde(default)]
    body: String,
    #[serde(rename = "createdAt", default)]
    created_at: String,
    #[serde(default)]
    url: String,
    #[serde(rename = "reactionGroups", default)]
    reaction_groups: serde_json::Value,
}
#[derive(Deserialize)]
struct RawReview {
    author: Option<RawUser>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: String,
    #[serde(rename = "submittedAt", default)]
    submitted_at: String,
}
#[derive(Deserialize)]
struct RawFile {
    path: String,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
    /// Numeric REST id (edit/delete/react target), extracted from the comment
    /// URL's `#issuecomment-<digits>` fragment. `None` when unavailable.
    pub id: Option<u64>,
    pub reactions: Vec<GhReactionGroup>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhReview {
    pub author: String,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhFile {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
}

fn map_comment(c: RawComment) -> GhComment {
    GhComment {
        author: c.author.map(|a| a.login).unwrap_or_default(),
        body: c.body,
        created_at: c.created_at,
        id: issue_comment_id_from_url(&c.url),
        reactions: parse_reaction_groups(Some(&c.reaction_groups)),
    }
}
fn map_review(r: RawReview) -> GhReview {
    GhReview {
        author: r.author.map(|a| a.login).unwrap_or_default(),
        state: r.state,
        body: r.body,
        submitted_at: r.submitted_at,
    }
}

// ---- PR commits (from `gh pr view --json commits`) ----
#[derive(Deserialize)]
struct RawPrCommit {
    oid: String,
    #[serde(rename = "messageHeadline", default)]
    message_headline: String,
    #[serde(rename = "committedDate", default)]
    committed_date: String,
    #[serde(default)]
    authors: Vec<RawUser>, // gh returns authors:[{login,name,...}]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhCommit {
    pub oid: String,
    pub message: String,
    pub author: String,
    pub committed_date: String,
}

fn map_pr_commit(c: RawPrCommit) -> GhCommit {
    GhCommit {
        oid: c.oid,
        message: c.message_headline,
        author: c
            .authors
            .into_iter()
            .next()
            .map(|a| a.login)
            .unwrap_or_default(),
        committed_date: c.committed_date,
    }
}

// ---- PR review threads (GraphQL — REST can't give isResolved) ----
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhInlineComment {
    pub author: String,
    pub body: String,
    pub path: String,
    pub line: i64,
    pub created_at: String,
    /// REST comment id — the reply target. `None` when GraphQL omits it.
    pub database_id: Option<i64>,
    pub reactions: Vec<GhReactionGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhReviewThread {
    /// GraphQL node id — the resolve/unresolve target. "" when missing.
    pub id: String,
    pub resolved: bool,
    pub path: String,
    pub line: i64,
    pub comments: Vec<GhInlineComment>,
}

/// Navigate `data.repository.pullRequest.reviewThreads.nodes[]` defensively;
/// any missing/null field defaults to ""/0 and never panics.
fn parse_review_threads(json: &str) -> Vec<GhReviewThread> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let nodes = v
        .pointer("/data/repository/pullRequest/reviewThreads/nodes")
        .and_then(|n| n.as_array());
    let Some(nodes) = nodes else {
        return Vec::new();
    };
    nodes
        .iter()
        .map(|t| {
            let id = t.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let resolved = t.get("isResolved").and_then(|x| x.as_bool()).unwrap_or(false);
            let path = t.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let line = t.get("line").and_then(|x| x.as_i64()).unwrap_or(0);
            let comments = t
                .pointer("/comments/nodes")
                .and_then(|n| n.as_array())
                .map(|cn| {
                    cn.iter()
                        .map(|c| {
                            let author = c
                                .pointer("/author/login")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let body = c.get("body").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let cpath = c.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let cline = c
                                .get("line")
                                .and_then(|x| x.as_i64())
                                .or_else(|| c.get("originalLine").and_then(|x| x.as_i64()))
                                .unwrap_or(0);
                            let created_at = c
                                .get("createdAt")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let database_id = c.get("databaseId").and_then(|x| x.as_i64());
                            let reactions = parse_reaction_groups(c.get("reactionGroups"));
                            GhInlineComment {
                                author,
                                body,
                                path: cpath,
                                line: cline,
                                created_at,
                                database_id,
                                reactions,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            GhReviewThread { id, resolved, path, line, comments }
        })
        .collect()
}

/// Best-effort: fetch resolved/unresolved inline review threads via GraphQL.
/// Missing perms / no threads → empty; never fails the whole detail.
fn fetch_review_threads(owner: &str, name: &str, number: i64) -> Vec<GhReviewThread> {
    let query = "query($o:String!,$n:String!,$num:Int!){repository(owner:$o,name:$n){pullRequest(number:$num){reviewThreads(first:100){nodes{id isResolved path line comments(first:50){nodes{databaseId author{login} body path originalLine line createdAt reactionGroups{content viewerHasReacted users{totalCount}}}}}}}}}";
    let q_arg = format!("query={query}");
    let o = format!("o={owner}");
    let n = format!("n={name}");
    let num = format!("num={number}");
    match run_gh(
        &["api", "graphql", "-f", &q_arg, "-F", &o, "-F", &n, "-F", &num],
        None,
    ) {
        Ok(out) => parse_review_threads(&out),
        Err(_) => Vec::new(),
    }
}

// ---- CI run history (best-effort REST) ----
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhCheckRun {
    pub name: String,
    pub status: String,     // queued | in_progress | completed
    pub conclusion: String, // success | failure | cancelled | "" when null
    pub started_at: String, // run_started_at
    pub updated_at: String,
    pub url: String,        // html_url
    pub head_sha: String,
}

/// Navigate `workflow_runs[]` defensively; null conclusion → "".
fn parse_check_runs(json: &str) -> Vec<GhCheckRun> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let runs = v.get("workflow_runs").and_then(|r| r.as_array());
    let Some(runs) = runs else {
        return Vec::new();
    };
    runs.iter()
        .map(|r| {
            let s = |k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            GhCheckRun {
                name: s("name"),
                status: s("status"),
                conclusion: r
                    .get("conclusion")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                started_at: s("run_started_at"),
                updated_at: s("updated_at"),
                url: s("html_url"),
                head_sha: s("head_sha"),
            }
        })
        .collect()
}

/// Best-effort: recent workflow runs for the PR's head branch.
fn fetch_check_runs(owner: &str, name: &str, head_ref: &str) -> Vec<GhCheckRun> {
    let path = format!("repos/{owner}/{name}/actions/runs?branch={head_ref}&per_page=30");
    match run_gh(&["api", &path], None) {
        Ok(out) => parse_check_runs(&out),
        Err(_) => Vec::new(),
    }
}

// ---- PR detail ----
#[derive(Deserialize)]
struct RawPullDetail {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    author: Option<RawUser>,
    state: String,
    #[serde(rename = "isDraft", default)]
    is_draft: bool,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    assignees: Vec<RawUser>,
    milestone: Option<RawMilestoneRef>,
    #[serde(rename = "baseRefName", default)]
    base_ref_name: String,
    #[serde(rename = "headRefName", default)]
    head_ref_name: String,
    #[serde(rename = "reviewDecision", default)]
    review_decision: String,
    #[serde(default)]
    mergeable: String,
    #[serde(rename = "mergeStateStatus", default)]
    merge_state_status: String,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(rename = "changedFiles", default)]
    changed_files: u64,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    reviews: Vec<RawReview>,
    #[serde(rename = "statusCheckRollup", default)]
    status_check_rollup: Vec<RawCheck>,
    #[serde(default)]
    comments: Vec<RawComment>,
    #[serde(default)]
    commits: Vec<RawPrCommit>,
    #[serde(rename = "reactionGroups", default)]
    reaction_groups: serde_json::Value,
    #[serde(rename = "createdAt", default)]
    created_at: String,
    #[serde(rename = "updatedAt", default)]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhPullDetail {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub state: String,
    pub is_draft: bool,
    pub labels: Vec<GhLabel>,
    pub assignees: Vec<String>,
    pub milestone: Option<String>,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub review_decision: String,
    pub mergeable: String,
    pub merge_state_status: String,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub files: Vec<GhFile>,
    pub reviews: Vec<GhReview>,
    pub checks: Vec<GhCheck>,
    pub comments: Vec<GhComment>,
    #[serde(default)]
    pub commits: Vec<GhCommit>,
    #[serde(default)]
    pub review_threads: Vec<GhReviewThread>,
    #[serde(default)]
    pub check_runs: Vec<GhCheckRun>,
    /// Reactions on the PR description itself.
    pub body_reactions: Vec<GhReactionGroup>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

fn map_pull_detail(p: RawPullDetail) -> GhPullDetail {
    GhPullDetail {
        number: p.number,
        title: p.title,
        body: p.body,
        author: p.author.map(|a| a.login).unwrap_or_default(),
        state: p.state,
        is_draft: p.is_draft,
        labels: p.labels.into_iter().map(map_label).collect(),
        assignees: p.assignees.into_iter().map(|a| a.login).collect(),
        milestone: p.milestone.map(|m| m.title),
        base_ref_name: p.base_ref_name,
        head_ref_name: p.head_ref_name,
        review_decision: p.review_decision,
        mergeable: p.mergeable,
        merge_state_status: p.merge_state_status,
        additions: p.additions,
        deletions: p.deletions,
        changed_files: p.changed_files,
        files: p.files.into_iter().map(|f| GhFile { path: f.path, additions: f.additions, deletions: f.deletions }).collect(),
        reviews: p.reviews.into_iter().map(map_review).collect(),
        checks: p.status_check_rollup.into_iter().map(map_check).collect(),
        comments: p.comments.into_iter().map(map_comment).collect(),
        commits: p.commits.into_iter().map(map_pr_commit).collect(),
        // Best-effort fields populated by `pr_detail` after the base view.
        review_threads: Vec::new(),
        check_runs: Vec::new(),
        body_reactions: parse_reaction_groups(Some(&p.reaction_groups)),
        created_at: p.created_at,
        updated_at: p.updated_at,
        url: p.url,
    }
}

pub fn pr_detail(repo: &Path, number: u64) -> Result<GhPullDetail, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    let json = run_gh(
        &[
            "pr", "view", &num, "--repo", &slug, "--json",
            "number,title,body,author,state,isDraft,labels,assignees,milestone,baseRefName,headRefName,reviewDecision,mergeable,mergeStateStatus,additions,deletions,changedFiles,files,reviews,statusCheckRollup,comments,commits,reactionGroups,createdAt,updatedAt,url",
        ],
        None,
    )?;
    let raw: RawPullDetail =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse pr detail: {e}")))?;
    let mut detail = map_pull_detail(raw);
    // Best-effort enrichments — never fail the command if they error.
    detail.review_threads = fetch_review_threads(&owner, &name, number as i64);
    detail.check_runs = fetch_check_runs(&owner, &name, &detail.head_ref_name);
    Ok(detail)
}

// ---- Issue detail ----
#[derive(Deserialize)]
struct RawIssueDetail {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    author: Option<RawUser>,
    state: String,
    #[serde(rename = "stateReason", default)]
    state_reason: Option<String>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    assignees: Vec<RawUser>,
    milestone: Option<RawMilestoneRef>,
    #[serde(default)]
    comments: Vec<RawComment>,
    #[serde(rename = "reactionGroups", default)]
    reaction_groups: serde_json::Value,
    #[serde(rename = "createdAt", default)]
    created_at: String,
    #[serde(rename = "updatedAt", default)]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhIssueDetail {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub state: String,
    pub state_reason: Option<String>,
    pub labels: Vec<GhLabel>,
    pub assignees: Vec<String>,
    pub milestone: Option<String>,
    pub comments: Vec<GhComment>,
    /// Reactions on the issue description itself.
    pub body_reactions: Vec<GhReactionGroup>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

pub fn issue_detail(repo: &Path, number: u64) -> Result<GhIssueDetail, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    let json = run_gh(
        &[
            "issue", "view", &num, "--repo", &slug, "--json",
            "number,title,body,author,state,stateReason,labels,assignees,milestone,comments,reactionGroups,createdAt,updatedAt,url",
        ],
        None,
    )?;
    let raw: RawIssueDetail =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse issue detail: {e}")))?;
    Ok(GhIssueDetail {
        number: raw.number,
        title: raw.title,
        body: raw.body,
        author: raw.author.map(|a| a.login).unwrap_or_default(),
        state: raw.state,
        state_reason: raw.state_reason,
        labels: raw.labels.into_iter().map(map_label).collect(),
        assignees: raw.assignees.into_iter().map(|a| a.login).collect(),
        milestone: raw.milestone.map(|m| m.title),
        comments: raw.comments.into_iter().map(map_comment).collect(),
        body_reactions: parse_reaction_groups(Some(&raw.reaction_groups)),
        created_at: raw.created_at,
        updated_at: raw.updated_at,
        url: raw.url,
    })
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn merge_flag_maps_known_methods() {
        assert_eq!(merge_flag("merge"), Some("--merge"));
        assert_eq!(merge_flag("squash"), Some("--squash"));
        assert_eq!(merge_flag("rebase"), Some("--rebase"));
        assert_eq!(merge_flag("bogus"), None);
    }

    #[test]
    fn committed_head_detection_distinguishes_an_empty_repository() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "git-it-github-head-test-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        let run_git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&path)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        };
        run_git(&["init", "-q", "--initial-branch=main"]);
        assert!(!has_committed_head(&path).unwrap());
        run_git(&["config", "user.name", "Test"]);
        run_git(&["config", "user.email", "test@example.com"]);
        fs::write(path.join("a.txt"), "a").unwrap();
        run_git(&["add", "a.txt"]);
        run_git(&["commit", "-q", "-m", "initial"]);
        assert!(has_committed_head(&path).unwrap());
        let _ = fs::remove_dir_all(path);
    }

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

    #[test]
    fn map_check_handles_both_typenames() {
        let cr: RawCheck = serde_json::from_str(
            r#"{"__typename":"CheckRun","name":"build","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"u"}"#,
        )
        .unwrap();
        assert_eq!(map_check(cr).bucket, "pass");
        let cr2: RawCheck = serde_json::from_str(
            r#"{"__typename":"CheckRun","name":"x","status":"IN_PROGRESS","conclusion":""}"#,
        )
        .unwrap();
        assert_eq!(map_check(cr2).bucket, "pending");
        let sc: RawCheck = serde_json::from_str(
            r#"{"__typename":"StatusContext","context":"ci/circleci","state":"FAILURE","targetUrl":"t"}"#,
        )
        .unwrap();
        let g = map_check(sc);
        assert_eq!(g.name, "ci/circleci");
        assert_eq!(g.bucket, "fail");
        assert_eq!(g.url, "t");
    }

    #[test]
    fn map_pull_detail_shapes_nested_arrays() {
        let json = r#"{
            "number":13,"title":"T","body":"hi","author":{"login":"me"},"state":"OPEN","isDraft":false,
            "labels":[{"name":"bug","color":"f00"}],"assignees":[{"login":"you"}],"milestone":{"title":"M1"},
            "baseRefName":"main","headRefName":"f","reviewDecision":"APPROVED","mergeable":"MERGEABLE",
            "mergeStateStatus":"CLEAN","additions":10,"deletions":2,"changedFiles":1,
            "files":[{"path":"a.rs","additions":10,"deletions":2}],
            "reviews":[{"author":{"login":"rev"},"state":"APPROVED","body":"lgtm","submittedAt":"2026-01-01T00:00:00Z"}],
            "statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"d"}],
            "comments":[{"author":{"login":"c"},"body":"nice","createdAt":"2026-01-02T00:00:00Z",
                "url":"https://github.com/o/r/pull/13#issuecomment-3987079426",
                "reactionGroups":[{"content":"THUMBS_UP","users":{"totalCount":2}}]}],
            "reactionGroups":[{"content":"HEART","users":{"totalCount":1}},{"content":"ROCKET","users":{"totalCount":0}}],
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-02T00:00:00Z","url":"u"
        }"#;
        let d = map_pull_detail(serde_json::from_str(json).unwrap());
        assert_eq!(d.author, "me");
        assert_eq!(d.milestone.as_deref(), Some("M1"));
        assert_eq!(d.assignees, vec!["you".to_string()]);
        assert_eq!(d.files[0].path, "a.rs");
        assert_eq!(d.reviews[0].author, "rev");
        assert_eq!(d.checks[0].bucket, "pass");
        assert_eq!(d.comments[0].author, "c");
        // Comment id from the url fragment + reactions normalized to REST names.
        assert_eq!(d.comments[0].id, Some(3987079426));
        assert_eq!(d.comments[0].reactions.len(), 1);
        assert_eq!(d.comments[0].reactions[0].content, "+1");
        assert_eq!(d.comments[0].reactions[0].count, 2);
        assert!(!d.comments[0].reactions[0].viewer_reacted); // gh export omits it → false
        // Body reactions: zero-count groups dropped.
        assert_eq!(d.body_reactions.len(), 1);
        assert_eq!(d.body_reactions[0].content, "heart");
    }

    #[test]
    fn map_pull_detail_tolerates_missing_reaction_fields() {
        // Pre-growth shape: no url / reactionGroups anywhere — must still parse.
        let json = r#"{
            "number":1,"title":"t","body":"","author":null,"state":"OPEN","isDraft":false,
            "labels":[],"assignees":[],"milestone":null,
            "baseRefName":"main","headRefName":"f","reviewDecision":"","mergeable":"",
            "mergeStateStatus":"","additions":0,"deletions":0,"changedFiles":0,
            "files":[],"reviews":[],"statusCheckRollup":[],
            "comments":[{"author":null,"body":"","createdAt":""}],
            "createdAt":"","updatedAt":"","url":"u"
        }"#;
        let d = map_pull_detail(serde_json::from_str(json).unwrap());
        assert_eq!(d.comments[0].id, None);
        assert!(d.comments[0].reactions.is_empty());
        assert!(d.body_reactions.is_empty());
    }

    #[test]
    fn normalize_reaction_covers_all_names() {
        // All 8 UPPER_SNAKE (GraphQL) names → REST names.
        let pairs = [
            ("THUMBS_UP", "+1"),
            ("THUMBS_DOWN", "-1"),
            ("LAUGH", "laugh"),
            ("CONFUSED", "confused"),
            ("HEART", "heart"),
            ("HOORAY", "hooray"),
            ("ROCKET", "rocket"),
            ("EYES", "eyes"),
        ];
        for (gql, rest) in pairs {
            assert_eq!(normalize_reaction(gql).as_deref(), Some(rest), "{gql}");
            // Already-REST names pass through unchanged.
            assert_eq!(normalize_reaction(rest).as_deref(), Some(rest), "{rest}");
        }
        // Unknown → None (group dropped).
        assert_eq!(normalize_reaction("SHRUG"), None);
        assert_eq!(normalize_reaction(""), None);
        assert_eq!(normalize_reaction("thumbs_up"), None);
    }

    #[test]
    fn issue_comment_id_from_url_extracts_fragment() {
        assert_eq!(
            issue_comment_id_from_url("https://github.com/cli/cli/pull/1#issuecomment-3987079426"),
            Some(3987079426)
        );
        assert_eq!(
            issue_comment_id_from_url("https://github.com/o/r/issues/2#issuecomment-1"),
            Some(1)
        );
        assert_eq!(issue_comment_id_from_url(""), None);
        assert_eq!(issue_comment_id_from_url("https://github.com/o/r/pull/1"), None);
        assert_eq!(issue_comment_id_from_url("x#issuecomment-"), None);
        assert_eq!(issue_comment_id_from_url("x#issuecomment-12ab"), None);
        // review-comment fragments are a different id space — must not match.
        assert_eq!(
            issue_comment_id_from_url("https://github.com/o/r/pull/1#discussion_r123"),
            None
        );
    }

    #[test]
    fn parse_reaction_groups_handles_gh_and_graphql_shapes() {
        // gh export shape (no viewerHasReacted).
        let gh: serde_json::Value = serde_json::from_str(
            r#"[{"content":"THUMBS_UP","users":{"totalCount":1}},{"content":"HOORAY","users":{"totalCount":3}}]"#,
        )
        .unwrap();
        let out = parse_reaction_groups(Some(&gh));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].content, "+1");
        assert!(!out[0].viewer_reacted);
        assert_eq!(out[1].content, "hooray");
        assert_eq!(out[1].count, 3);
        // GraphQL shape with viewerHasReacted.
        let gql: serde_json::Value = serde_json::from_str(
            r#"[{"content":"EYES","viewerHasReacted":true,"users":{"totalCount":2}}]"#,
        )
        .unwrap();
        let out = parse_reaction_groups(Some(&gql));
        assert_eq!(out[0].content, "eyes");
        assert!(out[0].viewer_reacted);
        // Unknown content + zero counts dropped; garbage → empty.
        let mixed: serde_json::Value = serde_json::from_str(
            r#"[{"content":"SHRUG","users":{"totalCount":5}},{"content":"HEART","users":{"totalCount":0}},{"bogus":1}]"#,
        )
        .unwrap();
        assert!(parse_reaction_groups(Some(&mixed)).is_empty());
        assert!(parse_reaction_groups(None).is_empty());
        assert!(parse_reaction_groups(Some(&serde_json::Value::Null)).is_empty());
    }

    #[test]
    fn issue_detail_dtos_tolerate_nulls() {
        // milestone null, no reviews/files; verify the issue detail struct compiles and maps.
        let raw: RawIssueDetail = serde_json::from_str(
            r#"{"number":1,"title":"t","body":"","author":null,"state":"CLOSED","stateReason":"completed",
                "labels":[],"assignees":[],"milestone":null,"comments":[],
                "createdAt":"","updatedAt":"","url":"u"}"#,
        )
        .unwrap();
        assert_eq!(raw.number, 1);
        assert_eq!(raw.state_reason.as_deref(), Some("completed"));
        assert!(raw.milestone.is_none());
    }

    #[test]
    fn map_pr_commit_takes_first_author_login() {
        let json = r#"[{
            "oid":"abc1234def","messageHeadline":"Fix bug","committedDate":"2026-01-02T00:00:00Z",
            "authors":[{"login":"alice","name":"Alice"},{"login":"bob"}]
        }]"#;
        let raw: Vec<RawPrCommit> = serde_json::from_str(json).unwrap();
        let c = &raw.into_iter().map(map_pr_commit).collect::<Vec<_>>()[0];
        assert_eq!(c.oid, "abc1234def");
        assert_eq!(c.message, "Fix bug");
        assert_eq!(c.author, "alice");
        assert_eq!(c.committed_date, "2026-01-02T00:00:00Z");
    }

    #[test]
    fn map_pr_commit_tolerates_no_authors() {
        let json = r#"[{"oid":"x","messageHeadline":"","committedDate":"","authors":[]}]"#;
        let raw: Vec<RawPrCommit> = serde_json::from_str(json).unwrap();
        let c = &raw.into_iter().map(map_pr_commit).collect::<Vec<_>>()[0];
        assert_eq!(c.author, "");
    }

    #[test]
    fn parse_review_threads_extracts_resolved_and_comments() {
        let json = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[
            {"id":"PRRT_abc","isResolved":true,"path":"a.rs","line":5,"comments":{"nodes":[
                {"databaseId":987654,"author":{"login":"rev"},"body":"fix this","path":"a.rs","originalLine":5,"line":null,"createdAt":"2026-01-03T00:00:00Z",
                 "reactionGroups":[{"content":"THUMBS_UP","viewerHasReacted":true,"users":{"totalCount":2}},{"content":"HEART","viewerHasReacted":false,"users":{"totalCount":0}}]}
            ]}},
            {"isResolved":false,"path":"b.rs","line":null,"comments":{"nodes":[
                {"author":{"login":"x"},"body":"","path":"b.rs","originalLine":null,"line":null,"createdAt":""}
            ]}}
        ]}}}}}"#;
        let out = parse_review_threads(json);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "PRRT_abc");
        assert!(out[0].resolved);
        assert_eq!(out[0].path, "a.rs");
        assert_eq!(out[0].comments.len(), 1);
        assert_eq!(out[0].comments[0].author, "rev");
        assert_eq!(out[0].comments[0].database_id, Some(987654));
        // line null falls back to originalLine.
        assert_eq!(out[0].comments[0].line, 5);
        // GraphQL reactionGroups: normalized, zero-count dropped, viewer flag kept.
        assert_eq!(out[0].comments[0].reactions.len(), 1);
        assert_eq!(out[0].comments[0].reactions[0].content, "+1");
        assert_eq!(out[0].comments[0].reactions[0].count, 2);
        assert!(out[0].comments[0].reactions[0].viewer_reacted);
        assert!(!out[1].resolved);
        assert_eq!(out[1].id, ""); // missing id → ""
        assert_eq!(out[1].line, 0); // null thread line → 0
        assert_eq!(out[1].comments[0].database_id, None); // missing databaseId → None
        assert!(out[1].comments[0].reactions.is_empty()); // missing reactionGroups → []
    }

    #[test]
    fn parse_review_threads_empty_on_garbage() {
        assert!(parse_review_threads("not json").is_empty());
        assert!(parse_review_threads("{}").is_empty());
    }

    #[test]
    fn parse_check_runs_extracts_runs_with_null_conclusion() {
        let json = r#"{"total_count":2,"workflow_runs":[
            {"name":"CI","status":"completed","conclusion":"success","run_started_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:05:00Z","html_url":"u1","head_sha":"aaa"},
            {"name":"Deploy","status":"in_progress","conclusion":null,"run_started_at":"2026-01-02T00:00:00Z","updated_at":"2026-01-02T00:01:00Z","html_url":"u2","head_sha":"bbb"}
        ]}"#;
        let out = parse_check_runs(json);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "CI");
        assert_eq!(out[0].conclusion, "success");
        assert_eq!(out[0].started_at, "2026-01-01T00:00:00Z");
        assert_eq!(out[0].head_sha, "aaa");
        assert_eq!(out[1].status, "in_progress");
        assert_eq!(out[1].conclusion, ""); // null → ""
    }

    #[test]
    fn parse_check_runs_empty_on_garbage() {
        assert!(parse_check_runs("nope").is_empty());
        assert!(parse_check_runs("{}").is_empty());
    }

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
}
