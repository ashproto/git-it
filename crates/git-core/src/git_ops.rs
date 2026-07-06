use crate::types::{BundleInfo, Commit, PrerequisiteCheck, SafetyRef};
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
