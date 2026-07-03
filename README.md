# Git It

A fast, native **macOS git client** — commit graph, branches, merges, rebases, working-copy diffs, and remotes — with **first-class commit-time editing**. Built with [Tauri 2](https://tauri.app) (Rust) and [SvelteKit 5](https://svelte.dev) (runes).

> macOS only. The window uses native vibrancy (frosted-glass panels over the desktop) via `NSVisualEffect`.

## Features

- **Commit graph** — multi-lane history with curved or angular edges, ref badges (branches, tags, remotes, `HEAD`), infinite-scroll loading, and selectable rows.
- **Navigation & refs** — checkout, create/delete branches and tags, fetch — from a right-click menu.
- **Integrate** — merge (plain / `--no-ff`), cherry-pick, and revert, with an in-app **conflict resolver** (use-ours / use-theirs / continue / abort).
- **Rewrite history** — soft/mixed/hard reset, amend, rebase-onto, and **interactive rebase** (reorder / squash / drop / reword), with a configurable **auto-backup** (git bundle) before destructive ops and **one-click undo** plus a reflog browser.
- **Working copy** — file list, whole-file and **per-hunk** stage / unstage / discard, a commit composer, and stash — reached via a synthetic "Uncommitted changes" row in the graph.
- **Diff viewer** — syntax-highlighted (Shiki, bundled offline), unified or split, with per-hunk staging.
- **Remotes** — manage remotes; streamed **pull** (merge or rebase) and **push** (`--force-with-lease`, `--set-upstream`) with live progress, cancel, and an ahead/behind indicator. Credentials are prompted on demand and never stored.
- **Commit-time editing** — select commits and shift them by an **offset**, set an **exact** time, or **compress** a range proportionally into a new window. Opens in an on-demand drawer; previews before it rewrites.
- **Multi-repo** — open several repositories at once, as tabs or a sidebar list (your choice).

## Prerequisites

Git It shells out to the system `git`, so you need it on `PATH`:

```sh
brew install git
```

The **commit-time editing** feature additionally uses [`git-filter-repo`](https://github.com/newren/git-filter-repo):

```sh
brew install git-filter-repo
```

The app shows a startup banner if `git` (or, for time editing, `git-filter-repo`) is missing. No Python interpreter is bundled — `git-filter-repo` is the only Python dependency and runs on the host.

## Install (end users)

1. Open the `.dmg` (built with `npm run tauri build`).
2. Drag **Git It.app** into `/Applications`.
3. First launch: right-click the app → **Open** (Gatekeeper warns for ad-hoc-signed builds).

## Build from source

```sh
npm install
npm run tauri dev      # run the app in development
npm run tauri build    # produce Git It.app + a .dmg under src-tauri/target/release/bundle/
```

Checks and tests:

```sh
npm run check                 # Svelte + TypeScript
npm test                      # Vitest (lane engine + diff parser)
cd src-tauri && cargo check   # Rust
cd src-tauri && cargo test    # Rust tests
```

## Architecture

| Layer | What it does | Files |
|---|---|---|
| Rust commands | typed Tauri command surface | `src-tauri/src/commands.rs`, `lib.rs` |
| Git operations | subprocess wrappers per area | `git_ops.rs` (core), `ops.rs` (nav/refs), `ops_merge.rs` (merge/cherry-pick/revert), `ops_rewrite.rs` (reset/amend/rebase), `ops_worktree.rs` (status/stage/diff/stash), `ops_remote.rs` (pull/push/credentials), `safety.rs` (backups/undo), `rewrite.rs` (timestamps) |
| Graph data | parents, refs, lanes, status | `src-tauri/src/graph.rs`, `types.rs` |
| Lane engine | pure-TS graph layout (fully unit-tested) | `src/lib/graph/` |
| TypeScript API | typed `invoke()` wrappers | `src/lib/api.ts`, `src/lib/gitActions.ts` |
| State | central reactive store (Svelte 5 runes) | `src/lib/store.svelte.ts` |
| UI | one component per panel/dialog/drawer | `src/lib/components/*.svelte` |
| Shell | app layout + chrome | `src/routes/+page.svelte` |

**Security note:** every `git` invocation that takes a user-supplied operand separates options from operands (`--` / `--end-of-options`) so a crafted branch/ref/path can't smuggle in a flag; force-push is always `--force-with-lease`, never bare `--force`; commit messages go through a temp file, never `-m`; and credentials are passed via a generated `GIT_ASKPASS` reading `0600` scratch files, never on `argv` or in app state.

The commit-time **critical path** — building the `git-filter-repo` callback that maps each selected commit to its new epoch/offset — lives in [`src-tauri/src/rewrite.rs`](src-tauri/src/rewrite.rs).

## Distribution

```sh
npm run tauri build -- --bundles dmg
# → src-tauri/target/release/bundle/dmg/Git It_0.1.0_aarch64.dmg
```

Ad-hoc signing (fine for trusted users; Gatekeeper warns on first launch):

```sh
codesign --force --deep --sign - "src-tauri/target/release/bundle/macos/Git It.app"
```

For zero-friction installs, sign with an Apple Developer ID and notarize — `tauri.conf.json` supports `bundle.macOS.signingIdentity` and the notarization environment variables.

## Known limitations

- macOS only (the vibrancy/glass and titlebar handling are macOS-specific).
- `git` must be on `PATH`; `git-filter-repo` too if you use commit-time editing. The startup banner surfaces this.
- Ad-hoc-signed builds trigger a Gatekeeper warning on first launch.
- Very large histories load incrementally but aren't yet DOM-virtualized.

## License

Not yet decided. Until a license is chosen, all rights are reserved — please don't redistribute. A license will be added before any public release.
