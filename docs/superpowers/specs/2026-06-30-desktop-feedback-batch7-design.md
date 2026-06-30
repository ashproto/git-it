# Desktop feedback batch 7 — design

**Date:** 2026-06-30
**Branch:** `batch7-desktop-feedback` (off `main` @ `6a3c240`)
**Scope:** Five real-world desktop "Git It" fixes/UX items from running the app. Each is an
independent slice with its own gates + adversarial review. iOS / Phase 1b stay parked.

---

## Item 2 — Fix the blank unstaged new-file in Local Changes (BUG)

### Root cause (confirmed by reproduction)
`working_changes` in [`crates/git-core/src/ops_worktree.rs:12`](../../../crates/git-core/src/ops_worktree.rs) runs
`git status --porcelain=v2 -z`. With git's default `-unormal`, a brand-new **untracked directory** is
collapsed to a single entry `? newdir/` (trailing slash) instead of listing its files. Downstream:
- The Local Changes list derives the leaf label with `basename(p) = p.split("/").pop()`
  ([`WorkingCopyView.svelte:31`](../../../src/lib/components/WorkingCopyView.svelte)). For `"newdir/"`
  that is `""` → **blank name**.
- The untracked diff path runs `git diff --no-index -- /dev/null newdir/`, which errors on a directory
  → **blank content**.
- `git add` makes git track the individual files (`1 A. newdir/a.txt`) → name + diff appear; `reset`
  collapses back → blank again. Exactly the reported "stage fixes it, unstage breaks it" behaviour, and
  only for new files **inside a new directory** ("sometimes, for some new files").

### Fix
1. Add `--untracked-files=all` to the `git status --porcelain=v2 -z` invocation so untracked directories
   expand to individual files. Git still honours `.gitignore`, so ignored trees (`node_modules`, `target`)
   are **not** walked. This matches Fork / GitHub Desktop behaviour.
2. Defensive guard: in `working_changes`, skip any untracked record whose path is empty or ends in `/`
   (so a future edge case can never surface a blank row). Add a Rust unit test that a new file in a new
   directory is listed individually (not as the directory).

### Files
- `crates/git-core/src/ops_worktree.rs` — add `-uall`; guard; test.

---

## Item 3 — Branch double-click checkout + right-click fast-forward to origin

### 3a — Double-click to checkout
- [`RefTree.svelte`](../../../src/lib/components/RefTree.svelte) ref-row button currently only has
  `onclick={onJump}` (selects/scrolls the commit). Add an `ondblclick` that calls a new `onCheckout(node)`
  prop. [`Sidebar.svelte`](../../../src/lib/components/Sidebar.svelte) supplies the handler:
  - `local` branch → `gitActions.checkout(name)`
  - `remote` branch → checkout creating/!switching a local tracking branch (mirror the existing
    "Create local branch…" menu path: `git checkout -b <short> --track <remote>/<short>` if absent, else
    plain checkout)
  - `tag` → no-op (jump only)
- Single-click jump behaviour is unchanged (dblclick does not double-fire onclick).

### 3b — Fast-forward a branch to its upstream in place (no checkout)
- New backend `ops::fast_forward_branch(repo, branch, remote)` running
  `git fetch --end-of-options <remote> <branch>:<branch>`. This updates the **local** ref to the remote
  tip without checking it out, and **fails safely** (git rejects a non-fast-forward; refs untouched) when
  the branch has diverged. The failure message surfaces in `appState.status`.
  - git refuses `fetch <remote> <branch>:<branch>` when `<branch>` is the **current** HEAD, so the
    "Fast-forward to <remote>" context-menu item is **only shown for non-current branches**. The current
    branch is already served by the existing Pull action.
- Plumb `api.fastForwardBranch(repo, branch, remote)` → `#[tauri::command] fast_forward_branch` →
  `ops::fast_forward_branch`. New `gitActions.fastForwardBranch(branch, remote)` via the `run()` wrapper.
- Context menu (Sidebar `onRefContext`, local non-current branch): add "Fast-forward to <remote>" where
  `<remote>` is parsed from the branch's `upstream` (e.g. `origin/main` → `origin`), defaulting to
  `origin`. Disabled/omitted if the branch has no upstream.

### Files
- `crates/git-core/src/ops.rs` (new `fast_forward_branch`), `src-tauri/src/commands.rs`,
  `src/lib/api.ts`, `src/lib/gitActions.ts`, `src/lib/components/RefTree.svelte`,
  `src/lib/components/Sidebar.svelte`. Rust test for the FF op (clean FF succeeds; diverged fails, refs
  unchanged).

---

## Item 5 — Branch delete: optional remote-branch delete + force toggle

