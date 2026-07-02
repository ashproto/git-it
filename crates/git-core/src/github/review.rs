//! PR-review functionality: diffs, review threads, and review actions.
//! Sibling of the main `github` module; shares its `gh` runner and error type.

use std::path::Path;

use super::{resolve_owner_repo, run_gh, GithubError};

/// Args for `gh pr diff <number> --repo <slug>` — the PR number and slug are
/// passed as separate operands (never a shell string).
fn pr_diff_args(slug: &str, number: u64) -> Vec<String> {
    vec![
        "pr".into(),
        "diff".into(),
        number.to_string(),
        "--repo".into(),
        slug.into(),
    ]
}

/// Fetch the unified diff of a pull request via `gh pr diff`.
pub fn pr_diff(repo: &Path, number: u64) -> Result<String, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let args = pr_diff_args(&slug, number);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_gh(&refs, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_diff_args_shape() {
        let args = pr_diff_args("ashproto/git-it", 42);
        assert_eq!(args, vec!["pr", "diff", "42", "--repo", "ashproto/git-it"]);
    }
}
