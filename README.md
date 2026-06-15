# Git It

A small macOS desktop app for batch-editing git commit timestamps via `git-filter-repo`. Tauri 2 (Rust) + SvelteKit.



## Prerequisites

The app shells out to `git` and `git-filter-repo`. Install once:

```sh
brew install git git-filter-repo
```

The app shows a banner at startup if either is missing.

## Install (end users)

1. Open the `.dmg` (built with `npm run tauri build`).
2. Drag `Git It.app` into `/Applications`.
3. First launch: right-click the app → **Open** (Gatekeeper warns for ad-hoc signed builds).

## Features

- Load N most recent commits or a custom `git log` range
- Three edit modes:
  - **Offset** — shift selected commits by ±days/hours/minutes/seconds
  - **Exact time** — set selected commits to a specific local time
  - **Compress range** — proportionally remap selected commits into a new time window
- Live preview before rewriting (queued in "New date" column)
- Auto bundle backup before each rewrite
- View/delete safety refs (`refs/original/*`) left behind by `git-filter-repo`
- Streamed `git-filter-repo` output in the log panel

## Develop

```sh
cd git-it
npm install
npm run tauri dev
```

- `npm run check` — Svelte + TS check
- `cd src-tauri && cargo check` — Rust check
- `npm run tauri build` — produce `.app` + `.dmg` in `src-tauri/target/release/bundle/`

## Architecture

| Layer | What it does | Files |
|---|---|---|
| Rust commands | git/filter-repo subprocess, file I/O | `src-tauri/src/commands.rs`, `git_ops.rs`, `rewrite.rs` |
| TypeScript API wrapper | typed `invoke()` calls | `src/lib/api.ts` |
| Svelte state | central reactive store (Svelte 5 runes) | `src/lib/store.svelte.ts` |
| Svelte UI | one component per panel | `src/lib/components/*.svelte` |
| Main page | layout shell | `src/routes/+page.svelte` |

The "critical path" — generating the Python `--commit-callback` payload — lives in [src-tauri/src/rewrite.rs](src-tauri/src/rewrite.rs). It produces a string that's structurally identical to [legacy/run_filter_repo_debug.sh](../legacy/run_filter_repo_debug.sh) and passes it to `git-filter-repo` as a CLI argument. No Python interpreter is bundled in the app — `git-filter-repo` itself is the only Python dependency, installed via Homebrew on the host.

## Distribution to colleagues

```sh
npm run tauri build -- --bundles dmg
# → src-tauri/target/release/bundle/dmg/Git It_0.1.0_aarch64.dmg
```

For ad-hoc signing (sufficient for trusted colleagues; Gatekeeper will warn on first launch):

```sh
codesign --force --deep --sign - "src-tauri/target/release/bundle/macos/Git It.app"
```

For zero-friction install, sign with an Apple Developer ID and notarize. Tauri's `tauri.conf.json` supports the `bundle.macOS.signingIdentity` and notarization environment variables.

## Known limitations

- `git-filter-repo` must be on `PATH`. The startup banner makes this visible.
- Ad-hoc-signed builds trigger a Gatekeeper warning on first launch.
