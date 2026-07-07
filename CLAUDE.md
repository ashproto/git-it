# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Git It** — a native macOS git client (commit graph, branches, merges, rebases, working-copy
diffs, remotes) with first-class commit-time editing. Also hosts the headless **agent** that
lets the sibling iOS app drive local git remotely. Tauri 2 (Rust) + SvelteKit 5 (runes).
macOS only; window uses native `NSVisualEffect` vibrancy (glass panels). Private; `UNLICENSED`
— licensing is intentionally undecided, do not add a LICENSE.

## Commands

Run from the repo root.

| Task | Command |
|------|---------|
| Run the app (dev) | `npm run tauri dev` |
| Production `.app` | `npm run tauri build` |
| Type gate (Svelte/TS) | `npm run check` |
| Frontend unit tests | `npm test` (watch: `npm run test:watch`) |
| Single frontend test | `npx vitest run src/lib/dates.test.ts` (or `-t "name"`) |
| Rust tests (all crates) | `cargo test` |
| Rust tests (one crate) | `cargo test -p git-core` · `-p git-it-agent` · `-p git-it` |

**Full green gate before claiming done:** `npm run check` (0 errors) + `npm test` + `cargo test`.

Frontend-only changes hot-reload under `npm run tauri dev`. **Vitest and the browser preview
cannot exercise Tauri-only paths** (anything behind `invoke`: native dialogs, working-copy
staging, reword, real git). Those require a running `tauri dev`/`build`.

Requires system `git` and `python3` on `PATH` (both ship with the Xcode Command Line Tools). Commit-time editing runs the **bundled** `git-filter-repo` script (`src-tauri/resources/git-filter-repo/`) via the host `python3` — it is not a `PATH` binary.

## Architecture

Three-layer split; a **cargo workspace** (root `Cargo.toml`, members below) with a SvelteKit frontend.

- **`crates/git-core`** (package `git-core`) — the **Tauri-free** git engine and the single source
  of truth. All git logic lives here: `graph.rs` (lane/graph engine), `ops*.rs` (nav, merge,
  remote, rewrite, worktree), `rewrite.rs` (commit-time editing via `git-filter-repo`), `safety.rs`
  (configurable pre-op auto-backup as git bundles + undo), `github/` (`gh` CLI wrappers for the
  GitHub screen). Depends on nothing Tauri.
- **`src-tauri`** (package `git-it`, lib `git_it_lib`) — the desktop shell. `commands.rs` exposes
  `git-core` to the frontend as Tauri commands; `fswatch.rs` watches the working tree; `lib.rs`
  wires it up.
- **`crates/agent`** (package/bin `git-it-agent`) — the headless CLI + **phone relay**. Reuses
  `git-core` (zero Tauri deps) and connects to the Convex relay so the iOS app can drive local git.
  `relay.rs` (Convex client + message loop), `auth.rs`, `repos.rs`, `crypto/` (HPKE/Ed25519 E2E).
- **`src/`** — SvelteKit 5 SPA (static adapter, single `+page.svelte` route). `lib/store.svelte.ts`
  is central runes state; `lib/gitActions.ts` calls Tauri commands; `lib/graph/` renders the lane
  graph; `lib/diff/` is Shiki-highlighted diffs; `lib/github/` is the GitHub dashboard screen.
  **Pure logic modules are unit-tested with vitest** and must stay Tauri-free: `dates.ts`,
  `fileTree.ts`, `refTree.ts`, `commitBody.ts`, graph windowing (`.test.ts` alongside each).

**Data flow:** Svelte `invoke` (`@tauri-apps/api`) → `src-tauri/commands.rs` → `git-core` →
shells out to system `git` (and `gh` for the GitHub screen).

## Conventions & gotchas

- **Shell-out safety (hard rule):** when passing user-controlled operands (refs, paths) to `git`,
  always place `--` / `--end-of-options` before them to prevent flag injection. This is the
  established pattern across `crates/git-core/src/ops*.rs` — match it in any new git shell-out.
- **Async Tauri commands:** any command that calls a slow external process (`gh`, network `git`)
  MUST be `#[tauri::command(async)]`. A synchronous command runs on the UI thread and freezes the
  whole app during the call (this exact bug froze the GitHub screen).
- Business logic that can be pure belongs in a testable `src/lib/*.ts` module (vitest), not inside
  a `.svelte` component — the graph/date/tree logic is covered this way.

## Sibling repo (`../git-it-ios`)

The iOS app + the shared **Convex relay backend** live in `../git-it-ios`. Two hard couplings:

- **KAT vectors:** the agent's canonical vectors in `crates/agent/tests/vectors/` MUST stay
  byte-identical to `git-it-ios/Tests/Resources/*.json`. `git-it-ios/scripts/check-kat-sync.sh`
  gates this — if you change either side, run it.
- **Convex deployment:** the agent loads `CONVEX_URL` / `CONVEX_SITE_URL` via `dotenvy` from
  `~/.config/git-it/.env` — a **fixed** path, see `crates/agent/src/repos.rs::env_path` — to reach
  the same Convex deployment the iOS app uses. (A gitignored `.env.local` sits at the repo root as a
  convenience copy, but the agent does not read it.) The Convex functions themselves live in
  `git-it-ios/backend/`, not here.
