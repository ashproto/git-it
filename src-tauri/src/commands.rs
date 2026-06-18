use crate::git_ops;
use crate::github;
use crate::graph;
use crate::ops;
use crate::ops_merge;
use crate::ops_remote;
use crate::ops_rewrite;
use crate::ops_worktree;
use crate::rewrite;
use crate::types::{BundleInfo, Commit, ConflictEntry, DateMapping, GraphCommit, OpOutcome, PrerequisiteCheck, RebaseOutcome, RebaseStep, Ref, ReflogEntry, RemoteInfo, RemoteOutcome, RepoStatus, RewriteOptions, RewriteResult, SafetyRef, StashEntry, WorkingFile};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

#[tauri::command]
pub fn check_prerequisites() -> PrerequisiteCheck {
    git_ops::check_prerequisites()
}

#[tauri::command]
pub fn is_git_repo(repo: String) -> bool {
    git_ops::is_git_repo(&PathBuf::from(repo))
}

#[tauri::command]
pub fn load_commits(
    repo: String,
    count: u32,
    range: Option<String>,
) -> Result<Vec<Commit>, String> {
    git_ops::load_commits(
        &PathBuf::from(repo),
        count,
        range.as_deref(),
    )
}

#[tauri::command]
pub fn create_bundle(repo: String) -> Result<String, String> {
    let path = git_ops::create_bundle(&PathBuf::from(repo))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn list_bundles(repo: String) -> Result<Vec<BundleInfo>, String> {
    git_ops::list_bundles(&PathBuf::from(repo))
}

#[tauri::command]
pub fn delete_bundle(path: String) -> Result<(), String> {
    git_ops::delete_bundle(&PathBuf::from(path))
}

#[tauri::command]
pub fn fetch_bundle(repo: String, bundle_path: String) -> Result<String, String> {
    git_ops::fetch_bundle(&PathBuf::from(repo), &PathBuf::from(bundle_path))
}

#[tauri::command]
pub fn list_safety_refs(repo: String) -> Result<Vec<SafetyRef>, String> {
    git_ops::list_safety_refs(&PathBuf::from(repo))
}

#[tauri::command]
pub fn delete_refs(repo: String, refs: Vec<String>) -> Result<(), String> {
    git_ops::delete_refs(&PathBuf::from(repo), &refs)
}

#[tauri::command]
pub fn preview_callback(mappings: Vec<DateMapping>, update_author: bool) -> String {
    rewrite::generate_callback(&mappings, update_author)
}

#[tauri::command]
pub async fn rewrite_history(
    repo: String,
    mappings: Vec<DateMapping>,
    options: RewriteOptions,
    on_event: Channel<String>,
) -> Result<(), String> {
    let auto_bundle = options.auto_bundle;
    let repo_path = PathBuf::from(repo);

    if auto_bundle {
        let _ = on_event.send("[bundle] creating safety bundle...".to_string());
        match git_ops::create_bundle(&repo_path) {
            Ok(p) => {
                let _ = on_event.send(format!("[bundle] saved {}", p.display()));
            }
            Err(e) => {
                let _ = on_event.send(format!("[bundle] WARNING: {}", e));
            }
        }
    }

    let _ = on_event.send(format!(
        "[rewrite] applying {} mapping(s) via git-filter-repo...",
        mappings.len()
    ));

    // The closure captures the channel by clone; tauri::ipc::Channel is cheaply cloneable.
    let chan = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        rewrite::rewrite_history(&repo_path, &mappings, &options, move |line| {
            let _ = chan.send(line);
        })
    })
    .await
    .map_err(|e| format!("join error: {}", e))?
}

#[tauri::command]
pub fn load_graph(repo: String, count: u32, skip: u32) -> Result<Vec<GraphCommit>, String> {
    graph::load_graph(&PathBuf::from(repo), count, skip)
}

#[tauri::command]
pub fn list_refs(repo: String) -> Result<Vec<Ref>, String> {
    graph::list_refs(&PathBuf::from(repo))
}

#[tauri::command]
pub fn repo_status(repo: String) -> Result<RepoStatus, String> {
    graph::repo_status(&PathBuf::from(repo))
}

#[tauri::command]
pub fn checkout(repo: String, target: String) -> Result<String, String> {
    ops::checkout(&PathBuf::from(repo), &target)
}

#[tauri::command]
pub fn create_branch(repo: String, name: String, start_point: String) -> Result<(), String> {
    ops::create_branch(&PathBuf::from(repo), &name, &start_point)
}

#[tauri::command]
pub fn rename_branch(repo: String, old: String, new: String) -> Result<(), String> {
    ops::rename_branch(&PathBuf::from(repo), &old, &new)
}

