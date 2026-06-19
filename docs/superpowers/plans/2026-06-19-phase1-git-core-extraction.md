# Phase 1 — `git-core` extraction + headless agent skeleton

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development to execute this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the open `git-it` repo into a cargo workspace, lift the (already Tauri-free) git logic into a reusable `crates/git-core` library, and stand up a headless `crates/agent` binary that answers a real `status` request locally with no Tauri runtime — all existing tests green, the desktop app still building and shipping unchanged.

**Architecture:** Today `src-tauri` is one crate; `commands.rs` is a thin `#[tauri::command]` layer that delegates to pure modules (`graph`, `ops*`, `rewrite`, `safety`, `git_ops`, `github`, `types`). We move those pure modules into a new `git-core` rlib, leave the Tauri binding layer (`commands.rs`, `fswatch.rs`, `lib.rs`, `main.rs`) in `src-tauri`, and add an `agent` binary that depends on `git-core`. The progress-streaming seam is *already* a plain `&dyn Fn(String)` callback in `ops_remote`/`rewrite`, so nothing in the core needs redesigning — the Tauri `Channel` glue stays in `commands.rs`.

**Tech Stack:** Rust 2021 (cargo workspace), Tauri 2 (desktop, unchanged), `serde`/`serde_json`/`chrono` (git-core). macOS, `git` + `git-filter-repo` on PATH.

---

## Scope & decisions

- **This plan is Phase 1a.** It delivers the workspace + `git-core` + a headless agent **CLI** that returns repo status (spec acceptance a/b/c). The always-on **menu-bar/login-item shell + Developer-ID signing/notarization** (acceptance d) is **Phase 1b**, a separate plan — it needs a small decision (Swift `MenuBarExtra` app embedding the Rust core via a static lib vs. a pure-Rust tray). The agent's **network transport** is Phase 1.5/2. Phase 1a is the mechanical, low-ambiguity foundation everything else builds on.
- **The Tauri app stays at `src-tauri/`** (a workspace member), rather than renaming to `crates/desktop` as the spec sketched. Rationale: keeping `src-tauri` in place avoids reconfiguring the Tauri CLI's source-dir + the bundle/build config and de-risks the shipping app; the rename is cosmetic and can happen later. (If you'd rather rename now, it's an extra task — flag it.)
- **No frontend changes.** Tauri command names/signatures are unchanged, so the Svelte UI and its `invoke(...)` calls keep working untouched; `vitest` stays green (no Rust→TS contract change).

## File structure (after Phase 1a)

```
git-it/
  Cargo.toml                 NEW — [workspace] manifest
  Cargo.lock                 (relocated to workspace root by cargo)
  package.json               (unchanged — Tauri CLI still finds src-tauri/)
  src-tauri/                 the Tauri desktop crate (package "git-it", lib "git_it_lib")
    Cargo.toml               + git-core path dep; moved modules removed
    src/
      commands.rs            stays — use crate::* → use git_core::*
      fswatch.rs             stays (Tauri-coupled fs watcher)
      lib.rs                 stays — mod decls trimmed; calls git_core::path_setup
      main.rs                stays
  crates/
    git-core/                NEW — Tauri-free git logic (rlib)
      Cargo.toml
      src/
        lib.rs               pub mod declarations
        path_setup.rs        NEW — ensure_homebrew_path (moved from lib.rs)
        types.rs git_ops.rs graph.rs ops.rs ops_merge.rs ops_remote.rs
        ops_rewrite.rs ops_worktree.rs rewrite.rs safety.rs   (moved verbatim)
        github/mod.rs        (moved verbatim)
    agent/                   NEW — headless binary (package "git-it-agent")
      Cargo.toml
      src/
        main.rs              CLI entry: `status <repo>`, `version`
        lib.rs               testable status_json(repo) -> Result<String,String>
  docs/superpowers/plans/2026-06-19-phase1-git-core-extraction.md   (this file)
```

The `git mv` set for Task 2 (move these from `src-tauri/src/` → `crates/git-core/src/`):
`types.rs git_ops.rs graph.rs ops.rs ops_merge.rs ops_remote.rs ops_rewrite.rs ops_worktree.rs rewrite.rs safety.rs github/`

---

## Task 1: Introduce the cargo workspace + empty git-core & agent crates

**Files:**
- Create: `git-it/Cargo.toml`
- Create: `git-it/crates/git-core/Cargo.toml`, `git-it/crates/git-core/src/lib.rs`
- Create: `git-it/crates/agent/Cargo.toml`, `git-it/crates/agent/src/main.rs`

- [ ] **Step 1: Confirm `src-tauri/Cargo.toml` has no `[profile.*]` table.**

Run: `grep -n "^\[profile" /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri/Cargo.toml`
Expected: no output. (If any `[profile.*]` exists it must move to the workspace root manifest below, because cargo only honors profiles at the workspace root. As of writing there are none.)

