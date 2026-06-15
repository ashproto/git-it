# Phase 6 — Remote Operations (pull / push / remotes / credentials) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** The final roadmap phase — remotes: **pull** (merge/rebase, persisted default), **push** (`--force-with-lease`, `--set-upstream`), full **remote management** (list/add/remove/set-url), **ahead/behind** surfaced in the UI, **streamed progress + Cancel** for hanging network ops, and a **GIT_ASKPASS credentials prompt + retry** (no secrets stored).

**Architecture:** New Rust `ops_remote.rs`. Remote management is simple sync git. Pull/push are **streamed async** commands (mirror `rewrite_history`: `async fn` + `Channel<String>` for progress) and register their `Child` in a Tauri-managed `RemoteState { Mutex<Option<Child>> }` so a `cancel_remote` command can `kill()` it. Auth: run with `GIT_TERMINAL_PROMPT=0`; on an auth-failure the op returns `auth_failed=true`; the UI shows a `CredentialsPrompt` and **retries the same op with credentials**, which the backend feeds to git via a generated `GIT_ASKPASS` script that `cat`s the username/password from `0600` temp files (creds never in args/URLs/Store; temp dir deleted after). Pull conflicts reuse the Phase 3b `ConflictView` (pull = merge/rebase → `repo_status.operation` already detected). The already-computed `list_refs` ahead/behind/upstream (unused today) is wired into the store → branch chip + sidebar badges + a `RemotePanel`.

**Tech Stack:** Rust (git shell-out, async streaming) + Tauri 2 managed state + `Channel`; SvelteKit 5 runes; reuse Phase 2–5 patterns.

**Branch:** `feat/phase6-remote` (already created).

## SECURITY (every git shell-out with a user operand)
- `push`: `--end-of-options` before `<remote> <refspec>`; force is **always `--force-with-lease`, never bare `--force`**.
- `pull`/`fetch`: `--end-of-options` before a remote operand; `GIT_TERMINAL_PROMPT=0`.
- Remote **names** are validated against `^[A-Za-z0-9][A-Za-z0-9._/-]*$` (reject leading `-` / odd chars) before use; URLs are passed in the 2nd positional slot (after the validated name) so they can't be parsed as flags. `remote remove`/`set-url` validate the name.
- **Credentials:** captured username/password written to `0600` temp files; a generated `GIT_ASKPASS` script `cat`s them by prompt type; creds are NEVER command-line args, NEVER embedded in URLs, NEVER persisted to the Store/app-state; the temp dir is removed after the op. `GIT_TERMINAL_PROMPT=0` always (askpass, not terminal).

## SAFETY
- A force-with-lease push **confirms** first (plain `dialogs.confirm`, danger) with a clear consequence; a normal (fast-forward) push does not.
- Cancel kills the in-flight git child cleanly (git handles SIGTERM).

## File structure
- Create `src-tauri/src/ops_remote.rs`; modify `types.rs`, `commands.rs`, `lib.rs`.
- Create `src/lib/components/RemotePanel.svelte`, `CredentialsPrompt.svelte`, `RemoteProgress.svelte`.
- Modify `store.svelte.ts`, `gitActions.ts`, `api.ts`, `types.ts`, `+page.svelte` (Pull/Push buttons + ahead/behind chip + progress), `Sidebar.svelte` (RemotePanel), `dialogs.svelte.ts` (credentials), `DateFormatMenu.svelte` (pull-strategy setting).

---

## Task 1 — Backend: remote management (list / add / remove / set-url) + wire list_refs

**Files:** create `ops_remote.rs`; modify `types.rs`, `commands.rs`, `lib.rs`, `src/lib/types.ts`, `src/lib/api.ts`.

