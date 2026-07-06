//! Resolves the invocation prefix for the bundled `git-filter-repo` script.
//!
//! git-core is Tauri-free and takes the invocation as an argv prefix
//! (`["python3", <abs script path>]`). This module owns the one Tauri-specific
//! concern: finding the script on disk. In a built `.app` it lives under the
//! bundled resource dir; under `tauri dev` there is no resource bundle, so we
//! fall back to the source tree relative to `CARGO_MANIFEST_DIR`.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Path of the vendored script *within* the resource bundle (and mirrored in the
/// source tree). Kept in one place so the prod resolver and dev fallback agree.
const SCRIPT_REL: &str = "resources/git-filter-repo/git-filter-repo";

/// The source-tree location of the vendored script, used under `tauri dev`
/// where no `.app` resource bundle exists.
fn dev_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SCRIPT_REL)
}

/// Build the argv prefix that runs the bundled git-filter-repo: `["python3", <abs path>]`.
///
/// Prefers the bundled resource path; if it can't be resolved or doesn't exist
/// on disk (the `tauri dev` case), falls back to the source tree.
pub fn filter_repo_argv(app: &AppHandle) -> Vec<String> {
    let script = app
        .path()
        .resolve(SCRIPT_REL, tauri::path::BaseDirectory::Resource)
        .ok()
        .filter(|p| p.exists())
        .unwrap_or_else(dev_script_path);

    vec!["python3".to_string(), script.to_string_lossy().into_owned()]
}