### Behaviour
Replace the current two-step confirm (`Sidebar.confirmDeleteBranch`, which shows a confirm then retries
with force on failure) with a single confirm dialog carrying **two checkboxes**:
- "Force delete (discard unmerged commits)" → `-D` instead of `-d`.
- "Also delete <remote>/<branch>" → runs `git push <remote> --delete <remote-branch>`. Shown **enabled
  only when the branch has an upstream**; the label names the actual upstream (not hard-coded "origin").

### Plumbing
- New dialog kind in [`dialogs.svelte.ts`](../../../src/lib/dialogs.svelte.ts): `branchDelete` with
  `{ title, branch, upstream: string | null, force: boolean, deleteRemote: boolean }`, rendered by
  [`Modal.svelte`](../../../src/lib/components/Modal.svelte) (follow the existing `confirmDestructive`
  checkbox pattern). Resolves to `{ confirmed, force, deleteRemote }`.
- `gitActions.deleteBranch(name, force, deleteRemote?, remote?, remoteBranch?)`.
- `api.deleteBranch(repo, name, force, deleteRemote?, remoteName?, remoteBranch?)` →
  `#[tauri::command] delete_branch(repo, name, force, delete_remote: Option<bool>, remote: Option<String>,
  remote_branch: Option<String>)`.
- `ops::delete_branch(repo, name, force, delete_remote, remote, remote_branch)`: (1) `git branch -d|-D --
  <name>`; (2) if `delete_remote` and remote known, `git push <remote> --delete -- <remote_branch>`. If the
  local delete fails, do **not** attempt the remote delete. If local succeeds but remote fails, report a
  combined warning ("deleted local; remote delete failed: …"). The upstream is resolved on the frontend
  from `appState.refsDetailed` (the `Ref.upstream` field, e.g. `origin/feature` → remote `origin`,
  remote-branch `feature`) and passed down — no fragile parsing in Rust beyond using the values given.

### Files
- `crates/git-core/src/ops.rs`, `src-tauri/src/commands.rs`, `src/lib/api.ts`, `src/lib/gitActions.ts`,
  `src/lib/dialogs.svelte.ts`, `src/lib/components/Modal.svelte`, `src/lib/components/Sidebar.svelte`.
  Rust test for remote-delete command composition (use a local bare repo as the "remote").

---

## Item 4 — Rework the branch-switch loading indicator ("top sweep + graph skeleton")

### Root cause
`repoLoading` exists ([`store.svelte.ts:1061`](../../../src/lib/store.svelte.ts)) and is set on repo change
/ cleared after `reloadGraph`, but **no component renders it** — `StatusBar` only watches
`remoteOpActive` / `isRewriting`. So a branch/repo switch shows no feedback (looks frozen).

### Design (chosen: top sweep + graph skeleton)
1. **Top accent sweep** — new `LoadingBar.svelte`, mounted once in
   [`+page.svelte`](../../../src/routes/+page.svelte) at the top edge of the window (in-flow at the top of
   the app shell, not `position:fixed` over the traffic lights). A thin (3px) indeterminate accent segment
   sweeps left→right while a "navigation busy" signal is true. Reuses the accent + motion language from
   [`github/motion.ts`](../../../src/lib/github/motion.ts) / `Skeleton.svelte`. `role="progressbar"` +
   `aria-busy`; respects `prefers-reduced-motion` (static 40%-opacity bar). Visible for the
   navigation-busy state = `repoLoading` OR an in-flight checkout/branch op (a small shared
   `appState.navBusy` flag set by `gitActions.run()` for navigation ops, cleared on completion). Remote
   pull/push keep their existing `RemoteProgress` UI.
2. **Graph skeleton** — when `repoLoading` is true and the graph has no rows yet to show, render a
   `GraphSkeleton` of ~8 shaped commit rows (dot + lane + subtitle + meta) using the existing accent-glow
   sweep, in the commit-graph area of the main view. It replaces the empty/frozen area during a fresh repo
   or branch load and crossfades out when the real graph arrives.

### Files
- New `src/lib/components/LoadingBar.svelte`, new `src/lib/components/GraphSkeleton.svelte`.
- `src/routes/+page.svelte` (mount LoadingBar; render GraphSkeleton in the graph area while loading),
  `src/lib/store.svelte.ts` (a `navBusy` signal), `src/lib/gitActions.ts` (set/clear `navBusy` around
  navigation ops; ensure `repoLoading`/`navBusy` always clear on every error path).

---

## Item 1 — PR detail as a single timeline + inline review comments + CI history

### Current state
[`GithubDetail.svelte`](../../../src/lib/components/github/GithubDetail.svelte) renders PR extras as
separate collapsible `<details>` sections (Checks / Reviews / Files) plus a flat comments list. The
backend `pr_detail` ([`crates/git-core/src/github/mod.rs:1150`](../../../crates/git-core/src/github/mod.rs))
fetches via `gh pr view --json …` but does **not** fetch commits, inline review comments, or CI run
history. So inline code-review comments are missing entirely.

