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

// ── Reactions + comment edit/delete ──────────────────────────────────────────

/// What a reaction/edit/delete targets. For the comment kinds `target` is the
/// numeric REST comment id; for the body kinds it is the PR/issue NUMBER.
/// Deserializes from the TS strings "issueComment" | "reviewComment" |
/// "prBody" | "issueBody".
#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CommentKind {
    IssueComment,
    ReviewComment,
    PrBody,
    IssueBody,
}

/// The 8 REST reaction content names. Returns the matched `&'static str` so an
/// arbitrary string is never forwarded to `gh`.
fn validate_reaction_content(content: &str) -> Result<&'static str, GithubError> {
    const ALLOWED: [&str; 8] = ["+1", "-1", "laugh", "confused", "heart", "hooray", "rocket", "eyes"];
    ALLOWED
        .iter()
        .find(|a| **a == content)
        .copied()
        .ok_or_else(|| GithubError::Other(format!("Invalid reaction: {content}")))
}

/// REST base path for a target's reactions. PRs are issues in REST, so both
/// body kinds react via the issues endpoint.
fn reaction_base_path(owner: &str, name: &str, kind: CommentKind, target: u64) -> String {
    match kind {
        CommentKind::IssueComment => {
            format!("repos/{owner}/{name}/issues/comments/{target}/reactions")
        }
        CommentKind::ReviewComment => {
            format!("repos/{owner}/{name}/pulls/comments/{target}/reactions")
        }
        CommentKind::PrBody | CommentKind::IssueBody => {
            format!("repos/{owner}/{name}/issues/{target}/reactions")
        }
    }
}

/// REST path of an editable/deletable comment; `None` for the body kinds
/// (which edit via `gh pr/issue edit` and can never be deleted).
fn comment_endpoint(owner: &str, name: &str, kind: CommentKind, target: u64) -> Option<String> {
    match kind {
        CommentKind::IssueComment => Some(format!("repos/{owner}/{name}/issues/comments/{target}")),
        CommentKind::ReviewComment => Some(format!("repos/{owner}/{name}/pulls/comments/{target}")),
        CommentKind::PrBody | CommentKind::IssueBody => None,
    }
}

/// Find the viewer's own reaction of `content` in a REST reactions-list
/// response (`[{id, content, user:{login}}, …]`). Panic-safe: malformed JSON
/// or missing fields → `None`.
fn find_own_reaction_id(list_json: &str, login: &str, content: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(list_json).ok()?;
    v.as_array()?.iter().find_map(|r| {
        let user = r.pointer("/user/login")?.as_str()?;
        let c = r.get("content")?.as_str()?;
        if user == login && c == content {
            r.get("id")?.as_u64()
        } else {
            None
        }
    })
}

/// Add (`add=true`) or remove (`add=false`) the viewer's `content` reaction on
/// the target. Add is a single POST (GitHub treats re-adding as a no-op).
/// Remove lists the reactions, finds the viewer's matching one, and DELETEs it
/// by id; nothing to remove → `Ok(())` (idempotent).
pub fn set_reaction(
    repo: &Path,
    kind: CommentKind,
    target: u64,
    content: &str,
    add: bool,
) -> Result<(), GithubError> {
    let content = validate_reaction_content(content)?;
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let base = reaction_base_path(&owner, &name, kind, target);
    if add {
        let body = serde_json::json!({ "content": content }).to_string();
        run_gh(&["api", &base, "--method", "POST", "--input", "-"], Some(&body)).map(|_| ())
    } else {
        let list_path = format!("{base}?per_page=100");
        let list = run_gh(&["api", &list_path], None)?;
        let login = super::current_login()?;
        match find_own_reaction_id(&list, &login, content) {
            Some(id) => {
                let del_path = format!("{base}/{id}");
                run_gh(&["api", &del_path, "--method", "DELETE"], None).map(|_| ())
            }
            None => Ok(()), // already gone — idempotent
        }
    }
}

/// Edit a comment or a PR/issue description. Comment kinds PATCH their REST
/// endpoint with a JSON body over stdin; body kinds go through
/// `gh pr/issue edit --body-file -` (body over stdin, never an arg).
pub fn edit_comment(
    repo: &Path,
    kind: CommentKind,
    target: u64,
    body: &str,
) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    if let Some(path) = comment_endpoint(&owner, &name, kind, target) {
        let json = serde_json::json!({ "body": body }).to_string();
        return run_gh(&["api", &path, "--method", "PATCH", "--input", "-"], Some(&json))
            .map(|_| ());
    }
    let slug = format!("{owner}/{name}");
    let num = target.to_string();
    let noun = match kind {
        CommentKind::PrBody => "pr",
        _ => "issue",
    };
    run_gh(&[noun, "edit", &num, "--repo", &slug, "--body-file", "-"], Some(body)).map(|_| ())
}

