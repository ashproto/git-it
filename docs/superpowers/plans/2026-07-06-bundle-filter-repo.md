# Bundle git-filter-repo + Actionable Prerequisites — Implementation Plan

> Spec: `docs/superpowers/specs/2026-07-06-bundle-filter-repo-design.md`. Branch: `bundle-filter-repo`.
> The vendored script already exists at `src-tauri/resources/git-filter-repo/{git-filter-repo, COPYING.mit, README.md}`.

**Gates (run the relevant ones after each task; all green before "done"):**
`cargo test -p git-core -p git-it` · `cargo clippy -p git-core -p git-it -- -D warnings` · `npm run check` · `npm test` · `npm run build`

**Ordering is sequential and load-bearing** — this is a coupled change: Task 1 (git-core signatures) must compile before Task 2 (src-tauri uses them); Task 4 (`types.ts`) must match Task 1 (`types.rs`). Do not reorder.

---

### Task 1 — git-core: parameterize filter-repo invocation + prerequisites (Rust)

**Files:** `crates/git-core/src/types.rs`, `crates/git-core/src/rewrite.rs`, `crates/git-core/src/git_ops.rs`

- `types.rs`: add to `PrerequisiteCheck`: `pub python3: bool`, `pub python3_version: Option<String>`. Keep `filter_repo` / `filter_repo_version` (now = the *bundled* script).
- `rewrite.rs::rewrite_history`: add a parameter `filter_repo_argv: &[String]` (the invocation prefix, e.g. `["python3", "/…/git-filter-repo"]`). Replace `Command::new("git-filter-repo")` with `Command::new(&filter_repo_argv[0])` then `.args(&filter_repo_argv[1..])`, then the existing `--commit-callback …`, `--force --partial --replace-refs delete-no-add`, and optional `--refs` args. Update the spawn-error message to drop `brew install git-filter-repo` (say the bundled filter-repo failed to launch and reference Python).
- Extract the argv assembly into a pure, testable helper, e.g. `fn build_filter_repo_argv(prefix: &[String], callback: &str, refs: Option<&str>) -> Vec<String>` and have `rewrite_history` use it, so it can be unit-tested without spawning.
- `git_ops.rs::check_prerequisites`: change signature to `check_prerequisites(filter_repo_argv: &[String]) -> PrerequisiteCheck`. Probe: `git --version` (existing), `python3 --version` → `python3`/`python3_version`, and `<filter_repo_argv> --version` → `filter_repo`/`filter_repo_version`. Preserve the existing shell-out safety conventions.

**Tests:** unit-test `build_filter_repo_argv` for a representative prefix + options (with and without `--refs`). Update any existing `check_prerequisites` callers/tests to pass a prefix.

**Gate:** `cargo test -p git-core` + `cargo clippy -p git-core -- -D warnings`.

---

### Task 2 — src-tauri: resource resolver, command wiring, install command (Rust)

**Files:** `src-tauri/src/filter_repo.rs` (new), `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`

- `filter_repo.rs`: `pub fn filter_repo_argv(app: &tauri::AppHandle) -> Vec<String>`. Resolve the bundled script: production via `app.path().resolve("resources/git-filter-repo/git-filter-repo", BaseDirectory::Resource)`; if that path doesn't exist (i.e. `tauri dev`), fall back to the source tree relative to `CARGO_MANIFEST_DIR` (`.../src-tauri/resources/git-filter-repo/git-filter-repo`). Return `vec!["python3".into(), <abs path string>]`.
- `commands.rs`: `check_prerequisites` becomes an `AppHandle`-taking command that resolves the argv and calls `git_ops::check_prerequisites(&argv)`. `apply_rewrite` resolves the argv and passes it to `rewrite_history`. Add `#[tauri::command] fn install_command_line_tools() -> Result<String, String>` that runs `xcode-select --install` and returns a friendly status (treat the "already installed" nonzero exit as an informative Ok/Err with a clear message).
- `lib.rs`: register `install_command_line_tools` in the invoke handler.
- `tauri.conf.json`: add the vendored dir under `bundle.resources` so `resources/git-filter-repo/**` ships in the `.app`.

**Gate:** `cargo test -p git-it` + `cargo clippy -p git-it -- -D warnings`.

---

### Task 3 — src-tauri integration test: real bundled-script run

**Files:** `src-tauri/tests/filter_repo_bundled.rs` (new; or a gated `#[test]` module)

- Resolve the dev resource path (source tree, `CARGO_MANIFEST_DIR`). Skip gracefully if `python3` is absent so the test never flakes on a bare box.
- Create a temp git repo with ≥2 commits (set `GIT_AUTHOR_*`/`GIT_COMMITTER_*` for determinism). Call `git_core::rewrite::rewrite_history(repo, &[DateMapping…], &RewriteOptions{…}, |_|{})` with prefix `["python3", <vendored script abs path>]`. Assert the targeted commit's committer/author date changed to the mapped value.

**Gate:** `cargo test -p git-it` (runs on macOS + CI).

---

### Task 4 — frontend: types, api, actionable banner (Svelte/TS)

**Files:** `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/components/PrereqBanner.svelte`

- `types.ts`: add `python3: boolean` and `python3Version: string | null` to `PrerequisiteCheck` (match the Rust serde camelCase).
- `api.ts`: add `installCommandLineTools: () => invoke<string>("install_command_line_tools")`.
- `PrereqBanner.svelte`: `ok = check.git && check.python3`. When `!git || !python3`, show one concise line + an **"Install Command Line Tools"** button that calls `api.installCommandLineTools()` and surfaces the returned status inline (and disables while running). Remove the `brew install git-filter-repo` block. If `check.python3 && !check.filterRepo` (packaging bug), show a short diagnostic line, no install button.

**Gate:** `npm run check` + `npm test` + `npm run build`.

---

### Task 5 — docs: README + CLAUDE.md

**Files:** `README.md`, `CLAUDE.md`

- README **Prerequisites**: replace the two `brew install` blocks with: Git It needs `git` and `python3`, both provided by the **Xcode Command Line Tools** (`xcode-select --install`); `git-filter-repo` is **bundled**. Keep the startup-banner mention (now offers a one-click install). Update the "No Python interpreter is bundled" sentence to: the interpreter is host-provided (CLT), and the `git-filter-repo` script is bundled.
- CLAUDE.md: update the note that currently says `git-filter-repo` must be on PATH — it's bundled; the app invokes it with host `python3`.

**Gate:** none (docs).

---

### Task 6 — full gates + adversarial review + manual preview

- Run the entire gate suite green.
- Adversarial multi-lens review (packaging/path-resolution/notarization, argv+prereq correctness & shell-out safety, banner UX & Rust/TS type-sync).
- Manual: a real commit-time edit through the running app to confirm the bundled script drives a rewrite end-to-end.