- [ ] **Step 1: Types (`types.rs`):**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}
```

- [ ] **Step 2: Create `ops_remote.rs` (this task) + tests.**
```rust
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
    if !valid_remote_name(name) { return Err(format!("Invalid remote name: {}", name)); }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "add", name, url]); // name validated; url is positional-2
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn remote_remove(repo: &Path, name: &str) -> Result<(), String> {
    if !valid_remote_name(name) { return Err(format!("Invalid remote name: {}", name)); }
    let mut c = Command::new("git");
    c.current_dir(repo).args(["remote", "remove", name]);
    git_ops::run(&mut c)?;
    Ok(())
}

pub fn remote_set_url(repo: &Path, name: &str, url: &str) -> Result<(), String> {
    if !valid_remote_name(name) { return Err(format!("Invalid remote name: {}", name)); }
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
    struct TempRepo { path: PathBuf }
    impl TempRepo {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!("gte-remote-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            let r = TempRepo { path };
            r.git(&["init", "-q", "-b", "main"]);
            r.git(&["config", "user.email", "t@e.com"]);
            r.git(&["config", "user.name", "T"]);
            r
        }
        fn git(&self, args: &[&str]) {
            let o = Command::new("git").current_dir(&self.path).args(args).output().unwrap();
            assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
        }
    }
    impl Drop for TempRepo { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); } }

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
```

- [ ] **Step 3: Tests pass.** From `src-tauri/`: `cargo test`.
- [ ] **Step 4: Commands + registration + bindings.** `commands.rs` (+ `use crate::ops_remote;`, `RemoteInfo` import): `remotes`, `remote_add`, `remote_remove`, `remote_set_url`. Register in `lib.rs`. `api.ts` (+ `RemoteInfo`, `Ref` already exist):
```ts
  remotes: (repo: string) => invoke<RemoteInfo[]>("remotes", { repo }),
  remoteAdd: (repo: string, name: string, url: string) => invoke<void>("remote_add", { repo, name, url }),
  remoteRemove: (repo: string, name: string) => invoke<void>("remote_remove", { repo, name }),
  remoteSetUrl: (repo: string, name: string, url: string) => invoke<void>("remote_set_url", { repo, name, url }),
```
`types.ts`: `export type RemoteInfo = { name: string; url: string };`

- [ ] **Step 5: Gates + commit.** `cargo test` + `cargo check` + `npm run check`. Commit: `feat(remote): remote management (list/add/remove/set-url) backend (Phase 6)`.

---

## Task 2 — Backend: streamed pull + push + cancel

**Files:** modify `ops_remote.rs`, `types.rs`, `commands.rs`, `lib.rs`, `src/lib/api.ts`.

- [ ] **Step 1: Types (`types.rs`):**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOutcome {
    pub ok: bool,
    pub auth_failed: bool,  // → UI shows CredentialsPrompt + retries
    pub conflicted: bool,   // pull merge/rebase stopped on conflict → ConflictView
    pub message: String,
}
```