- [ ] **Step 2: Write the workspace root manifest** `git-it/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["src-tauri", "crates/git-core", "crates/agent"]
```

- [ ] **Step 3: Write `crates/git-core/Cargo.toml`:**

```toml
[package]
name = "git-core"
version = "0.1.0"
edition = "2021"
description = "Tauri-free git logic shared by the Git It desktop app and the headless agent"

[lib]
name = "git_core"
crate-type = ["rlib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 4: Write a stub `crates/git-core/src/lib.rs`** (modules are filled in Task 2):

```rust
// git-core: the Tauri-free git logic shared by the desktop app and the agent.
// Module declarations are populated in Task 2 when the source files move in.
```

- [ ] **Step 5: Write `crates/agent/Cargo.toml`:**

```toml
[package]
name = "git-it-agent"
version = "0.1.0"
edition = "2021"
description = "Headless Git It agent — drives git-core on this machine"

[[bin]]
name = "git-it-agent"
path = "src/main.rs"

[dependencies]
git-core = { path = "../git-core" }
serde_json = "1"
```

- [ ] **Step 6: Write a stub `crates/agent/src/main.rs`:**

```rust
fn main() {
    println!("git-it-agent (skeleton)");
}
```

- [ ] **Step 7: Build the whole workspace.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo build 2>&1 | tail -15`
Expected: `Finished` — all three members compile (the desktop crate unchanged, git-core empty, agent stub). The `Cargo.lock` is now at the workspace root.

- [ ] **Step 8: Confirm the desktop crate's tests still run from the workspace.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo test -p git-it 2>&1 | tail -15`
Expected: the existing ~120 Rust tests still run and pass (they're still in `src-tauri` at this point).

- [ ] **Step 9: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add Cargo.toml crates/
git commit -m "build: introduce cargo workspace with empty git-core + agent crates

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Extract `git-core` (move the pure modules + rewire the desktop crate)

This is one atomic task — the move breaks the desktop crate until the rewire is done, so do all steps before verifying.

**Files:**
- Move: `src-tauri/src/{types,git_ops,graph,ops,ops_merge,ops_remote,ops_rewrite,ops_worktree,rewrite,safety}.rs` and `src-tauri/src/github/` → `crates/git-core/src/`
- Create: `crates/git-core/src/path_setup.rs`
- Modify: `crates/git-core/src/lib.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/Cargo.toml`

- [ ] **Step 1: Move the pure modules** (use `git mv` to preserve history):

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri/src
git mv types.rs git_ops.rs graph.rs ops.rs ops_merge.rs ops_remote.rs ops_rewrite.rs ops_worktree.rs rewrite.rs safety.rs ../../crates/git-core/src/
git mv github ../../crates/git-core/src/github
```

- [ ] **Step 2: Create `crates/git-core/src/path_setup.rs`** with the `ensure_homebrew_path` fn moved out of the desktop `lib.rs`, made `pub`:

```rust
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
```

- [ ] **Step 3: Fill in `crates/git-core/src/lib.rs`** with the module declarations:

```rust
// git-core: the Tauri-free git logic shared by the desktop app and the agent.
pub mod git_ops;
pub mod github;
pub mod graph;
pub mod ops;
pub mod ops_merge;
pub mod ops_remote;
pub mod ops_rewrite;
pub mod ops_worktree;
pub mod path_setup;
pub mod rewrite;
pub mod safety;
pub mod types;
```

The moved modules reference each other via `crate::<mod>` — still correct, since they're now in the same `git-core` crate.

- [ ] **Step 4: Build git-core in isolation and confirm zero Tauri deps.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo build -p git-core 2>&1 | tail -15`
Expected: `Finished`. If a moved module fails to resolve a sibling, it's a `crate::` path issue — they should all already be `crate::` and resolve.

Run: `cargo tree -p git-core -e normal 2>/dev/null | grep -i tauri; echo "exit=$?"`
Expected: no tauri lines, `exit=1` (grep found nothing) — git-core does not depend on Tauri.

- [ ] **Step 5: Run the moved tests under git-core.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo test -p git-core 2>&1 | tail -20`
Expected: ~100 tests pass (graph 8, ops 8, ops_merge 14, ops_remote 14, ops_rewrite 14, ops_worktree 20, github ~22, plus others). 0 failures.

- [ ] **Step 6: Rewire `src-tauri/src/lib.rs`.** Make these edits:
  1. Delete the moved `mod` declarations (the lines `mod git_ops;`, `mod github;`, `mod graph;`, `mod ops;`, `mod ops_merge;`, `mod ops_remote;`, `mod ops_rewrite;`, `mod ops_worktree;`, `mod rewrite;`, `mod safety;`, `mod types;`). **Keep** `mod commands;` and `mod fswatch;`.
  2. Delete the entire local `ensure_homebrew_path` fn (it now lives in git-core).
  3. Change the call `ensure_homebrew_path();` → `git_core::path_setup::ensure_homebrew_path();`.
  4. Change `.manage(std::sync::Arc::new(ops_remote::RemoteState::default()))` → `.manage(std::sync::Arc::new(git_core::ops_remote::RemoteState::default()))`.
  `fswatch::WatchState::default()` and the `fswatch::start_watch`/`stop_watch` handler entries stay unchanged.

- [ ] **Step 7: Rewire `src-tauri/src/commands.rs`.** Every `crate::` reference in this file points at a moved module, so rewrite them all to `git_core::`:

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri/src
sed -i '' -E 's/\bcrate::/git_core::/g' commands.rs
```

This converts the 9 `use crate::<mod>;` lines, the `use crate::types::{...}` line, and the inline `crate::safety::restore(...)` / `crate::ops_remote::...` references to `git_core::`. (commands.rs never references `fswatch`, so nothing desktop-local is wrongly rewritten.)

- [ ] **Step 8: Check `fswatch.rs` for any moved-module references.**

Run: `grep -n "crate::\(git_ops\|github\|graph\|ops\|ops_merge\|ops_remote\|ops_rewrite\|ops_worktree\|rewrite\|safety\|types\)" /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri/src/fswatch.rs; echo "exit=$?"`
Expected: no output (fswatch is self-contained). If any appear, change those `crate::` → `git_core::` too.

- [ ] **Step 9: Add the git-core dependency to `src-tauri/Cargo.toml`.** Under `[dependencies]` add:

```toml
git-core = { path = "../crates/git-core" }
```

Keep `tauri`, the `tauri-plugin-*` deps, `serde`, `serde_json`, and `notify` (used by `fswatch.rs`). `chrono` is no longer used directly by the desktop crate — leave it for now; it can be pruned later if `cargo build` warns it's unused (it won't error).

