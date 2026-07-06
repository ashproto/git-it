mod commands;
mod filter_repo;
mod fswatch;
mod openwith;
mod updater;

// `Manager` is used by the desktop `app.manage(...)` / `app.handle()` calls in
// `setup`. Gating to `desktop` keeps it out of any future mobile build without
// tripping an unused-import warning.
#[cfg(desktop)]
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    git_core::path_setup::ensure_homebrew_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        // Manage Arc<RemoteState> so pull/push/cancel_remote commands can share it
        // across async spawn_blocking boundaries (State<'_> is not 'static).
        .manage(std::sync::Arc::new(git_core::ops_remote::RemoteState::default()))
        // Manage the filesystem watcher so the active repo's worktree can be
        // watched for live "Local Changes" updates (see fswatch.rs).
        .manage(fswatch::WatchState::default())
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                // The updater's restart step (renderer → @tauri-apps/plugin-process
                // relaunch → `plugin:process|restart`) needs this plugin registered,
                // or the final "Restart Now" call rejects and the update never applies.
                app.handle().plugin(tauri_plugin_process::init())?;
                app.manage(updater::PendingUpdate::default());
            }

            // macOS ONLY: add "Settings…" and "Check for Updates…" to the app
            // (app-name) menu, under About. We start from the platform default
            // menu so every standard item (Edit, Window, Hide, Quit, …) is
            // preserved and only insert the two extra items; each click emits an
            // event the frontend routes to its existing flow (open Settings /
            // manual update-check). Windows/Linux reach these through the in-app
            // Settings panel instead.
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::Emitter;
                let menu = Menu::default(app.handle())?;
                let settings = MenuItem::with_id(
                    app.handle(),
                    "open-settings",
                    "Settings…",
                    true,
                    None::<&str>,
                )?;
                let check_updates = MenuItem::with_id(
                    app.handle(),
                    "check-updates",
                    "Check for Updates…",
                    true,
                    None::<&str>,
                )?;
                let items = menu.items()?;
                if let Some(app_menu) = items.first().and_then(|item| item.as_submenu()) {
                    // Insert just under "About" (index 0): Settings…, then Check for Updates…
                    app_menu.insert(&settings, 1)?;
                    app_menu.insert(&check_updates, 2)?;
                }
                app.set_menu(menu)?;
                app.on_menu_event(|app_handle, event| match event.id().as_ref() {
                    "open-settings" => {
                        let _ = app_handle.emit("menu:open-settings", ());
                    }
                    "check-updates" => {
                        let _ = app_handle.emit("menu:check-updates", ());
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_prerequisites,
            commands::install_command_line_tools,
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
            commands::fast_forward_branch,
            commands::delete_remote_branch,
            openwith::apps_for_file,
            commands::branch_subjects,
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
            commands::commit_message,
            commands::count_merges_in_range,
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
            commands::github_availability,
            commands::github_repo_stats,
            commands::github_pulls,
            commands::github_issues,
            commands::github_releases,
            commands::github_runs,
            commands::github_traffic,
            commands::github_contributors,
            commands::github_activity,
            commands::github_milestones,
            commands::github_labels,
            commands::github_pr_comment,
            commands::github_issue_comment,
            commands::github_issue_set_state,
            commands::github_pr_merge,
            commands::github_issue_create,
            commands::github_pr_detail,
            commands::github_issue_detail,
            commands::github_pr_diff,
            commands::github_pr_submit_review,
            commands::github_pr_reply_thread,
            commands::github_pr_resolve_thread,
            commands::github_current_login,
            commands::github_toggle_reaction,
            commands::github_edit_comment,
            commands::github_delete_comment,
            commands::github_readme,
            commands::github_create_repo,
            commands::github_pr_create,
            commands::github_pr_checkout,
            fswatch::start_watch,
            fswatch::stop_watch,
            #[cfg(desktop)]
            updater::check_update_on_channel,
            #[cfg(desktop)]
            updater::install_pending_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
