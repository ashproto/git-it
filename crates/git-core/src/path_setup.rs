/// When the app/agent is launched from Finder/Dock/launchd, it inherits a bare
/// PATH (typically /usr/bin:/bin:/usr/sbin:/sbin) — not the user's shell PATH.
/// That means `git` is found but `git-filter-repo` (in /opt/homebrew/bin on
/// Apple Silicon, /usr/local/bin on Intel) is not. Prepend both Homebrew bins so
/// child processes can find it. Call once at process startup, before threads.
pub fn ensure_homebrew_path() {
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
