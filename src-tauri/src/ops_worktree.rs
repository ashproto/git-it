use crate::git_ops;
use crate::types::WorkingFile;
use std::path::Path;
use std::process::Command;

/// All working-tree changes (staged, unstaged, untracked, conflicted) as a file list.
/// Parses `git status --porcelain=v2 -z`. Records: `1`/`2` (ordinary/rename, "XY" code),
/// `u` (unmerged), `?` (untracked). `-z` → NUL-terminated, verbatim paths.
pub fn working_changes(repo: &Path) -> Result<Vec<WorkingFile>, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).args(["status", "--porcelain=v2", "-z"]);
    let (out, _) = git_ops::run(&mut c)?;
    let mut files = Vec::new();
    // Records are NUL-separated; a `2` (rename) record is followed by an extra NUL-
    // separated origin path which we skip.
    let mut it = out.split('\0').peekable();
    while let Some(rec) = it.next() {
        if rec.is_empty() {
            continue;
        }
        if let Some(rest) = rec.strip_prefix("1 ").or_else(|| rec.strip_prefix("2 ")) {
            let is_rename = rec.starts_with("2 ");
            // rest = "<XY> <sub> <mH> <mI> <mW> <hH> <hI> [<Xscore>] <path>"
            let xy: Vec<char> = rest.chars().take(2).collect();
            let x = *xy.first().unwrap_or(&'.');
            let y = *xy.get(1).unwrap_or(&'.');
            // path is the last space-separated field of the record
            let path = rest.rsplit(' ').next().unwrap_or("").to_string();
            if is_rename {
                let _ = it.next(); // consume the rename origin path
            }
            files.push(WorkingFile {
                path,
                staged: x != '.',
                unstaged: y != '.',
                untracked: false,
                conflicted: false,
                status: status_label(x, y),
            });
        } else if let Some(rest) = rec.strip_prefix("u ") {
            let path = rest.rsplit(' ').next().unwrap_or("").to_string();
            files.push(WorkingFile {
                path,
                staged: false,
                unstaged: true,
                untracked: false,
                conflicted: true,
                status: "conflicted".into(),
            });
        } else if let Some(path) = rec.strip_prefix("? ") {
            files.push(WorkingFile {
                path: path.to_string(),
                staged: false,
                unstaged: true,
                untracked: true,
                conflicted: false,
                status: "untracked".into(),
            });
        }
    }
    Ok(files)
}

pub fn status_label(x: char, y: char) -> String {
    let c = if x != '.' { x } else { y };
    match c {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'T' => "typechange",
        _ => "modified",
    }
    .to_string()
}

/// Stage paths (also stages untracked files = intent to add + content).
pub fn stage(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).arg("add").arg("--");
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Unstage paths (index → HEAD), keeping worktree changes.
pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["restore", "--staged", "--"]);
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Discard tracked-file changes (index + worktree → HEAD). DESTRUCTIVE / not undoable.
pub fn discard(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["restore", "--staged", "--worktree", "--"]);
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Remove untracked files. DESTRUCTIVE / not undoable.
pub fn clean(repo: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["clean", "-f", "--"]);
    for p in paths {
        c.arg(p);
    }
    git_ops::run(&mut c)?;
    Ok(())
}

/// Commit the staged changes. Message via a temp file (never the command line).
pub fn commit(repo: &Path, message: &str, signoff: bool) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("gte-commit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("tmp: {}", e))?;
    let mf = dir.join("msg");
    std::fs::write(&mf, message).map_err(|e| format!("msg: {}", e))?;
    let mut c = Command::new("git");
    c.current_dir(repo).arg("commit").arg("-F").arg(&mf);
    if signoff {
        c.arg("--signoff");
    }
    let res = git_ops::run(&mut c);
    let _ = std::fs::remove_dir_all(&dir);
    res?;
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
            let path =
                std::env::temp_dir().join(format!("gte-wt-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "Tester"]);
            r
        }
        fn git(&self, args: &[&str]) {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(args)
                .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
                .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
                .output()
                .unwrap();
            assert!(
                o.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&o.stderr)
            );
        }
        fn write(&self, f: &str, s: &str) {
            fs::write(self.path.join(f), s).unwrap();
        }
        fn commit_file(&self, f: &str, s: &str, m: &str) {
            self.write(f, s);
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", m]);
        }
        fn staged_paths(&self) -> Vec<String> {
            let o = Command::new("git")
                .current_dir(&self.path)
                .args(["diff", "--cached", "--name-only"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        }
    }
    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn lists_untracked_modified_staged() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n"); // modified (unstaged)
        r.write("b.txt", "new\n"); // untracked
        r.write("c.txt", "c\n");
        r.git(&["add", "c.txt"]); // staged add
        let f = working_changes(&r.path).unwrap();
        let by = |p: &str| f.iter().find(|x| x.path == p).cloned();
        assert!(by("a.txt").unwrap().unstaged);
        assert!(by("b.txt").unwrap().untracked);
        assert!(by("c.txt").unwrap().staged);
    }

    #[test]
    fn stage_then_unstage_roundtrip() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n");
        stage(&r.path, &["a.txt".into()]).unwrap();
        assert_eq!(r.staged_paths(), vec!["a.txt".to_string()]);
        unstage(&r.path, &["a.txt".into()]).unwrap();
        assert!(r.staged_paths().is_empty());
    }

    #[test]
    fn discard_reverts_tracked_changes() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "orig\n", "init");
        r.write("a.txt", "changed\n");
        discard(&r.path, &["a.txt".into()]).unwrap();
        assert_eq!(
            fs::read_to_string(r.path.join("a.txt")).unwrap(),
            "orig\n"
        );
    }

    #[test]
    fn clean_removes_untracked() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("junk.txt", "x\n");
        clean(&r.path, &["junk.txt".into()]).unwrap();
        assert!(!r.path.join("junk.txt").exists());
    }

    #[test]
    fn commit_creates_commit_from_staged() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        r.write("a.txt", "2\n");
        stage(&r.path, &["a.txt".into()]).unwrap();
        commit(&r.path, "second commit", false).unwrap();
        let o = Command::new("git")
            .current_dir(&r.path)
            .args(["log", "-1", "--format=%s"])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&o.stdout).trim(),
            "second commit"
        );
    }

    #[test]
    fn stage_path_cannot_be_option() {
        let r = TempRepo::new();
        r.commit_file("a.txt", "1\n", "init");
        // A path that looks like an option must be a (non-matching) pathspec, not a flag.
        assert!(stage(&r.path, &["--all".into()]).is_err());
    }
}
