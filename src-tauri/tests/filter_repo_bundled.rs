//! Integration test for the *real* bundled git-filter-repo mechanic.
//!
//! Resolves the vendored script from the source tree (the `tauri dev` path,
//! keyed off `CARGO_MANIFEST_DIR`), builds a throwaway git repo, and drives
//! `git_core::rewrite::rewrite_history` with the `["python3", <script>]` prefix
//! — the same invocation the desktop shell uses in production. Asserts the
//! targeted commit's committer date actually changed. Skips gracefully when
//! `python3` is absent so it never flakes on a bare box.

use git_core::rewrite::rewrite_history;
use git_core::types::{DateMapping, RewriteOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the vendored script in the source tree (mirrors filter_repo.rs's dev fallback).
fn dev_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/git-filter-repo/git-filter-repo")
}

/// True iff a working `python3` is on PATH — otherwise the test is skipped.
fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        // Deterministic identity + dates for the seed commits.
        .env("GIT_AUTHOR_NAME", "Tester")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "Tester")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
        .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
        .output()
        .unwrap_or_else(|e| panic!("git {:?} spawn failed: {}", args, e));
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `git show -s --format=<fmt> <sha>` → trimmed stdout.
fn git_show(repo: &Path, sha: &str, fmt: &str) -> String {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["show", "-s", &format!("--format={}", fmt), sha])
        .output()
        .unwrap();
    assert!(out.status.success(), "git show failed for {}", sha);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn bundled_filter_repo_rewrites_a_commit_date() {
    if !python3_available() {
        eprintln!("skipping: python3 not available");
        return;
    }

    let script = dev_script_path();
    assert!(
        script.exists(),
        "vendored git-filter-repo script missing at {}",
        script.display()
    );

    // Throwaway repo, unique per process.
    let repo = std::env::temp_dir().join(format!("gitit-filterrepo-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).unwrap();

    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "1\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    std::fs::write(repo.join("b.txt"), "2\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "second"]);

    // Target HEAD (the "second" commit).
    let head_sha = git_show(&repo, "HEAD", "%H");
    let before_ct = git_show(&repo, "HEAD", "%ct");
    assert_eq!(before_ct, "1577836800", "seed committer date should be 2020-01-01T00:00:00Z");

    // Map HEAD to a new committer date: 2021-06-15T12:00:00Z = epoch 1623758400.
    let new_epoch: i64 = 1623758400;
    let mapping = DateMapping {
        sha: head_sha.clone(),
        epoch: new_epoch,
        tz_offset: "+0000".to_string(),
    };
    let options = RewriteOptions {
        update_author: false,
        auto_bundle: false,
        preserve_remote: false,
        optimize_range: false,
    };
    let prefix = vec![
        "python3".to_string(),
        script.to_string_lossy().into_owned(),
    ];

    let result = rewrite_history(&repo, &[mapping], &options, &prefix, |_line| {});
    assert!(result.is_ok(), "rewrite_history failed: {:?}", result.err());

    // filter-repo rewrites the SHA, so re-read HEAD by ref, not the old SHA.
    let after_ct = git_show(&repo, "HEAD", "%ct");
    assert_eq!(
        after_ct,
        new_epoch.to_string(),
        "HEAD committer date should have been rewritten to the mapped epoch"
    );
    // author date untouched (update_author=false).
    let after_at = git_show(&repo, "HEAD", "%at");
    assert_eq!(after_at, before_ct, "author date should be unchanged");

    let _ = std::fs::remove_dir_all(&repo);
}