- [ ] **Step 2: Managed cancel state + the streaming core (`ops_remote.rs`).** Add `use std::process::{Child, Stdio};`, `use std::io::{BufRead, BufReader};`, `use std::sync::Mutex`.
```rust
#[derive(Default)]
pub struct RemoteState {
    pub child: Mutex<Option<Child>>,
}

fn looks_like_auth_failure(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("authentication failed")
        || l.contains("could not read username")
        || l.contains("could not read password")
        || l.contains("permission denied")
        || l.contains("terminal prompts disabled")
        || l.contains("invalid username or password")
        || l.contains("fatal: authentication")
}

/// Run a git network op, streaming combined stdout+stderr to `on_line`, registering the
/// child in `state` so `cancel` can kill it. Returns (success, combined_output).
fn stream(
    mut cmd: Command,
    state: &RemoteState,
    on_line: &dyn Fn(String),
) -> Result<(bool, String), String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("spawn: {}", e))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let collected = std::sync::Arc::new(Mutex::new(String::new()));
    let mk = |pipe: std::process::ChildStdout| {};  // placeholder; see threads below

    // Reader threads: collect + stream each line.
    let c_out = std::sync::Arc::clone(&collected);
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let tx2 = tx.clone();
    let t_out = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) { let _ = tx.send(line); }
    });
    let t_err = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) { let _ = tx2.send(line); }
    });
    // Forward streamed lines to on_line + collect (drain until both readers end).
    drop(/* keep tx alive via threads */ ());
    // Move child into the cancel slot.
    { *state.child.lock().unwrap() = Some(child); }

    // Drain the channel on this thread until both reader threads finish.
    for line in rx { // rx ends when all tx clones are dropped (both threads done)
        on_line(line.clone());
        c_out.lock().unwrap().push_str(&line);
        c_out.lock().unwrap().push('\n');
    }
    let _ = t_out.join();
    let _ = t_err.join();

    // Wait for exit (or detect cancellation: the slot was taken+killed by `cancel`).
    let status = {
        let mut g = state.child.lock().unwrap();
        match g.take() {
            Some(mut c) => c.wait().map_err(|e| format!("wait: {}", e))?,
            None => return Ok((false, "Cancelled.".to_string())), // cancel took+killed it
        }
    };
    Ok((status.success(), collected.lock().unwrap().clone()))
}
```
> Implementer note: the snippet above is the intended shape; clean it up so it compiles (remove the `mk`/`drop` placeholders; ensure both `tx` clones are dropped by the threads so `rx` terminates; the channel drains on the calling thread which also streams via `on_line`). The KEY behaviors to preserve: (a) stream every line to `on_line` AND accumulate it; (b) register the `Child` in `state.child` so `cancel` can take+kill it; (c) if the slot is `None` at wait time, treat as cancelled. Verify with the tests below (a real push/pull to a local bare remote).

- [ ] **Step 3: pull / push / cancel.**
```rust
fn outcome(repo: &Path, ok: bool, msg: String) -> RemoteOutcome {
    if ok { return RemoteOutcome { ok: true, auth_failed: false, conflicted: false, message: msg }; }
    if looks_like_auth_failure(&msg) {
        return RemoteOutcome { ok: false, auth_failed: true, conflicted: false, message: msg };
    }
    // A pull that stopped on conflicts leaves unmerged files / an op in progress.
    let conflicted = crate::ops_merge::conflicted_files(repo).map(|f| !f.is_empty()).unwrap_or(false);
    RemoteOutcome { ok: false, auth_failed: false, conflicted, message: msg }
}

pub fn pull(repo: &Path, rebase: bool, state: &RemoteState, on_line: &dyn Fn(String)) -> Result<RemoteOutcome, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).env("GIT_TERMINAL_PROMPT", "0").env("GIT_EDITOR", "true").arg("pull");
    if rebase { c.arg("--rebase"); } else { c.arg("--no-edit"); }
    let (ok, msg) = stream(c, state, on_line)?;
    Ok(outcome(repo, ok, msg))
}

pub fn push(repo: &Path, remote: &str, refspec: Option<&str>, force_with_lease: bool, set_upstream: bool, state: &RemoteState, on_line: &dyn Fn(String)) -> Result<RemoteOutcome, String> {
    let mut c = Command::new("git");
    c.current_dir(repo).env("GIT_TERMINAL_PROMPT", "0").arg("push");
    if force_with_lease { c.arg("--force-with-lease"); } // NEVER bare --force
    if set_upstream { c.arg("--set-upstream"); }
    c.arg("--end-of-options").arg(remote);
    if let Some(rs) = refspec { c.arg(rs); }
    let (ok, msg) = stream(c, state, on_line)?;
    Ok(outcome(repo, ok, msg))
}

pub fn cancel(state: &RemoteState) {
    if let Some(mut child) = state.child.lock().unwrap().take() {
        let _ = child.kill();
    }
}
```

