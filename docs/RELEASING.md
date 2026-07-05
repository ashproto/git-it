# Releasing Git It

How Git It is built, signed, published, and auto-updated. The pipeline is macOS-only and
two-channel (stable `main` + beta `next`), driven by GitHub Actions and Conventional Commits.
It mirrors the approach used by the sibling Resume Designer app.

> **Prerequisite — make the repo public.** The in-app updater fetches
> `latest.json` and the installer from `https://github.com/ashproto/git-it/releases/...`
> with an **unauthenticated** request. While the repo is private those URLs 404, so the
> auto-updater cannot work until the repository is public. The build pipeline itself runs fine
> either way; only the live update check needs the repo public.

## One-time setup

### 1. Updater signing key (minisign)

Tauri signs every updater artifact with a minisign key and verifies it against the public key
baked into the app.

```bash
npx tauri signer generate -w ~/.tauri/git-it.key
# Set and remember a password when prompted.
```

- Paste the **public key** contents into [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json)
  under `plugins.updater.pubkey`, replacing `REPLACE_ME_AFTER_RUNNING_TAURI_SIGNER_GENERATE`.
- Add two GitHub repo secrets (Settings → Secrets and variables → Actions):
  - `TAURI_SIGNING_PRIVATE_KEY` — the **contents** of `~/.tauri/git-it.key` (not the path).
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you just set.

**This is required before the first release build.** A build with a `pubkey` that doesn't match
the signing key will produce updates that fail verification at install time.

### 2. Apple Developer ID signing + notarization

Required so macOS launches the app without Gatekeeper blocking it — and so the updater accepts
downloaded updates (an un-notarized update is rejected).

| Secret | Value |
| --- | --- |
| `CSC_LINK` | Base64 of your Developer ID Application `.p12` (`base64 -i cert.p12 \| tr -d '\n'`) |
| `CSC_KEY_PASSWORD` | Password set when exporting the `.p12` |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_APP_SPECIFIC_PASSWORD` | App-specific password from appleid.apple.com |
| `APPLE_TEAM_ID` | `847VH25R7U` |
| `APPLE_SIGNING_IDENTITY` | Full identity string, e.g. `Developer ID Application: Your Name (847VH25R7U)` — find it with `security find-identity -v -p codesigning` |

The `build-macos` job validates all eight secrets (these six + the two minisign secrets) and
fails fast if any are missing.

## Branch model & releasing

Feature PRs target **`next`**. Merging one builds a **beta** and publishes it to the rolling
`next` pre-release (a GitHub Release tagged `next`, marked *prerelease*); beta builds point their
updater at `…/releases/download/next/latest.json`.

Cut a **stable** release by promoting `next → main` (a PR). The `guard-main-source` check enforces
that only this repo's `next` — or a `skip-build`-labeled infra PR — merges into `main`. Merging it
builds a versioned `vX.Y.Z` release (`make_latest`), served by `…/releases/latest/download/latest.json`.
GitHub excludes prereleases from `/releases/latest`, so stable users never see betas.

**Version** is computed by [`scripts/ci/compute-version.mjs`](../scripts/ci/compute-version.mjs)
from the latest `v*` tag + Conventional Commits since it: `major` for a `!`/`BREAKING CHANGE`,
`minor` for `feat:`, else `patch`. Beta builds append `-next.<run-number>`.

- **First release** computes to **`0.2.0`** from the current `0.1.0` + a `feat` in range. Override
  with the `version` `workflow_dispatch` input if you want a different starting version.
- **Skip a build** on a merge: add the **`skip-build`** label to the PR before merging.
- **Force / manual build**: run the **Release Desktop App** workflow via `workflow_dispatch`,
  optionally passing a `version` override.

> A freshly published release is briefly asset-less — the signed installers and `latest.json`
> attach a few minutes later once the build jobs finish — so an in-app update check during that
> window degrades gracefully (the updater just reports "up to date" until the manifest lands).

## In-app updater behavior

- On launch, `startupUpdateCheck()` runs (unless **Settings → Updates → Check for updates
  automatically** is off, or it's a dev build). A 30-minute background poll runs while the app is open.
- The check routes through the Rust `check_update_on_channel` command for the chosen channel
  (Settings → Updates → **Stable / Beta**). The beta channel is a *superset*: it checks both the
  `next` and stable manifests and offers the highest version, so beta users still get a newer
  stable release. Both channels are signed with the same key.
- Flow: prompt to download → progress on the status bar → prompt to **Restart Now** →
  `@tauri-apps/plugin-process` `relaunch()` boots into the installed update. A 10-second watchdog
  surfaces a signing/notarization hint if the relaunch-into-installer step never starts.
- The macOS app menu gains **Settings…** and **Check for Updates…** items (Windows/Linux reach
  these through the in-app Settings panel).

## Testing updates end-to-end

1. Publish a signed build (merge to `next` for beta, or promote `next → main` for stable — or run
   `workflow_dispatch`). Install it.
2. Publish a newer build the same way.
3. Reopen the older app; on startup it should prompt "Update available", show download progress,
   prompt "Restart Now", and relaunch into the new version. Confirm the version in Settings → Updates.

Requires the repo to be **public** and all eight secrets set.

## Windows (deferred)

Git It is macOS-only today. Adding Windows later is a scoped port: `#[cfg(target_os = "macos")]`-gate
`src-tauri/src/openwith.rs` (and hide the "Open With" submenu off-mac), add a Windows `write_600`
in `crates/git-core/src/ops_remote.rs`, handle the `#!/bin/sh` `GIT_ASKPASS`/`GIT_SEQUENCE_EDITOR`
scripts and `git-filter-repo` availability on Windows, verify the window chrome, then re-add the
`build-windows` job, the `windows-x86_64` platform in `latest.json`, and the `nsis` bundle target.
Verify on real Windows before shipping to the updater.
