use crate::git_ops;
use crate::types::RemoteInfo;
use std::path::Path;
use std::process::Command;

/// Validate a remote name: no leading dash, conservative charset. Prevents a name
/// like "-x" or "--upload-pack=…" from being parsed as a git option.
fn valid_remote_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
}

/// List remotes (one entry per name, using the fetch URL).
pub fn remotes(repo: &Path) -> Result<Vec<RemoteInfo>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "-v"]);
    let (out, _) = git_ops::run(&mut c)?;
    let mut seen = std::collections::BTreeMap::new();
    for line in out.lines() {
        // "<name>\t<url> (fetch|push)"
        let mut parts = line.split('\t');
        let name = parts.next().unwrap_or("").trim().to_string();
        let rest = parts.next().unwrap_or("");
        let url = rest.split(" (").next().unwrap_or("").trim().to_string();
        if !name.is_empty() && line.ends_with("(fetch)") {
            seen.insert(name, url);
        }
    }
    Ok(seen.into_iter().map(|(name, url)| RemoteInfo { name, url }).collect())
}

pub fn remote_add(repo: &Path, name: &str, url: &str) -> Result<(), String> {
    if !valid_remote_name(name) {
        return Err(format!("Invalid remote name: {}", name));
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "add", name, url]); // name validated; url is positional-2
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn remote_remove(repo: &Path, name: &str) -> Result<(), String> {
    if !valid_remote_name(name) {
        return Err(format!("Invalid remote name: {}", name));
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "remove", name]);
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn remote_set_url(repo: &Path, name: &str, url: &str) -> Result<(), String> {
    if !valid_remote_name(name) {
        return Err(format!("Invalid remote name: {}", name));
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "set-url", name, url]);
    git_ops::run(&mut c)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("gte-remote-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@e.com"]);
            r.git(&["config", "user.name", "T"]);
            r
        }

        fn git(&self, args: &[&str]) {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(args)
                .output()
                .unwrap();
            assert!(
                o.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&o.stderr)
            );
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn add_list_seturl_remove() {
        let r = TempRepo::new();
        remote_add(&r.path, "origin", "https://example.com/x.git").unwrap();
        let list = remotes(&r.path).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "origin");
        assert_eq!(list[0].url, "https://example.com/x.git");
        remote_set_url(&r.path, "origin", "https://example.com/y.git").unwrap();
        assert_eq!(remotes(&r.path).unwrap()[0].url, "https://example.com/y.git");
        remote_remove(&r.path, "origin").unwrap();
        assert!(remotes(&r.path).unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_remote_name() {
        let r = TempRepo::new();
        assert!(remote_add(&r.path, "-x", "https://e/x.git").is_err());
        assert!(remote_add(&r.path, "--upload-pack=evil", "u").is_err());
    }
}