- [ ] **Step 4: Tests against a LOCAL BARE REMOTE (offline).** Add tests: create a bare repo (`git init --bare`) in a temp dir, a working clone, `remote_add origin <bare>`, commit, `push(origin, Some("main"), false, true, …)` succeeds; a second clone `pull`s and sees the commit; a diverged push without force fails (non-fast-forward), and with `force_with_lease` succeeds. (A no-op `on_line` closure + a throwaway `RemoteState::default()`.) For pull-conflict: two clones edit the same line, one pushes, the other `pull`s → `RemoteOutcome.conflicted == true`. These exercise the streaming + outcome logic with no network.

```rust
    // sketch — implementer fleshes out with the TempRepo helper + a bare remote:
    #[test]
    fn push_then_pull_roundtrip_local_bare() { /* init --bare; clone A; commit; push; clone B; pull; assert commit present */ }
    #[test]
    fn push_non_fastforward_needs_force_with_lease() { /* diverge; plain push fails; force_with_lease succeeds */ }
    #[test]
    fn pull_conflict_sets_conflicted() { /* both edit same line; push one; pull other → outcome.conflicted */ }
```

- [ ] **Step 5: Commands (async + Channel + managed state) + registration.** In `lib.rs`: `.manage(ops_remote::RemoteState::default())` and register the commands. In `commands.rs`:
```rust
use tauri::State;

#[tauri::command]
pub async fn pull(repo: String, rebase: bool, on_event: Channel<String>, state: State<'_, ops_remote::RemoteState>) -> Result<RemoteOutcome, String> {
    // spawn_blocking can't borrow State; clone what's needed. RemoteState holds a Mutex —
    // wrap it in Arc in manage(), or run inline on a blocking task. Simplest: run the
    // blocking git in spawn_blocking using an Arc<RemoteState> from manage().
    let st = state.inner_arc(); // see note
    let repo = PathBuf::from(repo);
    let chan = on_event.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops_remote::pull(&repo, rebase, &st, &move |l| { let _ = chan.send(l); })
    }).await.map_err(|e| format!("join: {}", e))?
}
```
> Implementer note: Tauri `State` isn't `'static` for `spawn_blocking`. Manage an `Arc<RemoteState>` (`app.manage(Arc::new(RemoteState::default()))`) and in each command take `state: State<'_, Arc<RemoteState>>` then `let st = state.inner().clone();` (clone the Arc) before `spawn_blocking`. `cancel_remote` takes the same `State<Arc<RemoteState>>` and calls `ops_remote::cancel(&st)` synchronously (no spawn). Mirror the `rewrite_history` async+Channel command shape. Add `push` (params: repo, remote, refspec: Option<String>, forceWithLease, setUpstream, on_event, state) and `cancel_remote` (state).

- [ ] **Step 6: api.ts bindings** (Channel like `rewriteHistory`):
```ts
  pull: (repo: string, rebase: boolean, onEvent: (l: string) => void) => {
    const ch = new Channel<string>(); ch.onmessage = onEvent;
    return invoke<RemoteOutcome>("pull", { repo, rebase, onEvent: ch });
  },
  push: (repo: string, remote: string, refspec: string | null, forceWithLease: boolean, setUpstream: boolean, onEvent: (l: string) => void) => {
    const ch = new Channel<string>(); ch.onmessage = onEvent;
    return invoke<RemoteOutcome>("push", { repo, remote, refspec, forceWithLease, setUpstream, onEvent: ch });
  },
  cancelRemote: (repo: string) => invoke<void>("cancel_remote", { repo }),
```
(+ `RemoteOutcome` type in types.ts mirroring the Rust struct; `cancel_remote` ignores repo if unused but keep a param for uniformity, or drop it.)

- [ ] **Step 7: Gates + commit.** `cargo test` + `cargo check` + `npm run check`. Commit: `feat(remote): streamed pull/push + cancel backend (Phase 6)`.

---

## Task 3 — Backend: GIT_ASKPASS credentials + retry

**Files:** modify `ops_remote.rs`, `commands.rs`, `src/lib/api.ts`.

