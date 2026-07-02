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

/// Body for the reply-to-review-comment REST call — serde_json handles all
/// escaping, so the user's text never touches a shell string.
fn reply_body_json(body: &str) -> String {
    serde_json::json!({ "body": body }).to_string()
}

/// REST path for replying to an existing inline review comment. All segments
/// are interpolated by Rust from numeric/resolved values — no user text.
fn reply_api_path(owner: &str, name: &str, pr_number: u64, comment_id: u64) -> String {
    format!("repos/{owner}/{name}/pulls/{pr_number}/comments/{comment_id}/replies")
}

/// Reply to an inline review thread via the REST "replies" endpoint. The
/// target is the thread's first comment id; the body goes over stdin.
pub fn pr_reply_thread(
    repo: &Path,
    pr_number: u64,
    comment_id: u64,
    body: &str,
) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let path = reply_api_path(&owner, &name, pr_number, comment_id);
    run_gh(
        &["api", &path, "--method", "POST", "--input", "-"],
        Some(&reply_body_json(body)),
    )
    .map(|_| ())
}

/// The GraphQL mutation for (un)resolving a review thread — REST has no
/// equivalent endpoint.
fn resolve_mutation(resolve: bool) -> &'static str {
    if resolve {
        "mutation($id: ID!) { resolveReviewThread(input: {threadId: $id}) { thread { id } } }"
    } else {
        "mutation($id: ID!) { unresolveReviewThread(input: {threadId: $id}) { thread { id } } }"
    }
}

/// GraphQL node ids are base64-ish (`PRRT_…`); reject anything else before
/// shelling out so a hostile id can never reach `gh` as an argument.
fn validate_thread_id(id: &str) -> Result<(), GithubError> {
    let ok = !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'=' | b'+' | b'/' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(GithubError::Other("Invalid thread id.".into()))
    }
}

/// Resolve or unresolve an inline review thread via a GraphQL mutation. The
/// query and id travel as separate `-f key=value` args (single argv entries —
/// never a shell string).
pub fn pr_resolve_thread(repo: &Path, thread_id: &str, resolve: bool) -> Result<(), GithubError> {
    validate_thread_id(thread_id)?;
    resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let q_arg = format!("query={}", resolve_mutation(resolve));
    let id_arg = format!("id={thread_id}");
    run_gh(&["api", "graphql", "-f", &q_arg, "-f", &id_arg], None).map(|_| ())
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
    fn reply_body_json_escapes() {
        let body = reply_body_json("line1\n\"quoted\" — done");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["body"], "line1\n\"quoted\" — done");
        assert_eq!(v.as_object().unwrap().len(), 1);
    }

    #[test]
    fn reply_api_path_shape() {
        assert_eq!(
            reply_api_path("ashproto", "git-it", 42, 987654),
            "repos/ashproto/git-it/pulls/42/comments/987654/replies"
        );
    }

    #[test]
    fn resolve_mutation_selects_by_flag() {
        assert_eq!(
            resolve_mutation(true),
            "mutation($id: ID!) { resolveReviewThread(input: {threadId: $id}) { thread { id } } }"
        );
        assert_eq!(
            resolve_mutation(false),
            "mutation($id: ID!) { unresolveReviewThread(input: {threadId: $id}) { thread { id } } }"
        );
    }

    #[test]
    fn thread_id_validator_rejects_bad_ids() {
        assert!(validate_thread_id("").is_err());
        assert!(validate_thread_id("abc def").is_err());
        assert!(validate_thread_id("$(rm -rf)").is_err());
        assert!(validate_thread_id("id;drop").is_err());
    }

    #[test]
    fn thread_id_validator_accepts_node_ids() {
        assert!(validate_thread_id("PRRT_kwDOJx2R3M5FQz-a").is_ok());
        assert!(validate_thread_id("MDIzOlB1bGxSZXF1ZXN0UmV2aWV3VGhyZWFkMQ==").is_ok());
        assert!(validate_thread_id("a+b/c_d-e=").is_ok());
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
