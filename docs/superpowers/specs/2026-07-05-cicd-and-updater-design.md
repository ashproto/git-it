# CI/CD + In-App Updater — Design Spec

**Date:** 2026-07-05
**Goal:** Give Git It the same GitHub CI/CD pipeline and in-app auto-updater that Resume-Designer has, adapted to Git It's stack (Tauri 2 + SvelteKit 5, cargo workspace) and constraints.

## Decisions (locked with user)

1. **Update-feed hosting:** Literal exact copy of Resume-Designer — the updater points at **git-it's own GitHub Releases** (`https://github.com/ashproto/git-it/releases/...`). The repo is currently private; **the user will make it public before releasing**, so unauthenticated updater fetches will resolve. Until then, the pipeline is complete but the live updater endpoint 404s (documented, non-blocking).
2. **Platforms:** **macOS only** for now (aarch64 + x86_64, signed + notarized). Git It's Rust doesn't compile on Windows today (`openwith.rs` is Cocoa/`objc2`; `git-core::ops_remote::setup_askpass` calls the `#[cfg(unix)]` `write_600`) and several runtime paths are unix-only (`GIT_ASKPASS`/`GIT_SEQUENCE_EDITOR` `#!/bin/sh` scripts, `git-filter-repo`, mac window chrome). Windows is deferred to a separate, verified port project. Recorded in "Windows deferral" below.
3. **Release model:** **Two-channel** (exact RD copy) — `main` = stable, `next` = beta. Includes `guard-main-source` (only `next`→`main` promotions) and an in-app Stable/Beta channel toggle.

## Reference mapping (Resume-Designer → Git It)

| Concern | Resume-Designer | Git It |
| --- | --- | --- |
| App dir | `resume-designer/` subdir | **repo root** (`working-directory: .`, `projectPath: .`) |
| Frontend | React + Vite, `../dist` | SvelteKit 5 (runes), adapter-static, `../build` |
| Lint gate | ESLint | **`npm run check`** (svelte-check; no ESLint in repo) |
| Frontend tests | Vitest | Vitest (`npm test`) |
| Rust | single `src-tauri` crate | **cargo workspace** (`src-tauri` + `crates/git-core` + `crates/agent`) |
| Rust CI scope | `cargo check`/clippy on `src-tauri` | **`cargo test` + clippy on `-p git-core -p git-it`** (agent excluded — its live-relay tests need Convex env; run locally) |
| Version files | `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` | same three (root `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`) |
| Updater backend | `src-tauri/src/commands/updater.rs` | **`src-tauri/src/updater.rs`** (Git It uses flat modules, not a `commands/` dir) |
| Updater frontend | `src/native.js` + `src/updateFlow.js` (React/Sonner) | **`src/lib/updater.svelte.ts`** (Svelte 5; StatusBar + `dialogs.confirm`) |
| Node in CI | 24 | 24 |

## Component 1 — CI/CD workflows

Copy RD's `.github/` structure, dropping all Windows and website (`deploy-pages.yml`) pieces.

