# Phase 1 · Plan 2 — Rust data layer (implementation plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Rust/Tauri backend that feeds the graph: `load_graph` (commits + parents + ref decorations across all branches), `list_refs` (sidebar branches/tags/remotes), and `repo_status` (working-tree counts + HEAD + in-progress operation), exposed as Tauri commands with TypeScript bindings.

**Architecture:** A new `src-tauri/src/graph.rs` module shells out to the `git` CLI (reusing `git_ops::run`), parses NUL/`\x1f`/`\x1e`-delimited output into serde structs, and is exercised by Rust integration tests that build throwaway fixture repos. New types live in `types.rs`; commands in `commands.rs`; registration in `lib.rs`; JS bindings + mirrored types in `src/lib/api.ts` and `src/lib/types.ts`.

**Tech Stack:** Rust 2021, Tauri 2, serde, the `git` CLI; TypeScript bindings via `@tauri-apps/api`.

**Reference spec:** `docs/specs/2026-06-14-git-graph-client-design.md` §5.1 (data layer) and §4 (backend conventions).

**Prerequisites:** Plan 1 (lane engine) is merged to `main`. The lane engine consumes `LaneCommit = {sha, parents}`; `GraphCommit` is a superset, so Plan 3 maps `GraphCommit → LaneCommit` trivially.

**Branch:** create `feat/phase1-data-layer` off `main` before Task 1 (`git checkout -b feat/phase1-data-layer`). All work lands there; the controller finishes the branch at the end.

**Commit trailer:** every commit message ends with a second `-m` line:
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`

---

## File structure

| File | Change |
|---|---|
| `src-tauri/src/types.rs` | Add `GraphCommit`, `RefDecoration`, `RefKind`, `Ref`, `HeadInfo`, `RepoStatus` |
| `src-tauri/src/git_ops.rs` | Make `run` `pub(crate)` so `graph.rs` can reuse it |
| `src-tauri/src/graph.rs` | New module: `load_graph`, `list_refs`, `repo_status` + helpers + tests |
| `src-tauri/src/commands.rs` | Add 3 `#[tauri::command]` wrappers |
| `src-tauri/src/lib.rs` | `mod graph;` + register the 3 commands |
| `src/lib/types.ts` | Mirror the 6 new types (snake_case fields, matching serde output) |
| `src/lib/api.ts` | `loadGraph`, `listRefs`, `repoStatus` bindings |

Rust commands run from `src-tauri/`; JS checks run from `git-it/`.

---

## Task 1: Rust types + expose `run`

**Files:**
- Modify: `src-tauri/src/types.rs`
- Modify: `src-tauri/src/git_ops.rs:7` (`fn run` → `pub(crate) fn run`)

- [ ] **Step 1: Add the new types** to the end of `src-tauri/src/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCommit {
    pub sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_date: String,
    pub refs: Vec<RefDecoration>,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefDecoration {
    pub name: String,
    pub kind: RefKind,
    pub is_head: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    Local,
    Remote,
    Tag,
    Head,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ref {
    pub name: String,
    pub kind: RefKind,
    pub target_sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub head: HeadInfo,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub operation: Option<String>,
}
```

- [ ] **Step 2: Make `run` reusable.** In `src-tauri/src/git_ops.rs`, change the signature at line 7 from:

```rust
fn run(cmd: &mut Command) -> Result<(String, String), String> {
```
to:
```rust
pub(crate) fn run(cmd: &mut Command) -> Result<(String, String), String> {
```

