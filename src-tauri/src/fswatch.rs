//! Filesystem watcher for live updates.
//!
//! Watches the active repo and pushes debounced, CLASSIFIED change events to the
//! frontend (via the same `tauri::ipc::Channel` pattern used by streamed
//! pull/push):
//!   - `"git"`   — a ref/HEAD/log change (commit, branch, checkout, fetch,
//!                 merge, reset…) made by the app OR externally → reload the graph.
//!   - `"local"` — a worktree file or index change → refresh Local Changes.
//!   - `"both"`  — both kinds occurred within one debounce window.
//! Object writes (`.git/objects`), lock files and other `.git` churn are ignored,
//! so git's own bookkeeping doesn't spam refreshes.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::State;

/// Which refresh a filesystem change should trigger.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Git,
    Local,
}

/// Classify a changed path, or `None` to ignore it. Worktree paths and the index
/// are `Local`; ref/HEAD/log changes are `Git`; object writes and lock files are
/// pure churn we drop.
fn classify(path: &Path) -> Option<Kind> {
    let comps: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    match comps.iter().position(|c| *c == ".git") {
        None => Some(Kind::Local), // a worktree path
        Some(i) => {
            let rest = &comps[i + 1..];
            let last = *rest.last()?; // the `.git` dir itself → None
            if last.ends_with(".lock") || rest.iter().any(|c| *c == "objects") {
                return None;
            }
            if rest.iter().any(|c| *c == "refs" || *c == "logs") {
                return Some(Kind::Git);
            }
            const HEADISH: &[&str] = &[
                "HEAD",
                "packed-refs",
                "ORIG_HEAD",
                "MERGE_HEAD",
                "CHERRY_PICK_HEAD",
                "REVERT_HEAD",
                "REBASE_HEAD",
                "FETCH_HEAD",
            ];
            if HEADISH.contains(&last) {
                Some(Kind::Git)
            } else if last == "index" {
                Some(Kind::Local) // external staging
            } else {
                None
            }
        }
    }
}

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

    // (tx, rx): the notify handler classifies + signals; the debounce thread drains.
    let (tx, rx) = mpsc::channel::<Kind>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            for p in &ev.paths {
                if let Some(kind) = classify(p) {
                    // Best-effort: if the debounce thread is gone the send fails harmlessly.
                    let _ = tx.send(kind);
                }
            }
        }
    })
    .map_err(|e| format!("watch init: {e}"))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("watch start: {e}"))?;

    // Debounce/coalesce: block for the first event, then drain a 300ms quiet
    // window so a burst (a save touching several files, a commit touching HEAD +
    // refs + logs, an editor's atomic write-rename) yields ONE refresh. Track
    // which kinds occurred so we tell the frontend exactly what to reload. The
    // thread exits when the Sender is dropped (watcher replaced/stopped).
    std::thread::spawn(move || loop {
        let first = match rx.recv() {
            Ok(k) => k,
            Err(_) => return, // watcher dropped
        };
        let mut git = first == Kind::Git;
        let mut local = first == Kind::Local;
        loop {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(Kind::Git) => git = true,
                Ok(Kind::Local) => local = true,
                Err(mpsc::RecvTimeoutError::Timeout) => break, // quiet → emit
                Err(mpsc::RecvTimeoutError::Disconnected) => return, // watcher dropped
            }
        }
        let payload = match (git, local) {
            (true, true) => "both",
            (true, false) => "git",
            (false, true) => "local",
            (false, false) => continue, // unreachable — we only send real kinds
        };
        let _ = on_change.send(payload.to_string());
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

#[cfg(test)]
mod tests {
    use super::{classify, Kind};
    use std::path::Path;

    #[test]
    fn worktree_paths_are_local() {
        assert_eq!(classify(Path::new("/r/src/main.rs")), Some(Kind::Local));
        assert_eq!(classify(Path::new("/r/README.md")), Some(Kind::Local));
    }

    #[test]
    fn ref_head_and_log_changes_are_git() {
        assert_eq!(classify(Path::new("/r/.git/refs/heads/main")), Some(Kind::Git));
        assert_eq!(classify(Path::new("/r/.git/refs/remotes/origin/main")), Some(Kind::Git));
        assert_eq!(classify(Path::new("/r/.git/refs/tags/v1")), Some(Kind::Git));
        assert_eq!(classify(Path::new("/r/.git/HEAD")), Some(Kind::Git));
        assert_eq!(classify(Path::new("/r/.git/packed-refs")), Some(Kind::Git));
        assert_eq!(classify(Path::new("/r/.git/logs/HEAD")), Some(Kind::Git));
        assert_eq!(classify(Path::new("/r/.git/MERGE_HEAD")), Some(Kind::Git));
    }

    #[test]
    fn index_change_is_local() {
        assert_eq!(classify(Path::new("/r/.git/index")), Some(Kind::Local));
    }

    #[test]
    fn objects_locks_and_other_git_churn_are_ignored() {
        assert_eq!(classify(Path::new("/r/.git/objects/ab/cdef0123")), None);
        assert_eq!(classify(Path::new("/r/.git/refs/heads/main.lock")), None);
        assert_eq!(classify(Path::new("/r/.git/index.lock")), None);
        assert_eq!(classify(Path::new("/r/.git/COMMIT_EDITMSG")), None);
        assert_eq!(classify(Path::new("/r/.git/config")), None);
        assert_eq!(classify(Path::new("/r/.git")), None);
    }
}
