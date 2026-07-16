/// When the app/agent is launched from Finder/Dock/launchd, it inherits a bare
/// PATH (typically /usr/bin:/bin:/usr/sbin:/sbin) — not the user's shell PATH.
/// System `git`/`python3` (Xcode CLT, in /usr/bin) are found, but Homebrew-
/// installed tools — notably `gh` for the GitHub screen — live in /opt/homebrew/bin
/// (Apple Silicon) or /usr/local/bin (Intel) and are not. Prepend both Homebrew
/// bins so child processes can find them. (`git-filter-repo` no longer relies on
/// this: it's bundled and run via python3.) Call once at startup, before threads.
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
