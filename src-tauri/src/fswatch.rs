//! Filesystem watcher for live "Local Changes" updates.
//!
//! Watches the active repo's worktree and pushes a debounced "changed" event to
//! the frontend (via the same `tauri::ipc::Channel` pattern used by streamed
//! pull/push) whenever a non-`.git` path changes — so the working-copy file list
//! refreshes in realtime instead of only after explicit git operations.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::State;

/// Holds the single active watcher. Only one repo's changes are shown at a time,
/// so one watcher suffices; starting a new one replaces (and thereby stops) the
/// previous. Dropping the watcher stops watching AND — because the notify event
/// handler owns the only `Sender` — disconnects the debounce thread, which exits.
#[derive(Default)]
pub struct WatchState {
    current: Mutex<Option<RecommendedWatcher>>,
}

/// Start watching `repo`'s worktree, replacing any existing watcher. Sends
/// `"changed"` over `on_change` after a brief quiet period whenever a non-`.git`
/// path changes. Returns an error if the watcher can't be created or started.
#[tauri::command]
pub fn start_watch(
    repo: String,
    on_change: Channel<String>,
    state: State<'_, WatchState>,
) -> Result<(), String> {
    let root = PathBuf::from(&repo);

    // (tx, rx): the notify handler signals raw events; the debounce thread drains.
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            // Ignore churn inside `.git` (index/refs/logs/objects). We only care
            // about worktree file changes here; without this the app's own git
            // operations and git's own bookkeeping would spam refreshes.
            if ev
                .paths
                .iter()
                .any(|p| p.components().any(|c| c.as_os_str() == ".git"))
            {
                return;
            }
            // Best-effort: if the debounce thread is gone the send fails harmlessly.
            let _ = tx.send(());
        }
    })
    .map_err(|e| format!("watch init: {e}"))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("watch start: {e}"))?;

    // Debounce/coalesce: block for the first event, then drain a 300ms quiet
    // window so a burst (e.g. a save touching several files, or an editor's
    // atomic write-rename) yields a single refresh. The thread exits when the
    // Sender is dropped (watcher replaced/stopped → `rx` disconnects).
    std::thread::spawn(move || loop {
        if rx.recv().is_err() {
            return; // watcher dropped
        }
        loop {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(()) => continue, // more events in the window — keep draining
                Err(mpsc::RecvTimeoutError::Timeout) => break, // quiet → emit
                Err(mpsc::RecvTimeoutError::Disconnected) => return, // watcher dropped
            }
        }
        let _ = on_change.send("changed".to_string());
    });

    // Store the new watcher; assigning over the previous `Some` drops the old
    // watcher here, which stops the old watch and ends its debounce thread.
    *state.current.lock().map_err(|e| format!("lock: {e}"))? = Some(watcher);
    Ok(())
}

/// Stop watching (e.g. when the last repo is closed). Idempotent.
#[tauri::command]
pub fn stop_watch(state: State<'_, WatchState>) -> Result<(), String> {
    *state.current.lock().map_err(|e| format!("lock: {e}"))? = None;
    Ok(())
}
