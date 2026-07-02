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

/// One inline line-anchored review comment, drafted in the UI and submitted
/// as part of a one-shot review. Field names double as the REST payload keys.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DraftComment {
    pub path: String,
    pub line: u64,
    pub side: String, // "LEFT" | "RIGHT"
    pub body: String,
}

/// GitHub accepts exactly these review events; reject anything else up front
/// so a typo never reaches the API.
fn validate_review_event(event: &str) -> Result<(), GithubError> {
    match event {
        "APPROVE" | "REQUEST_CHANGES" | "COMMENT" => Ok(()),
        _ => Err(GithubError::Other(format!("Invalid review event: {event}"))),
    }
}

/// Every comment must anchor to the LEFT (old) or RIGHT (new) side of the diff.
fn validate_comment_sides(comments: &[DraftComment]) -> Result<(), GithubError> {
    for c in comments {
        if c.side != "LEFT" && c.side != "RIGHT" {
            return Err(GithubError::Other(format!(
                "Invalid comment side: {}",
                c.side
            )));
        }
    }
    Ok(())
}

/// Build the `POST …/pulls/{n}/reviews` request body. `body` and `comments`
/// are omitted entirely when empty (GitHub treats an empty body/array
/// differently from an absent key).
fn review_request_json(event: &str, body: &str, comments: &[DraftComment]) -> String {
    let mut map = serde_json::Map::new();
    map.insert("event".into(), serde_json::json!(event));
    if !body.is_empty() {
        map.insert("body".into(), serde_json::json!(body));
    }
    if !comments.is_empty() {
        let arr: Vec<serde_json::Value> = comments
            .iter()
            .map(|c| {
                serde_json::json!({
                    "path": c.path,
                    "line": c.line,
                    "side": c.side,
                    "body": c.body,
                })
            })
            .collect();
        map.insert("comments".into(), serde_json::Value::Array(arr));
    }
    serde_json::Value::Object(map).to_string()
}

/// Submit a one-shot PR review (verdict + summary + inline comments) via
/// `gh api` — a single atomic REST call; the JSON body goes over stdin.
/// No extra client-side rules beyond event/side shape: GitHub's own
/// validation errors must surface verbatim so the frontend can keep the draft.
pub fn pr_submit_review(
    repo: &Path,
    number: u64,
    event: &str,
    body: &str,
    comments: Vec<DraftComment>,
) -> Result<(), GithubError> {
    validate_review_event(event)?;
    validate_comment_sides(&comments)?;
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let path = format!("repos/{owner}/{name}/pulls/{number}/reviews");
    run_gh(
        &["api", &path, "--method", "POST", "--input", "-"],
        Some(&review_request_json(event, body, &comments)),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_diff_args_shape() {
        let args = pr_diff_args("ashproto/git-it", 42);
        assert_eq!(args, vec!["pr", "diff", "42", "--repo", "ashproto/git-it"]);
    }

    #[test]
    fn review_body_serializes_comments() {
        let body = review_request_json(
            "REQUEST_CHANGES",
            "needs work",
            &[DraftComment {
                path: "src/a.ts".into(),
                line: 12,
                side: "RIGHT".into(),
                body: "why?".into(),
            }],
        );
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["event"], "REQUEST_CHANGES");
        assert_eq!(v["body"], "needs work");
        assert_eq!(v["comments"][0]["path"], "src/a.ts");
        assert_eq!(v["comments"][0]["line"], 12);
        assert_eq!(v["comments"][0]["side"], "RIGHT");
    }

    #[test]
    fn review_body_omits_empty_comments_and_body() {
        let body = review_request_json("APPROVE", "", &[]);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v.get("comments").is_none());
        assert!(v.get("body").is_none());
        assert_eq!(v["event"], "APPROVE");
    }

    #[test]
    fn submit_review_rejects_bad_event() {
        assert!(validate_review_event("LGTM").is_err());
        assert!(validate_review_event("APPROVE").is_ok());
        assert!(validate_review_event("REQUEST_CHANGES").is_ok());
        assert!(validate_review_event("COMMENT").is_ok());
    }

    #[test]
    fn submit_review_rejects_bad_side() {
        assert!(validate_comment_sides(&[DraftComment {
            path: "a".into(),
            line: 1,
            side: "MIDDLE".into(),
            body: "x".into()
        }])
        .is_err());
        assert!(validate_comment_sides(&[DraftComment {
            path: "a".into(),
            line: 1,
            side: "LEFT".into(),
            body: "x".into()
        }])
        .is_ok());
    }
}
