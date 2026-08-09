use crate::types::{
    BundleInfo, Commit, InitializeRepositoryResult, PrerequisiteCheck, SafetyRef,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Run a command; return (stdout, stderr) on success, an error string otherwise.
pub(crate) fn run(cmd: &mut Command) -> Result<(String, String), String> {
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(if stderr.trim().is_empty() {
            format!("exit code {:?}", output.status.code())
        } else {
            stderr
        });
    }
    Ok((stdout, stderr))
}

pub fn is_git_repo(repo: &Path) -> bool {
    repo.join(".git").exists()
}

fn is_inside_worktree(path: &Path) -> bool {
    Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Is `path` ITSELF a Git directory? True for a bare repository's own folder — which has no
/// `.git` child, so an existence check on that never sees it — and for a normal repository's
/// `.git`. `--resolve-git-dir` answers for the path given and does NOT walk up to a parent,
/// so a plain folder that merely sits inside a repository is not mistaken for one; that case
/// belongs to `is_inside_worktree`. Callers pass an absolute path (`parent` is canonicalized
/// before the join), so the operand cannot be read as a flag.
fn is_git_dir(path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--resolve-git-dir"])
        .arg(path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Initialize a repository in one direct child of an existing parent folder.
/// A non-empty destination is reported without mutation until the caller
/// explicitly retries with `allow_non_empty`.
pub fn initialize_repository(
    parent: &Path,
    folder_name: &str,
    initial_branch: &str,
    allow_non_empty: bool,
) -> Result<InitializeRepositoryResult, String> {
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("Could not open the parent folder: {error}"))?;
    if !parent.is_dir() {
        return Err("The selected parent path is not a folder.".to_string());
    }

    let name = folder_name.trim();
    if name.is_empty()
        || name != folder_name
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(
            "Repository name must be a single folder name without path separators."
                .to_string(),
        );
    }
    let branch = initial_branch.trim();
    if branch.is_empty() || branch != initial_branch || branch.starts_with('-') {
        return Err("Initial branch name is invalid.".to_string());
    }
    let branch_check = Command::new("git")
        .current_dir(&parent)
        .args(["check-ref-format", "--branch", branch])
        .output()
        .map_err(|error| format!("Could not validate the initial branch: {error}"))?;
    if !branch_check.status.success() {
        return Err("Initial branch name is invalid.".to_string());
    }

    let destination = parent.join(name);
    if fs::symlink_metadata(&destination)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(
            "The requested repository path is a symbolic link. Choose the real folder instead."
                .to_string(),
        );
    }
    if destination.exists() && !destination.is_dir() {
        return Err("A file already exists at the requested repository path.".to_string());
    }
    // `.git` catches a normal repository; `is_git_dir` catches a bare one, which has no `.git`
    // child and reports `false` for `--is-inside-work-tree`, so it slipped past both guards and
    // `git init` would nest a fresh repository inside it.
    if destination.join(".git").exists() || is_git_dir(&destination) {
        return Err("That folder is already a Git repository. Open it instead.".to_string());
    }
    let nesting_probe = if destination.is_dir() {
        &destination
    } else {
        &parent
    };
    if is_inside_worktree(nesting_probe) {
        return Err(
            "Cannot create a nested repository inside another Git worktree. Choose a different parent folder."
                .to_string(),
        );
    }

    let existing_entries = if destination.is_dir() {
        fs::read_dir(&destination)
            .map_err(|error| format!("Could not inspect the destination folder: {error}"))?
            .try_fold(0usize, |count, entry| entry.map(|_| count + 1))
            .map_err(|error| format!("Could not inspect the destination folder: {error}"))?
    } else {
        0
    };
    let planned_path = destination.to_string_lossy().into_owned();
    if existing_entries > 0 && !allow_non_empty {
        return Ok(InitializeRepositoryResult {
            path: planned_path,
            initialized: false,
            existing_entries,
        });
    }

    if !destination.exists() {
        fs::create_dir(&destination)
            .map_err(|error| format!("Could not create the repository folder: {error}"))?;
    }
    let mut init = Command::new("git");
    init.current_dir(&destination)
        .arg("init")
        .arg(format!("--initial-branch={branch}"));
    run(&mut init)?;
    let canonical = fs::canonicalize(&destination)
        .map_err(|error| format!("Could not resolve the new repository path: {error}"))?;
    Ok(InitializeRepositoryResult {
        path: canonical.to_string_lossy().into_owned(),
        initialized: true,
        existing_entries,
    })
}

