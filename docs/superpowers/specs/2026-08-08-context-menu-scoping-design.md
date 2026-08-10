# Scoping the native context menu

**Date:** 2026-08-08
**Status:** approved
**Branch:** `fix/scope-native-context-menu` off `next`

## Problem

`src/routes/+page.svelte` installs a window-level handler that cancels every native context
menu in the app:

```ts
const suppressNativeContextMenu = (event: MouseEvent) => event.preventDefault();
window.addEventListener("contextmenu", suppressNativeContextMenu);
```

It arrived in `15e51c3` (2026-07-17). The app supplies its own menu for a handful of row
types — commit rows, sidebar refs, worktrees, working-copy files — and everywhere else the
handler only takes functionality away. Right-clicking a commit-message box, a PR review
textarea, diff code, or rendered Markdown yields nothing: no Copy, no Paste, no Look Up, no
spelling suggestions. On macOS that reads as broken.

Reported by Codex review on PR #26 as P2. The code is not part of that PR, so it is being
fixed on its own branch.

## What the investigation changed

Three findings moved this away from the obvious framing.

**The original intent is already satisfied by the build configuration.** The handler was
presumably written to hide WebKit's developer menu (Reload / Inspect Element), which looks
unprofessional in a shipped app. But `src-tauri/Cargo.toml` declares
`tauri = { version = "2", features = ["macos-private-api"] }` — no `devtools` feature. Tauri
enables the inspector on debug builds only, so **"Inspect Element" never appears in a release
build**. The dev menu is a dev-build-only phenomenon, and no `tauri.conf.json` option is needed
(or exists) to suppress it.

**The global handler is not what makes the custom menus work.** Every custom handler already
calls `preventDefault()` itself: `GraphHistory.svelte:247`, `Sidebar.svelte:238`,
`WorktreePanel.svelte:67`, and the `WorkingCopyView` / `RefTree` callbacks. Removing the global
handler would not produce two menus stacked on a commit row.

**The app already decides where text matters.** `+page.svelte` sets a no-select baseline and
opts back in on a curated list:

```css
:global(body) { user-select: none; }
:global(input), :global(textarea), :global([contenteditable="true"]),
:global(.diff-cell), :global(.md), :global(.body-msg), :global(.sha),
:global(.selectable), :global(.selectable *) { user-select: text; }
```

That list is, almost by definition, the set of places a native context menu is useful. The fix
should defer to it rather than introduce a second, independently-maintained notion of
"editable or selectable".

## Design

Gate the suppression on the computed `user-select` value at the event target:

```ts
const suppressNativeContextMenu = (event: MouseEvent) => {
  const el = event.target as Element | null;
  // The native menu earns its place exactly where text is selectable — read the
  // `user-select` allowlist below rather than keeping a second list in sync with it.
  const style = el && getComputedStyle(el);
  if (style && (style.webkitUserSelect || style.userSelect) !== "none") return;
  event.preventDefault();
};
```

`user-select` inherits, so a descendant of `.md` or `.selectable` computes to `text` and is
allowed without enumerating descendants. `DiffView`'s line-number gutter sets its own
`user-select: none`, so it re-suppresses automatically — gutter-drag line staging never offers
a text menu. Both the prefixed and unprefixed properties are read: `minimumSystemVersion` is
12.3 (Monterey / Safari 15.x), where unprefixed `user-select` support is not guaranteed.

### Resulting behavior

| Right-click target | Before | After |
|---|---|---|
| Commit row, sidebar ref, worktree, working-copy file | app menu | app menu (unchanged) |
| Toolbar, panel header, labels, tabs | nothing | nothing (unchanged) |
| Commit-message input, review textarea | nothing | native Copy / Paste / spelling |
| Diff code (`.diff-cell`) | nothing | native Copy / Look Up |
| Rendered Markdown (`.md`), commit body, SHA | nothing | native Copy / Look Up |
| Diff line-number gutter | nothing | nothing (own `user-select: none`) |

### Rejected alternatives

**An explicit selector constant** mirroring the CSS list, extracted as a pure predicate in
`src/lib/`. More obvious to read and unit-testable, but it duplicates the allowlist. The two
drift the first time someone adds a selectable class to the CSS and not to the TS, and the
failure is silent — a text area that quietly stops offering a menu.

**Deleting the global handler entirely** and relying on components to self-suppress. The most
surgical option (three lines) and the truest to native behavior, since all custom handlers
already `preventDefault()`. Rejected because WKWebView's default menu would then appear on
chrome, and what that menu contains in a *release* build (no inspector, everything
`user-select: none`) cannot be confirmed from a debug build — the two differ precisely in the
inspector item. Possibly empty and therefore invisible, possibly a stray "Reload" / "Services"
on every toolbar. Not worth the uncertainty for three lines.

## Scope

One handler in `src/routes/+page.svelte`. No component changes, no CSS changes, no Tauri
config or Cargo feature changes.

## Verification

Automated gates cannot reach this. Whether a native menu appears is a WKWebView decision made
after `preventDefault()` is or is not called, and no test environment renders one.
`npm run check`, `npm test`, and `cargo test` confirm only that nothing else broke.

By hand, in a bundled debug `.app` (see the `gitit-tauri-gui-automation` notes — `tauri dev`
runs a bare binary with no bundle identifier and is invisible to screenshots):

1. Right-click a commit-message box — Copy / Paste / spelling appear.
2. Right-click diff code — Copy / Look Up appear.
3. Right-click a commit row in the graph — the app's own menu appears, and no WebKit menu
   leaks in behind it.
4. Right-click a toolbar or panel header — nothing appears.
5. Right-click the diff line-number gutter — nothing appears.

Note that a debug build *does* carry the inspector, so "Inspect Element" showing up in step 1
or 2 is expected there and absent from the shipped app.
