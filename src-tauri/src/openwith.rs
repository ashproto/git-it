//! macOS "Open With" support for the Local Changes file menu.
//!
//! Asks LaunchServices (via `NSWorkspace`) which applications can open a given
//! file, returning each app's display name and `.app` bundle path. This is a
//! desktop-only concern (Cocoa), so it lives in the Tauri shell, not `git-core`.

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSFileManager, NSString, NSURL};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Serialize)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
}

/// Applications that can open `path`, as (display name, `.app` bundle path).
/// Returns an empty list for a missing file or when nothing handles it. The
/// leading-dash guard mirrors the shell-out safety rule even though this is an
/// API call, not a shell-out.
///
/// async: the LaunchServices lookup + per-app `displayNameAtPath` can take tens
/// of ms and sits on the critical path of opening the context menu — a sync
/// command would run it on the UI thread and freeze the app (the caller guards
/// against the resulting out-of-order IPC with a sequence token). NSWorkspace is
/// not main-thread-only, so running off-thread is safe.
#[tauri::command(async)]
pub fn apps_for_file(path: String) -> Result<Vec<AppEntry>, String> {
    if path.is_empty() || path.starts_with('-') {
        return Err("invalid path".into());
    }
    if !Path::new(&path).exists() {
        return Ok(Vec::new());
    }

    let ns_path = NSString::from_str(&path);
    let url = NSURL::fileURLWithPath(&ns_path);
    let ws = NSWorkspace::sharedWorkspace();
    let urls = ws.URLsForApplicationsToOpenURL(&url);
    let fm = NSFileManager::defaultManager();

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<AppEntry> = Vec::new();
    for app_url in urls.iter() {
        let Some(app_path) = app_url.path() else {
            continue;
        };
        let app_path = app_path.to_string();
        if !Path::new(&app_path).exists() || !seen.insert(app_path.clone()) {
            continue;
        }
        let disp = fm.displayNameAtPath(&NSString::from_str(&app_path)).to_string();
        let name = disp.strip_suffix(".app").unwrap_or(&disp).to_string();
        out.push(AppEntry { name, path: app_path });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lists_apps_for_a_text_file() {
        let p = std::env::temp_dir().join(format!("gte-openwith-{}.txt", std::process::id()));
        fs::write(&p, "hello").unwrap();
        let apps = apps_for_file(p.to_str().unwrap().to_string()).unwrap();
        assert!(!apps.is_empty(), "a .txt should have at least one handler app");
        assert!(apps.iter().all(|a| !a.name.is_empty() && Path::new(&a.path).exists()));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_empty_and_dash_is_rejected() {
        assert!(apps_for_file("/no/such/file.xyz".into()).unwrap().is_empty());
        assert!(apps_for_file("-x".into()).is_err());
    }
}