pub fn load_commits(repo: &Path, count: u32, range: Option<&str>) -> Result<Vec<Commit>, String> {
    // Replicates: git log --no-decorate --pretty='%H%x1f%aN%x1f%aI%x1f%cN%x1f%cI%x1f%s%x1e' -n N [RANGE]
    // The \x1f field separator and \x1e record separator match the Python implementation.
    let fmt = "%H%x1f%aN%x1f%aI%x1f%cN%x1f%cI%x1f%s%x1e";
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .arg("log")
        .arg("--no-decorate")
        .arg(format!("--pretty=format:{}", fmt))
        .arg("-n")
        .arg(count.to_string());
    if let Some(r) = range {
        let r = r.trim();
        if !r.is_empty() {
            cmd.arg(r);
        }
    }
    let (stdout, _) = run(&mut cmd)?;

    let mut commits = Vec::new();
    for rec in stdout.split('\x1e') {
        let rec = rec.trim_matches(|c| c == '\n' || c == '\r');
        if rec.is_empty() {
            continue;
        }
        let parts: Vec<&str> = rec.split('\x1f').collect();
        if parts.len() < 6 {
            continue;
        }
        commits.push(Commit {
            sha: parts[0].to_string(),
            author_name: parts[1].to_string(),
            author_date: parts[2].to_string(),
            committer_name: parts[3].to_string(),
            committer_date: parts[4].to_string(),
            subject: parts[5].to_string(),
        });
    }
    Ok(commits)
}

/// Bundles live under `<repo>/.git/git-it-bundles/` to keep them out of the
/// working tree (and untouched by checkouts/cleans).
fn bundles_dir(repo: &Path) -> PathBuf {
    repo.join(".git").join("git-it-bundles")
}

pub fn create_bundle(repo: &Path) -> Result<PathBuf, String> {
    let dir = bundles_dir(repo);
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir bundles dir failed: {}", e))?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("backup-{}.bundle", ts);
    let path = dir.join(&name);
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .arg("bundle")
        .arg("create")
        .arg(&path)
        .arg("--all");
    run(&mut cmd)?;
    Ok(path)
}