- [ ] **Step 3: Verify it compiles.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo check 2>&1 | tail -20`
Expected: `Finished` with no errors. (Dead-code warnings for the not-yet-used types are acceptable at this step.)

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/types.rs src-tauri/src/git_ops.rs
git commit -m "feat(graph): add Rust graph/ref/status types" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `load_graph` + ref decorations (TDD)

**Files:**
- Create: `src-tauri/src/graph.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod graph;` so the module + its tests compile)

- [ ] **Step 1: Register the module.** In `src-tauri/src/lib.rs`, add `mod graph;` next to the other `mod` lines at the top (after `mod commands;`):

```rust
mod commands;
mod git_ops;
mod graph;
mod rewrite;
mod types;
```

- [ ] **Step 2: Create `src-tauri/src/graph.rs` with `load_graph`, its helpers, and a failing test.**

Write this file:

```rust
use crate::git_ops;
use crate::types::{GraphCommit, HeadInfo, Ref, RefDecoration, RefKind, RepoStatus};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// The short branch HEAD points at, or None when detached / unborn.
fn current_branch(repo: &Path) -> Option<String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["symbolic-ref", "--short", "-q", "HEAD"])
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
        // Annotated tags: %(*objectname) is the dereferenced commit; use it.
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
        // Skip the remote's symbolic HEAD (e.g. "origin/HEAD").
        if kind == RefKind::Remote && name.ends_with("/HEAD") {
            continue;
        }
        let is_head = kind == RefKind::Local && current.as_deref() == Some(name.as_str());
        map.entry(target.to_string())
            .or_default()
            .push(RefDecoration { name, kind, is_head });
    }

    // Detached HEAD: decorate the checked-out commit with a synthetic Head label.
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
    // Unborn repo (no commits, no refs): nothing to show.
    if head_commit_sha(repo).is_none() {
        let empty = build_ref_decorations(repo).map(|m| m.is_empty()).unwrap_or(true);
        if empty {
            return Ok(Vec::new());
        }
    }
    let ref_map = build_ref_decorations(repo)?;

    // %P = parents (space separated), %D omitted (we attach refs from for-each-ref).
    let fmt = "%H%x1f%P%x1f%an%x1f%ae%x1f%aI%x1f%cn%x1f%cI%x1f%s%x1e";
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
        });
    }
    Ok(commits)
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

    /// Build: A (root) -> on main: C ; on feature: B ; merge feature into main (M); tag v1 on M.
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
        // A, B, C, M across both branches.
        assert_eq!(commits.len(), 4);

        let m_sha = r.rev("main");
        let merge = commits.iter().find(|c| c.sha == m_sha).unwrap();
        assert_eq!(merge.parents.len(), 2, "merge commit has two parents");
        assert_eq!(merge.subject, "M");

        // Root commit A has no parents.
        let root = commits.iter().find(|c| c.subject == "A").unwrap();
        assert!(root.parents.is_empty());
    }

    #[test]
    fn load_graph_attaches_ref_decorations() {
        let r = merge_fixture();
        let commits = load_graph(&r.path, 50, 0).unwrap();
        let m_sha = r.rev("main");
        let merge = commits.iter().find(|c| c.sha == m_sha).unwrap();

        // The merge tip carries the current branch (is_head) and the tag.
        let main_ref = merge.refs.iter().find(|d| d.name == "main").unwrap();
        assert_eq!(main_ref.kind, RefKind::Local);
        assert!(main_ref.is_head, "HEAD is on main");
        assert!(merge.refs.iter().any(|d| d.name == "v1" && d.kind == RefKind::Tag));

        // feature points at B.
        let b = commits.iter().find(|c| c.subject == "B").unwrap();
        assert!(b.refs.iter().any(|d| d.name == "feature" && d.kind == RefKind::Local));
    }

    #[test]
    fn load_graph_empty_repo_is_empty() {
        let r = TempRepo::new(); // initialized, no commits
        assert!(load_graph(&r.path, 50, 0).unwrap().is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify they pass (TDD: implementation + tests land together here because the fixture harness and the function are interdependent).**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test graph:: 2>&1 | tail -25`
Expected: `test result: ok.` with the 3 `load_graph_*` tests passing. If a test fails, fix `graph.rs` (never the assertions) until green. If `git` is missing in the test environment, report BLOCKED.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/graph.rs src-tauri/src/lib.rs
git commit -m "feat(graph): add load_graph with ref decorations + fixture tests" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `list_refs` (TDD)

**Files:**
- Modify: `src-tauri/src/graph.rs` (add `list_refs` + `parse_track`, and tests)

- [ ] **Step 1: Add `list_refs` and `parse_track`** to `src-tauri/src/graph.rs` (after `load_graph`, before the `#[cfg(test)]` module):

```rust
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
```

- [ ] **Step 2: Add tests** inside the existing `#[cfg(test)] mod tests` block in `graph.rs` (append these before the closing `}` of the module):

```rust
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
        // No remotes configured in the fixture.
        assert!(!refs.iter().any(|x| x.kind == RefKind::Remote));
    }
```

- [ ] **Step 3: Run the tests.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test graph:: 2>&1 | tail -25`
Expected: all `graph::` tests pass (now including `parse_track_*` and `list_refs_*`). Fix `graph.rs` if red.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/graph.rs
git commit -m "feat(graph): add list_refs with upstream ahead/behind" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `repo_status` (TDD)

**Files:**
- Modify: `src-tauri/src/graph.rs` (add `repo_status` + `count_xy` + `detect_operation`, and tests)

- [ ] **Step 1: Add the functions** to `src-tauri/src/graph.rs` (after `list_refs`, before the test module):

```rust
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

/// Detect an in-progress multi-step operation from control files under .git.
fn detect_operation(repo: &Path) -> Option<String> {
    let g = repo.join(".git");
    if g.join("MERGE_HEAD").exists() {
        return Some("merge".to_string());
    }
    if g.join("rebase-merge").exists() || g.join("rebase-apply").exists() {
        return Some("rebase".to_string());
    }
    if g.join("CHERRY_PICK_HEAD").exists() {
        return Some("cherry-pick".to_string());
    }
    if g.join("REVERT_HEAD").exists() {
        return Some("revert".to_string());
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
```

- [ ] **Step 2: Add tests** inside the `#[cfg(test)] mod tests` block in `graph.rs` (append before the module's closing `}`):

```rust
    #[test]
    fn count_xy_counts_each_side() {
        let mut s = 0;
        let mut u = 0;
        count_xy("M. file", &mut s, &mut u); // staged only
        count_xy(".M file", &mut s, &mut u); // unstaged only
        count_xy("MM file", &mut s, &mut u); // both
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

        // Untracked file.
        fs::write(r.path.join("new.txt"), "x").unwrap();
        assert_eq!(repo_status(&r.path).unwrap().untracked, 1);

        // Stage it.
        r.git(&["add", "new.txt"]);
        assert_eq!(repo_status(&r.path).unwrap().staged, 1);
    }
```

- [ ] **Step 3: Run the tests.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test graph:: 2>&1 | tail -25`
Expected: all `graph::` tests pass. Fix `graph.rs` if red.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/graph.rs
git commit -m "feat(graph): add repo_status (counts, head, in-progress op)" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Tauri commands + TS bindings

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add command wrappers.** In `src-tauri/src/commands.rs`, update the `use` for types and add three commands. Change the existing types import line to include the new types:

```rust
use crate::types::{
    BundleInfo, Commit, DateMapping, GraphCommit, PrerequisiteCheck, Ref, RepoStatus,
    RewriteOptions, SafetyRef,
};
```
Add `use crate::graph;` below the existing `use crate::git_ops;` / `use crate::rewrite;` lines. Then add these three commands at the end of the file:

```rust
#[tauri::command]
pub fn load_graph(repo: String, count: u32, skip: u32) -> Result<Vec<GraphCommit>, String> {
    graph::load_graph(&PathBuf::from(repo), count, skip)
}

#[tauri::command]
pub fn list_refs(repo: String) -> Result<Vec<Ref>, String> {
    graph::list_refs(&PathBuf::from(repo))
}

#[tauri::command]
pub fn repo_status(repo: String) -> Result<RepoStatus, String> {
    graph::repo_status(&PathBuf::from(repo))
}
```

- [ ] **Step 2: Register the commands.** In `src-tauri/src/lib.rs`, add the three to the `tauri::generate_handler!` list (after `commands::load_commits,`):

```rust
            commands::load_commits,
            commands::load_graph,
            commands::list_refs,
            commands::repo_status,
```

- [ ] **Step 3: Mirror the types in TypeScript.** Append to `src/lib/types.ts`:

```ts
export type RefKind = "local" | "remote" | "tag" | "head";

export type RefDecoration = {
  name: string;
  kind: RefKind;
  is_head: boolean;
};

export type GraphCommit = {
  sha: string;
  parents: string[];
  author_name: string;
  author_email: string;
  author_date: string;
  committer_name: string;
  committer_date: string;
  refs: RefDecoration[];
  subject: string;
};

export type Ref = {
  name: string;
  kind: RefKind;
  target_sha: string;
  upstream: string | null;
  ahead: number;
  behind: number;
};

export type HeadInfo = {
  sha: string | null;
  branch: string | null;
  detached: boolean;
};

export type RepoStatus = {
  head: HeadInfo;
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  operation: string | null;
};
```

- [ ] **Step 4: Add the API bindings.** In `src/lib/api.ts`, add `GraphCommit`, `Ref`, `RepoStatus` to the type import block, then add three methods to the `api` object (after the `loadCommits` entry):

```ts
  loadGraph: (repo: string, count: number, skip = 0) =>
    invoke<GraphCommit[]>("load_graph", { repo, count, skip }),
  listRefs: (repo: string) => invoke<Ref[]>("list_refs", { repo }),
  repoStatus: (repo: string) => invoke<RepoStatus>("repo_status", { repo }),
```

- [ ] **Step 5: Verify both sides compile.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo check 2>&1 | tail -20`
Expected: `Finished`, no errors, and no dead-code warnings for the graph types (they are now used by the commands).

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -4`
Expected: svelte-check 0 errors, 0 warnings.

- [ ] **Step 6: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/types.ts src/lib/api.ts
git commit -m "feat(graph): expose load_graph/list_refs/repo_status commands + TS bindings" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Full verification

- [ ] **Step 1: Run every gate.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test 2>&1 | tail -20
cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo check 2>&1 | tail -5
cd /Users/ashshah/Projects/GIT-GUI/git-it && npm test 2>&1 | tail -4
cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -3
```
Expected: all Rust tests pass; `cargo check` clean; vitest still 25/25 (unchanged by this plan); svelte-check 0/0.

- [ ] **Step 2: Confirm the command set is registered.** Run:
`grep -A2 'load_commits' src-tauri/src/lib.rs` and confirm `load_graph`, `list_refs`, `repo_status` are in the handler list.

- [ ] **Step 3: No commit needed** if Tasks 1–5 already committed everything (`git status --porcelain` should be empty). If anything is unstaged, commit it with an appropriate message + the co-author trailer.

---

## Self-review (completed by plan author)

- **Spec coverage (§5.1):** `load_graph` via `git log --all --topo-order` with `%H/%P/%an/%ae/%aI/%cn/%cI/%s` ✓ (`%D` replaced by authoritative for-each-ref join — an improvement, noted); parents ✓; ref decorations w/ kind+is_head ✓; `list_refs` with upstream/ahead-behind ✓; `repo_status` head/counts/operation ✓; new types mirrored in TS ✓; commands registered + bound ✓.
- **Placeholder scan:** none — every step has full code + exact commands + expected output.
- **Type consistency:** Rust `GraphCommit/RefDecoration/RefKind/Ref/HeadInfo/RepoStatus` field names match the TS mirror (snake_case, matching the existing `Commit` precedent); `RefKind` serializes lowercase to match the TS union; command param names (`repo/count/skip`) are single-word so no Tauri camelCase remap is needed; `git_ops::run` made `pub(crate)` before `graph.rs` uses it.
- **Test reality:** fixture repos are built with real `git` and assertions compare against `git rev-parse`, not hard-coded SHAs — robust + deterministic (`GIT_*_DATE` pinned).

## Out of scope (later plans)
Plan 3 (shell UI: render `GraphCommit[]` via the lane engine, sidebar from `Ref[]`, status bar from `RepoStatus`, settings, glass). All Phase 2–6 operations.
