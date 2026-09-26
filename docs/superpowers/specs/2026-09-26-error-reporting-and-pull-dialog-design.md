# Error popups for failed operations, and a Pull dialog

**Date:** 2026-09-26
**Status:** approved
**Branches:** `feat/error-reporting-core` first; then `feat/error-reporting-coverage` and
`feat/pull-dialog` in parallel. All three target `next`.

## Problem

When an operation fails in Git It, the only trace is one line of text in the bottom-right of the
status bar. That line is ellipsized at 380px, has no error colour, and looks the same as a
success message. The reported case: a pull that fails in one repository, where the reason is cut
off and cannot be read anywhere in the app.

## What the investigation found

**1. Only one wrapper opens a dialog.** `run()` in `src/lib/gitActions.ts` shows `dialogs.alert`
on failure. Every other wrapper writes only to `appState.status`.

| Wrapper | Operations it covers | Failure surface today |
|---|---|---|
| `run` | checkout, create/rename/delete branch, tags, fetch, abort, undo, remotes, remove worktree, PR checkout/create | status + alert |
| `runOp` | merge, cherry-pick, revert, continue, skip | status only |
| `runResolve` | use ours/theirs, keep, remove | status only |
| `runWorktree` | stage/unstage (file, hunk, lines), discard, clean, commit, stash push/apply/pop/drop | status only |
| `runDestructive` | reset, amend | status only |
| `runDestructiveRebase` | rebase onto, interactive rebase | status only |
| `runRemote` | pull, push, force push | status only |
| direct | `amendCommit` (composer amend), `squashMerge` | status only |

**2. The status bar shows the wrong line.** The status text is `${label} failed: ${firstLine(e)}`.
For a remote operation, `e` is git's whole streamed output, and its first line is the fetch
header. The real error is at the end. A pull on a diverged branch (git 2.54, `LC_ALL=C`):

```
From github.com:owner/repo
   d728e86..b362e35  main       -> origin/main
hint: You have divergent branches and need to specify how to reconcile them.
hint: …
fatal: Need to specify how to reconcile divergent branches.
```

The status bar reads `Pull failed: From github.com:owner/repo`.

**3. Probable cause of the reported pull failure.** Plain "Pull" runs `git pull --no-edit` with no
strategy flag. Neither `pull.rebase` nor `pull.ff` is set in the user's global config, so any branch
that has diverged from its upstream fails with the error above. Reproduced in a scratch repo.
`git pull --no-rebase` on the same repo merges cleanly.

**4. Other failures are swallowed or disguised.**

- **Unhandled promise rejections:** Open…, open from recents, Open / Open With / Show in Finder,
  Cancel remote op, Copy SHA, the reword pre-checks, and loading HEAD's message when "Amend" is
  ticked.
- **Failed loads rendered as empty states:**
  - The Stash panel says "No stashes."
  - Reflog says "No reflog entries".
  - The conflict panel can claim "All conflicts resolved".
  - A working-copy diff shows as empty.
  - The GitHub screen tells the user to install `gh` whatever the error was.
- **Refusals that are status-only**, e.g. "Can't reword a merge commit here." Two refusals print
  nothing at all: `runResolve` with no repo open, and `squashMerge` during a remote op.

**5. `dialogs.alert` shares the single dialog slot.** Opening any dialog settles the pending one as
cancelled (`settlePending`). A failure that lands while the user is in a prompt, for example a push
failing while they type a branch name, would cancel that prompt.

**6. How other clients handle the pull strategy.**

| Client | Clicking Pull | Merge vs rebase |
|---|---|---|
| Tower | Dialog every time; offers to auto-stash | "Use Rebase Instead of Merge" checkbox in the dialog |
| Fork | Dialog every time; "Stash and reapply local changes" | Last choice remembered app-wide |
| GitKraken | Pulls immediately | Dropdown: fast-forward if possible / fast-forward only / rebase. The ticked one becomes the default. |
| GitHub Desktop | Pulls immediately | Uses git config; passes `--ff` when unset (their fix for this exact error) |