### Backend additions (Rust, `github/mod.rs`)
Extend `pr_detail` to assemble a richer detail from three sources:
1. `gh pr view <n> --json …,commits` — add `commits` (oid, messageHeadline, authoredDate, author) to the
   existing field list. Keep `statusCheckRollup` (drives the pinned strip) and `reviews`/`comments`.
2. `gh api graphql` for **review threads**: `pullRequest.reviewThreads(first:100){ nodes{ isResolved, path,
   line, comments(first:50){ nodes{ author{login}, body, path, originalLine, line, createdAt } } } }`.
   Each thread → an inline-comment group carrying `resolved`. (GraphQL chosen over REST `/pulls/{n}/comments`
   because only GraphQL exposes `isResolved`.)
3. `gh api "repos/{owner}/{repo}/actions/runs?branch=<headRefName>&per_page=30"` for **CI run history**:
   workflow_runs → (name, status, conclusion, run_started_at, updated_at, html_url, head_sha, run_number).

New serde structs + public `Gh*` types: `GhCommit`, `GhReviewThread { resolved, path, line, comments:
Vec<GhInlineComment> }`, `GhInlineComment { author, body, path, line, created_at }`, `GhCheckRun {
name, status, conclusion, started_at, updated_at, url }`. Extend `GhPullDetail` with `commits`,
`review_threads`, `check_runs`. All three secondary calls are best-effort: a failure (no Actions, GraphQL
error, permissions) degrades to an empty list, never failing the whole detail. `pr_detail` is already
`#[tauri::command(async)]`.

### Frontend (TypeScript + Svelte)
- Extend `GhPullDetail` in [`types.ts`](../../../src/lib/types.ts) with the new arrays; add `GhCommit`,
  `GhReviewThread`, `GhInlineComment`, `GhCheckRun`, and a `TimelineEvent` union
  (`commit | comment | review | reviewThread | ciRun`).
- New `PrTimeline.svelte`: builds the event list and sorts **ascending by timestamp** (oldest → newest,
  GitHub-style). Event kinds:
  - **commit** — author + short sha + headline.
  - **comment** (issue/PR conversation) — author + markdown body (via existing `Markdown.svelte`).
  - **review** — author + decision (approved / changes requested / commented) with state colour + optional
    body; its inline threads render nested under it grouped by file:line.
  - **reviewThread** (inline code comment) — file:line header + the commented code line + body. Carries a
    resolved/unresolved badge. **Resolved threads render dimmed + collapsed to a one-line summary that
    expands on click** (unresolved stay fully expanded). [user choice]
  - **ciRun** — workflow name + conclusion glyph + "started <rel> · ran <duration>" (run_started_at →
    updated_at). Interleaved at its start time. [user choice: historical runs in the timeline]
- **Pinned checks strip** at the top of the detail (above the timeline) summarising the **latest**
  `statusCheckRollup` ("N passed · M failing", expandable to the per-check list). [user choice: current
  checks pinned] The historical `check_runs` populate the ciRun timeline nodes.
- `GithubDetail.svelte`: replace the `<details>` Checks/Reviews sections with the pinned strip + `PrTimeline`.
  Keep the Files list (it's not a timeline event) as a collapsible section below, or fold file links into
  the relevant review threads — Files stays a separate collapsible section for now (YAGNI).

### Files
- `crates/git-core/src/github/mod.rs`, `src-tauri/src/commands.rs` (signature already in place),
  `src/lib/types.ts`, new `src/lib/components/github/PrTimeline.svelte`,
  `src/lib/components/github/GithubDetail.svelte`. A small pure TS `buildPrTimeline(detail)` helper with
  Vitest tests for ordering + thread grouping + resolved handling.

---

## Sequencing & verification
Implement as independent slices, smallest/most-isolated first, each with its own gates + adversarial
review before the next:
1. **Item 2** (bug) — tiny backend fix + Rust test.
2. **Item 3** (dblclick + FF) — backend op + frontend wiring + Rust test.
3. **Item 5** (delete options) — backend + dialog + frontend + Rust test.
4. **Item 4** (loading) — LoadingBar + GraphSkeleton + navBusy wiring.
5. **Item 1** (PR timeline) — backend fetch + types + timeline component + Vitest.

**Per-slice gates:** `npm run check` (svelte-check 0/0), `npm test` (vitest), `cargo test`,
`cargo build -p git-it`. Browser-previewable UI (loading bar, timeline, delete dialog, branch menus)
verified via the preview tools; Tauri-only paths (real `git status -uall`, FF op, remote delete, live `gh`
PR detail) reasoned + adversarially reviewed, then confirmed by the user via `npm run tauri build` /
`npm run tauri dev`. ff-merge `batch7-desktop-feedback` → `main` after the user signs off on the batch.