- [ ] **Step 1: Optional credentials on pull/push.** Add a `Credentials { username, password }` concept passed as two `Option<String>` params (avoid a struct over IPC if simpler). When BOTH are present, set up a `GIT_ASKPASS` helper for that invocation:
```rust
/// Write creds to 0600 files + a GIT_ASKPASS script that cats them by prompt; returns the
/// scratch dir (delete after the op) and configures the Command's env. No creds on argv,
/// in URLs, or persisted. Caller deletes `dir` when done.
fn setup_askpass(cmd: &mut Command, username: &str, password: &str) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("gte-cred-{}-{}", std::process::id(), /* a per-call nonce, e.g. an AtomicU32 */ next_nonce()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cred dir: {}", e))?;
    let uf = dir.join("u"); let pf = dir.join("p");
    write_600(&uf, username)?; write_600(&pf, password)?;
    let script = dir.join("askpass.sh");
    // git calls askpass with the prompt as $1 (e.g. "Username for 'https://…':").
    std::fs::write(&script, format!("#!/bin/sh\ncase \"$1\" in\n  *[Uu]sername*) cat '{}' ;;\n  *) cat '{}' ;;\nesac\n", uf.display(), pf.display())).map_err(|e| format!("askpass: {}", e))?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).ok(); }
    cmd.env("GIT_ASKPASS", &script).env("GIT_TERMINAL_PROMPT", "0");
    Ok(dir)
}
// write_600: create file with 0600 perms (umask-safe) and write contents.
```
Thread `username/password` `Option`s into `pull`/`push`: if both `Some`, `let dir = setup_askpass(&mut c, u, p)?;` before `stream`, and `let _ = std::fs::remove_dir_all(dir);` after. (Single-quote-escape the temp paths in the script as in Phase 4's `sq`.)

- [ ] **Step 2: Tests.** Unit-test `setup_askpass`: it creates an `askpass.sh` that, invoked with a "Username for ..." arg, prints the username, and with a "Password for ..." arg prints the password; the cred files are `0600`; the script is removable. (Run the generated script with `sh <script> "Username for 'x'"` and assert stdout.) Also assert creds never appear in the `Command`'s args (they're only in files/env).

- [ ] **Step 3: Commands.** Extend the `pull`/`push` commands with `username: Option<String>, password: Option<String>` params (passed after the existing ones). Update `api.ts` `pull`/`push` to accept optional `creds?: { username: string; password: string }` and forward them. (First call: omit creds → relies on the system helper; on `auth_failed`, the UI re-calls WITH creds.)

- [ ] **Step 4: Gates + commit.** `cargo test` + `cargo check` + `npm run check`. Commit: `feat(remote): GIT_ASKPASS credentials helper + retry (Phase 6)`.

---

## Task 4 — Frontend: store + gitActions + dialogs (ahead/behind, pull-strategy, progress/cancel, credentials)

**Files:** modify `store.svelte.ts`, `gitActions.ts`, `dialogs.svelte.ts`, `Modal.svelte`.

- [ ] **Step 1: Store.** Add: persisted `pullRebase: boolean` setting (mirror `diffSplit`; default false=merge); `refsDetailed: Ref[]` (from `api.listRefs`, has ahead/behind/upstream) + setter; `remotes: RemoteInfo[]` + setter; `remoteProgress: { active: boolean; lines: string[] } | null` (or `remoteOpActive: boolean` + `remoteLog: string[]`) for the streamed spinner; a derived `currentUpstream`/`ahead`/`behind` for the current branch (find `refsDetailed` entry matching the current branch). Clear remote state on repo change.

- [ ] **Step 2: `refreshRefs()` in gitActions** — Tauri-only: `appState.setRefsDetailed(await api.listRefs(repo))` + `appState.setRemotes(await api.remotes(repo))`; call it inside `reloadGraph` (after `refreshStatus`). This finally surfaces ahead/behind.