- [ ] **Step 10: Build the desktop crate and run its tests.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo build -p git-it 2>&1 | tail -20`
Expected: `Finished`. Fix any remaining `crate::` → `git_core::` miss the compiler points at.

Run: `cargo test -p git-it 2>&1 | tail -10`
Expected: passes (few/no inline tests remain in the desktop crate; all moved to git-core).

- [ ] **Step 11: Full workspace test + svelte-check.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo test --workspace 2>&1 | tail -15`
Expected: all tests pass across git-core + desktop (~120 total, matching the pre-extraction count).

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -5`
Expected: 0 errors, 0 warnings (frontend untouched).

- [ ] **Step 12: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add -A
git commit -m "refactor: extract Tauri-free git-core crate from src-tauri

Move the pure git logic (graph, ops*, rewrite, safety, git_ops, github, types)
into crates/git-core; rewire the desktop crate to depend on it. No behavior or
Tauri command-signature changes; the desktop app and all tests are unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Headless agent skeleton — `status` over the CLI

Prove the core runs headless (no Tauri runtime) by having the agent return real repo status.

**Files:**
- Create: `crates/agent/src/lib.rs`
- Modify: `crates/agent/src/main.rs`

- [ ] **Step 1: Write the failing test first** in `crates/agent/src/lib.rs`:

```rust
//! Headless agent core: drives git-core on this machine. Transport-agnostic —
//! Phase 1a exposes a single `status_json` entry the CLI calls.

use git_core::{graph, path_setup};

/// Return the repo's status as a JSON string, or an error message.
pub fn status_json(repo: &str) -> Result<String, String> {
    path_setup::ensure_homebrew_path();
    let status = graph::repo_status(repo.to_string()).map_err(|e| e.to_string())?;
    serde_json::to_string(&status).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init_temp_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gitit-agent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(&dir).output().unwrap();
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("a.txt"), "hi").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "first"]);
        dir
    }

    #[test]
    fn status_json_returns_branch_for_a_real_repo() {
        let dir = init_temp_repo();
        let out = status_json(dir.to_str().unwrap()).expect("status_json should succeed");
        assert!(out.contains("\"branch\""), "status JSON should include a branch field: {out}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

> Verify the field name: confirm `git_core::types::RepoStatus` serializes a `branch` field (check `crates/git-core/src/types.rs`). If the serialized key differs (e.g. `head`/`current`), assert on the actual key instead. `repo_status`'s exact signature is in `crates/git-core/src/graph.rs` — adjust the call if it takes `&str`/`PathBuf` rather than `String`.

- [ ] **Step 2: Run the test to verify it compiles + passes.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo test -p git-it-agent 2>&1 | tail -15`
Expected: PASS. (If it fails to compile on the `repo_status` signature or the `branch` key, fix per the note above, then re-run.)

- [ ] **Step 3: Wire `crates/agent/src/main.rs`** to the lib:

```rust
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("version") => println!("git-it-agent {}", env!("CARGO_PKG_VERSION")),
        Some("status") => match args.get(2) {
            Some(repo) => match git_it_agent::status_json(repo) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            },
            None => {
                eprintln!("usage: git-it-agent status <repo-path>");
                exit(2);
            }
        },
        _ => {
            eprintln!("usage: git-it-agent <version|status <repo-path>>");
            exit(2);
        }
    }
}
```

Add `[lib]` to `crates/agent/Cargo.toml` so `main.rs` can call the lib crate `git_it_agent`:

```toml
[lib]
name = "git_it_agent"
path = "src/lib.rs"
```

- [ ] **Step 4: Confirm the agent runs headless with no Tauri dep.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo run -p git-it-agent -- status "$PWD" 2>&1 | tail -5`
Expected: a one-line JSON status object for the git-it repo printed to stdout.

