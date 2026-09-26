# Pull Dialog — Implementation Plan (Part C, PR 3, `feat/pull-dialog`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every pull carries an explicit strategy (merge / rebase / fast-forward only) and optional autostash, chosen in a Pull dialog that opens before each pull (or on ⌥-click) and remembered as three persisted settings that replace `pullRebase`.

**Architecture:** `git-core` gains `PullStrategy`; `ops_remote::pull` and the Tauri `pull` command take `strategy` + `autostash` instead of `rebase: bool`, so git's "Need to specify how to reconcile divergent branches" refusal can no longer happen. On the frontend, every string the dialog shows and the `pullRebase → pullStrategy` migration live in a pure, vitest-covered `src/lib/pullOptions.ts`; the store swaps `pullRebase` for `pullStrategy` / `pullAutostash` / `pullAskEveryTime` using the existing Tauri-store + localStorage pattern; `dialogs.svelte.ts` gains a `pull` kind rendered by `Modal.svelte`; and `gitActions.pull({ showOptions })` asks first (when enabled or ⌥-clicked), saves the choice as the new default, then runs the unchanged `runRemote`.

**Tech Stack:** Rust (cargo workspace: `git-core` + `src-tauri`, serde), Tauri 2 async commands, SvelteKit 5 runes, TypeScript, vitest (node for pure/store/dialog tests, jsdom for component tests).

## Global Constraints

- **Starting state:** branch `feat/pull-dialog` is cut from `next` **after PR 1 (`feat/error-reporting-core`) has merged.** PR 1 provides, verbatim — never re-implement:
  ```ts
  // src/lib/errors.svelte.ts
  export function reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void;
  export function reportNotice(title: string, message: string): void; // ⓘ popup + appState.status = message
  export const errors: { readonly current: ErrorReport | null; readonly pending: number; readonly last: ErrorReport | null; dismiss(): void; reopenLast(): void; reset(): void };
  // ErrorReport = { id: number; kind: "error" | "notice"; title: string; headline: string; hints: string; output: string }
  ```
  After PR 1, `runRemote` in `src/lib/gitActions.ts` reports its own failures (popup via `reportError`; conflicts / user Cancel / dismissed sign-in / repo-changed-during-sign-in are NOT popups) and still sets `appState.status = \`${label} — done.\`` on success. The pull flow only calls `runRemote(label, fn)` as today.