#[tauri::command]
pub fn delete_branch(repo: String, name: String, force: bool) -> Result<(), String> {
    ops::delete_branch(&PathBuf::from(repo), &name, force)
}

#[tauri::command]
pub fn create_tag(
    repo: String,
    name: String,
    target: String,
    message: Option<String>,
) -> Result<(), String> {
    ops::create_tag(&PathBuf::from(repo), &name, &target, message.as_deref())
}

#[tauri::command]
pub fn delete_tag(repo: String, name: String) -> Result<(), String> {
    ops::delete_tag(&PathBuf::from(repo), &name)
}

#[tauri::command]
pub fn fetch(repo: String, remote: Option<String>) -> Result<String, String> {
    ops::fetch(&PathBuf::from(repo), remote.as_deref())
}

#[tauri::command]
pub fn merge(repo: String, reference: String, no_ff: bool, squash: bool) -> Result<OpOutcome, String> {
    ops_merge::merge(&PathBuf::from(repo), &reference, no_ff, squash)
}

#[tauri::command]
pub fn cherry_pick(repo: String, shas: Vec<String>) -> Result<OpOutcome, String> {
    ops_merge::cherry_pick(&PathBuf::from(repo), &shas)
}

#[tauri::command]
pub fn revert(repo: String, shas: Vec<String>) -> Result<OpOutcome, String> {
    ops_merge::revert(&PathBuf::from(repo), &shas)
}

#[tauri::command]
pub fn op_abort(repo: String, kind: String) -> Result<(), String> {
    ops_merge::abort(&PathBuf::from(repo), &kind)
}

#[tauri::command]
pub fn op_continue(repo: String, kind: String) -> Result<OpOutcome, String> {
    ops_merge::continue_op(&PathBuf::from(repo), &kind)
}

#[tauri::command]
pub fn resolve_conflict(repo: String, path: String, ours: bool) -> Result<(), String> {
    ops_merge::resolve_side(&PathBuf::from(repo), &path, ours)
}

#[tauri::command]
pub fn conflicted_files(repo: String) -> Result<Vec<String>, String> {
    ops_merge::conflicted_files(&PathBuf::from(repo))
}

#[tauri::command]
pub fn conflict_details(repo: String) -> Result<Vec<ConflictEntry>, String> {
    ops_merge::conflict_details(&PathBuf::from(repo))
}

#[tauri::command]
pub fn resolve_keep(repo: String, path: String) -> Result<(), String> {
    ops_merge::resolve_keep(&PathBuf::from(repo), &path)
}

#[tauri::command]
pub fn resolve_remove(repo: String, path: String) -> Result<(), String> {
    ops_merge::resolve_remove(&PathBuf::from(repo), &path)
}

#[tauri::command]
pub fn op_skip(repo: String, kind: String) -> Result<OpOutcome, String> {
    ops_merge::skip(&PathBuf::from(repo), &kind)
}

#[tauri::command]
pub fn reset(repo: String, target: String, mode: String, auto_backup: bool) -> Result<RewriteResult, String> {
    ops_rewrite::reset(&PathBuf::from(repo), &target, &mode, auto_backup)
}

#[tauri::command]
pub fn amend(
    repo: String,
    message: Option<String>,
    reset_author_date: bool,
    reset_committer_date: bool,
    auto_backup: bool,
) -> Result<RewriteResult, String> {
    ops_rewrite::amend(&PathBuf::from(repo), message.as_deref(), reset_author_date, reset_committer_date, auto_backup)
}

#[tauri::command]
pub fn undo_op(repo: String, sha: String) -> Result<(), String> {
    crate::safety::restore(&PathBuf::from(repo), &sha)
}

#[tauri::command]
pub fn rebase(repo: String, onto: String, auto_backup: bool) -> Result<RebaseOutcome, String> {
    ops_rewrite::rebase(&PathBuf::from(repo), &onto, auto_backup)
}

#[tauri::command]
pub fn rebase_todo_preview(repo: String, base: String) -> Result<Vec<ReflogEntry>, String> {
    ops_rewrite::rebase_todo_preview(&PathBuf::from(repo), &base)
}

#[tauri::command]
pub fn commit_message(repo: String, sha: String) -> Result<String, String> {
    ops_rewrite::commit_message(&PathBuf::from(repo), &sha)
}

#[tauri::command]
pub fn count_merges_in_range(repo: String, base: String) -> Result<usize, String> {
    ops_rewrite::count_merges_in_range(&PathBuf::from(repo), &base)
}

#[tauri::command]
pub fn rebase_interactive(repo: String, base: String, steps: Vec<RebaseStep>, auto_backup: bool) -> Result<RebaseOutcome, String> {
    ops_rewrite::rebase_interactive(&PathBuf::from(repo), &base, &steps, auto_backup)
}

