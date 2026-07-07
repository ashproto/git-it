# Bundle git-filter-repo + Actionable Prerequisites — Design

**Goal:** On a typical developer Mac, require **zero manual installs** to use Git It (including commit-time editing). On a bare Mac, require **one action**.

## Background

Today the README asks users to `brew install git` and `brew install git-filter-repo`.
- `git` is needed for everything; `git-filter-repo` is needed **only** for commit-time editing.
- The app shells out to `git-filter-repo` as a `PATH` binary at `crates/git-core/src/rewrite.rs:133` (`Command::new("git-filter-repo")`), and `crates/git-core/src/path_setup.rs` prepends Homebrew bins so a Finder-launched app can find it.
- `crates/git-core/src/git_ops.rs::check_prerequisites` probes `git --version` and `git-filter-repo --version` on `PATH`.
- `src/lib/components/PrereqBanner.svelte` shows static "brew install" text when `!git || !filterRepo`.

Key facts established during design:
- `git-filter-repo` is a **single, self-contained Python script** (~206 KB), shebang `#!/usr/bin/env python3`, **MIT-licensed** (only its test harness is GPL; we do not vendor that).
- `python3 <script> …` runs it correctly as a plain script (verified: `--version` prints the version).
- `python3` ships with the **Xcode Command Line Tools** — the same package that provides `git`. So a Mac that has `git` also has `python3`; no new dependency is introduced.

## Design

### Part 1 — Bundle git-filter-repo

- **Vendor** the single script into `src-tauri/resources/git-filter-repo/` (`git-filter-repo` + `COPYING.mit` + `README.md`), shipped inside the `.app` via `tauri.conf.json → bundle.resources`.
- **Invoke** it as `python3 <resolved-script>` instead of a `PATH` binary. Uses host `python3`.
- **git-core stays Tauri-free.** `rewrite_history` and the filter-repo prerequisite probe take a new **argv-prefix parameter** (`&[String]`, e.g. `["python3", "/…/Resources/git-filter-repo/git-filter-repo"]`). The Tauri layer (`src-tauri`) resolves the concrete path and passes it in. git-core no longer hardcodes `Command::new("git-filter-repo")`.
- **Path resolution (src-tauri):** in the built app, resolve via Tauri's resource-dir path resolver; under `npm run tauri dev`, fall back to the source-tree `src-tauri/resources/git-filter-repo/git-filter-repo`. Encapsulated in one helper that returns the argv prefix `["python3", <abs script path>]`.
- **Prerequisite check** changes from "is `git-filter-repo` on `PATH`?" to: (a) `python3` present? (b) does the bundled script run (`python3 <script> --version` exits 0)? `PrerequisiteCheck` gains `python3: bool` + `python3_version: Option<String>`; `filter_repo`/`filter_repo_version` now reflect the **bundled** script.

### Part 2 — Actionable prerequisite banner

- New Tauri command `install_command_line_tools` → runs `xcode-select --install` (opens Apple's installer, which delivers **both** `git` and `python3`). Returns a human-readable status string (including the benign "already installed / in progress" case).
- `PrereqBanner.svelte`: when `git` **or** `python3` is missing, render an **"Install Command Line Tools"** button that calls the command, plus concise explanatory text. Remove the `brew install git-filter-repo` line entirely. If `python3` is present but the bundled script somehow fails to run (packaging bug), show a short diagnostic instead of an install prompt.

## Components / files

- **Add:** `src-tauri/resources/git-filter-repo/{git-filter-repo, COPYING.mit, README.md}` (done during setup).
- `src-tauri/tauri.conf.json` — add `bundle.resources`.
- `src-tauri/src/` — a small resolver (e.g. `filter_repo.rs`) returning the argv prefix (prod resource dir vs dev source tree); wire into `commands.rs`.
- `crates/git-core/src/rewrite.rs` — add argv-prefix param; build `Command::new(&argv[0]).args(&argv[1..])`; drop the "brew install" hint in the spawn-error message.
- `crates/git-core/src/git_ops.rs` — `check_prerequisites` accepts the argv prefix (probe `python3` + bundled script); populate new fields.
- `crates/git-core/src/types.rs` + `src/lib/types.ts` — add `python3` / `python3_version` to `PrerequisiteCheck`.
- `src-tauri/src/commands.rs` — resolve argv and pass to `check_prerequisites` and the rewrite command; add `install_command_line_tools`; register it in `lib.rs`.
- `src/lib/components/PrereqBanner.svelte` + `src/lib/api.ts` — actionable button; bind new command.
- `README.md` + `CLAUDE.md` — Prerequisites shrink to "needs Xcode Command Line Tools (`git` + `python3`)"; note git-filter-repo is bundled; update the "No Python interpreter is bundled" line to clarify only the interpreter is host-provided.
- `path_setup.rs` — unchanged (still helps a Finder-launched app find `git`/`python3`).

## Data flow

- **Startup:** `ensure_homebrew_path()` → resolve argv prefix → `check_prerequisites(argv)` → `{git, python3, filterRepo}` → `PrereqBanner` (with install button when needed).
- **Rewrite:** `ApplyPanel` → `apply_rewrite` → resolve argv → `rewrite_history(…, argv)` → `python3 <bundled> --commit-callback … --force --partial …`.

## Error handling

- Bundled script missing (packaging bug) → `filterRepo=false` + banner diagnostic; rewrite fails with a clear message.
- `python3`/`git` missing → banner "Install Command Line Tools" button; surface `xcode-select`'s own output (incl. already-installed).
- Signature/verification and `already_ran` handling in `rewrite.rs` are untouched.

## Testing

- **Unit (git-core):** assert `rewrite_history` builds the expected argv from a given prefix + options (no process spawn). Keep existing `generate_callback` tests.
- **Integration (src-tauri, new):** resolve the dev resource path, create a temp git repo, run `rewrite_history` with the `["python3", <vendored script>]` prefix, assert a target commit's date changed. This exercises the real bundled-script mechanic and runs in CI (`macos-latest` has `python3`) — closing a gap where the rewrite path had no automated test.
- **Manual/preview:** a real commit-time edit through the running app (Tauri-only UI path), verified via the preview tooling.

## Non-goals (YAGNI)

- Not bundling `git` itself (too heavy; CLT provides it).
- Not bundling a Python interpreter (CLT provides `python3`).
- Not reimplementing filter-repo natively.
- No "use my own system git-filter-repo" override — the bundled script is the single source of truth.