Run: `cargo tree -p git-it-agent -e normal 2>/dev/null | grep -i tauri; echo "exit=$?"`
Expected: no tauri lines, `exit=1` — the agent links only git-core, not Tauri.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add -A
git commit -m "feat(agent): headless status CLI over git-core (no Tauri runtime)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Full-gate verification (desktop app still ships)

Prove Phase 1a's acceptance criteria end to end.

- [ ] **Step 1: Rust — all gates.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && cargo test --workspace 2>&1 | tail -10`
Expected: all tests pass (git-core ~100 + agent 1 + desktop remainder; total ≈ prior 120).

- [ ] **Step 2: Frontend gates unchanged.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check 2>&1 | tail -5 && npm test 2>&1 | tail -8`
Expected: svelte-check 0/0; vitest still green at its prior count (no frontend changes).

- [ ] **Step 3: The big one — the desktop `.app` still builds from the workspace.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run tauri build 2>&1 | tail -25`
Expected: a successful bundle. The cargo target dir is now the workspace root `target/`; confirm Tauri locates and bundles the `.app` (it reads the target dir from cargo metadata, so this should just work). If Tauri reports a path it can't find, the fix is in `src-tauri/tauri.conf.json` build paths — adjust and re-run. (This step is slow; it is the acceptance gate for "desktop app unchanged.")

- [ ] **Step 4: Confirm the dependency boundaries one more time.**

Run:
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
echo "git-core tauri deps:"; cargo tree -p git-core -e normal 2>/dev/null | grep -ic tauri
echo "agent tauri deps:";    cargo tree -p git-it-agent -e normal 2>/dev/null | grep -ic tauri
```
Expected: both print `0`.

- [ ] **Step 5: Commit any build-config fixes** (only if Step 3 required tauri.conf.json edits):

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add -A
git commit -m "build: fix Tauri bundle paths for workspace target dir

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Acceptance criteria (Phase 1a)
- [ ] `cargo build -p git-core` compiles with **zero** Tauri deps; all moved tests pass under `cargo test -p git-core`.
- [ ] The desktop app still builds (`npm run tauri build`) and all gates stay green (cargo workspace tests, svelte-check 0/0, vitest unchanged) — no frontend changes.
- [ ] `cargo run -p git-it-agent -- status <repo>` returns real repo status with **no Tauri runtime present** (agent has 0 tauri deps).
- [ ] Git history preserved via `git mv`; commits are scoped (workspace, extraction, agent, optional build-fix).

## Deferred to Phase 1b (separate plan)
- Always-on **menu-bar / login-item** agent shell (`SMAppService`) + the macOS app-shell decision (Swift `MenuBarExtra` embedding git-core as a static lib vs. pure-Rust tray).
- **Developer-ID code-signing + notarization** pipeline for the agent (long lead time — start early in 1b).
- Optionally extract `fswatch`'s pure `classify`/`Kind` into git-core (left in `src-tauri` here; the agent doesn't need it yet).
- Optional cosmetic rename `src-tauri` → `crates/desktop`.

## Self-review notes
- **Coverage vs spec Phase 1:** acceptance (a)(b)(c) are covered here; (d) menu-bar/login-item is explicitly Phase 1b (it needs a design decision, so splitting keeps this plan unambiguous and shippable on its own).
- **No placeholders:** all new files have complete code; edits to existing files are exact (line-targeted or `sed`). The two values to confirm against source during Task 3 (the `RepoStatus` `branch` key and `repo_status`'s exact arg type) are called out inline with how to adapt.
- **Risk watch:** the one genuine risk is the workspace `target/` relocation affecting `npm run tauri build` — Task 4 Step 3 verifies it explicitly. Keeping `src-tauri` in place (not renaming) removes the Tauri-source-dir reconfiguration risk entirely.