#[tauri::command]
pub fn reflog(repo: String, limit: u32) -> Result<Vec<ReflogEntry>, String> {
    ops_rewrite::reflog(&PathBuf::from(repo), limit)
}

#[tauri::command]
pub fn working_changes(repo: String) -> Result<Vec<WorkingFile>, String> {
    ops_worktree::working_changes(&PathBuf::from(repo))
}

#[tauri::command]
pub fn stage(repo: String, paths: Vec<String>) -> Result<(), String> {
    ops_worktree::stage(&PathBuf::from(repo), &paths)
}

#[tauri::command]
pub fn unstage(repo: String, paths: Vec<String>) -> Result<(), String> {
    ops_worktree::unstage(&PathBuf::from(repo), &paths)
}

#[tauri::command]
pub fn discard(repo: String, paths: Vec<String>) -> Result<(), String> {
    ops_worktree::discard(&PathBuf::from(repo), &paths)
}

#[tauri::command]
pub fn clean(repo: String, paths: Vec<String>) -> Result<(), String> {
    ops_worktree::clean(&PathBuf::from(repo), &paths)
}

#[tauri::command]
pub fn commit(repo: String, message: String, signoff: bool) -> Result<(), String> {
    ops_worktree::commit(&PathBuf::from(repo), &message, signoff)
}

#[tauri::command]
pub fn diff(
    repo: String,
    path: Option<String>,
    staged: bool,
    untracked: bool,
    context: u32,
) -> Result<String, String> {
    let repo = PathBuf::from(repo);
    if untracked {
        match path.as_deref() {
            Some(p) => ops_worktree::diff_untracked(&repo, p, context),
            None => Ok(String::new()),
        }
    } else {
        ops_worktree::diff(&repo, path.as_deref(), staged, context)
    }
}

#[tauri::command]
pub fn commit_diff(
    repo: String,
    sha: String,
    path: Option<String>,
    context: u32,
) -> Result<String, String> {
    ops_worktree::commit_diff(&PathBuf::from(repo), &sha, path.as_deref(), context)
}

#[tauri::command]
pub fn stage_hunk(repo: String, path: String, hunk_index: usize, context: u32) -> Result<(), String> {
    ops_worktree::stage_hunk(&PathBuf::from(repo), &path, hunk_index, context)
}

#[tauri::command]
pub fn unstage_hunk(repo: String, path: String, hunk_index: usize, context: u32) -> Result<(), String> {
    ops_worktree::unstage_hunk(&PathBuf::from(repo), &path, hunk_index, context)
}

#[tauri::command]
pub fn stage_lines(repo: String, path: String, hunk_index: usize, selected: Vec<usize>, context: u32) -> Result<(), String> {
    ops_worktree::stage_lines(&PathBuf::from(repo), &path, hunk_index, &selected, context)
}

#[tauri::command]
pub fn unstage_lines(repo: String, path: String, hunk_index: usize, selected: Vec<usize>, context: u32) -> Result<(), String> {
    ops_worktree::unstage_lines(&PathBuf::from(repo), &path, hunk_index, &selected, context)
}

#[tauri::command]
pub fn stash_push(repo: String, message: Option<String>) -> Result<(), String> {
    ops_worktree::stash_push(&PathBuf::from(repo), message.as_deref())
}

#[tauri::command]
pub fn stash_list(repo: String) -> Result<Vec<StashEntry>, String> {
    ops_worktree::stash_list(&PathBuf::from(repo))
}

#[tauri::command]
pub fn stash_apply(repo: String, index: u32) -> Result<(), String> {
    ops_worktree::stash_apply(&PathBuf::from(repo), index)
}

#[tauri::command]
pub fn stash_pop(repo: String, index: u32) -> Result<(), String> {
    ops_worktree::stash_pop(&PathBuf::from(repo), index)
}

#[tauri::command]
pub fn stash_drop(repo: String, index: u32) -> Result<(), String> {
    ops_worktree::stash_drop(&PathBuf::from(repo), index)
}

// ── Remote management ────────────────────────────────────────────────────────

#[tauri::command]
pub fn remotes(repo: String) -> Result<Vec<RemoteInfo>, String> {
    ops_remote::remotes(&PathBuf::from(repo))
}

#[tauri::command]
pub fn remote_add(repo: String, name: String, url: String) -> Result<(), String> {
    ops_remote::remote_add(&PathBuf::from(repo), &name, &url)
}

#[tauri::command]
pub fn remote_remove(repo: String, name: String) -> Result<(), String> {
    ops_remote::remote_remove(&PathBuf::from(repo), &name)
}

#[tauri::command]
pub fn remote_set_url(repo: String, name: String, url: String) -> Result<(), String> {
    ops_remote::remote_set_url(&PathBuf::from(repo), &name, &url)
}

