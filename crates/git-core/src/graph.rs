use crate::git_ops;
use crate::types::{GraphCommit, HeadInfo, Ref, RefDecoration, RefKind, RepoStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The short branch HEAD points at, or None when detached / unborn.
fn current_branch(repo: &Path) -> Option<String> {
    // NOT `--short`/`--abbrev-ref`: those abbreviate to the shortest UNAMBIGUOUS
    // name, so when a tag shares the branch's name (e.g. a rolling `next` release
    // tag) they return "heads/next" instead of "next" — which breaks is_head
    // matching. Read the full ref and strip refs/heads/ ourselves.
    let out = Command::new("git")
        .current_dir(repo)
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let name = s.strip_prefix("refs/heads/").unwrap_or(&s);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// The commit SHA HEAD resolves to, or None in an unborn repo.
fn head_commit_sha(repo: &Path) -> Option<String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "-q", "--verify", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Map commit-SHA -> ref labels that decorate it, with authoritative kinds taken
/// from the refname prefix (not the ambiguous `git log %D` text).
fn build_ref_decorations(repo: &Path) -> Result<HashMap<String, Vec<RefDecoration>>, String> {
    let mut map: HashMap<String, Vec<RefDecoration>> = HashMap::new();
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .arg("for-each-ref")
        .arg("--format=%(objectname)%00%(*objectname)%00%(refname)")
        .args(["refs/heads", "refs/remotes", "refs/tags"]);
    let (out, _) = git_ops::run(&mut cmd)?;
    let current = current_branch(repo);

    for line in out.lines() {
        let parts: Vec<&str> = line.split('\u{0}').collect();
        if parts.len() < 3 {
            continue;
        }
        let obj = parts[0];
        let deref = parts[1];
        let refname = parts[2];
        let target = if deref.is_empty() { obj } else { deref };

        let (kind, name) = if let Some(n) = refname.strip_prefix("refs/heads/") {
            (RefKind::Local, n.to_string())
        } else if let Some(n) = refname.strip_prefix("refs/remotes/") {
            (RefKind::Remote, n.to_string())
        } else if let Some(n) = refname.strip_prefix("refs/tags/") {
            (RefKind::Tag, n.to_string())
        } else {
            continue;
        };
        if kind == RefKind::Remote && name.ends_with("/HEAD") {
            continue;
        }
        let is_head = kind == RefKind::Local && current.as_deref() == Some(name.as_str());
        map.entry(target.to_string())
            .or_default()
            .push(RefDecoration { name, kind, is_head });
    }

    if current.is_none() {
        if let Some(h) = head_commit_sha(repo) {
            map.entry(h).or_default().push(RefDecoration {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
                is_head: true,
            });
        }
    }
    Ok(map)
}

/// All commits across all refs, newest first (topological), with parents + ref labels.
pub fn load_graph(repo: &Path, count: u32, skip: u32) -> Result<Vec<GraphCommit>, String> {
    if head_commit_sha(repo).is_none() {
        let empty = build_ref_decorations(repo).map(|m| m.is_empty()).unwrap_or(true);
        if empty {
            return Ok(Vec::new());
        }
    }
    let ref_map = build_ref_decorations(repo)?;

    // Body (%b) is the LAST field so its embedded newlines can't be mistaken for a
    // field separator (\x1f); records are split on \x1e. The record-level newline trim
    // below then strips %b's trailing blank line.
    let fmt = "%H%x1f%P%x1f%an%x1f%ae%x1f%aI%x1f%cn%x1f%cI%x1f%s%x1f%b%x1e";
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .args(["-c", "log.showSignature=false", "log", "--all", "--topo-order", "--date-order"])
        .arg(format!("--pretty=format:{}", fmt))
        .arg("-n")
        .arg(count.to_string())
        .arg(format!("--skip={}", skip));
    let (stdout, _) = git_ops::run(&mut cmd)?;

    let mut commits = Vec::new();
    for rec in stdout.split('\x1e') {
        let rec = rec.trim_matches(|c| c == '\n' || c == '\r');
        if rec.is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.split('\x1f').collect();
        if f.len() < 8 {
            continue;
        }
        let sha = f[0].to_string();
        let parents: Vec<String> = f[1].split_whitespace().map(|s| s.to_string()).collect();
        let refs = ref_map.get(&sha).cloned().unwrap_or_default();
        commits.push(GraphCommit {
            sha,
            parents,
            author_name: f[2].to_string(),
            author_email: f[3].to_string(),
            author_date: f[4].to_string(),
            committer_name: f[5].to_string(),
            committer_date: f[6].to_string(),
            refs,
            subject: f[7].to_string(),
            // f[8] is %b; absent only on a malformed record. trim() drops the blank
            // line git leaves between subject and body plus any trailing newline.
            body: f.get(8).map(|s| s.trim().to_string()).unwrap_or_default(),
        });
    }
    Ok(commits)
}

/// Parse a `git for-each-ref %(upstream:track)` value like "[ahead 2, behind 1]"
/// into (ahead, behind). Empty / "[gone]" -> (0, 0).
fn parse_track(track: &str) -> (u32, u32) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in track.trim_matches(|c| c == '[' || c == ']').split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.trim().parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.trim().parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

/// All branches (local + remote-tracking) and tags, for the sidebar.
pub fn list_refs(repo: &Path) -> Result<Vec<Ref>, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .arg("for-each-ref")
        .arg("--format=%(refname)%00%(objectname)%00%(*objectname)%00%(upstream:short)%00%(upstream:track)")
        .args(["refs/heads", "refs/remotes", "refs/tags"]);
    let (out, _) = git_ops::run(&mut cmd)?;

    let mut refs = Vec::new();
    for line in out.lines() {
        let p: Vec<&str> = line.split('\u{0}').collect();
        if p.len() < 5 {
            continue;
        }
        let refname = p[0];
        let obj = p[1];
        let deref = p[2];
        let upstream_short = p[3];
        let track = p[4];

        let (kind, name) = if let Some(n) = refname.strip_prefix("refs/heads/") {
            (RefKind::Local, n.to_string())
        } else if let Some(n) = refname.strip_prefix("refs/remotes/") {
            (RefKind::Remote, n.to_string())
        } else if let Some(n) = refname.strip_prefix("refs/tags/") {
            (RefKind::Tag, n.to_string())
        } else {
            continue;
        };
        if kind == RefKind::Remote && name.ends_with("/HEAD") {
            continue;
        }
        let target_sha = if deref.is_empty() { obj.to_string() } else { deref.to_string() };
        let upstream = if upstream_short.is_empty() {
            None
        } else {
            Some(upstream_short.to_string())
        };
        let (ahead, behind) = parse_track(track);
        refs.push(Ref { name, kind, target_sha, upstream, ahead, behind });
    }
    Ok(refs)
}

/// porcelain=v2 ordinary/renamed line: the two chars after the "1 "/"2 " prefix
/// are the staged (X) and unstaged (Y) status; "." means unchanged on that side.
fn count_xy(rest: &str, staged: &mut u32, unstaged: &mut u32) {
    let b = rest.as_bytes();
    if b.len() >= 2 {
        if b[0] as char != '.' {
            *staged += 1;
        }
        if b[1] as char != '.' {
            *unstaged += 1;
        }
    }
}

/// Resolve a control-file path under the git dir, correct for linked worktrees
/// (where `.git` is a file pointer, not a directory). Falls back to a plain
/// `.git/<name>` join if `git rev-parse` is unavailable.
fn git_path(repo: &Path, name: &str) -> PathBuf {
    if let Ok(out) = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--git-path", name])
        .output()
    {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                let pb = PathBuf::from(&p);
                return if pb.is_absolute() { pb } else { repo.join(pb) };
            }
        }
    }
    repo.join(".git").join(name)
}

