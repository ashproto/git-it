# RESUME — Git It → Fork/SourceTree-style git client

> Handoff note for continuing after a context compaction. Read this first, then
> continue with **Phase 3b** (or whatever the user asks). Everything below is true
> as of 2026-06-14, all merged to `main`, working tree clean, all gates green.

## TL;DR — where we are
Building `git-it` into a full Fork/SourceTree-style
git client. Designed up-front; built in reviewed, merged phases.

**Done & on `main`** (32 commits, 26 Rust tests + 25 Vitest, `cargo check`/`svelte-check` clean):
- Up-front design (6-phase spec).
- Phase 1 — commit graph: lane engine (pure TS) · Rust data layer · rendered graph view.
- 3-pane shell: sidebar ref tree · graph · bottom commit-detail · branch chip.
- Phase 2 — nav/ref operations: checkout, branch/tag CRUD, fetch + right-click menus & dialogs.
- Phase 3a — integrate ops backend: merge / cherry-pick / revert + conflict handling.

**NEXT: Phase 3b** — the conflict-resolution UI (the backend exists; the UI doesn't). Then Phases 4–6.

## Process (FOLLOW THIS — it's been catching real bugs)
Per phase: `writing-plans` (spec the slice) → build → **adversarial code review** (Agent `superpowers:code-reviewer`, model opus) → fix findings → merge to `main`.
- ALWAYS work on a feature branch: `git checkout -b feat/<phase>` … then `git checkout main && git merge --ff-only feat/<phase> && git branch -d feat/<phase>`. (I once slipped and built on `main` directly — don't.)
- Gates before merge: from `src-tauri/`: `cargo test` + `cargo check`; from `git-it/`: `npm run check` + `npm test`.
- Verify UI in the browser preview: Claude_Preview, `preview_start` name `vite` (port 1420), resize to width ~1200. Sample data auto-loads in the browser; **git ops hit a "needs desktop app" guard there** — so verify menus/dialogs render + the wiring logic, not the actual mutation (that needs the real Tauri app).
- **SECURITY PRINCIPLE (non-negotiable):** every `git` shell-out with a user-supplied operand MUST separate options from operands — `--` for branch/tag/paths, `--end-of-options` for checkout-ref/fetch/merge/cherry-pick/revert. Separate `Command` args stop SHELL injection but NOT git's own option parsing (`--upload-pack=<cmd>` = RCE; `-D`/`-f` = data loss). `ops.rs` + `ops_merge.rs` have regression tests for this.
- Commit trailer: end every commit message with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- After each phase: update the two memory files (see Memory section) + report + checkpoint with the user.

## NEXT TASK — Phase 3b: conflict-resolution UI (branch `feat/phase3b-conflict-ui`)
Build:
1. **Load `repo_status`** so the app knows when an op is in progress. The store does NOT currently load it. Add a `repoStatus` state to `store.svelte.ts` and load it in Tauri alongside the graph (extend `reloadGraph()` in `gitActions.ts` to also `api.repoStatus(repo)` → set it). In browser/sample mode there's no op (leave null).
2. **`ConflictView.svelte`** — shown when `repoStatus.operation != null && repoStatus.conflicted > 0`. Lists conflicted files (`api.conflictedFiles(repo)`), each with **Use ours** / **Use theirs** (`api.resolveConflict(repo, path, ours)`), plus **Continue** (`api.opContinue(repo, kind)`) and **Abort** (`api.opAbort(repo, kind)`). `kind` = `repoStatus.operation`. Mount it in `+page.svelte`'s `.main-col` (e.g. above `CommitDetail`), only when an op is in progress.
3. **`gitActions` wrappers** for merge/cherryPick/revert/opAbort/opContinue/resolveConflict that handle `OpOutcome` — if `.conflicted`, surface the ConflictView (the reloadGraph + repoStatus reload will make it appear); refresh after.
4. **Context-menu items** (reuse the existing `contextMenu` store): in `Sidebar.svelte` ref menu → "Merge into current" (`api.merge`); in `GraphHistory.svelte` row menu → "Cherry-pick onto current" (`api.cherryPick([sha])`), "Revert" (`api.revert([sha])`).
5. **Honor these UI contracts (from the 3a review):**
   - (a) After any `cherryPick`/`revert`/`opContinue` that returns Err, re-read `repo_status`; if `operation` present, offer Abort/Skip (`detect_operation`+`abort` cleanly recover every half-state).
   - (b) `resolve_side` only handles both-sides-have-content conflicts; modify/delete conflicts need an explicit keep-file (`git add`) vs remove-file (`git rm`) choice — detect & offer (may need a small new Rust op for `git rm`).
   - (c) Gate `abort`/`continue` UI on `operation` being present (idle `--abort` errors).
   - (d) Don't surface raw multi-line git `OpOutcome.message` as a toast — summarize.
   - (e) Make `--squash` / `--no-ff` mutually exclusive in any merge UI (the backend `merge()` already errors if both).

## Backend already available for 3b (`ops_merge.rs` → `api.ts`)
- `api.merge(repo, reference, noFf?, squash?)` → `OpOutcome`
- `api.cherryPick(repo, shas[])` → `OpOutcome`  ·  `api.revert(repo, shas[])` → `OpOutcome`
- `api.opAbort(repo, kind)`  ·  `api.opContinue(repo, kind)` → `OpOutcome`
- `api.resolveConflict(repo, path, ours)`  ·  `api.conflictedFiles(repo)` → `string[]`
- `api.repoStatus(repo)` → `{ head, staged, unstaged, untracked, conflicted, operation: "merge"|"rebase"|"cherry-pick"|"revert"|null }`
- `OpOutcome = { conflicted: boolean; files: string[]; message: string }` — an empty cherry-pick/revert auto-`--skip`s (returns `conflicted:false`).

## Patterns/files to REUSE (don't reinvent)
- `src/lib/gitActions.ts` — Tauri-guarded action layer: `run(label, fn)` → guard `isTauri()`+`repo`, run op, `reloadGraph()`, set `appState.status`, return bool. Add the 3b ops here.
- `src/lib/contextMenu.svelte.ts` + `components/ContextMenu.svelte` — app-wide right-click menu (`contextMenu.openAt(x, y, items)`; `MenuItem{label?, action?, danger?, separator?}`).
- `src/lib/dialogs.svelte.ts` + `components/Modal.svelte` — promise-based `await dialogs.prompt(...)` / `dialogs.confirm(...)`; settles a pending dialog on replace.
- `src/lib/store.svelte.ts` (`appState`) — `graphCommits`, `rows` (`$derived computeLanes`), `refsByKind {local,remote,tags,head}`, `currentSha`/`selectedCommit`, `selected` (Set), `setGraphCommits` (resets selection/newDates/currentSha + populates flat `commits`), `graphLineStyle`. **ADD `repoStatus` here.**
- Layout = `src/routes/+page.svelte`: header toolbar (branch chip + Fetch + DateFormatMenu) / `.shell` = `.side-col`[RepoLoader+Sidebar] | `.main-col`[GraphHistory, CommitDetail, EditTabs+ApplyPanel, LogPanel]. `<ContextMenu/>` + `<Modal/>` mounted once.
- Lane engine: `src/lib/graph/` (`computeLanes`, `laneColor`, `curvedEdgePath`/`angularEdgePath`, types). Pure, fully tested. Renderer: `GraphGutter.svelte` (SVG) + `GraphHistory.svelte` (rows).
- Rust ops live in `ops.rs` (nav/ref) + `ops_merge.rs` (integrate); commands in `commands.rs`; registered in `lib.rs`; shared `git_ops::run` (strict) / `ops_merge::run_status` (tolerant, distinguishes conflict-exit from error).

## DEFERRED cleanups (non-blocking; pick up opportunistically)
- Diff viewer + changed-files list in `CommitDetail` (currently a placeholder note) → Phase 5.
- Status bar from `repo_status`; branch color-override UI (store key `graph.branchColors` exists, no UI); graph virtualization for huge histories.
- Extract a shared `GeomConfig` (laneWidth=16/offsetX=12 dup'd in GraphGutter + GraphHistory).
- Row `onkeydown` casts KeyboardEvent→MouseEvent; menu/dialog a11y (no arrow-key nav, no focus trap; row `role="row"` lacks a grid parent).
- `gitActions.run` returns a bare bool (can't distinguish guard-fail from op-fail) → result union when harder ops need it.
- `BackupsPanel` double-fetches on mount (`$effect`+`onMount`); pre-existing.
- `TempRepo` test harness duplicated across `graph.rs`/`ops.rs`/`ops_merge.rs` (test-only).

## Repo facts
- Git repo root = `git-it/` (NOT the `GIT-GUI` parent). Default branch `main`. **No remote.**
- `.gitignore` (app-level) + `src-tauri/.gitignore` exclude `node_modules` + the ~4.2 GB `src-tauri/target`.
- Stack: Tauri 2 (macOS, glass via NSVisualEffect + `data-tauri` CSS tokens), SvelteKit 2.9 + Svelte 5 runes (`ssr=false`, adapter-static), TS 5.6, Vite 6, Vitest 3, `@tauri-apps/plugin-store` 2.4.3.
- `ensure_homebrew_path()` in `lib.rs` prepends Homebrew bins so Finder-launched builds find `git`/`git-filter-repo`.
- Manual-only (user, when ready): `npm run tauri build` to use against real repos; decide whether to add a git remote.

## Roadmap after Phase 3b
- **Phase 4 — history rewriting:** rebase (+ interactive via a generated todo + `GIT_SEQUENCE_EDITOR`), reset (soft/mixed/hard), amend. DESTRUCTIVE → wire the **configurable auto-backup safety** (spec decision #10: default ON, per-op disable; extends existing `create_bundle`) + confirmations + reflog undo. Reuse the existing `rewrite.rs`/filter-repo timestamp path as the `Edit timestamps` engine.
- **Phase 5 — working copy:** status/stage/unstage/discard/diff/commit/stash + the diff viewer (bundle a highlighter via npm, NOT CDN — offline desktop app). The "Uncommitted changes" entry + staging in `CommitDetail`.
- **Phase 6 — remote:** pull, push (`--force-with-lease`, never bare `--force`), credentials (system helper/SSH agent; `GIT_TERMINAL_PROMPT=0` + a `CredentialsPrompt`/`GIT_ASKPASS`). `fetch`/network ops can hang on SSH host-key/slow network — run off the UI thread with a spinner.

## Where the durable record lives
- Spec: `git-it/docs/specs/2026-06-14-git-graph-client-design.md`
- Plans: `git-it/docs/plans/` (one per slice built so far)
- Memory: `~/.claude/projects/-Users-ashshah-Projects-GIT-GUI/memory/` — `git-client-migration.md` (full status + 3b contracts + security principle) + `user-prefers-thorough-multiagent-review.md`, indexed in `MEMORY.md`.
