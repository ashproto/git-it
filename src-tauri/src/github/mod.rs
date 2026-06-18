//! GitHub integration: spawns the `gh` CLI and parses JSON. Auth is delegated to
//! `gh` (no token is stored here). All owner/repo operands are validated before
//! reaching `gh`, and `gh` is spawned with args passed directly (never a shell).

use crate::ops_remote;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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
}
