use crate::git_ops;
use crate::rewrite;
use crate::types::{BundleInfo, Commit, DateMapping, PrerequisiteCheck, RewriteOptions, SafetyRef};
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
