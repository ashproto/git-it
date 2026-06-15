use crate::git_ops;
use crate::graph;
use crate::ops;
use crate::ops_merge;
use crate::rewrite;
use crate::types::{BundleInfo, Commit, DateMapping, GraphCommit, OpOutcome, PrerequisiteCheck, Ref, RepoStatus, RewriteOptions, SafetyRef};
use std::path::PathBuf;
use tauri::ipc::Channel;

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
