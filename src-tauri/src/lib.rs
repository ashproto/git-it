mod commands;
mod git_ops;
mod graph;
mod ops;
mod ops_merge;
mod ops_remote;
mod ops_rewrite;
mod ops_worktree;
mod rewrite;
mod safety;
mod types;

/// When the app is launched from Finder/Dock, it inherits launchd's bare PATH
/// (typically /usr/bin:/bin:/usr/sbin:/sbin) — not the user's shell PATH. That
/// means `git` (in /usr/bin) is found but `git-filter-repo` (in
/// /opt/homebrew/bin on Apple Silicon, /usr/local/bin on Intel Homebrew) is not.
/// Prepend both Homebrew bin paths so child processes can find it.
fn ensure_homebrew_path() {
    let current = std::env::var("PATH").unwrap_or_default();
    let extras = ["/opt/homebrew/bin", "/usr/local/bin"];
    let mut to_prepend: Vec<&str> = Vec::new();
    for p in &extras {
        if !current.split(':').any(|seg| seg == *p) {
            to_prepend.push(p);
        }
    }
    if to_prepend.is_empty() {
        return;
    }
    let prefix = to_prepend.join(":");
    let new_path = if current.is_empty() {
        prefix
    } else {
        format!("{}:{}", prefix, current)
    };
    // SAFETY: called once at process startup before any threads or child
    // processes are spawned. set_var is only racy in multi-threaded contexts.
    unsafe { std::env::set_var("PATH", new_path) };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    ensure_homebrew_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        // Manage Arc<RemoteState> so pull/push/cancel_remote commands can share it
        // across async spawn_blocking boundaries (State<'_> is not 'static).
        .manage(std::sync::Arc::new(ops_remote::RemoteState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::check_prerequisites,
            commands::is_git_repo,
            commands::load_commits,
            commands::load_graph,
            commands::list_refs,
            commands::repo_status,
            commands::checkout,
            commands::create_branch,
            commands::rename_branch,
            commands::delete_branch,
            commands::create_tag,
            commands::delete_tag,
            commands::fetch,
            commands::merge,
            commands::cherry_pick,
            commands::revert,
            commands::op_abort,
            commands::op_continue,
            commands::resolve_conflict,
            commands::conflicted_files,
            commands::conflict_details,
            commands::resolve_keep,
            commands::resolve_remove,
            commands::op_skip,
            commands::create_bundle,
            commands::list_bundles,
            commands::delete_bundle,
            commands::fetch_bundle,
            commands::list_safety_refs,
            commands::delete_refs,
            commands::preview_callback,
            commands::rewrite_history,
            commands::reset,
            commands::amend,
            commands::undo_op,
            commands::rebase,
            commands::rebase_todo_preview,
            commands::rebase_interactive,
            commands::reflog,
            commands::working_changes,
            commands::stage,
            commands::unstage,
            commands::discard,
            commands::clean,
            commands::commit,
            commands::diff,
            commands::commit_diff,
            commands::stage_hunk,
            commands::unstage_hunk,
            commands::stage_lines,
            commands::unstage_lines,
            commands::stash_push,
            commands::stash_list,
            commands::stash_apply,
            commands::stash_pop,
            commands::stash_drop,
            commands::remotes,
            commands::remote_add,
            commands::remote_remove,
            commands::remote_set_url,
            commands::pull,
            commands::push,
            commands::cancel_remote,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