- [ ] **Step 3: `dialogs.confirmCredentials`** — add a `"credentials"` dialog kind returning `Promise<{ username: string; password: string } | null>` (username + password fields; password input `type="password"`; "the token is used once and never stored" hint). Render in `Modal.svelte`. (Mirror the existing `prompt`/`confirmDestructive` shape; `settlePending` resolves it null.)

- [ ] **Step 4: gitActions remote wrappers.**
```ts
// Streamed op with progress + cancel + credentials retry.
async function runRemote(label, fn: (onLine, creds?) => Promise<RemoteOutcome>): Promise<boolean> {
  guard tauri+repo;
  appState.startRemoteProgress(label);          // sets remoteOpActive=true, clears log
  try {
    let outcome = await fn((l) => appState.pushRemoteLog(l));   // first try: no creds (system helper)
    if (outcome.authFailed) {
      const creds = await dialogs.confirmCredentials({ title: `${label}: sign in` });
      if (creds) outcome = await fn((l) => appState.pushRemoteLog(l), creds);  // retry with creds
    }
    await reloadGraph();
    if (outcome.conflicted) appState.status = `${label}: conflicts to resolve.`;   // ConflictView appears
    else if (outcome.ok) appState.status = `${label} — done.`;
    else appState.status = `${label} failed: ${firstLine(outcome.message)}`;
    return outcome.ok;
  } catch (e) { await reloadGraph().catch(()=>{}); appState.status = `${label} failed: ${firstLine(e)}`; return false; }
  finally { appState.endRemoteProgress(); }
}

export const gitActions = { …,
  pull: () => runRemote(appState.pullRebase ? "Pull (rebase)" : "Pull",
    (onLine, creds) => api.pull(appState.repo, appState.pullRebase, onLine, creds)),
  push: async (forceWithLease = false) => {
    if (forceWithLease) { const ok = await dialogs.confirm({ title:"Force push", message:"Force-push with lease? This overwrites the remote branch if it matches your last fetch.", confirmLabel:"Force push", danger:true }); if (!ok) return false; }
    const branch = appState.refsByKind.local.find(r=>r.isHead)?.name ?? null;
    const up = /* does current branch have an upstream? from refsDetailed */;
    return runRemote(forceWithLease ? "Force push" : "Push",
      (onLine, creds) => api.push(appState.repo, "origin", branch, forceWithLease, !up, onLine, creds));
  },
  cancelRemote: () => api.cancelRemote(appState.repo),
  remoteAdd: (name, url) => run(`Add remote ${name}`, () => api.remoteAdd(appState.repo, name, url)),
  remoteRemove: (name) => run(`Remove remote ${name}`, () => api.remoteRemove(appState.repo, name)),
  remoteSetUrl: (name, url) => run(`Set ${name} url`, () => api.remoteSetUrl(appState.repo, name, url)),
};
```
(Adapt to the real api signatures from Task 2/3; `api.pull/push` take an `onLine` callback + optional creds. `push` `remote` is "origin" for v1 — or the first remote.)

- [ ] **Step 5: Gates + commit.** `npm run check` + `npm test`. Commit: `feat(remote-ui): store + actions + credentials/progress for pull/push (Phase 6)`.

---

## Task 5 — Frontend: Pull/Push UI + ahead/behind + RemotePanel + progress + credentials

**Files:** create `RemotePanel.svelte`, `CredentialsPrompt.svelte` (or fold into Modal), `RemoteProgress.svelte`; modify `+page.svelte`, `Sidebar.svelte`, `DateFormatMenu.svelte`.

- [ ] **Step 1: Header Pull/Push + ahead/behind chip** (`+page.svelte`). Next to the existing **Fetch** button add **Pull** (`gitActions.pull()`) and **Push** (`gitActions.push()`; a small ▾ or right-click / modifier for "Force push (with lease)" → `gitActions.push(true)`). Extend the branch chip to show `↑{ahead} ↓{behind}` when the current branch has an upstream with ahead/behind > 0 (from `appState` derived ahead/behind). Disable Pull/Push when no remote exists (guide the user to add one).

