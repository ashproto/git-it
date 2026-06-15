use crate::types::{DateMapping, RewriteOptions};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

/// Build the Python --commit-callback string passed to git-filter-repo.
/// Mirrors the structure of legacy/run_filter_repo_debug.sh.
pub fn generate_callback(mappings: &[DateMapping], update_author: bool) -> String {
    let mut entries = String::new();
    for m in mappings {
        entries.push_str(&format!(
            "    bytes.fromhex(\"{}\"): ({}, b\"{}\"),\n",
            m.sha, m.epoch, m.tz_offset
        ));
    }

    let date_assignments = if update_author {
        "    commit.author_date = date_bytes\n    commit.committer_date = date_bytes"
    } else {
        "    commit.committer_date = date_bytes"
    };

    format!(
        "import sys\n\
mapping_raw = {{\n{entries}}}\n\
oid = commit.original_id\n\
if isinstance(oid, memoryview):\n    oid = oid.tobytes()\n\
if isinstance(oid, bytes):\n    oid_hex_str = oid.decode('ascii')\n    oid_bytes = bytes.fromhex(oid_hex_str)\n\
else:\n    oid_hex_str = str(oid)\n    oid_bytes = bytes.fromhex(oid_hex_str)\n\
new_tuple = mapping_raw.get(oid_bytes)\n\
if new_tuple is not None:\n    ts, tz = new_tuple\n    date_str = str(ts) + ' ' + tz.decode('ascii')\n    date_bytes = date_str.encode('utf-8')\n\
{date_assignments}\n",
        entries = entries,
        date_assignments = date_assignments,
    )
}

/// Find the earliest (oldest in git history) commit among the targets.
/// Returns the full SHA, or None on failure. Mirrors legacy _find_earliest_commit.
fn find_earliest_target(repo: &Path, target_shas: &[String]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["rev-list", "--reverse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let all_commits: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty())
        .collect();
    let pos: HashMap<&str, usize> = all_commits
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    let mut earliest: Option<(usize, String)> = None;
    for sha in target_shas {
        let key = sha.to_lowercase();
        if let Some(&p) = pos.get(key.as_str()) {
            if earliest.as_ref().map_or(true, |(ep, _)| p < *ep) {
                earliest = Some((p, key));
            }
        }
    }
    earliest.map(|(_, sha)| sha)
}

/// Count commit parents. 0 for a root commit, 1 for linear history, 2+ for merges.
fn parent_count(repo: &Path, sha: &str) -> usize {
    let out = match Command::new("git")
        .current_dir(repo)
        .args(["rev-list", "--parents", "-n", "1"])
        .arg(sha)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return 0,
    };
    let line = String::from_utf8_lossy(&out.stdout);
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    parts.len().saturating_sub(1)
}

/// Spawn `git filter-repo` with the generated callback and stream stdout/stderr lines.
/// `on_line` is called for every line from either stream.
pub fn rewrite_history<F>(
    repo: &Path,
    mappings: &[DateMapping],
    options: &RewriteOptions,
    on_line: F,
) -> Result<(), String>
where
    F: Fn(String) + Send + Sync + 'static,
{
    let callback = generate_callback(mappings, options.update_author);

    let origin_url = if options.preserve_remote {
        Command::new("git")
            .current_dir(repo)
            .args(["config", "--get", "remote.origin.url"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    // Remove .git/filter-repo/already_ran so we don't hit the interactive
    // "treat this as a continuation?" prompt when re-running on the same repo.
    let already_ran = repo.join(".git").join("filter-repo").join("already_ran");
    if already_ran.exists() {
        match fs::remove_file(&already_ran) {
            Ok(_) => on_line("[rewrite] removed .git/filter-repo/already_ran to allow re-run".to_string()),
            Err(e) => on_line(format!("[rewrite] warning: could not remove already_ran: {}", e)),
        }
    }

    let mut cmd = Command::new("git-filter-repo");
    cmd.current_dir(repo)
        .arg("--commit-callback")
        .arg(&callback);

    if options.optimize_range {
        let target_shas: Vec<String> = mappings.iter().map(|m| m.sha.clone()).collect();
        if let Some(earliest) = find_earliest_target(repo, &target_shas) {
            if parent_count(repo, &earliest) > 0 {
                let range = format!("{}^..HEAD", earliest);
                on_line(format!(
                    "[rewrite] limiting range to {}^..HEAD (older commits untouched)",
                    &earliest[..12.min(earliest.len())]
                ));
                cmd.arg("--refs").arg(range);
            } else {
                on_line("[rewrite] earliest target is a root commit; rewriting HEAD ancestry".to_string());
                cmd.arg("--refs").arg("HEAD");
            }
        }
    }

    cmd.arg("--force")
        .arg("--partial")
        .arg("--replace-refs")
        .arg("delete-no-add");

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Failed to spawn git-filter-repo: {}. Is it installed? (brew install git-filter-repo)",
            e
        )
    })?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let on_line = Arc::new(on_line);

    let oline_out = Arc::clone(&on_line);
    let t_out = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            oline_out(line);
        }
    });
    let oline_err = Arc::clone(&on_line);
    let t_err = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            oline_err(format!("[stderr] {}", line));
        }
    });

    let status = child
        .wait()
        .map_err(|e| format!("wait on git-filter-repo failed: {}", e))?;
    let _ = t_out.join();
    let _ = t_err.join();

    if !status.success() {
        return Err(format!(
            "git-filter-repo exited with code {:?}",
            status.code()
        ));
    }

    if let Some(url) = origin_url {
        let _ = Command::new("git")
            .current_dir(repo)
            .args(["remote", "add", "origin"])
            .arg(&url)
            .output();
    }

    Ok(())
}