/// Delete a comment. Only the comment kinds are deletable; descriptions are
/// part of the PR/issue itself. The kind check runs before any repo/gh work.
pub fn delete_comment(repo: &Path, kind: CommentKind, target: u64) -> Result<(), GithubError> {
    if matches!(kind, CommentKind::PrBody | CommentKind::IssueBody) {
        return Err(GithubError::Other("Descriptions can't be deleted.".into()));
    }
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let Some(path) = comment_endpoint(&owner, &name, kind, target) else {
        // Unreachable after the kind check above; kept for panic-safety.
        return Err(GithubError::Other("Descriptions can't be deleted.".into()));
    };
    run_gh(&["api", &path, "--method", "DELETE"], None).map(|_| ())
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
    fn comment_kind_deserializes_from_ts_strings() {
        let cases = [
            ("\"issueComment\"", CommentKind::IssueComment),
            ("\"reviewComment\"", CommentKind::ReviewComment),
            ("\"prBody\"", CommentKind::PrBody),
            ("\"issueBody\"", CommentKind::IssueBody),
        ];
        for (json, want) in cases {
            let got: CommentKind = serde_json::from_str(json).unwrap();
            assert_eq!(got, want, "{json}");
        }
        assert!(serde_json::from_str::<CommentKind>("\"IssueComment\"").is_err());
        assert!(serde_json::from_str::<CommentKind>("\"bogus\"").is_err());
    }

    #[test]
    fn reaction_content_validated_against_rest_names() {
        for ok in ["+1", "-1", "laugh", "confused", "heart", "hooray", "rocket", "eyes"] {
            assert_eq!(validate_reaction_content(ok).unwrap(), ok);
        }
        assert!(validate_reaction_content("THUMBS_UP").is_err()); // must be REST form
        assert!(validate_reaction_content("shrug").is_err());
        assert!(validate_reaction_content("").is_err());
    }

    #[test]
    fn reaction_base_path_per_kind() {
        assert_eq!(
            reaction_base_path("o", "r", CommentKind::IssueComment, 7),
            "repos/o/r/issues/comments/7/reactions"
        );
        assert_eq!(
            reaction_base_path("o", "r", CommentKind::ReviewComment, 7),
            "repos/o/r/pulls/comments/7/reactions"
        );
        // PRs are issues in REST — both body kinds use the issues endpoint,
        // with the PR/issue NUMBER as the target.
        assert_eq!(
            reaction_base_path("o", "r", CommentKind::PrBody, 42),
            "repos/o/r/issues/42/reactions"
        );
        assert_eq!(
            reaction_base_path("o", "r", CommentKind::IssueBody, 42),
            "repos/o/r/issues/42/reactions"
        );
    }

    #[test]
    fn comment_endpoint_only_for_comment_kinds() {
        assert_eq!(
            comment_endpoint("o", "r", CommentKind::IssueComment, 7).as_deref(),
            Some("repos/o/r/issues/comments/7")
        );
        assert_eq!(
            comment_endpoint("o", "r", CommentKind::ReviewComment, 7).as_deref(),
            Some("repos/o/r/pulls/comments/7")
        );
        assert_eq!(comment_endpoint("o", "r", CommentKind::PrBody, 1), None);
        assert_eq!(comment_endpoint("o", "r", CommentKind::IssueBody, 1), None);
    }

    #[test]
    fn find_own_reaction_id_matches_login_and_content() {
        // Real REST list shape: [{id, content (REST name), user:{login}}].
        let json = r#"[
            {"id":11,"content":"+1","user":{"login":"alice"}},
            {"id":22,"content":"heart","user":{"login":"me"}},
            {"id":33,"content":"+1","user":{"login":"me"}}
        ]"#;
        assert_eq!(find_own_reaction_id(json, "me", "+1"), Some(33));
        assert_eq!(find_own_reaction_id(json, "me", "heart"), Some(22));
        assert_eq!(find_own_reaction_id(json, "me", "eyes"), None);
        assert_eq!(find_own_reaction_id(json, "nobody", "+1"), None);
        // Panic-safe on malformed input.
        assert_eq!(find_own_reaction_id("not json", "me", "+1"), None);
        assert_eq!(find_own_reaction_id("{}", "me", "+1"), None);
        assert_eq!(find_own_reaction_id(r#"[{"id":1}]"#, "me", "+1"), None);
    }

    #[test]
    fn set_reaction_rejects_invalid_content_before_gh() {
        // Invalid content must fail up front — even with a bogus repo path,
        // validation runs first and never reaches resolve/gh.
        let err = set_reaction(
            Path::new("/nonexistent"),
            CommentKind::IssueComment,
            1,
            "THUMBS_UP",
            true,
        )
        .unwrap_err();
        match err {
            GithubError::Other(m) => assert!(m.contains("Invalid reaction"), "{m}"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn delete_comment_refuses_body_kinds() {
        // The kind check runs before any repo resolution or gh call, so a
        // nonexistent path proves nothing external was touched.
        for kind in [CommentKind::PrBody, CommentKind::IssueBody] {
            let err = delete_comment(Path::new("/nonexistent"), kind, 1).unwrap_err();
            match err {
                GithubError::Other(m) => assert_eq!(m, "Descriptions can't be deleted."),
                other => panic!("expected Other, got {other:?}"),
            }
        }
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