/// Detect an in-progress multi-step operation from the repo's control files
/// (worktree-aware via `git rev-parse --git-path`).
fn detect_operation(repo: &Path) -> Option<String> {
    let exists = |name: &str| git_path(repo, name).exists();
    if exists("MERGE_HEAD") {
        return Some("merge".to_string());
    }
    if exists("rebase-merge") || exists("rebase-apply") {
        return Some("rebase".to_string());
    }
    if exists("CHERRY_PICK_HEAD") {
        return Some("cherry-pick".to_string());
    }
    if exists("REVERT_HEAD") {
        return Some("revert".to_string());
    }
    if exists("BISECT_START") {
        return Some("bisect".to_string());
    }
    None
}

/// Working-tree summary: staged/unstaged/untracked/conflicted counts, HEAD info,
/// and any in-progress operation.
pub fn repo_status(repo: &Path) -> Result<RepoStatus, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .args(["status", "--porcelain=v2", "--branch"]);
    let (out, _) = git_ops::run(&mut cmd)?;

    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;
    let mut conflicted = 0u32;
    let mut branch: Option<String> = None;
    let mut sha: Option<String> = None;
    let mut detached = false;

    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("# branch.oid ") {
            let v = rest.trim();
            sha = if v == "(initial)" { None } else { Some(v.to_string()) };
        } else if let Some(rest) = line.strip_prefix("# branch.head ") {
            let v = rest.trim();
            if v == "(detached)" {
                detached = true;
            } else {
                branch = Some(v.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("1 ") {
            count_xy(rest, &mut staged, &mut unstaged);
        } else if let Some(rest) = line.strip_prefix("2 ") {
            count_xy(rest, &mut staged, &mut unstaged);
        } else if line.starts_with("u ") {
            conflicted += 1;
        } else if line.starts_with("? ") {
            untracked += 1;
        }
    }

    Ok(RepoStatus {
        head: HeadInfo { sha, branch, detached },
        staged,
        unstaged,
        untracked,
        conflicted,
        operation: detect_operation(repo),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RefKind;
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
            let path = std::env::temp_dir().join(format!("gte-graph-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "Tester"]);
            r
        }

        fn git(&self, args: &[&str]) {
            let out = Command::new("git")
                .current_dir(&self.path)
                .args(args)
                .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
                .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn write_commit(&self, file: &str, msg: &str) {
            fs::write(self.path.join(file), msg).unwrap();
            self.git(&["add", "."]);
            self.git(&["commit", "-q", "-m", msg]);
        }

        fn rev(&self, refname: &str) -> String {
            let out = Command::new("git")
                .current_dir(&self.path)
                .args(["rev-parse", refname])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn merge_fixture() -> TempRepo {
        let r = TempRepo::new();
        r.write_commit("a.txt", "A");
        r.git(&["checkout", "-q", "-b", "feature"]);
        r.write_commit("b.txt", "B");
        r.git(&["checkout", "-q", "main"]);
        r.write_commit("c.txt", "C");
        r.git(&["merge", "-q", "--no-ff", "-m", "M", "feature"]);
        r.git(&["tag", "v1"]);
        r
    }

    #[test]
    fn load_graph_returns_all_commits_with_parents() {
        let r = merge_fixture();
        let commits = load_graph(&r.path, 50, 0).unwrap();
        assert_eq!(commits.len(), 4);

        let m_sha = r.rev("main");
        let merge = commits.iter().find(|c| c.sha == m_sha).unwrap();
        assert_eq!(merge.parents.len(), 2, "merge commit has two parents");
        assert_eq!(merge.subject, "M");

        let root = commits.iter().find(|c| c.subject == "A").unwrap();
        assert!(root.parents.is_empty());
    }

    #[test]
    fn load_graph_attaches_ref_decorations() {
        let r = merge_fixture();
        let commits = load_graph(&r.path, 50, 0).unwrap();
        let m_sha = r.rev("main");
        let merge = commits.iter().find(|c| c.sha == m_sha).unwrap();

        let main_ref = merge.refs.iter().find(|d| d.name == "main").unwrap();
        assert_eq!(main_ref.kind, RefKind::Local);
        assert!(main_ref.is_head, "HEAD is on main");
        assert!(merge.refs.iter().any(|d| d.name == "v1" && d.kind == RefKind::Tag));

        let b = commits.iter().find(|c| c.subject == "B").unwrap();
        assert!(b.refs.iter().any(|d| d.name == "feature" && d.kind == RefKind::Local));
    }

    #[test]
    fn load_graph_empty_repo_is_empty() {
        let r = TempRepo::new();
        assert!(load_graph(&r.path, 50, 0).unwrap().is_empty());
    }

    #[test]
    fn parse_track_reads_ahead_behind() {
        assert_eq!(parse_track("[ahead 2, behind 1]"), (2, 1));
        assert_eq!(parse_track("[ahead 3]"), (3, 0));
        assert_eq!(parse_track("[behind 4]"), (0, 4));
        assert_eq!(parse_track("[gone]"), (0, 0));
        assert_eq!(parse_track(""), (0, 0));
    }

    #[test]
    fn list_refs_lists_branches_and_tags() {
        let r = merge_fixture();
        let refs = list_refs(&r.path).unwrap();

        let main = refs.iter().find(|x| x.name == "main").unwrap();
        assert_eq!(main.kind, RefKind::Local);
        assert_eq!(main.target_sha, r.rev("main"));

        assert!(refs.iter().any(|x| x.name == "feature" && x.kind == RefKind::Local));
        assert!(refs.iter().any(|x| x.name == "v1" && x.kind == RefKind::Tag));
        assert!(!refs.iter().any(|x| x.kind == RefKind::Remote));
    }

    #[test]
    fn load_graph_marks_head_branch_shadowed_by_same_named_tag() {
        // A branch and a tag sharing a name (e.g. a rolling `next` release tag)
        // makes `git symbolic-ref --short HEAD` abbreviate to "heads/next"
        // (disambiguated), which used to break is_head matching so the app never
        // showed the branch as current.
        let r = TempRepo::new();
        r.write_commit("a.txt", "a");
        r.git(&["branch", "next"]);
        r.git(&["tag", "next"]);
        // Plain `next` DWIMs to the branch and attaches HEAD (refs/heads/next), which is what
        // the assertion below needs. No `--end-of-options`: the operand is a hardcoded literal
        // (not user input, so the shell-safety rule doesn't apply) and `git checkout` rejects
        // the marker on some Git versions. Not `refs/heads/next` either — that DETACHES HEAD,
        // which would make is_head below fail.
        r.git(&["checkout", "-q", "next"]);

        let commits = load_graph(&r.path, 50, 0).unwrap();
        let head_sha = r.rev("HEAD");
        let c = commits.iter().find(|c| c.sha == head_sha).expect("HEAD commit in graph");
        let next_local = c
            .refs
            .iter()
            .find(|d| d.name == "next" && d.kind == RefKind::Local)
            .expect("local branch `next` should decorate HEAD");
        assert!(
            next_local.is_head,
            "branch `next` must be is_head even with a same-named tag",
        );
    }

    #[test]
    fn count_xy_counts_each_side() {
        let mut s = 0;
        let mut u = 0;
        count_xy("M. file", &mut s, &mut u);
        count_xy(".M file", &mut s, &mut u);
        count_xy("MM file", &mut s, &mut u);
        assert_eq!(s, 2);
        assert_eq!(u, 2);
    }

    #[test]
    fn repo_status_clean_then_staged() {
        let r = merge_fixture();
        let clean = repo_status(&r.path).unwrap();
        assert_eq!(clean.head.branch.as_deref(), Some("main"));
        assert!(!clean.head.detached);
        assert_eq!((clean.staged, clean.unstaged, clean.untracked, clean.conflicted), (0, 0, 0, 0));
        assert_eq!(clean.operation, None);

        fs::write(r.path.join("new.txt"), "x").unwrap();
        assert_eq!(repo_status(&r.path).unwrap().untracked, 1);

        r.git(&["add", "new.txt"]);
        assert_eq!(repo_status(&r.path).unwrap().staged, 1);
    }

    #[test]
    fn repo_status_detects_in_progress_merge() {
        let r = TempRepo::new();
        r.write_commit("base.txt", "base");
        r.git(&["checkout", "-q", "-b", "side"]);
        r.write_commit("side.txt", "side");
        r.git(&["checkout", "-q", "main"]);
        r.write_commit("main.txt", "main");
        // --no-commit leaves the merge in progress (MERGE_HEAD set), no conflict.
        let _ = Command::new("git")
            .current_dir(&r.path)
            .args(["merge", "--no-commit", "--no-ff", "side"])
            .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
            .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
            .output()
            .unwrap();
        assert_eq!(repo_status(&r.path).unwrap().operation.as_deref(), Some("merge"));
    }

    #[test]
    fn repo_status_detects_in_progress_bisect() {
        let r = TempRepo::new();
        r.write_commit("base.txt", "base");
        r.git(&["bisect", "start"]);

        assert_eq!(repo_status(&r.path).unwrap().operation.as_deref(), Some("bisect"));
    }

    #[test]
    fn detect_operation_resolves_linked_worktree_gitdir() {
        let r = TempRepo::new();
        r.write_commit("base.txt", "base");
        r.git(&["checkout", "-q", "-b", "side"]);
        r.write_commit("side.txt", "side");
        r.git(&["checkout", "-q", "main"]);
        // Linked worktree inside the repo dir so TempRepo::drop cleans it up.
        let wt = r.path.join("wt");
        r.git(&["worktree", "add", "-q", "-b", "wtbranch", wt.to_str().unwrap(), "main"]);
        // In-progress merge INSIDE the worktree, whose .git is a file pointer.
        let _ = Command::new("git")
            .current_dir(&wt)
            .args(["merge", "--no-commit", "--no-ff", "side"])
            .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00 +0000")
            .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00 +0000")
            .output()
            .unwrap();
        // Pre-fix this returned None because <wt>/.git is a file, not a directory.
        assert_eq!(repo_status(&wt).unwrap().operation.as_deref(), Some("merge"));
    }
}