Sources: [Tower pull help](https://www.git-tower.com/help/guides/branches-and-tags/pull/mac),
[Fork issue #2658](https://github.com/fork-dev/TrackerWin/issues/2658),
[GitKraken push & pull help](https://help.gitkraken.com/gitkraken-desktop/pushing-and-pulling/),
[GitHub Desktop PR #14439](https://github.com/desktop/desktop/pull/14439).

## Design

Three parts, one PR each.

### Part A — Error reporting core (PR 1, `feat/error-reporting-core`)

#### A1. `src/lib/errorReport.ts` (pure, vitest)

- `errorText(e: unknown): string` moves here from `gitActions.ts`.
  - A GitHub error object `{kind, message?}` yields `message` for `kind: "Other"` and `kind`
    otherwise, as today.
  - An `Error` yields its `message`.
  - `null` and `undefined` yield `""`, so they fall through to the "failed without an error
    message" headline instead of reading "undefined".
  - Anything else yields `String(e)`.
- `summarizeGitOutput(text: string): { headline: string; hints: string; output: string }`
  - **headline:** the first `fatal:` or `error:` line with the prefix removed, plus the
    continuation lines that follow it. Continuations are tab-indented file lists and unprefixed
    sentences such as "Please commit your changes or stash them before you merge." They end at
    the next prefixed line, and the following never count as continuation, wherever they appear:
    - `Aborting`
    - `Updating <sha>..<sha>`
    - `From …` / `To …`
    - space-indented ref-update lines

    The ref-update lines matter because stdout and stderr are read on separate threads, so these
    lines can interleave anywhere. Later `fatal:`/`error:` lines stay visible in the output only.
    With no prefixed line (a non-git message, or git running under a translated locale), the
    headline is the last non-empty line that is not fetch or ref-update noise. Blank input gives
    "The operation failed without an error message." The text is kept exactly as git wrote it,
    with no re-capitalisation.
  - **hints:** every `hint:` line, in order, with the prefix removed and indentation and blank
    lines kept. Lines starting "Disable this message with" are dropped; they are CLI advice that
    means nothing in a GUI. Leading and trailing blank lines are trimmed. Empty string when there
    are none.
  - **output:** the input, unchanged.

Acceptance cases (fixtures captured from git 2.54 with `LC_ALL=C`):

| Input | headline | hints |
|---|---|---|
| Pull on a diverged branch (above) | `Need to specify how to reconcile divergent branches.` | The divergent-branches block, including its indented `git config` examples |
| `git pull --ff-only` on a diverged branch | `Not possible to fast-forward, aborting.` | `Diverging branches can't be fast-forwarded, you need to either:` … `git rebase`, without the "Disable this message" line |
| Merge pull over overlapping local edits | `Your local changes to the following files would be overwritten by merge:` / `\tf` / `Please commit your changes or stash them before you merge.` | (none) |
| Rejected push | `failed to push some refs to '<url>'` | `Updates were rejected because the tip of your current branch is behind` … (4 lines) |
| `A remote operation is already in progress.` | the same text | (none) |
| Unprefixed multi-line text | its last meaningful line | (none) |
| `""` | `The operation failed without an error message.` | (none) |

#### A2. `src/lib/errors.svelte.ts` (runes store)

```ts
type ErrorReport = {
  id: number;
  kind: "error" | "notice";
  title: string;    // "Pull failed" · "Can't reword this commit"
  headline: string; // summarized error, or the notice message
  hints: string;    // "" when none
  output: string;   // raw text; "" for notices
};
```

- A FIFO **queue** that is separate from `dialogs`. A report never settles or cancels an open
  dialog.
- `current` is the head of the queue, or `null`. `dismiss()` removes the head and shows the next.
- A report identical to the last queued one (same kind, title, headline and output) is dropped,
  so an error reported twice shows once.
- `last` is the most recent *error* report, kept after it is dismissed. `reopenLast()` queues it
  again.
- `reportError(title, error, opts?: { quiet?: boolean })`:
  - builds the report with `summarizeGitOutput(errorText(error))`
  - sets the status bar to `${title}: ${first line of headline}` in the error state (A4)
  - appends `[error] ${title}` plus the raw output to the Output log
  - queues the popup unless `quiet` is set. A quiet report still becomes `last`, so the status
    bar can open it.
- `reportNotice(title, message)` queues a `notice` and sets the status text in the normal
  (non-error) state.
- **Ownership rule:** a wrapper that returns `false` has already reported. Callers never report
  that failure again.

#### A3. `src/lib/components/ErrorDialog.svelte`

- Mounted once in `+page.svelte` after `<Modal />`. Its overlay sits at z-index 4100, one step
  above the dialog layer (4000), so it covers any open dialog without closing it.
- Layout: `.dialog` (so the NERV corner brackets apply), width `min(560px, 100vw - 32px)`,
  `max-height: 80vh`. The content between the title row and the buttons scrolls.
  - **Title row:** the title, plus "1 of N" when more reports are queued.
  - **Headline:** ⚠ in `--danger` (red) for errors, ⓘ in `--accent` for notices. Keeps line
    breaks, wraps long lines. `--err` is not used: in this codebase it is the amber *warning*
    token.
  - **Hints block:** muted, preformatted. Only when `hints` is non-empty.
  - **"Show git output" disclosure** (collapsed by default): a monospace `<pre>`, max-height
    about 260px, scrolls both ways. Shown only for errors whose trimmed output differs from the
    headline. A one-line error has nothing more to show.
  - **Buttons:**
    - Errors: **Copy details** (writes title + headline + raw output to the clipboard, then shows
      "Copied" or "Copy failed" on the button) and **OK**.
    - Notices: **OK** only.
- Headline, hints and output are selectable (`.selectable`), since the body defaults to
  `user-select: none`.
- Focus moves to OK on open. It returns to the previously focused element on close, if that
  element is still in the document.
- Escape and a backdrop click dismiss. Enter activates the focused control (OK by default).
- `role="alertdialog"`, `aria-modal`, labelled by the title, described by the headline.
- `overlays.ts` `anyOverlayOpen()` includes `errors.current !== null`.

#### A4. Status bar

- The store gains `statusIsError`. `setErrorStatus(text)` sets the text and the flag.
- The existing `status` setter clears the flag, so the next ordinary message clears the red.
- While the flag is set, the right-hand status text renders as a button: ⚠ and the text in
  `--danger` (red), tooltip "Show error details". Clicking it calls `errors.reopenLast()`.

#### A5. Wrapper and direct-path changes

- `run`, `runOp`, `runResolve`, `runWorktree`, `runDestructive`, `runDestructiveRebase`,
  `amendCommit` and `squashMerge` call `reportError(`${label} failed`, e)` in place of the
  status-only write. Their refresh-on-failure behaviour is unchanged.
- `runRemote` reports these outcomes:

  | Outcome | Report |
  |---|---|
  | `ok` | none (status "— done.") |
  | `conflicted` | none; the conflict panel takes over, as today |
  | The user pressed Cancel | none; status "`${label}` cancelled." `cancelRemote` sets a flag that `runRemote` reads, rather than matching the backend's "Cancelled." text. |
  | `authFailed` and the sign-in prompt was dismissed | none; status "`${label}`: sign-in cancelled." |
  | The repository changed during the sign-in prompt | none beyond the repo-changed message. The stale auth failure is not reported. |
  | Anything else not `ok` | `reportError` with `outcome.message`, i.e. after a failed retry following sign-in, or any other failure |
  | Thrown error | `reportError` |

- `reloadGraph`'s "Could not open `<path>`" failure calls `reportError(`Couldn't open
  ${repoName}`, e)`, where `repoName` is the folder name; the full path stays in the output. It runs only on repo switch, on launch restore and after operations. Focus and watcher
  refreshes use the preserving refresh, whose failures stay in the console.
- The two existing `dialogs.alert` callers (`run` and `repositoryFlows.ts`) switch to
  `reportError`. The `alert` kind, `resolveAlert` and `handleAlertKey` are deleted.
  `repositoryFlows.test.ts` is updated to match.

### Part B — Coverage (PR 2, `feat/error-reporting-coverage`)

**B1. Swallowed errors become `reportError`:**
- `repositoryFlows.ts`: `pickRepoFolder` / `isGitRepo` rejections.
- `RepoList.svelte` and `RepoTabs.svelte`: open-from-recents rejections.
- `WorkingCopyView.svelte`: Open / Open With / Show in Finder.
- `WorktreePanel.svelte`: Open Worktree Folder.
- `gitActions.cancelRemote`.
- `GraphHistory.svelte` and `PrCommitsTab.svelte`: Copy SHA.
- `gitActions.setCommitMessage`: the `countMergesInRange` / `rebaseTodoPreview` pre-checks.
- `CommitComposer.svelte`: loading HEAD's message when "Amend" is ticked. The checkbox still
  unticks.
- `CommitMessageEdit.svelte`: message load.
- `ApplyPanel.svelte`: rewrite failure.
- `BackupsPanel.svelte`: create, delete and fetch-from bundle; delete selected and delete all
  safety refs.
- `updater.svelte.ts`: *manual* check, download and install.

**B2. Refusals become `reportNotice`.** Each keeps its existing message as the body, with a title
naming what could not happen:
- "Open a repository first."
- "Repository is still loading — try again in a moment."
- "A remote operation is already in progress."
- "Repository changed while that dialog was open — nothing was done." (`sameRepoAfterPrompt`,
  `Sidebar.svelte`)
- "Still opening the repository — try that again in a moment."
- The four reword refusals.
- "Undo unavailable…"
- The two Create-PR guards.
- "`<path>` is not a git repo." (three sites)
- "A repository is already being created."
- The two ApplyPanel guards.
- The `EditTabs.svelte` date-edit validation messages.
- RebaseTodo's "at least one non-drop step".
- A manual "Already checking for updates…".
- The two refusals that are silent today (`runResolve` without a repo; `squashMerge` during a
  remote op or rewrite) gain messages.

The "needs the desktop app" guards stay status-only. They occur only in the browser preview.

**B3. Inline errors replace fake empty states.** Each shows "Couldn't load `<thing>`:
`<headline>`" with a **Retry** button, in place of the empty-state text:
- the Stash panel list
- the Reflog panel list
- the conflict panel's details
- the working-copy file diff (matching `CommitDetail`'s existing "Could not load diff")
- the Backups panel's bundle and safety-ref lists
- GitHub availability (`githubState.ensure` keeps the error instead of coercing it to
  `NotInstalled`; the setup card shows it with Retry)

**B4. Quiet reports (red status bar, no popup):** automatic update checks (startup, the 30-minute
poll, activation) and filesystem-watcher start failures.

**B5. Unchanged:**
- settings-persistence failures (console)
- GitHub list panels and GitHub write actions (already inline; write modals stay open)
- Shiki highlight fallback, window-drag helpers, localStorage reads
- graph paging failures (console; jump-to-ref has its own message)

### Part C — Pull dialog (PR 3, `feat/pull-dialog`)

**C1. Backend.**
- `git-core` gains `PullStrategy { Merge, Rebase, FfOnly }` (serde `camelCase`: `"merge"`,
  `"rebase"`, `"ffOnly"`).
- `ops_remote::pull` and the Tauri `pull` command replace `rebase: bool` with `strategy` and
  `autostash: bool`. Every pull passes an explicit strategy, so git's "need to specify how to
  reconcile" refusal cannot occur.

| Strategy | Flags | Verified behaviour on a diverged branch |
|---|---|---|
| Merge | `--no-rebase --no-edit` | Merge commit. Also wins over a repo's `pull.ff=only` or `pull.rebase=true`. |
| Rebase | `--rebase` | Local commits replayed on the upstream |
| Fast-forward only | `--ff-only` | Refuses with "Not possible to fast-forward, aborting.", even with `pull.rebase=true` |

- `autostash` adds `--autostash`. Verified with Merge on a dirty tree: the local edit is
  re-applied.
- The phone agent does not call `pull`, so the change stays inside `git-core`, `src-tauri` and
  `api.ts`.

**C2. Settings.**
- `pullRebase` is replaced by three persisted settings, using the existing Tauri-store +
  localStorage pattern:

  | Setting | Values | Default |
  |---|---|---|
  | `pullStrategy` | `"merge" \| "rebase" \| "ffOnly"` | `"merge"` |
  | `pullAutostash` | boolean | `false` |
  | `pullAskEveryTime` | boolean | `true` |

- Migration: with no saved `pullStrategy`, a saved legacy `pullRebase: true` becomes `"rebase"`.
  The legacy key is left in place.
- Settings → the "Pull with rebase" checkbox becomes:
  - a **Pull strategy** radio group: Merge (fast-forward when possible) / Rebase / Fast-forward only
  - **Stash & reapply local changes when pulling**
  - **Ask before every pull**

**C3. The dialog.** A new `pull` kind in `dialogs.svelte.ts`, rendered by `Modal.svelte` like the
other kinds. It resolves to `{ strategy, autostash, dontAsk } | null`, and `settlePending`
resolves it to `null`.

- **Header:** "Pull `origin/main` → `main`".
- **Status line, from the store's upstream and ahead/behind.** Only non-zero counts are named:
  - "`main` is 2 ahead, 3 behind `origin/main` as of the last fetch."
  - "`main` is 3 behind `origin/main` as of the last fetch."
  - or "Up to date with `origin/main` as of the last fetch."
- **Strategy radios**, each with a one-line description, preselected from `pullStrategy`.
- **"Stash & reapply local changes"**, preselected from `pullAutostash`.
- **"Don't ask again"**, unticked, with the note "Change this in Settings".
- **Buttons:** Pull (primary, Enter) and Cancel (Escape).
- **No upstream or detached HEAD:** the dialog explains why ("`main` has no upstream branch — push
  it first to set one." / "No branch is checked out.") and Pull is disabled.

**C4. Flow.** `gitActions.pull({ showOptions?: boolean })`:

1. Refuse while the repository is loading (a notice).
2. If `pullAskEveryTime` or `showOptions`: capture `sameRepoAfterPrompt()`, open the dialog, and
   return on cancel or if the repository changed. Save the chosen strategy and autostash as the
   new defaults, as Fork does. `dontAsk` sets `pullAskEveryTime = false`.
3. Run `runRemote` with a label naming the strategy ("Pull", "Pull (rebase)", "Pull
   (fast-forward only)"), passing the chosen values rather than re-reading the store.

The toolbar Pull button passes `showOptions: e.altKey`. When asking is off, its tooltip reads "Pull
(`<strategy>`) — ⌥-click for options".

## Testing and verification

- **Unit (vitest):**
  - `errorReport.test.ts`: every acceptance case in A1, plus `errorText` for GitHub error objects,
    `Error` instances and strings.
  - Error queue: order, dedupe, `quiet`, `last`, `reopenLast`.
  - Wrapper tests in the style of `gitActions.partialFailure.test.ts`: each wrapper reports on
    failure, and each `runRemote` row in A5 behaves as specified.
  - The `pullStrategy` migration and the pull flow: asks when on, skips when off, ⌥ forces the
    dialog, and a changed repository aborts.
- **Component (jsdom, like `Modal.worktree.test.ts`):**
  - `ErrorDialog`: rendering, the disclosure, queue advance, focus and Escape, Copy.
  - Status bar: error styling and reopen.
  - The pull dialog: preselection, disabled with no upstream, "Don't ask again".
  - The B3 inline states with Retry.
- **Rust:** `ops_remote` tests on real diverged scratch repositories:
  - Merge succeeds under `pull.ff=only` and under `pull.rebase=true`.
  - Rebase produces linear history.
  - Fast-forward only refuses.
  - A dirty edit that would block a pull survives with autostash.
- **Gates for every PR:** `npm run check` (0 errors), `npm test`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings` (CI runs clippy).
- **Real app:** a debug `.app` driven against scratch repositories, with the installed Git It quit
  for the run:
  - A diverged pull with Fast-forward only shows the popup; Merge and Rebase then succeed.
  - Overlapping local edits, with and without autostash.
  - A rejected push.
  - Copy details.
  - Reopen from the red status bar.
  - Classic and NERV themes.

## Delivery

1. **PR 1** (`feat/error-reporting-core`) merges into `next` first. On its own, it makes the
   reported failure readable.
2. **PR 2** (`feat/error-reporting-coverage`) and **PR 3** (`feat/pull-dialog`) then branch from
   the updated `next`. PR 3 owns the whole `gitActions.pull` export, including its loading guard,
   and PR 2 leaves it untouched. Beyond that, the two touch only separate functions of
   `gitActions.ts`. There are no stacked PRs, which avoids the retargeting CI trap.
3. Each PR goes through adversarial review and the Codex review loop until approval, with review
   threads resolved. Commits follow commitlint.
4. Nothing reaches stable until a separate `next → main` promotion.

## Out of scope (follow-ups)

- The Amend and Interactive-Rebase dialogs close even when the operation fails, and "Update
  message" calls `onDone` on failure, so typed input is lost.
- GitHub list panels show only the error kind and have no per-panel Retry.
- A failed prerequisite probe leaves a tooltip that wrongly blames git-filter-repo.
- Pulling from a branch other than the upstream (Tower and Fork offer a branch picker).
- Plain-English explanations with one-click fixes for common git errors (considered and deferred).
- Pinning `LC_ALL=C` for all git shell-outs. Today only streamed remote ops pin it; the headline
  falls back to the last meaningful line when prefixes are translated.
