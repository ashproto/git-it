mod commands;
mod git_ops;
mod graph;
mod ops;
mod ops_merge;
mod rewrite;
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