### `.github/workflows/ci.yml` (PR checks → `main`, `next`)
- Job `checks` (ubuntu): `npm ci` → `npm run check` (svelte-check) → commitlint (`--from base.sha --to head.sha`) → `npm test` → `npm run build` → `npm audit --omit=dev --audit-level=high`.
- Job `rust-check` (macos-latest — Git It's Rust is mac-only): `npm ci` → `npm run build` (so tauri-build finds `../build`) → install Rust + clippy → `Swatinem/rust-cache` (workspace = repo root) → `cargo test -p git-core -p git-it` → `cargo clippy -p git-core -p git-it -- -D warnings`.
- All `working-directory` at repo root; cargo manifest at `src-tauri/Cargo.toml` (or `-p` from root).

### `.github/workflows/release.yml` (push → `main`/`next`, or `workflow_dispatch`)
- `decide` job (ubuntu): skip-build label gate + `scripts/ci/compute-version.mjs` → outputs `skip`, `channel`, `version`, `tag`, `previous_tag`.
- `build-macos` job (macos-latest, matrix `aarch64`/`x86_64`): checkout → node → Rust (matrix target) → rust-cache → `npm ci` → apply CI version to the three files → (next channel only) rewrite `tauri.conf.json` updater endpoint to `.../releases/download/next/latest.json` → **validate 8 signing secrets, fail fast** → `tauri-apps/tauri-action@v0.6.2` (`projectPath: .`, `args: --target <target>`) with Apple + Tauri signing env → normalize artifact filenames (strip spaces / arch-suffix the `.app.tar.gz`) → upload `dmg` + `app.tar.gz` + `.sig`.
- **No `build-windows` job.**
- `release` job (ubuntu): download both mac artifacts → group changelog (`gen-changelog.mjs`) → GitHub Models AI rewrite (best-effort, fallback to grouped) → assemble `latest.json` with **only `darwin-aarch64` + `darwin-x86_64`** platforms → delete existing tag assets → publish via `softprops/action-gh-release@v3` (`prerelease`/`make_latest`/`generate_release_notes` keyed on channel).

### `.github/workflows/guard-main-source.yml`
- Verbatim: PRs into `main` must come from this repo's `next` (or be `skip-build`-labeled).

### `.github/release.yml`
- Verbatim category config (labels → sections) for GitHub's auto-generated notes.

### `scripts/ci/compute-version.mjs`
- Verbatim. Reads `../../package.json` (resolves to repo-root `package.json` since the script sits at `scripts/ci/`). Semver from latest `v*` tag + Conventional Commits; `-next.<run>` for beta.

### `scripts/ci/gen-changelog.mjs`
- Copy; change the `## Resume Designer` heading → **`## Git It`**, and replace `AREA_NAMES` with Git It's commit scopes (`graph`, `remotes`, `tabs`, `github`, `pr`, `working-copy`, `diff`, `search`, `ff`, `menu`, `settings`, `desktop`, …). Sections (`feat`/`fix`/`perf`) unchanged.

### `commitlint.config.js` + deps
- Copy verbatim (extends config-conventional, ignores dependabot). Add `@commitlint/cli` + `@commitlint/config-conventional` to `devDependencies`. Git It's history is already Conventional.

## Component 2 — In-app updater (Rust)

### `src-tauri/src/updater.rs` (new)
- Near-verbatim copy of RD's `commands/updater.rs`, with Git It endpoints:
  - `STABLE_ENDPOINT = https://github.com/ashproto/git-it/releases/latest/download/latest.json`
  - `BETA_ENDPOINT   = https://github.com/ashproto/git-it/releases/download/next/latest.json`
- Exposes `PendingUpdate` state, `check_update_on_channel(app, channel, pending) -> Option<UpdateInfo>` (beta = superset of both endpoints, max-version wins, per-endpoint failure non-fatal), and `install_pending_update(pending, on_event: Channel<DownloadEvent>)` (streams `Started{contentLength}` / `Progress{chunkLength}` / `Finished`).

### `src-tauri/src/lib.rs` (modify)
- In a `.setup()` closure, `#[cfg(desktop)]`: register `tauri_plugin_updater::Builder::new().build()` and `app.manage(updater::PendingUpdate::default())`.
- `#[cfg(target_os = "macos")]`: build a default `Menu`, insert **"Settings…"** and **"Check for Updates…"** into the app menu under About, and `on_menu_event` emit `menu:open-settings` / `menu:check-updates`. (No existing menu to collide with — confirmed.)
- Add `mod updater;` and register `updater::check_update_on_channel` + `updater::install_pending_update` in `generate_handler!` behind `#[cfg(desktop)]`.

### `src-tauri/Cargo.toml` (modify)
- Add desktop-only deps:
  ```toml
  [target."cfg(not(any(target_os = \"android\", target_os = \"ios\")))".dependencies]
  tauri-plugin-updater = "2"
  tauri-plugin-process = "2"
  semver = "1"
  ```

### `src-tauri/capabilities/default.json` (modify)
- Add permissions: `"updater:default"`, `"process:default"`, `"process:allow-restart"`.

### `src-tauri/tauri.conf.json` (modify)
- `plugins.updater`: `active: true`, `endpoints: [STABLE_ENDPOINT]`, `pubkey: "<REPLACE_AFTER_KEYGEN>"`.
- `bundle.createUpdaterArtifacts: true`.
- `bundle.targets`: `["dmg", "app"]` (was `"all"` — drop nsis/appimage; mac-only).
- `bundle.macOS`: add `"hardenedRuntime": true`, `"entitlements": "Entitlements.plist"`, `"providerShortName": null`; keep `minimumSystemVersion: "12.3"`.

### `src-tauri/Entitlements.plist` (new)
- Minimal hardened-runtime entitlements (matches RD): allow JIT / unsigned-executable-memory as needed by the webview. Required for notarization under hardened runtime.

## Component 3 — In-app updater (frontend, Svelte port)

### `src/lib/store.svelte.ts` (modify)
- Add `updateChannel: "stable" | "beta"` (default `"stable"`) and `autoUpdateCheck: boolean` (default `true`), each following the existing `pullRebase` dual-persist pattern exactly: `*_KEY` + `*_STORE_KEY` consts, `loadSync*()`, `$state` + `*Touched` guard, async hydrate from `getStore()`, `persist*()` (Tauri store fire-and-forget + localStorage fallback), and getter/`set*()`.

### `src/lib/api.ts` (modify)
- Add to the `api` object:
  - `checkUpdateOnChannel(channel) => invoke<UpdateInfo | null>("check_update_on_channel", { channel })`.
  - `installPendingUpdate(onEvent) => { const ch = new Channel<DownloadEvent>(); ch.onmessage = onEvent; return invoke<void>("install_pending_update", { onEvent: ch }); }` (`Channel` already imported).
- Add `UpdateInfo`/`DownloadEvent` types to `src/lib/types.ts`.

### `src/lib/updater.svelte.ts` (new) — the flow module
Port of RD's `native.js` + `updateFlow.js` state machine, simplified for Git It's surfaces:
- `startupUpdateCheck()` — desktop + non-DEV only; seeds channel from build type (pre-release version → beta); gated on `autoUpdateCheck`; calls `checkForUpdates("startup")`.
- `checkForUpdates(source)` — DEV guard; re-entrancy guard; drives **StatusBar** via `appState.setBusyOp("Checking for updates")` / `appState.status`; on available → `dialogs.confirm({ title, message: notes summary, confirmLabel: "Download" })`; on accept → `installPendingUpdate` with progress → StatusBar percent; on finish → `dialogs.confirm({ title: "Update ready", confirmLabel: "Restart Now" })` → `@tauri-apps/plugin-process` `relaunch()` (with a 10 s watchdog surfacing a signature/notarization hint). Signature-failure errors get RD's friendly message.
- `manualCheckForUpdates()` — Settings button + menu entry point.
- Background 30-min poll (notify-only via StatusBar/UndoBar action), respecting `autoUpdateCheck`, never in DEV.
- **Simplification vs RD:** no pre-relaunch durability gate (RD guards unsaved resume data; Git It only has fire-and-forget settings/repo-view state — acceptable to relaunch directly; best-effort `store.save()` flush is optional).

### `src/lib/components/SettingsPanel.svelte` (modify)
- New "Updates" group: `.opt` checkbox "Check for updates automatically" (`autoUpdateCheck`), `.seg-row` Stable/Beta (`updateChannel`), a "Check for Updates" button (→ `manualCheckForUpdates()`), and a muted "Version X.Y.Z" line (`@tauri-apps/api/app` `getVersion()`).

### `src/routes/+page.svelte` (modify)
- In the existing `onMount`, when in Tauri: `void startupUpdateCheck()` and register menu-event listeners — `menu:check-updates` → `manualCheckForUpdates()`, `menu:open-settings` → open the settings panel (`settingsPanel` controller). Unlisten on destroy.

### `package.json` (modify)
- `dependencies`: add `@tauri-apps/plugin-updater` `^2`, `@tauri-apps/plugin-process` `^2`.
- `devDependencies`: add `@commitlint/cli` + `@commitlint/config-conventional`.

## Component 4 — Signing, secrets, keys (user prerequisites)

Documented in a new **`docs/RELEASING.md`** (Git It's analogue of RD's `TAURI.md`), covering:
- **Minisign keypair:** `npx tauri signer generate -w ~/.tauri/git-it.key`; paste the public key into `tauri.conf.json` `plugins.updater.pubkey`; add `TAURI_SIGNING_PRIVATE_KEY` (key contents) + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets. **Prerequisite for the first release build.**
- **Apple Developer ID + notarization:** the 6 secrets (`CSC_LINK`, `CSC_KEY_PASSWORD`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID` = `847VH25R7U`, `APPLE_SIGNING_IDENTITY`). Without notarization the updater rejects downloaded updates.
- **Branch model / releasing:** feature PRs → `next` (beta on the rolling `next` pre-release); promote `next`→`main` for a stable `vX.Y.Z`; `skip-build` label to skip; `workflow_dispatch` with `version` override. First stable computes to `0.2.0` from the current `0.1.0` + a `feat` in range (overridable).
- **Make the repo public** before the updater endpoint works.

## Windows deferral (recorded, out of scope here)

To add Windows later as a verified port: `#[cfg(target_os = "macos")]`-gate `openwith.rs` (module, deps, `apps_for_file` registration) and hide the "Open With" submenu off-mac; add a Windows `write_600` (plain create+write, no mode) in `ops_remote.rs`; handle `#!/bin/sh` `GIT_ASKPASS`/`GIT_SEQUENCE_EDITOR` on Windows (Git-for-Windows sh) and `git-filter-repo` availability; verify window chrome; re-add the `build-windows` job + `windows-x86_64` platform in `latest.json` + `nsis` bundle target. Verify on real Windows before shipping to the updater.

## Testing / verification

- `npm run check` (0 errors) + `npm test` + `cargo test -p git-core -p git-it` green.
- `compute-version.mjs` + `gen-changelog.mjs` get unit tests (RD ships gen-changelog logic importable; add a Git It vitest for the area mapping + a node smoke test for compute-version).
- YAML lint / `actionlint` pass on the three workflows (mentally or via `actionlint` if available).
- Local `npm run tauri build` produces `dmg` + `app.tar.gz` + `.sig` once the minisign key exists (documented; can't be fully exercised in CI-less local without the key).
- End-to-end updater test (install older signed build → publish newer → auto-update) requires the repo public + secrets set — documented as the acceptance test, run by the user.

## Out of scope
- Windows/Linux builds (deferred).
- A marketing website / GitHub Pages (RD's `deploy-pages.yml` is not copied).
- Auto-generating the minisign keypair or entering any secret (user does this in GitHub settings).