- **Ownership:** PR 3 owns the ENTIRE `gitActions.pull` export, including its "Repository is still loading — try again in a moment." guard, which becomes `reportNotice("Can't pull yet", "Repository is still loading — try again in a moment.")`. Do not touch other `gitActions` exports (PR 2 handles the fetch/push guards).
- **This plan was written before PR 1 landed.** Code is referenced by function name and the "replace this" quotes come from today's tree. If PR 1 changed a quoted line (e.g. an import list), match on the function name and keep PR 1's version of the untouched lines.
- **Gate before claiming any task done:** `npm run check` (0 errors, 0 warnings) + `npm test` + `cargo test` + `cargo clippy -p git-core -p git-it -- -D warnings` (CI's exact clippy command — CI runs clippy, so a warning fails the PR).
- **No fake controls (standing user rule):** never commit a visible control that nothing honours yet. The "Ask before every pull" checkbox therefore lands in Task 6, in the same commit as the flow that reads it; the dialog's "Don't ask again" (Task 5) is unreachable until Task 6 wires a caller.
- **Bind a confirmed action to what was confirmed (PRs #32/#33):** capture before the first `await`, verify after every one. For Pull that means the repository identity, its loading state, and the `{ branch, upstream }` the dialog showed — all re-checked after the dialog and before any preference write or `api.pull` call.
- **Shell-out safety (hard rule):** `--` / `--end-of-options` before user-controlled operands passed to `git`. `git pull` here takes no operands, but keep the rule for any new invocation.
- **Async Tauri commands:** `pull` stays `#[tauri::command] pub async fn` with `spawn_blocking`. The command name stays `pull`, so `src-tauri/src/lib.rs` registration is unchanged.
- **Serde/flag contract (verified on git 2.54):** `PullStrategy { Merge, Rebase, FfOnly }` with `#[serde(rename_all = "camelCase")]` → `"merge" | "rebase" | "ffOnly"`. Flags: Merge `--no-rebase --no-edit`, Rebase `--rebase`, FfOnly `--ff-only`; `autostash` adds `--autostash`. Merge wins over a repo's `pull.ff=only` AND `pull.rebase=true`; `--ff-only` refuses a diverged branch with `fatal: Not possible to fast-forward, aborting.` even under `pull.rebase=true`.
- **Surgical changes, match surrounding style.** Remove only what your change orphans (the `pullRebase` getter/setter/loader/persist and their only callers). Don't "improve" adjacent code.
- **Pure logic lives in a testable `src/lib/*.ts`** (vitest), not inside a `.svelte` component.
- **Svelte 5 runes.** The UI must look native in both Classic and NERV themes. NERV restyles `.dialog` globally from `src/lib/theme/nerv.css` (corner brackets, `h3::before` glyph) — this plan touches no `nerv.css`; if you ever do, never use `:global()` there.
- **Vitest cannot exercise Tauri paths** (a real `git pull`). Those need a debug `.app` (Task 7).
- **Commits:** conventional commits, subject lowercase-first and ≤100 chars, body lines ≤100 chars, multi-line messages via heredoc, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## File structure

| File | Responsibility |
|---|---|
| `crates/git-core/src/types.rs` (modify) | `PullStrategy` enum (serde camelCase). |
| `crates/git-core/src/ops_remote.rs` (modify) | `pull(repo, strategy, autostash, …)` builds the explicit flags; tests on real diverged clones. |
| `src-tauri/src/commands.rs` (modify) | Tauri `pull` command takes `strategy` + `autostash`. |
| `src/lib/types.ts` (modify) | `export type PullStrategy = "merge" \| "rebase" \| "ffOnly"`. |
| `src/lib/pullOptions.ts` (create) + `pullOptions.test.ts` | Pure copy (labels, descriptions, status line, title, tooltip), `canPull`, `pullContext` (branch/upstream/ahead/behind from the repository status + ref inventory, never the loaded graph page), and the `resolvePullStrategy` migration helper. |
| `src/lib/api.ts` (modify) | `api.pull(repo, strategy, autostash, onEvent, creds)`. |
| `src/lib/store.svelte.ts` (modify) + `store.pullSettings.test.ts` | `pullStrategy` / `pullAutostash` / `pullAskEveryTime` replace `pullRebase` (same persistence pattern; legacy key migrated, never written) + a `waitForPullSettingsHydration()` barrier mirroring `waitForUpdateSettingsHydration()`. |
| `src/lib/components/SettingsPanel.svelte` (modify) | Behavior tab: strategy radio group + autostash checkbox (Task 3); "Ask before every pull" arrives in Task 6 with the flow that honours it. |
| `src/lib/dialogs.svelte.ts` (modify) + `dialogs.pull.test.ts` | `pull` dialog kind: opener, setters, resolver, `settlePending`. |
| `src/lib/components/Modal.svelte` (modify) + `Modal.pull.test.ts` | Renders the pull dialog (radios with descriptions, autostash, don't-ask, Cancel/Pull, Enter/Escape, focus). |
| `src/lib/gitActions.ts` (modify) + `gitActions.pull.test.ts` | `gitActions.pull({ showOptions })`: notice guard → hydration barrier → dialog → re-check repo / loading / `{ branch, upstream }` → persist → `runRemote`. |
| `src/lib/gitActions.errorReporting.test.ts` (modify — PR 1's suite) | Its seven `runRemote` cases call `gitActions.pull()`; Task 6 turns asking off in their `beforeEach` and makes the cancel case wait for `api.pull` to be in flight. |
| `src/routes/+page.svelte` (modify) | Toolbar Pull button: ⌥-click passes `showOptions`, tooltip names the strategy when asking is off. |

Task order keeps every commit green: Rust first (Task 1), then the pure module (2), then the settings + IPC signature + minimal `pull` rewrite (3), then the dialog state (4), its rendering (5), and finally the ask-first flow + toolbar (6). Task 1's Step 3b bridges `api.pull`'s IPC payload (`rebase` → `strategy`) in the same commit as the backend change, so the real app's Pull works at every commit.

---

### Task 1: `PullStrategy` + explicit pull flags in git-core and the Tauri command

**Files:**
- Modify: `crates/git-core/src/types.rs` (append after `RemoteOutcome`)
- Modify: `crates/git-core/src/ops_remote.rs` (`pull` and its `#[cfg(test)] mod tests`)
- Modify: `src-tauri/src/commands.rs` (`pull` command + the `git_core::types` import)
- Modify: `src/lib/api.ts` (`pull`'s invoke payload only — a bridge that keeps Pull working until Task 3)
- Test: `crates/git-core/src/ops_remote.rs` (`mod tests`)

**Interfaces:**
- Consumes: `crate::types::RemoteOutcome`, `ops_remote::{stream, outcome, setup_askpass, RemoteState}` (unchanged).
- Produces:
  - `git_core::types::PullStrategy { Merge, Rebase, FfOnly }` — `Deserialize, Clone, Copy, Debug, PartialEq, Eq`, serde `camelCase`.
  - `ops_remote::pull(repo: &Path, strategy: PullStrategy, autostash: bool, username: Option<&str>, password: Option<&str>, state: &RemoteState, on_line: &dyn Fn(String)) -> Result<RemoteOutcome, String>`.
  - Tauri command `pull(repo: String, strategy: PullStrategy, autostash: bool, username: Option<String>, password: Option<String>, on_event: Channel<String>, state)` — the frontend invokes it as `{ repo, strategy, autostash, username, password, onEvent }`.

- [ ] **Step 1: Write the failing tests**

In `crates/git-core/src/ops_remote.rs`, inside `mod tests`, add a stdout helper to `impl TempRepo` right after the existing `git_may_fail` method:

```rust
        /// Run git and return its trimmed stdout — for inspecting history after a pull.
        fn git_stdout(&self, args: &[&str]) -> String {
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
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
```

Update the two existing `pull(...)` call sites to the new signature. In `push_then_pull_roundtrip_local_bare`, replace:

```rust
        let pull_result = pull(&clone_b.path, false, None, None, &state_b, &noop_line).unwrap();
```

with:

```rust
        let pull_result =
            pull(&clone_b.path, PullStrategy::Merge, false, None, None, &state_b, &noop_line).unwrap();
```

In `pull_conflict_sets_conflicted`, replace:

```rust
        let pull_result = pull(&clone_b.path, false, None, None, &sb, &noop_line).unwrap();
```

with:

```rust
        let pull_result =
            pull(&clone_b.path, PullStrategy::Merge, false, None, None, &sb, &noop_line).unwrap();
```

Then append these fixtures and tests at the end of `mod tests` (before its closing `}`):

```rust
    // ── Pull strategies — real diverged clones ───────────────────────────────

    /// Bare origin + two clones whose `main` branches have DIVERGED. Clone A pushed a
    /// commit that rewrites line 1 of `f.txt`; clone B has its own commit (a new file,
    /// so a merge is clean) that origin lacks. Returns (bare, a, b); tests pull in B.
    fn diverged_clones(tag: &str) -> (PathBuf, TempRepo, TempRepo) {
        let bare = TempRepo::new_bare(bare_path(tag));
        let a = TempRepo::new_clone(bare.to_str().unwrap(), bare_path(&format!("a-{tag}")));
        a.write_file("f.txt", "line1\nline2\nline3\n");
        a.git(&["add", "f.txt"]);
        a.git(&["commit", "-q", "-m", "base"]);
        a.git(&["push", "-q", "-u", "origin", "main"]);

        // B clones AFTER the base push, so its `main` tracks origin/main from the start.
        let b = TempRepo::new_clone(bare.to_str().unwrap(), bare_path(&format!("b-{tag}")));

        a.write_file("f.txt", "LINE1\nline2\nline3\n");
        a.git(&["add", "f.txt"]);
        a.git(&["commit", "-q", "-m", "remote edit"]);
        a.git(&["push", "-q", "origin", "main"]);

        b.write_file("local.txt", "local\n");
        b.git(&["add", "local.txt"]);
        b.git(&["commit", "-q", "-m", "local commit"]);
        (bare, a, b)
    }

    /// Number of parents of HEAD: 1 on a linear history, 2 after a merge commit.
    fn head_parent_count(r: &TempRepo) -> usize {
        r.git_stdout(&["rev-list", "--parents", "-n", "1", "HEAD"])
            .split_whitespace()
            .count()
            - 1
    }

    #[test]
    fn pull_merge_creates_merge_commit_despite_pull_ff_only_config() {
        let (bare, _a, b) = diverged_clones("merge-ff-only");
        b.git(&["config", "pull.ff", "only"]);
        let st = RemoteState::default();
        let r = pull(&b.path, PullStrategy::Merge, false, None, None, &st, &noop_line).unwrap();
        assert!(r.ok, "merge pull should succeed; msg={}", r.message);
        assert_eq!(head_parent_count(&b), 2, "a diverged merge pull creates a merge commit");
        assert_eq!(fs::read_to_string(b.path.join("f.txt")).unwrap(), "LINE1\nline2\nline3\n");
        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn pull_merge_creates_merge_commit_despite_pull_rebase_config() {
        let (bare, _a, b) = diverged_clones("merge-rebase-cfg");
        b.git(&["config", "pull.rebase", "true"]);
        let st = RemoteState::default();
        let r = pull(&b.path, PullStrategy::Merge, false, None, None, &st, &noop_line).unwrap();
        assert!(r.ok, "merge pull should succeed; msg={}", r.message);
        assert_eq!(head_parent_count(&b), 2, "--no-rebase must win over pull.rebase=true");
        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn pull_rebase_replays_local_commit_on_the_remote_tip() {
        let (bare, _a, b) = diverged_clones("rebase");
        let st = RemoteState::default();
        let r = pull(&b.path, PullStrategy::Rebase, false, None, None, &st, &noop_line).unwrap();
        assert!(r.ok, "rebase pull should succeed; msg={}", r.message);
        assert_eq!(head_parent_count(&b), 1, "a rebase pull keeps history linear");
        assert_eq!(
            b.git_stdout(&["rev-parse", "HEAD^"]),
            b.git_stdout(&["rev-parse", "origin/main"]),
            "the local commit should sit directly on top of origin/main"
        );
        assert_eq!(b.git_stdout(&["log", "-1", "--format=%s"]), "local commit");
        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn pull_ff_only_refuses_a_diverged_branch() {
        let (bare, _a, b) = diverged_clones("ff-only");
        // Even a repo that asks for rebases gets the fast-forward-only refusal.
        b.git(&["config", "pull.rebase", "true"]);
        let before = b.git_stdout(&["rev-parse", "HEAD"]);
        let st = RemoteState::default();
        let r = pull(&b.path, PullStrategy::FfOnly, false, None, None, &st, &noop_line).unwrap();
        assert!(!r.ok, "ff-only must refuse a diverged branch; msg={}", r.message);
        assert!(!r.auth_failed);
        assert!(!r.conflicted, "a refusal leaves no conflicts behind");
        assert!(r.message.contains("Not possible to fast-forward"), "msg={}", r.message);
        assert_eq!(b.git_stdout(&["rev-parse", "HEAD"]), before, "HEAD must not move");
        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn pull_autostash_keeps_a_dirty_edit_that_would_block_the_merge() {
        let (bare, _a, b) = diverged_clones("autostash");
        // Dirty edit on line 3; the remote commit rewrote line 1 of the same file.
        b.write_file("f.txt", "line1\nline2\nLINE3-local\n");

        let st = RemoteState::default();
        let refused =
            pull(&b.path, PullStrategy::Merge, false, None, None, &st, &noop_line).unwrap();
        assert!(!refused.ok, "an overlapping dirty file blocks a plain merge pull");
        assert!(
            refused.message.contains("would be overwritten by merge"),
            "msg={}",
            refused.message
        );
        assert_eq!(
            fs::read_to_string(b.path.join("f.txt")).unwrap(),
            "line1\nline2\nLINE3-local\n",
            "a refused pull leaves the dirty edit untouched"
        );

        let st2 = RemoteState::default();
        let r = pull(&b.path, PullStrategy::Merge, true, None, None, &st2, &noop_line).unwrap();
        assert!(r.ok, "autostash pull should succeed; msg={}", r.message);
        assert_eq!(head_parent_count(&b), 2);
        assert_eq!(
            fs::read_to_string(b.path.join("f.txt")).unwrap(),
            "LINE1\nline2\nLINE3-local\n",
            "the remote's line 1 and the local line 3 edit should both survive"
        );
        let _ = fs::remove_dir_all(&bare);
    }
```

- [ ] **Step 2: Run the tests to verify they fail to compile**

Run: `cargo test -p git-core pull`
Expected: compile error — `error[E0433]: failed to resolve: use of undeclared type `PullStrategy`` (and a mismatched-arity error on `pull(...)`). No test runs.

- [ ] **Step 3: Add the enum, the flags, and the command**

`crates/git-core/src/types.rs` — append after the `RemoteOutcome` struct:

```rust
/// How `git pull` reconciles a branch that has diverged from its upstream. Every pull
/// names one explicitly, so git's "Need to specify how to reconcile divergent branches"
/// refusal cannot occur and the choice wins over the repo's own pull.rebase / pull.ff.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PullStrategy {
    Merge,
    Rebase,
    FfOnly,
}
```

`crates/git-core/src/ops_remote.rs` — change the import at the top from:

```rust
use crate::types::{RemoteInfo, RemoteOutcome};
```

to:

```rust
use crate::types::{PullStrategy, RemoteInfo, RemoteOutcome};
```

Then replace the `pull` function. Current code:

```rust
/// Pull from the configured upstream. Uses `--rebase` or `--no-edit` (merge).
/// `GIT_TERMINAL_PROMPT=0` + `GIT_EDITOR=true` so git never blocks waiting for input.
///
/// When `username` AND `password` are both `Some`, credentials are written to `0600`
/// temp files and a `GIT_ASKPASS` script is generated — credentials are NEVER placed
/// on argv, in URLs, or persisted. The scratch dir is deleted after the op.
pub fn pull(
    repo: &Path,
    rebase: bool,
    username: Option<&str>,
    password: Option<&str>,
    state: &RemoteState,
    on_line: &dyn Fn(String),
) -> Result<RemoteOutcome, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .arg("pull");
    if rebase {
        c.arg("--rebase");
    } else {
        c.arg("--no-edit");
    }
```

New code (everything from `// Set up askpass only when both credentials are present.` down stays as is):

```rust
/// Pull from the configured upstream with an explicit `strategy` (see `PullStrategy`),
/// optionally `--autostash`ing local changes around it.
/// `GIT_TERMINAL_PROMPT=0` + `GIT_EDITOR=true` so git never blocks waiting for input.
///
/// When `username` AND `password` are both `Some`, credentials are written to `0600`
/// temp files and a `GIT_ASKPASS` script is generated — credentials are NEVER placed
/// on argv, in URLs, or persisted. The scratch dir is deleted after the op.
pub fn pull(
    repo: &Path,
    strategy: PullStrategy,
    autostash: bool,
    username: Option<&str>,
    password: Option<&str>,
    state: &RemoteState,
    on_line: &dyn Fn(String),
) -> Result<RemoteOutcome, String> {
    let mut c = Command::new("git");
    c.current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .arg("pull");
    // Every pull names its strategy, so git never asks how to reconcile a diverged
    // branch, and the explicit flag wins over the repo's own pull.rebase / pull.ff.
    match strategy {
        PullStrategy::Merge => {
            c.args(["--no-rebase", "--no-edit"]);
        }
        PullStrategy::Rebase => {
            c.arg("--rebase");
        }
        PullStrategy::FfOnly => {
            c.arg("--ff-only");
        }
    }
    if autostash {
        c.arg("--autostash");
    }
```

`src-tauri/src/commands.rs` — add `PullStrategy` to the `git_core::types` import so it reads:

```rust
use git_core::types::{
    BundleInfo, Commit, ConflictEntry, DateMapping, GraphCommit, InitializeRepositoryResult,
    OpOutcome, PrerequisiteCheck, PullStrategy, RebaseOutcome, RebaseStep, Ref, ReflogEntry,
    RemoteInfo, RemoteOutcome, RepoStatus, RewriteOptions, RewriteResult, SafetyRef, StashEntry,
    WorkingFile, WorktreeInfo,
};
```

and replace the `pull` command. Current code:

```rust
#[tauri::command]
pub async fn pull(
    repo: String,
    rebase: bool,
    username: Option<String>,
    password: Option<String>,
    on_event: Channel<String>,
    state: State<'_, Arc<ops_remote::RemoteState>>,
) -> Result<RemoteOutcome, String> {
    let st = state.inner().clone(); // clone the Arc — now 'static-safe
    let repo_path = PathBuf::from(repo);
    let chan = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops_remote::pull(
            &repo_path,
            rebase,
            username.as_deref(),
            password.as_deref(),
            &st,
            &|l| { let _ = chan.send(l); },
        )
    })
    .await
    .map_err(|e| format!("join: {}", e))?
}
```

New code:

```rust
#[tauri::command]
pub async fn pull(
    repo: String,
    strategy: PullStrategy,
    autostash: bool,
    username: Option<String>,
    password: Option<String>,
    on_event: Channel<String>,
    state: State<'_, Arc<ops_remote::RemoteState>>,
) -> Result<RemoteOutcome, String> {
    let st = state.inner().clone(); // clone the Arc — now 'static-safe
    let repo_path = PathBuf::from(repo);
    let chan = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops_remote::pull(
            &repo_path,
            strategy,
            autostash,
            username.as_deref(),
            password.as_deref(),
            &st,
            &|l| { let _ = chan.send(l); },
        )
    })
    .await
    .map_err(|e| format!("join: {}", e))?
}
```

- [ ] **Step 3b: Keep the frontend in step with the command**

The command now takes `strategy` and `autostash` instead of `rebase`. Until Task 3 changes `api.pull`'s signature, translate at the IPC boundary so every commit leaves Pull working. As a bonus, plain Pull now sends an explicit merge, which already fixes the divergent-branch failure. In `src/lib/api.ts`, inside `pull`, replace:

```ts
    return invoke<RemoteOutcome>("pull", {
      repo,
      rebase,
```

with:

```ts
    return invoke<RemoteOutcome>("pull", {
      repo,
      // Bridge until the pull settings land (Task 3): the command takes a strategy now.
      strategy: rebase ? "rebase" : "merge",
      autostash: false,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm run check`
Expected: 0 errors and 0 warnings.

Run: `cargo test -p git-core pull`
Expected: `test result: ok.` with the 5 new tests plus `push_then_pull_roundtrip_local_bare` and `pull_conflict_sets_conflicted` passing.

Run: `cargo test && cargo clippy -p git-core -p git-it -- -D warnings`
Expected: all crates green, clippy clean (the new `pull` has 7 parameters, which is exactly clippy's `too_many_arguments` limit — no `#[allow]` needed).

- [ ] **Step 5: Commit**

```bash
git add crates/git-core/src/types.rs crates/git-core/src/ops_remote.rs src-tauri/src/commands.rs src/lib/api.ts
git commit -m "$(cat <<'EOF'
feat: pass an explicit pull strategy and optional autostash to git pull

Every pull now names merge, rebase or fast-forward-only, so git's "Need to
specify how to reconcile divergent branches" refusal cannot occur and the
choice wins over a repo's own pull.rebase / pull.ff. Merge is
--no-rebase --no-edit, rebase is --rebase, ff-only is --ff-only, and
autostash adds --autostash. Covered by tests on real diverged clones.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Pure pull copy + settings migration (`pullOptions.ts`)

**Files:**
- Modify: `src/lib/types.ts` (append after `RemoteOutcome`)
- Create: `src/lib/pullOptions.ts`
- Test: `src/lib/pullOptions.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces (all used by Tasks 3–6):
  - `export type PullStrategy = "merge" | "rebase" | "ffOnly"` (in `types.ts`)
  - `PULL_STRATEGIES: readonly PullStrategy[]` — `["merge", "rebase", "ffOnly"]`
  - `PULL_STRATEGY_LABELS: Record<PullStrategy, string>` — `Merge` / `Rebase` / `Fast-forward only`
  - `PULL_STRATEGY_DESCRIPTIONS: Record<PullStrategy, string>`
  - `pullLabel(strategy): string` — `"Pull"` / `"Pull (rebase)"` / `"Pull (fast-forward only)"`
  - `pullButtonTitle(askEveryTime: boolean, strategy): string`
  - `canPull(branch: string | null, upstream: string | null): boolean`
  - `pullDialogTitle(branch, upstream): string`
  - `pullStatusLine(branch, upstream, ahead: number, behind: number): string`
  - `resolvePullStrategy(saved: unknown, legacyRebase: unknown): PullStrategy`
  - `export type PullContext = { branch: string | null; upstream: string | null; ahead: number; behind: number }`
  - `pullContext(status: RepoStatus | null, refs: Ref[]): PullContext` — the branch comes from the repository status (`null` when detached or unknown), tracking data from the `{ kind: "local", name: branch }` inventory entry. Never from graph decorations: a checked-out branch whose commit is outside the loaded page has no `is_head` decoration.

- [ ] **Step 1: Write the failing test**

Create `src/lib/pullOptions.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  PULL_STRATEGIES,
  PULL_STRATEGY_DESCRIPTIONS,
  PULL_STRATEGY_LABELS,
  canPull,
  pullButtonTitle,
  pullContext,
  pullDialogTitle,
  pullLabel,
  pullStatusLine,
  resolvePullStrategy,
} from "./pullOptions";
import type { Ref, RepoStatus } from "./types";

const status: RepoStatus = {
  head: { sha: "a".repeat(40), branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};
const refs: Ref[] = [
  { name: "main", kind: "local", target_sha: "a".repeat(40), upstream: "origin/main", ahead: 2, behind: 3 },
  { name: "main", kind: "remote", target_sha: "b".repeat(40), upstream: null, ahead: 0, behind: 0 },
  { name: "lonely", kind: "local", target_sha: "c".repeat(40), upstream: null, ahead: 0, behind: 0 },
];

describe("pull labels", () => {
  it("names the strategy in the operation label, except for the default merge", () => {
    expect(pullLabel("merge")).toBe("Pull");
    expect(pullLabel("rebase")).toBe("Pull (rebase)");
    expect(pullLabel("ffOnly")).toBe("Pull (fast-forward only)");
  });

  it("lists every strategy with a radio label and a one-line description", () => {
    expect(PULL_STRATEGIES).toEqual(["merge", "rebase", "ffOnly"]);
    expect(PULL_STRATEGY_LABELS).toEqual({
      merge: "Merge",
      rebase: "Rebase",
      ffOnly: "Fast-forward only",
    });
    expect(PULL_STRATEGY_DESCRIPTIONS).toEqual({
      merge: "Fast-forward when possible, otherwise create a merge commit",
      rebase: "Replay your commits on top of the remote's",
      ffOnly: "Stop if the branches have diverged",
    });
  });

  it("explains the toolbar button only when the dialog is off", () => {
    expect(pullButtonTitle(true, "rebase")).toBe("Pull changes from remote");
    expect(pullButtonTitle(false, "merge")).toBe("Pull (merge) — ⌥-click for options");
    expect(pullButtonTitle(false, "ffOnly")).toBe(
      "Pull (fast-forward only) — ⌥-click for options",
    );
  });
});

describe("pull dialog copy", () => {
  it("titles the dialog with the upstream and the branch", () => {
    expect(pullDialogTitle("main", "origin/main")).toBe("Pull origin/main → main");
    expect(pullDialogTitle("main", null)).toBe("Pull");
    expect(pullDialogTitle(null, null)).toBe("Pull");
  });

  it("names only the non-zero counts", () => {
    expect(pullStatusLine("main", "origin/main", 2, 3)).toBe(
      "main is 2 ahead, 3 behind origin/main as of the last fetch.",
    );
    expect(pullStatusLine("main", "origin/main", 0, 3)).toBe(
      "main is 3 behind origin/main as of the last fetch.",
    );
    expect(pullStatusLine("main", "origin/main", 2, 0)).toBe(
      "main is 2 ahead of origin/main as of the last fetch.",
    );
    expect(pullStatusLine("main", "origin/main", 0, 0)).toBe(
      "Up to date with origin/main as of the last fetch.",
    );
  });

  it("explains why a pull is impossible", () => {
    expect(pullStatusLine("main", null, 0, 0)).toBe(
      "main has no upstream branch — push it first to set one.",
    );
    expect(pullStatusLine(null, null, 0, 0)).toBe("No branch is checked out.");
    expect(canPull("main", "origin/main")).toBe(true);
    expect(canPull("main", null)).toBe(false);
    expect(canPull(null, null)).toBe(false);
  });
});

describe("resolvePullStrategy (settings migration)", () => {
  it("keeps a saved strategy over the legacy flag", () => {
    expect(resolvePullStrategy("ffOnly", true)).toBe("ffOnly");
    expect(resolvePullStrategy("merge", "true")).toBe("merge");
  });

  it("promotes the legacy pullRebase flag when nothing newer is saved", () => {
    expect(resolvePullStrategy(null, true)).toBe("rebase"); // Tauri store boolean
    expect(resolvePullStrategy(undefined, "true")).toBe("rebase"); // localStorage string
  });

  it("defaults to merge", () => {
    expect(resolvePullStrategy(null, null)).toBe("merge");
    expect(resolvePullStrategy(undefined, false)).toBe("merge");
    expect(resolvePullStrategy("bogus", "false")).toBe("merge");
  });
});

describe("pullContext (what the dialog shows)", () => {
  it("takes the branch from the repository status and tracking data from the local ref", () => {
    expect(pullContext(status, refs)).toEqual({
      branch: "main",
      upstream: "origin/main",
      ahead: 2,
      behind: 3,
    });
    expect(pullContext({ ...status, head: { ...status.head, branch: "lonely" } }, refs)).toEqual({
      branch: "lonely",
      upstream: null,
      ahead: 0,
      behind: 0,
    });
  });

  it("reports no branch for a detached HEAD, an unknown status, or an empty inventory", () => {
    const detached = { ...status, head: { sha: "a".repeat(40), branch: null, detached: true } };
    expect(pullContext(detached, refs)).toEqual({ branch: null, upstream: null, ahead: 0, behind: 0 });
    expect(pullContext(null, refs)).toEqual({ branch: null, upstream: null, ahead: 0, behind: 0 });
    expect(pullContext(status, [])).toEqual({ branch: "main", upstream: null, ahead: 0, behind: 0 });
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/lib/pullOptions.test.ts`
Expected: FAIL — `Failed to resolve import "./pullOptions"` (module does not exist).

- [ ] **Step 3: Write the implementation**

`src/lib/types.ts` — append after the `RemoteOutcome` type:

```ts
// How `git pull` reconciles a diverged branch; mirrors git-core's PullStrategy (serde camelCase).
export type PullStrategy = "merge" | "rebase" | "ffOnly";
```

Create `src/lib/pullOptions.ts`:

```ts
// Pure copy + settings logic for the Pull dialog, the toolbar Pull button and the pull
// settings. Tauri-free so vitest covers every string the dialog can show.
import type { PullStrategy, Ref, RepoStatus } from "./types";

export const PULL_STRATEGIES: readonly PullStrategy[] = ["merge", "rebase", "ffOnly"];

// Radio labels in the dialog.
export const PULL_STRATEGY_LABELS: Record<PullStrategy, string> = {
  merge: "Merge",
  rebase: "Rebase",
  ffOnly: "Fast-forward only",
};

// One-line description under each radio.
export const PULL_STRATEGY_DESCRIPTIONS: Record<PullStrategy, string> = {
  merge: "Fast-forward when possible, otherwise create a merge commit",
  rebase: "Replay your commits on top of the remote's",
  ffOnly: "Stop if the branches have diverged",
};

// Lowercase form for running text ("Pull (fast-forward only)").
const PULL_STRATEGY_WORDS: Record<PullStrategy, string> = {
  merge: "merge",
  rebase: "rebase",
  ffOnly: "fast-forward only",
};

// Operation label for runRemote / the status bar: "Pull", "Pull (rebase)", …
export function pullLabel(strategy: PullStrategy): string {
  return strategy === "merge" ? "Pull" : `Pull (${PULL_STRATEGY_WORDS[strategy]})`;
}

// Toolbar tooltip. When the dialog is off, name the strategy a click will use and how
// to get the options back.
export function pullButtonTitle(askEveryTime: boolean, strategy: PullStrategy): string {
  return askEveryTime
    ? "Pull changes from remote"
    : `Pull (${PULL_STRATEGY_WORDS[strategy]}) — ⌥-click for options`;
}

// A pull needs a checked-out branch with an upstream to pull from.
export function canPull(branch: string | null, upstream: string | null): boolean {
  return branch !== null && upstream !== null;
}

export function pullDialogTitle(branch: string | null, upstream: string | null): string {
  return canPull(branch, upstream) ? `Pull ${upstream} → ${branch}` : "Pull";
}

// Status line under the title. Only non-zero counts are named; ahead/behind come from
// the tracking ref, i.e. the last fetch, so say so.
export function pullStatusLine(
  branch: string | null,
  upstream: string | null,
  ahead: number,
  behind: number,
): string {
  if (branch === null) return "No branch is checked out.";
  if (upstream === null) return `${branch} has no upstream branch — push it first to set one.`;
  if (ahead > 0 && behind > 0) {
    return `${branch} is ${ahead} ahead, ${behind} behind ${upstream} as of the last fetch.`;
  }
  if (ahead > 0) return `${branch} is ${ahead} ahead of ${upstream} as of the last fetch.`;
  if (behind > 0) return `${branch} is ${behind} behind ${upstream} as of the last fetch.`;
  return `Up to date with ${upstream} as of the last fetch.`;
}

// Saved-settings migration. `saved` is the persisted pullStrategy (a string, or
// null/undefined when never saved); `legacyRebase` is the pre-dialog `pullRebase` flag
// (a boolean in the Tauri store, the string "true"/"false" in localStorage). A saved
// strategy wins; otherwise a legacy rebase preference becomes "rebase"; otherwise merge.
export function resolvePullStrategy(saved: unknown, legacyRebase: unknown): PullStrategy {
  if (saved === "merge" || saved === "rebase" || saved === "ffOnly") return saved;
  return legacyRebase === true || legacyRebase === "true" ? "rebase" : "merge";
}

export type PullContext = {
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
};

// What the dialog shows for the checked-out branch. The branch comes from `git status`
// (the repository status), NOT from graph decorations: a branch whose commit sits outside
// the loaded page carries no `is_head` decoration and would wrongly read as "no branch".
// Tracking data comes from the `list_refs` inventory entry for that local branch.
export function pullContext(status: RepoStatus | null, refs: Ref[]): PullContext {
  const branch = status && !status.head.detached ? status.head.branch : null;
  const tracking = branch
    ? (refs.find((r) => r.kind === "local" && r.name === branch) ?? null)
    : null;
  return {
    branch,
    upstream: tracking?.upstream ?? null,
    ahead: tracking?.ahead ?? 0,
    behind: tracking?.behind ?? 0,
  };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/lib/pullOptions.test.ts`
Expected: PASS (11 tests).

Run: `npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib/types.ts src/lib/pullOptions.ts src/lib/pullOptions.test.ts
git commit -m "$(cat <<'EOF'
feat: add pure pull-dialog copy and the pull strategy migration helper

Labels, descriptions, the ahead/behind status line, the toolbar tooltip, the
dialog's branch/upstream context (from the repository status and the ref
inventory, never the loaded graph page) and the pullRebase → pullStrategy
migration live in a Tauri-free module so vitest covers every string the
dialog can show.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Pull settings replace `pullRebase`; settings UI; `api.pull` signature; `gitActions.pull` reads the settings

**Files:**
- Modify: `src/lib/api.ts` (`pull` + the `./types` import)
- Modify: `src/lib/store.svelte.ts` (constants, sync loaders, the "pullRebase persisted setting (Phase 6)" state block, the getter/setter block, two comments that name `pullRebase`)
- Modify: `src/lib/components/SettingsPanel.svelte` (Behavior tab + styles)
- Modify: `src/lib/gitActions.ts` (`gitActions.pull` + imports)
- Test: `src/lib/store.pullSettings.test.ts`, `src/lib/gitActions.pull.test.ts`

**Interfaces:**
- Consumes: `PullStrategy`, `pullLabel`, `resolvePullStrategy` (Task 2); `reportNotice` from `src/lib/errors.svelte.ts` (PR 1); `runRemote` (unchanged, in `gitActions.ts`).
- Produces:
  - `api.pull(repo: string, strategy: PullStrategy, autostash: boolean, onEvent: (line: string) => void, creds?: { username: string; password: string }): Promise<RemoteOutcome>`
  - `appState.pullStrategy: PullStrategy` / `appState.setPullStrategy(v)` — default `"merge"`
  - `appState.pullAutostash: boolean` / `appState.setPullAutostash(v)` — default `false`
  - `appState.pullAskEveryTime: boolean` / `appState.setPullAskEveryTime(v)` — default `true` (persisted here; its Settings checkbox and the flow that reads it both land in Task 6)
  - `appState.waitForPullSettingsHydration(): Promise<void>` — resolves once all three have been read from the Tauri store (immediately outside Tauri); mirrors `waitForUpdateSettingsHydration()`
  - `gitActions.pull(): Promise<boolean>` (no dialog yet; Task 6 adds `{ showOptions }`)
  - `appState.pullRebase` / `setPullRebase` are **gone** after this task.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/store.pullSettings.test.ts`:

```ts
// @vitest-environment jsdom
// The pull settings are read at module init (sync localStorage seed outside Tauri,
// async Tauri-store hydrate inside it), so every case re-imports a fresh store after
// seeding the storage it should read. jsdom supplies window + localStorage.
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  localStorage.clear();
  vi.resetModules();
});

describe("pull settings persistence", () => {
  it("defaults to merge, no autostash, ask every time", async () => {
    const { appState } = await import("./store.svelte");
    expect(appState.pullStrategy).toBe("merge");
    expect(appState.pullAutostash).toBe(false);
    expect(appState.pullAskEveryTime).toBe(true);
  });

  it("promotes a legacy pullRebase=true to rebase when no strategy was saved", async () => {
    localStorage.setItem("gitit.pullRebase.v1", "true");
    const { appState } = await import("./store.svelte");
    expect(appState.pullStrategy).toBe("rebase");
  });

  it("prefers a saved strategy over the legacy flag", async () => {
    localStorage.setItem("gitit.pullRebase.v1", "true");
    localStorage.setItem("gitit.pullStrategy.v1", "ffOnly");
    const { appState } = await import("./store.svelte");
    expect(appState.pullStrategy).toBe("ffOnly");
  });

  it("persists the three settings and leaves the legacy key in place", async () => {
    localStorage.setItem("gitit.pullRebase.v1", "true");
    const { appState } = await import("./store.svelte");
    appState.setPullStrategy("ffOnly");
    appState.setPullAutostash(true);
    appState.setPullAskEveryTime(false);
    expect(localStorage.getItem("gitit.pullStrategy.v1")).toBe("ffOnly");
    expect(localStorage.getItem("gitit.pullAutostash.v1")).toBe("true");
    expect(localStorage.getItem("gitit.pullAskEveryTime.v1")).toBe("false");
    expect(localStorage.getItem("gitit.pullRebase.v1")).toBe("true");
    expect(appState.pullStrategy).toBe("ffOnly");
    expect(appState.pullAutostash).toBe(true);
    expect(appState.pullAskEveryTime).toBe(false);
  });

  it("migrates a Tauri-store pullRebase=true when the desktop store has no pullStrategy", async () => {
    const values = new Map<string, unknown>([["pullRebase", true]]);
    vi.doMock("@tauri-apps/plugin-store", () => ({
      load: async () => ({
        get: async (key: string) => values.get(key),
        set: async (key: string, v: unknown) => {
          values.set(key, v);
        },
        save: async () => {},
      }),
    }));
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    try {
      const { appState } = await import("./store.svelte");
      await vi.waitFor(() => expect(appState.pullStrategy).toBe("rebase"));
      expect(values.has("pullStrategy")).toBe(false); // hydrating never writes
      expect(values.get("pullRebase")).toBe(true); // legacy key left in place
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
      vi.doUnmock("@tauri-apps/plugin-store");
    }
  });

  // A desktop store whose read completes only when the test says so, to expose the
  // window between launch and hydration.
  function gatedDesktopStore(values: Map<string, unknown>) {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    vi.doMock("@tauri-apps/plugin-store", () => ({
      load: () =>
        gate.then(() => ({
          get: async (key: string) => values.get(key),
          set: async () => {},
          save: async () => {},
        })),
    }));
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    return {
      release,
      cleanup() {
        Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
        vi.doUnmock("@tauri-apps/plugin-store");
      },
    };
  }

  it("exposes a barrier that resolves only once the desktop store has been read", async () => {
    const desktop = gatedDesktopStore(
      new Map<string, unknown>([
        ["pullStrategy", "ffOnly"],
        ["pullAutostash", true],
        ["pullAskEveryTime", false],
      ]),
    );
    try {
      const { appState } = await import("./store.svelte");
      expect(appState.pullStrategy).toBe("merge"); // startup default until the store is read
      let hydrated = false;
      const waited = appState.waitForPullSettingsHydration().then(() => {
        hydrated = true;
      });
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(hydrated).toBe(false);

      desktop.release();
      await waited;
      expect(appState.pullStrategy).toBe("ffOnly");
      expect(appState.pullAutostash).toBe(true);
      expect(appState.pullAskEveryTime).toBe(false);
    } finally {
      desktop.cleanup();
    }
  });

  it("keeps a choice made before hydration finished (touched guard)", async () => {
    const desktop = gatedDesktopStore(new Map<string, unknown>([["pullStrategy", "ffOnly"]]));
    try {
      const { appState } = await import("./store.svelte");
      appState.setPullStrategy("rebase");
      desktop.release();
      await appState.waitForPullSettingsHydration();
      expect(appState.pullStrategy).toBe("rebase");
    } finally {
      desktop.cleanup();
    }
  });
});
```

Create `src/lib/gitActions.pull.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { errors } from "./errors.svelte";
import { gitActions } from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";
import { appState } from "./store.svelte";
import type { Ref, RemoteOutcome, RepoStatus } from "./types";

// Once `window.__TAURI_INTERNALS__` is stubbed, the settings persist through the Tauri
// store plugin. There is no IPC under vitest, so give it an in-memory stand-in.
vi.mock("@tauri-apps/plugin-store", () => ({
  load: async () => ({
    get: async () => undefined,
    set: async () => {},
    save: async () => {},
  }),
}));

const REPO = "/tmp/git-it-pull-flow-test";
const pulled: RemoteOutcome = { ok: true, authFailed: false, conflicted: false, message: "" };
const repoStatus: RepoStatus = {
  head: { sha: SAMPLE_GRAPH[0].sha, branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};
// SAMPLE_GRAPH decorates its first commit with `main` as HEAD; this inventory entry
// gives that branch an upstream and ahead/behind counts.
const mainRef: Ref = {
  name: "main",
  kind: "local",
  target_sha: SAMPLE_GRAPH[0].sha,
  upstream: "origin/main",
  ahead: 2,
  behind: 3,
};

beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = REPO;
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH);
  appState.setRefsDetailed([mainRef]);
  appState.setPullStrategy("merge");
  appState.setPullAutostash(false);
  appState.setPullAskEveryTime(false);
  vi.spyOn(api, "pull").mockResolvedValue(pulled);
  // runRemote reloads the graph after the op.
  vi.spyOn(api, "loadGraph").mockResolvedValue(SAMPLE_GRAPH);
  vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
  vi.spyOn(api, "remotes").mockResolvedValue([{ name: "origin", url: "https://example.com/x.git" }]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
});

afterEach(() => {
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("gitActions.pull", () => {
  it("refuses with a notice while the repository is still loading", async () => {
    appState.setRepoLoading(true);

    const ok = await gitActions.pull();

    expect(ok).toBe(false);
    expect(api.pull).not.toHaveBeenCalled();
    expect(appState.status).toBe("Repository is still loading — try again in a moment.");
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull yet",
      headline: "Repository is still loading — try again in a moment.",
    });
  });

  it("pulls with the saved strategy and autostash when asking is off", async () => {
    appState.setPullStrategy("ffOnly");
    appState.setPullAutostash(true);

    const ok = await gitActions.pull();

    expect(ok).toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "ffOnly", true, expect.any(Function), undefined);
    expect(appState.status).toBe("Pull (fast-forward only) — done.");
  });
});
```

(This file deliberately does not touch `dialogs` yet — the `pull` dialog kind only exists after Task 4, which adds the import and an `afterEach` cleanup line; Task 6 adds the dialog-driven cases.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/store.pullSettings.test.ts src/lib/gitActions.pull.test.ts`
Expected: FAIL — the store tests report `expected undefined to be 'merge'` (no `pullStrategy` getter), and `appState.setPullStrategy is not a function` in the gitActions suite's `beforeEach`.

- [ ] **Step 3: `api.pull` takes strategy + autostash**

`src/lib/api.ts` — add `PullStrategy` to the `import type { … } from "./types"` list (after `PrerequisiteCheck,`):

```ts
  PrerequisiteCheck,
  PullStrategy,
```

Replace the `pull` member. Current code:

```ts
  pull: (
    repo: string,
    rebase: boolean,
    onEvent: (line: string) => void,
    creds?: { username: string; password: string },
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onEvent;
    return invoke<RemoteOutcome>("pull", {
      repo,
      // Bridge until the pull settings land (Task 3): the command takes a strategy now.
      strategy: rebase ? "rebase" : "merge",
      autostash: false,
      username: creds?.username ?? null,
      password: creds?.password ?? null,
      onEvent: channel,
    });
  },
```

New code:

```ts
  pull: (
    repo: string,
    strategy: PullStrategy,
    autostash: boolean,
    onEvent: (line: string) => void,
    creds?: { username: string; password: string },
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onEvent;
    return invoke<RemoteOutcome>("pull", {
      repo,
      strategy,
      autostash,
      username: creds?.username ?? null,
      password: creds?.password ?? null,
      onEvent: channel,
    });
  },
```

- [ ] **Step 4: Store — three settings replace `pullRebase`**

`src/lib/store.svelte.ts`:

(a) Add `PullStrategy` to the first `import type { … } from "./types"` line (it currently lists `Commit, GraphCommit, Ref, RefEntry, RemoteInfo, RepoStatus, UndoSnapshot, WorkingFile, WorktreeInfo`) so it includes `PullStrategy`, and add this import after the `import { reorder } from "./reorder";` line:

```ts
import { resolvePullStrategy } from "./pullOptions";
```

(b) Replace the constants:

```ts
const PULLREBASE_KEY = "gitit.pullRebase.v1";
const PULLREBASE_STORE_KEY = "pullRebase";
```

with:

```ts
// Legacy pre-dialog flag: read once to migrate it into pullStrategy, never written.
const PULLREBASE_KEY = "gitit.pullRebase.v1";
const PULLREBASE_STORE_KEY = "pullRebase";
const PULLSTRATEGY_KEY = "gitit.pullStrategy.v1";
const PULLSTRATEGY_STORE_KEY = "pullStrategy";
const PULLAUTOSTASH_KEY = "gitit.pullAutostash.v1";
const PULLAUTOSTASH_STORE_KEY = "pullAutostash";
const PULLASK_KEY = "gitit.pullAskEveryTime.v1";
const PULLASK_STORE_KEY = "pullAskEveryTime";
```

(c) Replace the sync loader:

```ts
function loadSyncPullRebase(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    const raw = localStorage.getItem(PULLREBASE_KEY);
    return raw === "true";
  } catch {
    return false;
  }
}
```

with:

```ts
function loadSyncPullStrategy(): PullStrategy {
  if (isTauri()) return "merge";
  try {
    if (typeof localStorage === "undefined") return "merge";
    return resolvePullStrategy(
      localStorage.getItem(PULLSTRATEGY_KEY),
      localStorage.getItem(PULLREBASE_KEY),
    );
  } catch {
    return "merge";
  }
}

function loadSyncPullAutostash(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    return localStorage.getItem(PULLAUTOSTASH_KEY) === "true";
  } catch {
    return false;
  }
}

function loadSyncPullAskEveryTime(): boolean {
  if (isTauri()) return true;
  try {
    if (typeof localStorage === "undefined") return true;
    const raw = localStorage.getItem(PULLASK_KEY);
    return raw === null ? true : raw !== "false";
  } catch {
    return true;
  }
}
```

(d) Replace the state block. Current code:

```ts
  // ── pullRebase persisted setting (Phase 6) ────────────────────────────────
  // Whether `git pull` should use --rebase (true) or --no-edit merge (false, default).
  // Mirrors the diffSplit pattern exactly.
  let pullRebase = $state<boolean>(loadSyncPullRebase());
  let pullRebaseTouched = false;

  const prHydrate = getStore();
  if (prHydrate) {
    prHydrate
      .then((store) => store.get<boolean>(PULLREBASE_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !pullRebaseTouched) {
          pullRebase = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load pullRebase setting", e));
  }

  function persistPullRebase() {
    const snapshot = pullRebase;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(PULLREBASE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist pullRebase setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(PULLREBASE_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist pullRebase setting", e);
    }
  }
```

New code:

```ts
  // ── Pull settings: pullStrategy / pullAutostash / pullAskEveryTime ────────
  // How `git pull` reconciles a diverged branch, whether it stashes and reapplies
  // local changes, and whether the Pull dialog opens before every pull. Each mirrors
  // the diffSplit pattern exactly. pullStrategy also migrates the pre-dialog
  // `pullRebase` flag (read here, never written again) via resolvePullStrategy.
  let pullStrategy = $state<PullStrategy>(loadSyncPullStrategy());
  let pullStrategyTouched = false;

  // Each hydrate is a Promise<void> (resolved immediately outside Tauri) so the pull
  // flow can wait for all three — mirrors updateSettingsHydration below.
  const psHydrate = getStore();
  const pullStrategyHydration: Promise<void> = psHydrate
    ? psHydrate
      .then(async (store) => ({
        saved: await store.get<string>(PULLSTRATEGY_STORE_KEY),
        legacyRebase: await store.get<boolean>(PULLREBASE_STORE_KEY),
      }))
      .then(({ saved, legacyRebase }) => {
        if (!pullStrategyTouched) pullStrategy = resolvePullStrategy(saved, legacyRebase);
      })
      .catch((e) => console.warn("[gte] could not load pullStrategy setting", e))
    : Promise.resolve();

  function persistPullStrategy() {
    const snapshot = pullStrategy;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(PULLSTRATEGY_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist pullStrategy setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(PULLSTRATEGY_KEY, snapshot);
    } catch (e) {
      console.warn("[gte] could not persist pullStrategy setting", e);
    }
  }

  let pullAutostash = $state<boolean>(loadSyncPullAutostash());
  let pullAutostashTouched = false;

  const paHydrate = getStore();
  const pullAutostashHydration: Promise<void> = paHydrate
    ? paHydrate
      .then((store) => store.get<boolean>(PULLAUTOSTASH_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !pullAutostashTouched) {
          pullAutostash = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load pullAutostash setting", e))
    : Promise.resolve();

  function persistPullAutostash() {
    const snapshot = pullAutostash;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(PULLAUTOSTASH_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist pullAutostash setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(PULLAUTOSTASH_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist pullAutostash setting", e);
    }
  }

  let pullAskEveryTime = $state<boolean>(loadSyncPullAskEveryTime());
  let pullAskEveryTimeTouched = false;

  const pkHydrate = getStore();
  const pullAskEveryTimeHydration: Promise<void> = pkHydrate
    ? pkHydrate
      .then((store) => store.get<boolean>(PULLASK_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !pullAskEveryTimeTouched) {
          pullAskEveryTime = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load pullAskEveryTime setting", e))
    : Promise.resolve();

  const pullSettingsHydration = Promise.all([
    pullStrategyHydration,
    pullAutostashHydration,
    pullAskEveryTimeHydration,
  ]).then(() => undefined);

  function persistPullAskEveryTime() {
    const snapshot = pullAskEveryTime;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(PULLASK_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist pullAskEveryTime setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(PULLASK_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist pullAskEveryTime setting", e);
    }
  }
```

(e) Two neighbouring comments name the removed block; they are the only orphans of this edit. In the "update settings (updater)" comment change

```ts
  // Both mirror the pullRebase persist pattern exactly.
```

to

```ts
  // Both mirror the diffSplit persist pattern exactly.
```

and in the PR-timeline-order comment change

```ts
  // default) or newest-first (true). Mirrors the pullRebase pattern exactly.
```

to

```ts
  // default) or newest-first (true). Mirrors the diffSplit pattern exactly.
```

(f) Replace the getter/setter block in the returned object. Current code:

```ts
    // ── pullRebase persisted setting (Phase 6) ────────────────────────────────
    get pullRebase() {
      return pullRebase;
    },
    setPullRebase(v: boolean) {
      pullRebaseTouched = true;
      pullRebase = v;
      persistPullRebase();
    },
```

New code:

```ts
    // ── Pull settings ─────────────────────────────────────────────────────────
    get pullStrategy() {
      return pullStrategy;
    },
    setPullStrategy(v: PullStrategy) {
      pullStrategyTouched = true;
      pullStrategy = v;
      persistPullStrategy();
    },
    get pullAutostash() {
      return pullAutostash;
    },
    setPullAutostash(v: boolean) {
      pullAutostashTouched = true;
      pullAutostash = v;
      persistPullAutostash();
    },
    get pullAskEveryTime() {
      return pullAskEveryTime;
    },
    setPullAskEveryTime(v: boolean) {
      pullAskEveryTimeTouched = true;
      pullAskEveryTime = v;
      persistPullAskEveryTime();
    },
    /** A pull must read the *persisted* preferences. Before this barrier resolves the
     * three values above are startup defaults, and confirming the dialog would persist
     * those defaults over the saved ones (the touched guards then keep hydration from
     * correcting them). Resolves immediately outside Tauri. */
    waitForPullSettingsHydration(): Promise<void> {
      return pullSettingsHydration;
    },
```

- [ ] **Step 5: Settings panel — strategy radios + the autostash checkbox**

`src/lib/components/SettingsPanel.svelte`, Behavior tab. Replace:

```svelte
          <label class="opt">
            <input
              type="checkbox"
              checked={appState.pullRebase}
              onchange={() => appState.setPullRebase(!appState.pullRebase)}
            />
            <span>Pull with rebase (instead of merge)</span>
          </label>
```

with:

```svelte
          <fieldset class="radio-group">
            <legend>Pull strategy</legend>
            <label class="opt">
              <input
                type="radio"
                name="settings-pull-strategy"
                checked={appState.pullStrategy === "merge"}
                onchange={() => appState.setPullStrategy("merge")}
              />
              <span>Merge (fast-forward when possible)</span>
            </label>
            <label class="opt">
              <input
                type="radio"
                name="settings-pull-strategy"
                checked={appState.pullStrategy === "rebase"}
                onchange={() => appState.setPullStrategy("rebase")}
              />
              <span>Rebase</span>
            </label>
            <label class="opt">
              <input
                type="radio"
                name="settings-pull-strategy"
                checked={appState.pullStrategy === "ffOnly"}
                onchange={() => appState.setPullStrategy("ffOnly")}
              />
              <span>Fast-forward only</span>
            </label>
          </fieldset>
          <label class="opt">
            <input
              type="checkbox"
              checked={appState.pullAutostash}
              onchange={() => appState.setPullAutostash(!appState.pullAutostash)}
            />
            <span>Stash &amp; reapply local changes when pulling</span>
          </label>
```

Both controls are live in this commit (`gitActions.pull` below reads them). The "Ask before every pull" checkbox is NOT added here — nothing honours `pullAskEveryTime` until Task 6, and a visible control that does nothing is a fake control. Task 6 adds it together with the flow.

In the `<style>` block, after the `.opt input:focus-visible { … }` rule, add:

```css
  /* Exclusive-choice rows: a fieldset so the caption is announced with its radios,
     with the default fieldset chrome reset so it reads like the .opt rows around it. */
  .radio-group {
    margin: 0;
    padding: 0;
    border: 0;
    min-width: 0;
  }

  .radio-group legend {
    padding: 6px 4px 2px;
    font-size: 12.5px;
    color: var(--text-muted);
  }
```

(The radio `name` is `settings-pull-strategy`, distinct from the dialog's `pull-dialog-strategy` in Task 5: same-named radios anywhere in the document form one group.)

- [ ] **Step 6: `gitActions.pull` reads the settings and reports its guard as a notice**

`src/lib/gitActions.ts` — add, next to the existing `./dialogs.svelte` import:

```ts
import { pullLabel } from "./pullOptions";
```

and extend PR 1's `./errors.svelte` import so it also brings in `reportNotice` (PR 1 already imports `reportError` there; the line should read `import { reportError, reportNotice } from "./errors.svelte";` plus whatever else PR 1 imports from it).

Replace the `pull` member. Current code:

```ts
  pull: () => {
    if (appState.repoLoading) {
      appState.status = "Repository is still loading — try again in a moment.";
      return Promise.resolve(false);
    }
    const label = appState.pullRebase ? "Pull (rebase)" : "Pull";
    return runRemote(label, (onLine, creds) =>
      api.pull(appState.repo, appState.pullRebase, onLine, creds),
    );
  },
```

New code:

```ts
  pull: () => {
    if (appState.repoLoading) {
      reportNotice("Can't pull yet", "Repository is still loading — try again in a moment.");
      return Promise.resolve(false);
    }
    // Read once, up front, so a settings change mid-pull cannot split the label from
    // the flags actually sent.
    const strategy = appState.pullStrategy;
    const autostash = appState.pullAutostash;
    return runRemote(pullLabel(strategy), (onLine, creds) =>
      api.pull(appState.repo, strategy, autostash, onLine, creds),
    );
  },
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `npx vitest run src/lib/store.pullSettings.test.ts src/lib/gitActions.pull.test.ts`
Expected: PASS (7 store tests, 2 gitActions tests).

Run: `npm run check && npm test`
Expected: 0 errors, 0 warnings; every suite green.

Run: `grep -rn "appState.pullRebase\|setPullRebase(" src`
Expected: no output — the removed getter/setter has no callers left. (A plain `pullRebase` grep still matches the migration constants, comments and tests, which is intended.)

- [ ] **Step 8: Commit**

```bash
git add src/lib/api.ts src/lib/store.svelte.ts src/lib/store.pullSettings.test.ts src/lib/components/SettingsPanel.svelte src/lib/gitActions.ts src/lib/gitActions.pull.test.ts
git commit -m "$(cat <<'EOF'
feat: replace the pull-rebase setting with strategy, autostash and ask-every-time

pullStrategy (merge / rebase / ffOnly), pullAutostash and pullAskEveryTime
persist through the same Tauri-store + localStorage pattern as pullRebase
did, with a waitForPullSettingsHydration() barrier so a pull can read the
persisted values rather than startup defaults. A saved pullRebase=true
migrates to "rebase" when no strategy has been saved; the legacy key is left
in place. Settings → Behavior shows a strategy radio group and the autostash
checkbox (the ask-before-pull checkbox arrives with the dialog flow that
reads it). api.pull sends the strategy and autostash flags, and
gitActions.pull refuses a still-loading repository with a notice instead of
a status-bar line.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: The `pull` dialog kind in `dialogs.svelte.ts`

**Files:**
- Modify: `src/lib/dialogs.svelte.ts` (imports, `DialogState` union, `settlePending`, new members after `resolveCreatePr`)
- Modify: `src/lib/gitActions.pull.test.ts` (add the `dialogs` import and an `afterEach` cleanup line for the dialog kind)
- Test: `src/lib/dialogs.pull.test.ts`

**Interfaces:**
- Consumes: `PullStrategy` (Task 2), `canPull` (Task 2).
- Produces:
  - `export type PullDialogResult = { strategy: PullStrategy; autostash: boolean; dontAsk: boolean }`
  - `DialogState` member `{ kind: "pull"; branch: string | null; upstream: string | null; ahead: number; behind: number; strategy: PullStrategy; autostash: boolean; dontAsk: boolean; resolve: (v: PullDialogResult | null) => void }`
  - `dialogs.confirmPull(opts: { branch: string | null; upstream: string | null; ahead: number; behind: number; strategy: PullStrategy; autostash: boolean }): Promise<PullDialogResult | null>`
  - `dialogs.setPullStrategy(v: PullStrategy)`, `dialogs.setPullAutostash(v: boolean)`, `dialogs.setPullDontAsk(v: boolean)` — each REPLACES state, keeping `resolve`
  - `dialogs.resolvePull(accepted: boolean)` — resolves `null` on cancel; refuses (stays open) when `accepted` and `!canPull(branch, upstream)`
  - `settlePending()` resolves a pending pull dialog to `null`.

- [ ] **Step 1: Write the failing test**

Create `src/lib/dialogs.pull.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { dialogs } from "./dialogs.svelte";

function openPull(upstream: string | null) {
  return dialogs.confirmPull({
    branch: "main",
    upstream,
    ahead: 2,
    behind: 3,
    strategy: "merge",
    autostash: false,
  });
}

describe("pull options dialog", () => {
  it("opens preselected from the saved defaults and resolves the edited choice", async () => {
    const pending = openPull("origin/main");

    expect(dialogs.state.kind).toBe("pull");
    if (dialogs.state.kind !== "pull") throw new Error("pull dialog did not open");
    expect(dialogs.state.branch).toBe("main");
    expect(dialogs.state.upstream).toBe("origin/main");
    expect(dialogs.state.ahead).toBe(2);
    expect(dialogs.state.behind).toBe(3);
    expect(dialogs.state.strategy).toBe("merge");
    expect(dialogs.state.autostash).toBe(false);
    expect(dialogs.state.dontAsk).toBe(false);

    dialogs.setPullStrategy("rebase");
    dialogs.setPullAutostash(true);
    dialogs.setPullDontAsk(true);
    dialogs.resolvePull(true);

    await expect(pending).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: true });
    expect(dialogs.state.kind).toBe("none");
  });

  it("resolves null on cancel", async () => {
    const pending = openPull("origin/main");
    dialogs.setPullStrategy("ffOnly");
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
    expect(dialogs.state.kind).toBe("none");
  });

  it("refuses to pull when the branch has no upstream", async () => {
    const pending = openPull(null);
    dialogs.resolvePull(true);
    expect(dialogs.state.kind).toBe("pull"); // still open — nothing to pull from
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
  });

  it("refuses to pull with a detached HEAD", async () => {
    const pending = dialogs.confirmPull({
      branch: null,
      upstream: null,
      ahead: 0,
      behind: 0,
      strategy: "merge",
      autostash: false,
    });
    dialogs.resolvePull(true);
    expect(dialogs.state.kind).toBe("pull");
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
  });

  it("is settled as cancelled when another dialog replaces it", async () => {
    const pending = openPull("origin/main");
    const other = dialogs.confirm({ title: "Other", message: "Replaces the pull dialog" });
    await expect(pending).resolves.toBeNull();
    dialogs.resolveConfirm(false);
    await other;
  });
});
```

Also prepare `src/lib/gitActions.pull.test.ts` (from Task 3) for the dialog-driven cases Task 6 adds: insert `import { dialogs } from "./dialogs.svelte";` after the `./api` import, and make its `afterEach` settle a dialog a failing test may leave open, so it reads:

```ts
afterEach(() => {
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/lib/dialogs.pull.test.ts`
Expected: FAIL — `dialogs.confirmPull is not a function`.

- [ ] **Step 3: Implement the dialog kind**

`src/lib/dialogs.svelte.ts`:

(a) Imports — change

```ts
import type { WorktreeInfo } from "./types";
import { worktreeRemovalBlocker } from "./worktrees";
```

to

```ts
import type { PullStrategy, WorktreeInfo } from "./types";
import { worktreeRemovalBlocker } from "./worktrees";
import { canPull } from "./pullOptions";
```

(b) After the `export type NewRepositoryResult = { … };` block add:

```ts
export type PullDialogResult = {
  strategy: PullStrategy;
  autostash: boolean;
  dontAsk: boolean;
};
```

(c) Add a member to the `DialogState` union. The union currently ends with the `createPr` member's closing `}` followed by `;` — insert the following between that `}` and the `;`:

```ts
  | {
      kind: "pull";
      branch: string | null; // null when HEAD is detached
      upstream: string | null; // e.g. "origin/main"; null when the branch has no upstream
      ahead: number;
      behind: number;
      strategy: PullStrategy;
      autostash: boolean;
      dontAsk: boolean;
      resolve: (v: PullDialogResult | null) => void;
    };
```

(d) In `settlePending()`, after the line `else if (state.kind === "createPr") state.resolve(null);` add:

```ts
    else if (state.kind === "pull") state.resolve(null);
```

(e) After the `resolveCreatePr(accepted: boolean) { … }` member (the last one in the returned object) add:

```ts
    // ── Pull options dialog ────────────────────────────────────────────────────
    // Strategy + autostash preselected from the saved settings; resolves the edited
    // choice (plus "don't ask again"), or null on cancel / replacement.
    confirmPull(opts: {
      branch: string | null;
      upstream: string | null;
      ahead: number;
      behind: number;
      strategy: PullStrategy;
      autostash: boolean;
    }): Promise<PullDialogResult | null> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "pull",
          branch: opts.branch,
          upstream: opts.upstream,
          ahead: opts.ahead,
          behind: opts.behind,
          strategy: opts.strategy,
          autostash: opts.autostash,
          dontAsk: false,
          resolve,
        };
      });
    },
    setPullStrategy(v: PullStrategy) { if (state.kind === "pull") state = { ...state, strategy: v }; },
    setPullAutostash(v: boolean) { if (state.kind === "pull") state = { ...state, autostash: v }; },
    setPullDontAsk(v: boolean) { if (state.kind === "pull") state = { ...state, dontAsk: v }; },
    resolvePull(accepted: boolean) {
      if (state.kind === "pull") {
        const { branch, upstream, strategy, autostash, dontAsk } = state;
        // Defense in depth: Enter or a future caller cannot pull with nowhere to pull from.
        if (accepted && !canPull(branch, upstream)) return;
        state.resolve(accepted ? { strategy, autostash, dontAsk } : null);
        state = { kind: "none" };
      }
    },
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/lib/dialogs.pull.test.ts src/lib/gitActions.pull.test.ts`
Expected: PASS (5 + 2 tests).

Run: `npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib/dialogs.svelte.ts src/lib/dialogs.pull.test.ts src/lib/gitActions.pull.test.ts
git commit -m "$(cat <<'EOF'
feat: add the pull options dialog state

A `pull` dialog kind carries the branch, upstream, ahead/behind, the
preselected strategy and autostash, and a "don't ask again" flag. It
resolves the edited choice or null, refuses to pull with no upstream or a
detached HEAD, and is settled as cancelled when another dialog replaces it.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Render the pull dialog in `Modal.svelte`

**Files:**
- Modify: `src/lib/components/Modal.svelte` (imports, element refs, focus `$effect`, keydown handler, a new `{:else if}` block before `credentials`, styles)
- Test: `src/lib/components/Modal.pull.test.ts`

**Interfaces:**
- Consumes: `dialogs.confirmPull` / `setPullStrategy` / `setPullAutostash` / `setPullDontAsk` / `resolvePull` (Task 4); `PULL_STRATEGIES`, `PULL_STRATEGY_LABELS`, `PULL_STRATEGY_DESCRIPTIONS`, `canPull`, `pullDialogTitle`, `pullStatusLine` (Task 2).
- Produces: DOM hooks used by the test and by nothing else — `.dialog.pull`, `h3`, `p.pull-status` (`.blocked` when Pull is impossible), `.strategy-opt input[type=radio]` (values `merge` / `rebase` / `ffOnly`, name `pull-dialog-strategy`), `.pull-autostash input`, `.pull-dont-ask input`, `.actions button` (Cancel first, `button.primary` = Pull, disabled when `!canPull`). Enter → Pull, Escape → Cancel, backdrop click → Cancel. Focus lands on Pull when enabled, otherwise on Cancel.

- [ ] **Step 1: Write the failing test**

Create `src/lib/components/Modal.pull.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { mount, tick, unmount } from "svelte";
import { dialogs } from "../dialogs.svelte";
import Modal from "./Modal.svelte";

afterEach(() => {
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
  document.body.replaceChildren();
});

function mountModal() {
  const target = document.createElement("div");
  document.body.appendChild(target);
  return { target, modal: mount(Modal, { target }) };
}

function openPull(upstream: string | null) {
  return dialogs.confirmPull({
    branch: "main",
    upstream,
    ahead: 2,
    behind: 3,
    strategy: "rebase",
    autostash: true,
  });
}

function keydown(el: Element | null | undefined, key: string) {
  el?.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
}

describe("pull dialog", () => {
  it("preselects the saved strategy and autostash, then resolves the edited choice", async () => {
    const { target, modal } = mountModal();
    const pending = openPull("origin/main");
    await tick();

    expect(target.querySelector("h3")?.textContent?.trim()).toBe("Pull origin/main → main");
    expect(target.querySelector(".pull-status")?.textContent?.trim()).toBe(
      "main is 2 ahead, 3 behind origin/main as of the last fetch.",
    );
    const radios = [...target.querySelectorAll<HTMLInputElement>(".strategy-opt input")];
    expect(radios.map((r) => r.value)).toEqual(["merge", "rebase", "ffOnly"]);
    expect(radios.map((r) => r.checked)).toEqual([false, true, false]);
    expect(target.querySelector<HTMLInputElement>(".pull-autostash input")?.checked).toBe(true);
    expect(target.querySelector<HTMLInputElement>(".pull-dont-ask input")?.checked).toBe(false);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(false);
    expect(document.activeElement).toBe(pull);

    radios[2].click();
    target.querySelector<HTMLInputElement>(".pull-dont-ask input")?.click();
    await tick();
    expect(radios.map((r) => r.checked)).toEqual([false, false, true]);
    pull?.click();

    await expect(pending).resolves.toEqual({ strategy: "ffOnly", autostash: true, dontAsk: true });
    await unmount(modal);
  });

  it("pulls on Enter and cancels on Escape", async () => {
    const { target, modal } = mountModal();
    const first = openPull("origin/main");
    await tick();
    keydown(target.querySelector(".actions button.primary"), "Enter");
    await expect(first).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: false });

    const second = openPull("origin/main");
    await tick();
    keydown(target.querySelector(".actions button.primary"), "Escape");
    await expect(second).resolves.toBeNull();
    await unmount(modal);
  });

  it("disables Pull and explains when the branch has no upstream", async () => {
    const { target, modal } = mountModal();
    const pending = openPull(null);
    await tick();

    expect(target.querySelector("h3")?.textContent?.trim()).toBe("Pull");
    const status = target.querySelector(".pull-status");
    expect(status?.textContent?.trim()).toBe(
      "main has no upstream branch — push it first to set one.",
    );
    expect(status?.classList.contains("blocked")).toBe(true);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(true);
    const cancel = target.querySelector<HTMLButtonElement>(".actions button");
    expect(document.activeElement).toBe(cancel);

    keydown(cancel, "Enter");
    expect(dialogs.state.kind).toBe("pull"); // Enter cannot pull either
    keydown(cancel, "Escape");
    await expect(pending).resolves.toBeNull();
    await unmount(modal);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/lib/components/Modal.pull.test.ts`
Expected: FAIL — `expected undefined to be 'Pull origin/main → main'` (no markup renders for the `pull` kind).

- [ ] **Step 3: Render the dialog**

`src/lib/components/Modal.svelte`:

(a) Script imports — after `import WorktreeIcon from "./WorktreeIcon.svelte";` add:

```ts
  import {
    PULL_STRATEGIES,
    PULL_STRATEGY_DESCRIPTIONS,
    PULL_STRATEGY_LABELS,
    canPull,
    pullDialogTitle,
    pullStatusLine,
  } from "../pullOptions";
```

(b) Element refs — after the line `let branchDeleteCancelEl = $state<HTMLButtonElement | undefined>();` add:

```ts
  let pullPrimaryEl = $state<HTMLButtonElement | undefined>();
  let pullCancelEl = $state<HTMLButtonElement | undefined>();
```

(c) Focus `$effect` — after the branch

```ts
    } else if (s.kind === "branchDelete") {
      branchDeleteCancelEl?.focus();
    }
```

add one more branch so it reads:

```ts
    } else if (s.kind === "branchDelete") {
      branchDeleteCancelEl?.focus();
    } else if (s.kind === "pull") {
      // Enter = Pull, so the primary takes focus — unless there is nothing to pull from.
      (canPull(s.branch, s.upstream) ? pullPrimaryEl : pullCancelEl)?.focus();
    }
```

(d) Keydown handler — after `handleCredentialsKey` add:

```ts
  // Pull dialog: Enter pulls (the resolver refuses when there is nothing to pull from),
  // Escape cancels. Enter never toggles a radio or checkbox, so the two can't collide.
  function handlePullKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      dialogs.resolvePull(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      dialogs.resolvePull(false);
    }
  }
```

(e) Markup — insert this block immediately before `{:else if dialogs.state.kind === "credentials"}`:

```svelte
{:else if dialogs.state.kind === "pull"}
  {@const allowed = canPull(dialogs.state.branch, dialogs.state.upstream)}
  {@const title = pullDialogTitle(dialogs.state.branch, dialogs.state.upstream)}
  {@const statusLine = pullStatusLine(
    dialogs.state.branch,
    dialogs.state.upstream,
    dialogs.state.ahead,
    dialogs.state.behind,
  )}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) dialogs.resolvePull(false);
    }}
    onkeydown={handlePullKey}
  >
    <div class="dialog pull" role="dialog" aria-modal="true" aria-label={title}>
      <h3>{title}</h3>
      <p class="msg pull-status" class:blocked={!allowed}>{statusLine}</p>
      <div class="strategy-list" role="radiogroup" aria-label="Pull strategy">
        {#each PULL_STRATEGIES as s (s)}
          <label class="strategy-opt">
            <input
              type="radio"
              name="pull-dialog-strategy"
              value={s}
              checked={dialogs.state.strategy === s}
              onchange={() => dialogs.setPullStrategy(s)}
            />
            <span class="strategy-copy">
              <span class="strategy-name">{PULL_STRATEGY_LABELS[s]}</span>
              <span class="strategy-desc">{PULL_STRATEGY_DESCRIPTIONS[s]}</span>
            </span>
          </label>
        {/each}
      </div>
      <label class="backup-row pull-autostash">
        <input
          type="checkbox"
          checked={dialogs.state.autostash}
          onchange={(e) => dialogs.setPullAutostash((e.currentTarget as HTMLInputElement).checked)}
        />
        Stash &amp; reapply local changes
      </label>
      <label class="backup-row pull-dont-ask">
        <input
          type="checkbox"
          checked={dialogs.state.dontAsk}
          onchange={(e) => dialogs.setPullDontAsk((e.currentTarget as HTMLInputElement).checked)}
        />
        <span>Don't ask again <span class="dont-ask-note">Change this in Settings</span></span>
      </label>
      <div class="actions">
        <button bind:this={pullCancelEl} type="button" onclick={() => dialogs.resolvePull(false)}>Cancel</button>
        <button
          bind:this={pullPrimaryEl}
          type="button"
          class="primary"
          disabled={!allowed}
          onclick={() => dialogs.resolvePull(true)}
        >
          Pull
        </button>
      </div>
    </div>
  </div>
```

(f) Styles — append before `</style>`. Everything uses the existing tokens (`--accent`, `--border`, `--input-bg`, `--text-muted`, `--radius-md`) so NERV's global `.dialog` restyle (corner brackets, `h3::before`) applies unchanged; the selected-card tint mirrors `.worktree-context`.

```css
  /* Pull dialog: three strategy cards need more room than the 360px default. */
  .dialog.pull {
    width: 440px;
  }
  .pull-status {
    color: var(--text-muted);
  }
  /* When there is nothing to pull from, the status line IS the message. */
  .pull-status.blocked {
    color: var(--text);
  }
  .strategy-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }
  .strategy-opt {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    cursor: pointer;
    user-select: none;
  }
  .strategy-opt:has(input:checked) {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 7%, var(--input-bg));
  }
  .strategy-opt input[type="radio"] {
    width: auto;
    margin: 2px 0 0;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
  .strategy-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
  }
  .strategy-desc {
    font-size: 11.5px;
    color: var(--text-muted);
    line-height: 1.4;
  }
  .dont-ask-note {
    margin-left: 6px;
    font-size: 11px;
    color: var(--text-muted);
  }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/lib/components/Modal.pull.test.ts`
Expected: PASS (3 tests).

Run: `npm run check && npm test`
Expected: 0 errors, 0 warnings (no a11y warnings from the new block: the radios sit in a `role="radiogroup"` with an `aria-label`, every input is wrapped in a `<label>`), all suites green.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Modal.svelte src/lib/components/Modal.pull.test.ts
git commit -m "$(cat <<'EOF'
feat: render the pull dialog

"Pull origin/main → main" with the ahead/behind status line, three strategy
cards with one-line descriptions, "Stash & reapply local changes", "Don't
ask again" (change it in Settings), and Cancel / Pull. Enter pulls, Escape
cancels; with no upstream or a detached HEAD the dialog explains why and
Pull is disabled. Existing dialog tokens only, so NERV's .dialog restyle
applies as is.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Ask before pulling; ⌥-click and the toolbar tooltip

**Files:**
- Modify: `src/lib/gitActions.ts` (`gitActions.pull`, a `pullStillLoading` helper next to `sameRepoAfterPrompt`, one import line)
- Modify: `src/lib/components/SettingsPanel.svelte` (the "Ask before every pull" checkbox — added here, in the commit whose flow honours it)
- Modify: `src/routes/+page.svelte` (Pull button + one import)
- Modify: `src/lib/gitActions.errorReporting.test.ts` (PR 1's suite — its seven `runRemote` cases call `gitActions.pull()` and would otherwise wait on the dialog forever)
- Test: `src/lib/gitActions.pull.test.ts` (extend)

**Interfaces:**
- Consumes: `dialogs.confirmPull` (Task 4); `appState.pullStrategy` / `pullAutostash` / `pullAskEveryTime` + setters and `appState.waitForPullSettingsHydration()` (Task 3); `appState.repoStatus`, `appState.refsDetailed` (already `[]` unless `refsDetailedRepo === repo`), `appState.repoLoading` (store); `sameRepoAfterPrompt()` and `runRemote` (module-scope in `gitActions.ts`); `pullContext`, `pullLabel`, `pullButtonTitle` (Task 2); `reportNotice` (PR 1).
- Produces: `gitActions.pull(opts?: { showOptions?: boolean }): Promise<boolean>` — spec C4 plus two bindings: preferences are read only after the store has hydrated, and a confirmed pull runs only when the repository identity, its loading state and the `{ branch, upstream }` the dialog showed are all unchanged (checked in that order, before any preference write or `api.pull` call). Notice copy: `("Can't pull yet", "Repository is still loading — try again in a moment.")` and `("Can't pull", "The checked-out branch changed while that dialog was open — nothing was done.")`. The toolbar calls `gitActions.pull({ showOptions: e.altKey })`.

- [ ] **Step 1: Write the failing tests**

(a) `src/lib/gitActions.pull.test.ts` — in `beforeEach`, after `appState.setRefsDetailed([mainRef]);` add:

```ts
  // The dialog reads the checked-out branch from the repository status, never the graph.
  appState.setRepoStatus(repoStatus);
```

(b) In the "pulls with the saved strategy and autostash when asking is off" test, right after `expect(ok).toBe(true);` add:

```ts
    expect(dialogs.state.kind).toBe("none");
```

(c) Append to the `describe("gitActions.pull", …)` block. `pull()` awaits the hydration barrier before anything else, so the dialog never opens synchronously — every case waits for it:

```ts
  it("asks first when the setting is on, saves the choice as the new default, then pulls with it", async () => {
    appState.setPullAskEveryTime(true);

    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    if (dialogs.state.kind !== "pull") throw new Error("pull dialog did not open");
    expect(dialogs.state.branch).toBe("main");
    expect(dialogs.state.upstream).toBe("origin/main");
    expect(dialogs.state.ahead).toBe(2);
    expect(dialogs.state.behind).toBe(3);
    expect(dialogs.state.strategy).toBe("merge");
    expect(dialogs.state.autostash).toBe(false);
    expect(api.pull).not.toHaveBeenCalled();

    dialogs.setPullStrategy("rebase");
    dialogs.setPullAutostash(true);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "rebase", true, expect.any(Function), undefined);
    expect(appState.pullStrategy).toBe("rebase");
    expect(appState.pullAutostash).toBe(true);
    expect(appState.pullAskEveryTime).toBe(true);
    expect(appState.status).toBe("Pull (rebase) — done.");
  });

  it("shows the options on alt-click even when asking is off", async () => {
    const pulling = gitActions.pull({ showOptions: true });
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "merge", false, expect.any(Function), undefined);
  });

  it("turns asking off when 'Don't ask again' is ticked", async () => {
    appState.setPullAskEveryTime(true);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    dialogs.setPullDontAsk(true);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(true);
    expect(appState.pullAskEveryTime).toBe(false);
    expect(api.pull).toHaveBeenCalledTimes(1);
  });

  it("pulls nothing and keeps the defaults when the dialog is cancelled", async () => {
    appState.setPullAskEveryTime(true);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    dialogs.setPullStrategy("ffOnly");
    dialogs.resolvePull(false);

    await expect(pulling).resolves.toBe(false);
    expect(api.pull).not.toHaveBeenCalled();
    expect(appState.pullStrategy).toBe("merge");
  });

  it("reads the checked-out branch from the repository status, not the loaded graph page", async () => {
    // HEAD's commit is outside the loaded page, so no decoration carries is_head.
    appState.setGraphCommits(SAMPLE_GRAPH.slice(1));
    expect(appState.refsByKind.local.some((r) => r.isHead)).toBe(false);
    appState.setPullAskEveryTime(true);

    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    if (dialogs.state.kind !== "pull") throw new Error("pull dialog did not open");
    expect(dialogs.state.branch).toBe("main");
    expect(dialogs.state.upstream).toBe("origin/main");
    expect(dialogs.state.ahead).toBe(2);
    expect(dialogs.state.behind).toBe(3);

    dialogs.resolvePull(false);
    await expect(pulling).resolves.toBe(false);
  });

  it("reads preferences only after the store has hydrated", async () => {
    let release: () => void = () => {};
    const hydration = new Promise<void>((resolve) => {
      release = resolve;
    });
    vi.spyOn(appState, "waitForPullSettingsHydration").mockReturnValue(hydration);

    const pulling = gitActions.pull();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(api.pull).not.toHaveBeenCalled();

    // The persisted preference lands, then the barrier lifts.
    appState.setPullStrategy("ffOnly");
    release();

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "ffOnly", false, expect.any(Function), undefined);
  });
```

(d) Add a second `describe` after the first one. Each case opens the dialog, changes something underneath it, confirms, and expects no pull and no preference write:

```ts
describe("a confirmed pull is bound to what the dialog showed", () => {
  // Opens the dialog, then watches every preference setter. The returned assertion
  // proves the refusal wrote nothing and pulled nothing.
  async function openDialog() {
    appState.setPullAskEveryTime(true);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    const writes = [
      vi.spyOn(appState, "setPullStrategy"),
      vi.spyOn(appState, "setPullAutostash"),
      vi.spyOn(appState, "setPullAskEveryTime"),
    ];
    return {
      pulling,
      expectNothingHappened() {
        expect(api.pull).not.toHaveBeenCalled();
        for (const write of writes) expect(write).not.toHaveBeenCalled();
        expect(appState.pullStrategy).toBe("merge");
        expect(appState.pullAutostash).toBe(false);
      },
    };
  }

  it("refuses when the repository changed while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.repo = "/tmp/git-it-pull-flow-other";
    dialogs.setPullStrategy("ffOnly");
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
  });

  it("refuses when the repository is loading again after an A → B → A switch", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.repo = "/tmp/git-it-pull-flow-other";
    appState.repo = REPO; // same identity as the dialog's, but loading again
    expect(appState.repoLoading).toBe(true);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't pull yet" });
  });

  it("refuses when the checked-out branch changed while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.setRepoStatus({ ...repoStatus, head: { ...repoStatus.head, branch: "feature" } });
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull",
      headline: "The checked-out branch changed while that dialog was open — nothing was done.",
    });
  });

  it("refuses when the branch's upstream changed while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.setRefsDetailed([{ ...mainRef, upstream: "upstream/main" }]);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't pull" });
  });
});
```

(e) `src/lib/gitActions.errorReporting.test.ts` (PR 1's file). Four edits, none of which changes what the cases assert:

- After the last import line (`import type { RemoteOutcome, RepoStatus } from "./types";`) add the same persistence stand-in the pull suite uses — the pull settings below persist through the Tauri store plugin once `window` is stubbed:

```ts
// Pull's settings persist through the Tauri store plugin once `window.__TAURI_INTERNALS__`
// is stubbed. There is no IPC under vitest, so give it an in-memory stand-in.
vi.mock("@tauri-apps/plugin-store", () => ({
  load: async () => ({
    get: async () => undefined,
    set: async () => {},
    save: async () => {},
  }),
}));
```

- In `beforeEach`, after `appState.setGraphCommits(SAMPLE_GRAPH);` add:

```ts
  // These are runRemote cases: pull's own options must never wait on the pull dialog.
  appState.setPullStrategy("merge");
  appState.setPullAutostash(false);
  appState.setPullAskEveryTime(false);
```

- In `afterEach`, before `errors.reset();` add:

```ts
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
```

- In "treats a pull the user cancelled as cancelled", `pull()` now awaits the settings barrier before `runRemote`, so the request is no longer in flight the moment `pull()` returns. After `const pulling = gitActions.pull();` add:

```ts
    await vi.waitFor(() => expect(api.pull).toHaveBeenCalled());
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/gitActions.pull.test.ts src/lib/gitActions.errorReporting.test.ts`
Expected, against Task 3's `pull` (no barrier, no dialog):
- `gitActions.pull.test.ts`: the two Task 3 cases still pass (including the new `dialogs.state.kind === "none"` line). The seven cases that wait for the dialog — "asks first", "alt-click", "repository status, not the loaded graph page", and the four binding cases — FAIL with a `vi.waitFor` timeout on `expected 'none' to be 'pull'`. "Don't ask again" FAILS on `expected true to be false` (`pullAskEveryTime` never changes). "cancelled" FAILS on `expected true to be false` (the pull ran anyway). "only after the store has hydrated" FAILS on `expected "spy" to not be called` (`api.pull` fires before the barrier lifts).
- `gitActions.errorReporting.test.ts`: all cases still pass — the edits are compatible with Task 3's `pull`, which still calls `api.pull` synchronously.
- `npm run check` additionally reports `Expected 0 arguments, but got 1` for the `pull({ showOptions: true })` call until Step 3 lands.

- [ ] **Step 3: The ask-first flow**

`src/lib/gitActions.ts`:

(a) Change the Task 3 import `import { pullLabel } from "./pullOptions";` to:

```ts
import { pullContext, pullLabel } from "./pullOptions";
```

(b) Immediately after the `sameRepoAfterPrompt()` function add:

```ts
// Pull reads settings and refs that belong to the loaded repository, so it refuses during
// the switch window — and again after every await, since a switch (even A → B → A, which
// keeps the identity but restarts loading) can land in between.
function pullStillLoading(): boolean {
  if (!appState.repoLoading) return false;
  reportNotice("Can't pull yet", "Repository is still loading — try again in a moment.");
  return true;
}
```

(c) Replace the `pull` member written in Task 3:

```ts
  pull: () => {
    if (appState.repoLoading) {
      reportNotice("Can't pull yet", "Repository is still loading — try again in a moment.");
      return Promise.resolve(false);
    }
    // Read once, up front, so a settings change mid-pull cannot split the label from
    // the flags actually sent.
    const strategy = appState.pullStrategy;
    const autostash = appState.pullAutostash;
    return runRemote(pullLabel(strategy), (onLine, creds) =>
      api.pull(appState.repo, strategy, autostash, onLine, creds),
    );
  },
```

with:

```ts
  // Pull with an explicit strategy. Asks first when the setting is on or the toolbar
  // button was ⌥-clicked; the dialog's choice becomes the new default (as Fork does)
  // and the pull runs with the dialog's values, never a re-read of the store.
  pull: async (opts?: { showOptions?: boolean }): Promise<boolean> => {
    if (pullStillLoading()) return false;
    // Capture before the FIRST await: every check below compares against the repository
    // the user clicked Pull in.
    const sameRepo = sameRepoAfterPrompt();
    // Preferences arrive from the Tauri store asynchronously. Reading them earlier would
    // use startup defaults — and confirming the dialog would then persist those defaults
    // over the saved ones, with the touched guards stopping hydration from fixing it.
    await appState.waitForPullSettingsHydration();
    if (!sameRepo() || pullStillLoading()) return false;
    let strategy = appState.pullStrategy;
    let autostash = appState.pullAutostash;
    if (appState.pullAskEveryTime || opts?.showOptions) {
      const shown = pullContext(appState.repoStatus, appState.refsDetailed);
      const choice = await dialogs.confirmPull({ ...shown, strategy, autostash });
      // Bind the confirmed pull to what the dialog showed: same repository, not loading
      // again, same branch and upstream — all before any preference write or pull.
      if (!choice || !sameRepo() || pullStillLoading()) return false;
      const now = pullContext(appState.repoStatus, appState.refsDetailed);
      if (now.branch !== shown.branch || now.upstream !== shown.upstream) {
        reportNotice(
          "Can't pull",
          "The checked-out branch changed while that dialog was open — nothing was done.",
        );
        return false;
      }
      strategy = choice.strategy;
      autostash = choice.autostash;
      appState.setPullStrategy(strategy);
      appState.setPullAutostash(autostash);
      if (choice.dontAsk) appState.setPullAskEveryTime(false);
    }
    return runRemote(pullLabel(strategy), (onLine, creds) =>
      api.pull(appState.repo, strategy, autostash, onLine, creds),
    );
  },
```

- [ ] **Step 4: Settings checkbox and the toolbar**

`src/lib/components/SettingsPanel.svelte`, Behavior tab — the flow above now honours `pullAskEveryTime`, so its control may exist. Directly after the autostash row

```svelte
            <span>Stash &amp; reapply local changes when pulling</span>
          </label>
```

add:

```svelte
          <label class="opt">
            <input
              type="checkbox"
              checked={appState.pullAskEveryTime}
              onchange={() => appState.setPullAskEveryTime(!appState.pullAskEveryTime)}
            />
            <span>Ask before every pull</span>
          </label>
```

`src/routes/+page.svelte` — after `import { pullStateBadge } from "$lib/github/itemState";` add:

```ts
  import { pullButtonTitle } from "$lib/pullOptions";
```

Replace the Pull button. Current code:

```svelte
      <button
        class="fetch-btn"
        disabled={!hasRemotes || appState.remoteOpActive}
        title={hasRemotes ? "Pull changes from remote" : noRemoteTitle}
        onclick={() => gitActions.pull()}
      >Pull</button>
```

New code:

```svelte
      <button
        class="fetch-btn"
        disabled={!hasRemotes || appState.remoteOpActive}
        title={hasRemotes
          ? pullButtonTitle(appState.pullAskEveryTime, appState.pullStrategy)
          : noRemoteTitle}
        onclick={(e) => gitActions.pull({ showOptions: e.altKey })}
      >Pull</button>
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run src/lib/gitActions.pull.test.ts src/lib/gitActions.errorReporting.test.ts`
Expected: PASS — 12 tests in `gitActions.pull.test.ts` (2 + 6 + 4) and every case in `gitActions.errorReporting.test.ts`, including its seven `runRemote` cases.

Run: `npm run check && npm test && cargo test && cargo clippy -p git-core -p git-it -- -D warnings`
Expected: 0 errors, 0 warnings, every suite green, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add src/lib/gitActions.ts src/lib/gitActions.pull.test.ts src/lib/gitActions.errorReporting.test.ts src/lib/components/SettingsPanel.svelte src/routes/+page.svelte
git commit -m "$(cat <<'EOF'
feat: ask before pulling, with alt-click to force the options

gitActions.pull opens the pull dialog when "Ask before every pull" is on
(the Settings checkbox lands here, with the flow that reads it) or the
toolbar button is ⌥-clicked. Preferences are read only after the store has
hydrated, so a pull at launch cannot persist startup defaults over the saved
ones. A confirmed pull runs only if the repository, its loading state and the
branch/upstream the dialog showed are unchanged; the chosen strategy and
autostash become the new defaults and the pull uses the dialog's values.
With asking off the button's tooltip names the strategy a click will use.
PR 1's runRemote cases turn asking off and wait for the request to be in
flight before cancelling.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Full gate and real-app verification

**Files:** none modified unless a check fails (fix in place, re-run the gate, commit the fix with a `fix:` subject).

- [ ] **Step 1: Run the full gate once more from a clean tree**

Run: `git status --short` (expect empty) then `npm run check && npm test && cargo test && cargo clippy -p git-core -p git-it -- -D warnings`
Expected: 0 errors, 0 warnings; all vitest suites green (including the 5 new files: `pullOptions`, `store.pullSettings`, `dialogs.pull`, `Modal.pull`, `gitActions.pull`, plus PR 1's `gitActions.errorReporting`); all Rust tests green; clippy clean.

- [ ] **Step 2: Build a debug `.app` and define the fixture recipe**

Vitest cannot run a real `git pull`. Cargo's workspace target directory is the repo root `target/` (not `src-tauri/target/`):

Run: `npm run tauri build -- --debug --bundles app`
Expected: `target/debug/bundle/macos/Git It.app` (the `--bundles app` flag skips the dmg and updater artefacts).

Quit the installed Git It, then open `target/debug/bundle/macos/Git It.app`.

Every scenario below starts from a fresh fixture or an explicit reset to the recorded local tip. A successful rebase moves `main` onto the upstream tip, so `reset --hard HEAD~1` after it would land on the upstream, NOT on the diverged state — always reset to `$ORIG`. Paste these into the shell you drive the checks from (the whole sequence, including the refusal texts and the post-autostash file contents, was verified against git 2.54 with `LC_ALL=C`):

```bash
# A fresh diverged pair. Sets $B (the clone to open) and $ORIG (its local tip).
mkfixture() {
  local D; D=$(mktemp -d /tmp/gitit-pull.XXXX)
  git init -q --bare -b main "$D/origin.git"
  git clone -q "$D/origin.git" "$D/a" 2>/dev/null
  ( cd "$D/a" && printf 'line1\nline2\nline3\n' > f.txt && git add f.txt \
      && git commit -qm base && git push -qu origin main 2>/dev/null )
  git clone -q "$D/origin.git" "$D/b"
  ( cd "$D/a" && printf 'LINE1\nline2\nline3\n' > f.txt \
      && git commit -qam 'remote edit' && git push -q origin main 2>/dev/null )
  ( cd "$D/b" && echo local > local.txt && git add local.txt \
      && git commit -qm 'local commit' && git fetch -q )
  B="$D/b"; ORIG=$(git -C "$B" rev-parse HEAD)
  echo "open in Git It: $B"; echo "local tip: $ORIG"
}
# Back to the diverged state on the current fixture (discards dirty files, extra branches).
reset_fixture() {
  git -C "$B" checkout -q -f main
  git -C "$B" branch -q -D lonely elsewhere 2>/dev/null
  git -C "$B" reset -q --hard "$ORIG"
}
```

Preference reset (do this before the first scenario and again before the NERV pass): Settings → Behavior → **Merge**, **Stash & reapply local changes when pulling** off, **Ask before every pull** on.

- [ ] **Step 3: Walk the checklist (spec "Real app", Part C rows) in Classic**

1. `mkfixture`, reset preferences, open `$B`. Click **Pull** → dialog "Pull origin/main → main" / "main is 1 ahead, 1 behind origin/main as of the last fetch.", Merge preselected, autostash unticked, "Don't ask again" unticked, focus on Pull. Escape closes it; nothing ran (`git -C "$B" rev-parse HEAD` is `$ORIG`).
2. Same fixture. Pull → **Fast-forward only** → Pull → the PR 1 error popup: headline "Not possible to fast-forward, aborting." with the divergent-branches hints; red status bar; HEAD still `$ORIG`.
3. `reset_fixture`. Pull → **Merge** → a merge commit tops the graph (`git -C "$B" rev-list --parents -n 1 HEAD` lists two parents); `f.txt` starts with `LINE1`; status "Pull — done."
4. `reset_fixture`. Pull → **Rebase** → linear history: `git -C "$B" rev-parse HEAD^` equals `git -C "$B" rev-parse origin/main`; status "Pull (rebase) — done."
5. `reset_fixture`, then `printf 'line1\nline2\nLINE3-local\n' > "$B/f.txt"`. Pull → Merge, autostash off → popup headline "Your local changes to the following files would be overwritten by merge:" + `f.txt`; `f.txt` still reads `line1 / line2 / LINE3-local`; HEAD still `$ORIG`. Pull again with **Stash & reapply local changes** ticked → succeeds; `f.txt` reads `LINE1 / line2 / LINE3-local`; Local Changes shows `f.txt` modified (the reapplied edit); status "Pull — done."
6. `reset_fixture`. Pull → tick **Don't ask again** → Pull → the pull runs (merge). Settings → Behavior shows "Ask before every pull" unticked; the toolbar tooltip reads "Pull (merge) — ⌥-click for options". A plain click pulls immediately ("Pull — done.", already up to date); **⌥-click** opens the dialog; Escape. Re-tick "Ask before every pull".
7. Settings → Behavior → **Rebase**: the tooltip (with asking off) and the dialog's preselection follow; set it back to Merge.
8. `reset_fixture`, then `git -C "$B" checkout -q --detach`. ⌥-click Pull → "No branch is checked out.", Pull disabled, Enter does nothing, Escape closes.
9. `reset_fixture`, then `git -C "$B" checkout -q -b lonely`. ⌥-click Pull → "lonely has no upstream branch — push it first to set one.", Pull disabled.
10. Binding: `reset_fixture`. Click Pull (dialog open), then in the shell `git -C "$B" checkout -q -b elsewhere` and wait until the sidebar shows `elsewhere` checked out (the watcher refreshes behind the dialog). Click **Pull** in the dialog → notice "Can't pull" / "The checked-out branch changed while that dialog was open — nothing was done."; `git -C "$B" log --oneline -1` is unchanged and Settings still shows Merge.

- [ ] **Step 4: NERV pass on a fresh fixture**

`mkfixture` again (a new clone), reset preferences, Settings → Appearance → NERV, open the new `$B`. Repeat scenarios 1, 2 and 5: the dialog shows the corner brackets and the "◇" title glyph, the selected strategy card tints with the accent, and the error popup is readable.

- [ ] **Step 5: Report**

No commit unless Steps 3–4 exposed a bug. Note anything that could not be verified in the PR description.

---

## Spec coverage

| Spec item | Task |
|---|---|
| C1 `PullStrategy` (serde camelCase), `ops_remote::pull` + Tauri `pull` take `strategy` + `autostash`, flag table, `--autostash` | 1 |
| C1 Rust tests: merge under `pull.ff=only` and `pull.rebase=true`, rebase linear, ff-only refuses, dirty edit survives with autostash (and is refused without) | 1 |
| C2 three persisted settings with defaults, migration of legacy `pullRebase: true` (key left in place) | 3 (helper in 2) |
| C2 Settings → Behavior: strategy radio group + two checkboxes | 3 (radios + autostash) + 6 (ask before every pull, with the flow that honours it) |
| C3 `pull` dialog kind resolving `{ strategy, autostash, dontAsk } \| null`, `settlePending` → `null` | 4 |
| C3 header, status line (non-zero counts only), radios with descriptions, autostash, don't-ask + note, Pull/Cancel, Enter/Escape, no-upstream / detached-HEAD disabled states | 2 (copy) + 5 (rendering) |
| C4 flow: notice guard → capture `sameRepoAfterPrompt()` before the dialog → cancel / changed repo returns → persist defaults, `dontAsk` → `runRemote` with strategy-named label and the dialog's values | 3 (guard + settings) + 6 (dialog) |
| Review findings: hydration barrier before reading preferences; dialog context from the repository status + ref inventory (not the graph page); confirmed pull re-checks repo identity, loading state and `{ branch, upstream }`; PR 1's remote cases kept green | 3 (barrier) + 2 (`pullContext`) + 6 (flow + tests) |
| C4 toolbar ⌥-click and "Pull (`<strategy>`) — ⌥-click for options" tooltip | 6 |
| Testing: migration + hydration/touched guard; pull flow (asks when on, skips when off, ⌥ forces, changed repo / loading-again / branch-changed abort, graph page lacks HEAD, delayed hydration); pull dialog component (preselection, disabled with no upstream, "Don't ask again") | 3, 4, 5, 6 |
| Gates + real app (Part C rows, Classic and NERV) | 7 |

## Decisions made where the spec was silent

- Ahead-only status line reads "`main` is 2 ahead **of** `origin/main` as of the last fetch." (the spec gives only the both / behind-only / up-to-date forms).
- Dialog header falls back to plain "Pull" when there is no upstream or no branch; the status line carries the explanation.
- Names render as plain text (no code formatting) so the copy module returns exact strings.
- Radio descriptions are shown in sentence case ("Fast-forward when possible, otherwise create a merge commit").
- The tooltip while asking is on stays "Pull changes from remote" (today's text).
- `README.md` line 33 still says "streamed pull (merge or rebase)"; left untouched (not in the spec) — mention in the PR if the reviewer wants it updated.
- The dialog's branch comes from `appState.repoStatus.head` and its upstream/ahead/behind from the `appState.refsDetailed` entry for that branch (`pullContext`), not from the graph-derived `currentUpstream` family: a checked-out branch whose commit sits outside the loaded page has no `is_head` decoration.
- After the dialog, a changed branch or upstream is refused with `reportNotice("Can't pull", "The checked-out branch changed while that dialog was open — nothing was done.")`; a repository that is loading again (A → B → A) reuses the "Can't pull yet" notice. Both checks run before any preference write.
- `waitForPullSettingsHydration()` is awaited by every pull (asking on or off): with asking off, a pre-hydration pull would otherwise use the startup strategy instead of the saved one.
- The `pullAskEveryTime` setting persists from Task 3 (store-only), but its checkbox is added in Task 6 with the flow that honours it — no visible control ships before it works.