- [ ] **Step 2: `RemoteProgress.svelte`** — when `appState.remoteOpActive`, show a slim progress bar/spinner with the latest streamed line + a **Cancel** button (`gitActions.cancelRemote()`). Mount in `+page.svelte` (e.g. under the header or as a small overlay/toast).

- [ ] **Step 3: `CredentialsPrompt`** — render the `dialogs` "credentials" kind in `Modal.svelte` (username + password[type=password] + Sign in/Cancel; "used once, never stored" note).

- [ ] **Step 4: `RemotePanel.svelte`** — a `CollapsiblePanel` "Remotes" in the sidebar: list `appState.remotes` (name + url), each with Set-URL (prompt) + Remove (confirm); an "Add remote…" button (prompt name + url → `gitActions.remoteAdd`). Desktop-only note in browser. Mount under `StashPanel` in `Sidebar.svelte`.

- [ ] **Step 5: Pull-strategy setting** — add a "Pull with rebase" checkbox to the gear popover (`DateFormatMenu.svelte`), bound to `appState.pullRebase` / `setPullRebase`.

- [ ] **Step 6: Gates + commit.** `npm run check` + `npm test`. Commit: `feat(remote-ui): pull/push buttons, ahead-behind, RemotePanel, progress, credentials (Phase 6)`.

---

## Task 6 — Verification + preview + review + merge

- [ ] **Step 1: Gates.** `cargo test` + `cargo check`; `npm run check` + `npm test` + `npm run build`. All green.
- [ ] **Step 2: Preview.** Start preview. Verify (mock where Tauri-guarded, then revert): Pull/Push/Fetch buttons render; ahead/behind chip (seed a mock `refsDetailed`); `RemotePanel` (seed mock `remotes`); `RemoteProgress` spinner + Cancel (seed `remoteOpActive` + log); `CredentialsPrompt` dialog (force the "credentials" dialog); pull-strategy toggle in the gear popover. Screenshot light + dark; console clean. Revert seeds; re-run `npm run check`.
- [ ] **Step 3: Final adversarial review** (Agent `superpowers:code-reviewer`, opus) over the branch diff. Focus: push is ALWAYS `--force-with-lease` never bare `--force`; remote-name validation blocks option-injection; **credentials are never in argv/URLs/Store** and the askpass temp dir (0600 files) is deleted after; the auth-failure→prompt→retry flow works and a cancelled prompt aborts cleanly; cancel kills the child and the op reports cancellation without hanging; the streaming threads/channel terminate (no deadlock/leak); pull conflicts surface via ConflictView (`repo_status.operation`); ahead/behind wired correctly; `--end-of-options` on remote operands. Run the local-bare-remote tests. Fix findings.
- [ ] **Step 4: Merge.** `superpowers:finishing-a-development-branch` → ff-merge to `main`, delete branch. Update memory + `docs/RESUME.md` — **the 6-phase roadmap is COMPLETE**; note remaining optional follow-ups.

## Deferred (optional follow-ups, not this phase)
- Per-remote push target picker (v1 pushes to `origin` / the first remote).
- Pull/push of arbitrary refspecs beyond the current branch.
- Tag push (`--tags`), prune-on-push, push-all-branches.
- Line-level staging; squash-merge commit-UI; interactive-rebase `edit` pause flow (carried from Phase 5).

## Patterns to REUSE
- `rewrite_history` async + `Channel<String>` streaming (commands.rs) — the model for pull/push progress.
- `gitActions` guard/refresh/`firstLine`; `reloadGraph`/`refreshStatus`/`refreshWorkingChanges` (+ new `refreshRefs`); `dialogs.confirm`/`confirmDestructive`/`prompt`; the ConflictView (pull conflicts); the Tauri Store settings pattern (`diffSplit`) for `pullRebase`; `CollapsiblePanel` for `RemotePanel`.
- `ops.rs::fetch` (GIT_TERMINAL_PROMPT=0 + `--end-of-options`) — the remote-op security baseline.