pub fn list_bundles(repo: &Path) -> Result<Vec<BundleInfo>, String> {
    let dir = bundles_dir(repo);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in
        fs::read_dir(&dir).map_err(|e| format!("read_dir bundles dir failed: {}", e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("bundle") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(BundleInfo {
            name,
            size,
            path: path.to_string_lossy().into_owned(),
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

pub fn delete_bundle(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| format!("delete bundle failed: {}", e))
}

pub fn fetch_bundle(repo: &Path, bundle_path: &Path) -> Result<String, String> {
    // Fetches refs from a bundle into the repo so they're recoverable.
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .arg("fetch")
        .arg(bundle_path)
        .arg("--prune")
        .arg("refs/heads/*:refs/heads/restored/*")
        .arg("refs/tags/*:refs/tags/restored/*");
    let (out, err) = run(&mut cmd)?;
    Ok(format!("{}{}", out, err))
}

pub fn list_safety_refs(repo: &Path) -> Result<Vec<SafetyRef>, String> {
    // Captures git-filter-repo's `refs/original/*` backups, plus any branch/tag prefixed `backup/`.
    let mut out = Vec::new();
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .arg("for-each-ref")
        .arg("--format=%(objecttype) %(refname)")
        .arg("refs/original/")
        .arg("refs/heads/backup/")
        .arg("refs/tags/backup/");
    let (stdout, _) = run(&mut cmd)?;
    for line in stdout.lines() {
        let (kind, name) = match line.split_once(' ') {
            Some(parts) => parts,
            None => continue,
        };
        out.push(SafetyRef {
            kind: kind.to_string(),
            name: name.to_string(),
        });
    }
    Ok(out)
}

pub fn delete_refs(repo: &Path, refs: &[String]) -> Result<(), String> {
    for r in refs {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo).arg("update-ref").arg("-d").arg(r);
        run(&mut cmd)?;
    }
    Ok(())
}

/// Probe the runtime prerequisites for Git It.
///
/// `filter_repo_argv` is the invocation prefix for the bundled git-filter-repo
/// script (e.g. `["python3", "/…/git-filter-repo"]`), resolved by the desktop
/// shell. We check: `git --version`, `python3 --version`, and
/// `<argv-prefix> --version` (the bundled script, which needs python3 + the
/// script file to be present and runnable).
pub fn check_prerequisites(filter_repo_argv: &[String]) -> PrerequisiteCheck {
    let git_version = Command::new("git")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let python3_version = Command::new("python3")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        // python3 prints its version to stdout on modern releases, but older ones
        // used stderr; fall back so we don't report an empty string.
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            let trimmed = out.trim();
            if trimmed.is_empty() {
                String::from_utf8_lossy(&o.stderr).trim().to_string()
            } else {
                trimmed.to_string()
            }
        });

    let filter_repo_version = filter_repo_argv.split_first().and_then(|(program, args)| {
        Command::new(program)
            .args(args)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    });

    PrerequisiteCheck {
        git: git_version.is_some(),
        git_version,
        python3: python3_version.is_some(),
        python3_version,
        filter_repo: filter_repo_version.is_some(),
        filter_repo_version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // These tests run on parallel threads within ONE process, so the pid alone does not
    // make a path unique. A wall-clock stamp did not either: two threads could read the
    // same nanosecond, `fs::create_dir` then failed with AlreadyExists, and the `unwrap()`
    // panicked — a flake that hit a different test on each run. A process-wide counter is
    // collision-proof by construction, and is the pattern the TempRepo fixtures in ops.rs
    // and ops_worktree.rs already use.
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("git-it-init-test-{}-{}", std::process::id(), id));
            // pid + counter is unique among LIVE processes, but not against the dead: a run
            // killed before `Drop` leaves its directories behind, and once the OS recycles
            // that pid a fresh process counting from zero reproduces the same path. Clear
            // any such leftover first — the same guard the other TempRepo fixtures use.
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn initialize_repository_creates_requested_folder_and_branch() {
        let parent = TempFolder::new();

        let result = initialize_repository(&parent.0, "project", "main", false).unwrap();

        assert!(result.initialized);
        assert_eq!(result.existing_entries, 0);
        assert!(Path::new(&result.path).join(".git").is_dir());
        let branch = Command::new("git")
            .current_dir(&result.path)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .unwrap();
        assert!(branch.status.success());
        assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "main");
    }

    #[test]
    fn initialize_repository_requires_confirmation_for_existing_files() {
        let parent = TempFolder::new();
        let destination = parent.0.join("project");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep.txt"), "preserve me").unwrap();

        let preview = initialize_repository(&parent.0, "project", "main", false).unwrap();
        assert!(!preview.initialized);
        assert_eq!(preview.existing_entries, 1);
        assert!(!destination.join(".git").exists());

        let result = initialize_repository(&parent.0, "project", "main", true).unwrap();
        assert!(result.initialized);
        assert_eq!(
            fs::read_to_string(destination.join("keep.txt")).unwrap(),
            "preserve me"
        );
    }

    #[test]
    fn initialize_repository_rejects_invalid_branch_before_mutation() {
        let parent = TempFolder::new();

        let err = initialize_repository(&parent.0, "project", "-danger", false).unwrap_err();

        assert!(err.contains("branch name"));
        assert!(!parent.0.join("project").exists());
    }

    #[test]
    fn initialize_repository_rejects_nested_git_worktree() {
        let parent = TempFolder::new();
        let mut init = Command::new("git");
        init.current_dir(&parent.0).args(["init", "-q"]);
        run(&mut init).unwrap();

        let err = initialize_repository(&parent.0, "nested", "main", false).unwrap_err();

        assert!(err.contains("nested repository"));
        assert!(!parent.0.join("nested").exists());
    }

    /// A bare repository has no `.git` child, and `rev-parse --is-inside-work-tree` answers
    /// `false` inside one — so neither existing guard saw it. `allow_non_empty` here is the
    /// dangerous path: the user is warned the folder is not empty, confirms, and `git init`
    /// then creates a nested repository inside the bare one.
    #[test]
    fn initialize_repository_rejects_an_existing_bare_repository() {
        let parent = TempFolder::new();
        let mut init = Command::new("git");
        init.current_dir(&parent.0).args(["init", "-q", "--bare", "shipped.git"]);
        run(&mut init).unwrap();

        let err = initialize_repository(&parent.0, "shipped.git", "main", true).unwrap_err();

        assert!(
            err.contains("already a Git repository"),
            "should be refused as an existing repository, got: {err}"
        );
        assert!(
            !parent.0.join("shipped.git").join(".git").exists(),
            "must not have initialized a nested repository inside the bare one"
        );
    }

    /// The guard must not over-reach: a plain folder that merely sits next to a repository
    /// is still a valid destination. `--resolve-git-dir` answers for the path given and does
    /// not walk up, which is what keeps this case working.
    #[test]
    fn initialize_repository_still_accepts_a_plain_empty_folder() {
        let parent = TempFolder::new();
        fs::create_dir(parent.0.join("fresh")).unwrap();

        let result = initialize_repository(&parent.0, "fresh", "main", true).unwrap();

        assert!(result.initialized);
        assert!(parent.0.join("fresh").join(".git").is_dir());
    }
}