// ── Streamed pull / push / cancel ────────────────────────────────────────────
//
// Tauri `State<'_, T>` is not 'static, so we cannot move it into spawn_blocking.
// The solution: manage `Arc<RemoteState>` in lib.rs; commands take
// `State<'_, Arc<ops_remote::RemoteState>>` and clone the Arc before spawn_blocking.

#[tauri::command]
pub async fn pull(
    repo: String,
    rebase: bool,
    username: Option<String>,
    password: Option<String>,
    on_event: Channel<String>,
    state: State<'_, Arc<ops_remote::RemoteState>>,
) -> Result<RemoteOutcome, String> {
    let st = state.inner().clone(); // clone the Arc — now 'static-safe
    let repo_path = PathBuf::from(repo);
    let chan = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops_remote::pull(
            &repo_path,
            rebase,
            username.as_deref(),
            password.as_deref(),
            &st,
            &|l| { let _ = chan.send(l); },
        )
    })
    .await
    .map_err(|e| format!("join: {}", e))?
}

#[tauri::command]
pub async fn push(
    repo: String,
    remote: String,
    refspec: Option<String>,
    force_with_lease: bool,
    set_upstream: bool,
    username: Option<String>,
    password: Option<String>,
    on_event: Channel<String>,
    state: State<'_, Arc<ops_remote::RemoteState>>,
) -> Result<RemoteOutcome, String> {
    let st = state.inner().clone();
    let repo_path = PathBuf::from(repo);
    let chan = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops_remote::push(
            &repo_path,
            &remote,
            refspec.as_deref(),
            force_with_lease,
            set_upstream,
            username.as_deref(),
            password.as_deref(),
            &st,
            &|l| { let _ = chan.send(l); },
        )
    })
    .await
    .map_err(|e| format!("join: {}", e))?
}

/// Kill the in-flight remote operation. Synchronous — no spawn needed.
#[tauri::command]
pub fn cancel_remote(state: State<'_, Arc<ops_remote::RemoteState>>) {
    ops_remote::cancel(state.inner());
}

// ── GitHub integration ────────────────────────────────────────────────────────

#[tauri::command]
pub fn github_availability(repo: String) -> github::GhAvailability {
    github::availability(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_repo_stats(repo: String) -> Result<github::GhRepoStats, github::GithubError> {
    github::repo_stats(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_pulls(
    repo: String,
    state: String,
    limit: u32,
) -> Result<Vec<github::GhPull>, github::GithubError> {
    github::pulls(&PathBuf::from(repo), &state, limit)
}

#[tauri::command]
pub fn github_issues(
    repo: String,
    state: String,
    limit: u32,
) -> Result<Vec<github::GhIssue>, github::GithubError> {
    github::issues(&PathBuf::from(repo), &state, limit)
}

#[tauri::command]
pub fn github_releases(repo: String) -> Result<Vec<github::GhRelease>, github::GithubError> {
    github::releases(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_runs(repo: String, limit: u32) -> Result<Vec<github::GhRun>, github::GithubError> {
    github::runs(&PathBuf::from(repo), limit)
}

#[tauri::command]
pub fn github_traffic(repo: String) -> Result<github::GhTraffic, github::GithubError> {
    github::traffic(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_contributors(
    repo: String,
    limit: u32,
) -> Result<Vec<github::GhContributor>, github::GithubError> {
    github::contributors(&PathBuf::from(repo), limit)
}

#[tauri::command]
pub fn github_activity(repo: String) -> Result<github::GhActivity, github::GithubError> {
    github::commit_activity(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_milestones(repo: String) -> Result<Vec<github::GhMilestone>, github::GithubError> {
    github::milestones(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_labels(repo: String) -> Result<Vec<github::GhLabel>, github::GithubError> {
    github::labels(&PathBuf::from(repo))
}

#[tauri::command]
pub fn github_pr_comment(repo: String, number: u64, body: String) -> Result<(), github::GithubError> {
    github::pr_comment(&PathBuf::from(repo), number, &body)
}

#[tauri::command]
pub fn github_issue_comment(repo: String, number: u64, body: String) -> Result<(), github::GithubError> {
    github::issue_comment(&PathBuf::from(repo), number, &body)
}

#[tauri::command]
pub fn github_issue_set_state(repo: String, number: u64, state: String) -> Result<(), github::GithubError> {
    github::issue_set_state(&PathBuf::from(repo), number, &state)
}

#[tauri::command]
pub fn github_pr_merge(repo: String, number: u64, method: String) -> Result<(), github::GithubError> {
    github::pr_merge(&PathBuf::from(repo), number, &method)
}

#[tauri::command]
pub fn github_issue_create(repo: String, title: String, body: String) -> Result<String, github::GithubError> {
    github::issue_create(&PathBuf::from(repo), &title, &body)
}
