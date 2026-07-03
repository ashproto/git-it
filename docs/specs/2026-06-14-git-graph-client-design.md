# Design spec — Git It → visual git client

- Date: 2026-06-14
- Status: Draft — awaiting user review
- Author: design session (brainstorming)

## 1. Summary

Transform Git It into a
Fork/SourceTree-style git client built around a visual commit graph, with the full set of
git operations. The current timestamp-editing capability is preserved as one operation
within the new shell.

The whole client is designed up front (this document), then built in six phases, each one
independently shippable. Phase 1 (the graph) is the foundation every later phase hangs off.

## 2. Decision contract (locked)

| # | Decision | Choice |
|---|---|---|
| 1 | Functional scope | Full git operations (display + act), all six phases |
| 2 | Refs shown | All branches + tags + remote-tracking refs |
| 3 | Build approach | Design everything up front; build phased |
| 4 | Operation priority order | Nav/refs → history rewriting → merge/cherry-pick/revert → working copy → remote (last) |
| 5 | App layout | 3-pane git-client shell (sidebar · graph · bottom detail) — structural rewrite |
| 6 | Detail/diff placement | Full-width bottom panel |
| 7 | Graph line style | Build both curved (Fork) and angular (SourceTree); user toggles in settings |
| 8 | Branch coloring | Per-lane cycling palette by default, plus per-branch color overrides (pinnable, persisted) |
| 9 | Git engine | Git CLI (shell out), consistent with current code |
| 10 | Destructive-op safety | Configurable: auto-backup ON by default, disableable per-operation; + confirmations + reflog undo |

### Non-goals (explicit YAGNI — revisit later if wanted)
Submodule management UI, Git LFS UI, blame/annotate view, GPG-signing UI, multi-repo
workspace/tabs, graphical merge-tool integration (we surface conflicts and edit-in-place,
not a 3-way visual merge editor), and Git hooks management. None are precluded by this
design; they are simply out of scope for the six planned phases.

## 3. Architecture overview

Unchanged tech stack: Tauri 2 backend (Rust), SvelteKit 2.9 SPA (`ssr=false`,
adapter-static), Svelte 5 runes, plain CSS custom-property tokens, `@tauri-apps/plugin-store`
for persistence. All git access is the Rust process shelling out to the `git` CLI (the
existing `git_ops::run()` helper pattern), benefiting from `ensure_homebrew_path()` so
Finder-launched builds find `git`/`git-filter-repo`.

```
┌─────────────────────────── Svelte SPA (WKWebView) ───────────────────────────┐
│  state modules (runes)     components                                         │
│  repo / refs / graph  ───► AppShell ─ Sidebar · Toolbar · GraphHistory ·      │
│  selection / workingTree       └─ CommitDetail (meta · files · DiffView)      │
│  operation / settings                                                          │
└───────────────▲───────────────────────────────────────────────┬─────────────┘
        invoke()  │  Channel<T> (streamed progress)               │ api.ts
┌───────────────┴───────────────────────────────────────────────▼─────────────┐
│  Rust (Tauri commands)  graph · refs · ops_nav · ops_merge · ops_rewrite ·    │
│                         ops_worktree · ops_remote · safety                     │
│                                   │ shell out                                  │
│                                   ▼                                            │
│                               git  CLI                                         │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Frontend module map

State (each a `makeXState()` factory returning a runes-backed singleton, matching the
current `store.svelte.ts` pattern):

- `repoState` — current repo path, prerequisite check, open/close.
- `refsState` — branches (local/remote), tags, stashes, HEAD, current branch.
- `graphState` — loaded `GraphCommit[]`, computed `RowLayout[]`, paging cursor, loading flags.
- `selectionState` — selected SHAs (keeps the current click / ⌘-click / shift-click model), anchor.
- `workingTreeState` — status (staged/unstaged/untracked), in-progress operation flags.
- `operationState` — active multi-step op (merge/rebase/cherry-pick in progress), conflict list.
- `settingsState` — extends current `dateFormat` with graph/diff/safety prefs (see §5.5).

Components (new `src/lib/components/` tree; existing panels are absorbed or retired):

- `AppShell.svelte` — top-level 3-pane grid + toolbar.
- `Toolbar.svelte` — repo name, current-branch chip, global actions, the existing `DateFormatMenu` gear.
- `Sidebar.svelte` + `RefTree.svelte` — collapsible Local / Remotes / Tags / Stashes / Backups sections; current branch highlighted. (Backups = today's bundles/safety-refs feature, relocated here.)
- `GraphHistory.svelte` — virtualized list host; composes `GraphGutter.svelte` (the SVG lanes) + `CommitRow.svelte` (badges, subject, author, when, sha).
- `CommitDetail.svelte` — bottom panel: metadata grid, `FileList.svelte`, `DiffView.svelte`, and the action bar (operations + `Edit timestamps`).
- `ContextMenu.svelte` — right-click menu, contents vary by target (commit, branch, tag, remote, stash).
- `ConflictView.svelte` (Phase 3+), `RebaseTodo.svelte` (Phase 4), `CommitComposer.svelte` (Phase 5), `CredentialsPrompt.svelte` (Phase 6).
- Retained/adapted: `EditTabs.svelte` (timestamp tools), `BackupsPanel.svelte`, `LogPanel.svelte`, `PrereqBanner.svelte`, `CollapsiblePanel.svelte`, `DateFormatMenu.svelte`, `dates.ts`.

### 3.2 Backend module map

Split `commands.rs`/`git_ops.rs` into focused modules, all using the shared `run()` helper
and an extended structured error type:

- `graph.rs` — `load_graph`, `load_more`, `repo_status`.
- `refs.rs` — `list_refs`, branch/tag CRUD.
- `ops_nav.rs` — checkout, fetch.
- `ops_merge.rs` — merge, cherry-pick, revert.
- `ops_rewrite.rs` — rebase (+ interactive), reset, amend; reuses existing `rewrite.rs` for filter-repo timestamp edits.
- `ops_worktree.rs` — status, stage/unstage, diff, commit, stash.
- `ops_remote.rs` — pull, push.
- `safety.rs` — backup-before-destructive, reflog snapshot/undo (wraps existing `create_bundle`/`list_safety_refs`).

## 4. Backend conventions

- Invocation: keep `run(&mut Command)` → `Result<(stdout, stderr), String>`. Add a
  streaming variant for long ops using the existing `tauri::ipc::Channel<String>` pattern
  (as `rewrite_history` already does) so the UI shows live progress.
- Parsing: prefer NUL (`-z`) and explicit `--pretty=format:` with `%x00`/`%x1f`/`%x1e`
  separators (the app already uses `\x1f`/`\x1e`) — robust against newlines in subjects/refs.
- Error model: upgrade the `String` error to a small `GitError { code, message, stderr,
  command }` so the UI can distinguish "conflict" / "auth required" / "not a repo" / generic.
- Never run interactive git. For interactive rebase, drive it non-interactively via
  `GIT_SEQUENCE_EDITOR` pointing at a generated todo file (see §8). Set
  `GIT_TERMINAL_PROMPT=0` to fail fast instead of hanging on credential prompts.
- All commands take `repo: String` (path) as today and operate with `current_dir(repo)`.

## 5. Phase 1 — Commit graph (foundation)

### 5.1 Data layer

Single batched call to populate the graph (newest first, topologically ordered):

```
git -c log.showSignature=false log \
  --all --topo-order --date-order --parents \
  --pretty=format:%H%x1f%P%x1f%an%x1f%ae%x1f%aI%x1f%cn%x1f%cI%x1f%D%x1f%s%x1e \
  -n <count>            # paged; "Load more" advances with --skip or a cursor
```

- `%P` = parent SHAs (space-separated) → drives the graph.
- `%D` = ref decoration (e.g. `HEAD -> main, origin/main, tag: v1.2.0`) → ref badges.
- `--all` includes local + remote-tracking + tags (decision #2).

New Rust types (mirrored in `src/lib/types.ts`):

```rust
struct GraphCommit { sha, parents: Vec<String>, author_name, author_email,
                     author_date, committer_name, committer_date, refs: Vec<RefDecoration>, subject }
struct RefDecoration { name: String, kind: RefKind, is_head: bool }   // RefKind (as built) serializes lowercase: "local"|"remote"|"tag"|"head"
struct Ref { name, kind: RefKind, target_sha, upstream: Option<String>, ahead: u32, behind: u32 }
struct RepoStatus { head: HeadInfo, staged: u32, unstaged: u32, untracked: u32, conflicted: u32,
                    operation: Option<String> }  // (as built) "merge"|"rebase"|"cherry-pick"|"revert" or null
```

New commands: `load_graph(repo, count, skip)`, `list_refs(repo)`, `repo_status(repo)`.
`list_refs` uses `git for-each-ref --format=...` for accurate upstream/ahead-behind, which
`%D` alone doesn't give.

### 5.2 Lane-assignment engine (the linchpin)

A pure TypeScript function — `src/lib/graph/lanes.ts` — with zero DOM/Tauri dependencies,
so it is fully unit-testable. Built test-first (TDD).

Input: `GraphCommit[]` in topo order (children always before parents).
Output: `RowLayout[]`, one per commit:

```ts
type Edge = { fromLane: number; toLane: number; colorIndex: number;
              kind: "straight" | "branch" | "merge" };
type RowLayout = {        // IMPLEMENTED shape — see deviation note below
  sha: string; lane: number; colorIndex: number;
  isMerge: boolean;
  edges: Edge[];   // all band-below segments; pass-throughs are emitted here as
                   // same-lane "straight" edges (no separate passthrough array)
  width: number;   // column count spanning this row's band (gutter sizing)
};
```

> **Deviation from the original shape (Plan 1, as built):** `edgesBelow` is named
> `edges`; pass-throughs are emitted as same-lane `straight` edges inside `edges`
> rather than a separate `passthrough` array (one list for the renderer to draw);
> `width` was added for gutter sizing; and `isHead` is **deferred to the Plan 2 ref
> data layer** (head-ness comes from `%D` ref decoration, not parent topology, so the
> pure engine cannot know it — Plan 3 threads it in alongside `RowLayout`).

Algorithm — maintain `lanes: (sha | null)[]`, where `lanes[i]` is the SHA the lane is
currently "reserved" for (the next commit expected in that column). Process commits in
order:

1. Find every lane reserved for `C.sha`. The leftmost is `myLane`. Any others *converge*
   into `myLane` (emit branch edges from them, then free them) — this is a branch point
   where two children share a parent.
2. If none reserved, `C` is a tip/head with no loaded child → allocate the first free lane
   (or append) and assign it the next palette color.
3. First parent continues straight down `myLane`: `lanes[myLane] = parents[0]`.
   No parents (root) → free `myLane`.
4. Each additional parent (a merge, ≥2 parents; octopus handled by the loop): if already
   reserved in a lane, reuse it; else allocate a new lane + color. Emit a merge edge
   `myLane → parentLane`.
5. Snapshot the active lanes to compute `passthrough` + `edgesBelow`, push the row.

Properties: O(n × maxLanes); lane recycling keeps the graph narrow; deterministic given
input order. Edge cases enumerated for tests: linear history, simple branch+merge, octopus
merge (>2 parents), multiple roots/orphan branches, criss-cross merges, a tip with no
children in the loaded window, "load more" continuity (lane state must resume — the engine
recomputes from the full loaded set, not incrementally, to avoid drift).

### 5.3 Rendering

- `GraphGutter.svelte` renders one SVG per visible row band (or one absolutely-positioned
  SVG layered behind the virtualized rows), fixed row height (~30px), lane pitch ~16–18px.
- Dot styles: filled circle = normal commit; hollow ring = merge; outer ring = `HEAD`;
  selected row emphasized.
- Line style (decision #7): `curved` uses cubic-bezier bends; `angular` uses
  vertical + 45° segments. Both consume the same `Edge` list — only the path-builder differs
  (`buildCurvedPath` / `buildAngularPath`). Chosen via `settingsState.graph.lineStyle`.
- Color (decision #8): `colorIndex → hex` via a fixed palette
  (`palette[colorIndex % palette.length]`). A per-branch override map
  (`branchColors: Record<branchName, hex>`) takes precedence: when a lane is created for a
  ref tip whose branch has an override, that lane uses the override color and keeps it until
  recycled. Palette is colorblind-aware and works in light/dark.
- Virtualization: render only rows in view + small overscan; total height = `rows × rowHeight`.

### 5.4 Shell UI

The `AppShell` 3-pane layout (decision #5/#6): `Toolbar` on top; `Sidebar` left; main column
= `GraphHistory` (scrolls) over `CommitDetail` (bottom, resizable splitter). Selecting a row
populates `CommitDetail`. Multi-select (the existing click / ⌘-click / shift-click logic from
`CommitTable.svelte`) is retained for batch timestamp edits. Glass aesthetic preserved (the
`data-tauri` token overrides apply to the new surfaces).

The timestamp editor (`EditTabs`: Offset/Exact/Compress) is reachable two ways: the
`Edit timestamps` action in `CommitDetail` for the current selection, and a context-menu item.
No current capability is removed.

### 5.5 Settings & persistence

Extend the existing `settings.json` Store (same `load`/`get`/`set`/`save` + `store:default`
ACL). New keys, all with safe defaults + `coerce*` guards like `dateFormat`:

```
graph.lineStyle: "curved" | "angular"            (default "curved")
graph.colorMode: "per-lane"                       (reserved; future "per-branch")
graph.branchColors: Record<string, string>        (branch name → hex override)
graph.density: "comfortable" | "compact"          (row height)
diff.view: "unified" | "split"                    (default "unified")
diff.syntaxHighlight: boolean                      (default true)
safety.autoBackupDestructive: boolean             (default true; decision #10)
```

## 6. Phase 2 — Navigation & refs

- Commands (`ops_nav.rs`, `refs.rs`): `checkout(repo, target, {createBranch?})`,
  `create_branch`, `rename_branch`, `delete_branch(force?)`, `create_tag`, `delete_tag`,
  `fetch(repo, remote?)` (streamed).
- UI: right-click on a graph row → Checkout / Create branch here / Tag here / Copy SHA / Edit
  timestamps; right-click in the sidebar on a branch/tag/remote → contextual actions. Toolbar
  Fetch/Branch buttons. Double-click a branch = checkout.
- Safety: checkout warns on uncommitted changes (offer stash); delete-branch confirms when
  unmerged.

## 7. Phase 3 — Merge / cherry-pick / revert

- Commands (`ops_merge.rs`): `merge(repo, ref, {noFf?, squash?})`, `cherry_pick(repo, shas)`,
  `revert(repo, shas)` — all streamed, all returning a structured result incl. conflict list.
- Conflict handling: detect via exit status + `git status --porcelain=v2` /
  `git diff --name-only --diff-filter=U`. `operationState` tracks the in-progress op;
  `ConflictView` lists conflicted files with Use-ours / Use-theirs / Open-to-edit, then
  Continue / Abort. (We surface and resolve conflicts; we do not build a 3-way visual merge
  editor — see Non-goals.)
- Safety: governed by the configurable backup setting (these can rewrite the working tree).

## 8. Phase 4 — History rewriting

- Commands (`ops_rewrite.rs`): `reset(repo, sha, mode)` (soft/mixed/hard),
  `amend(repo, {message?, resetAuthorDate?})`, `rebase(repo, onto, {interactive?})`.
- Interactive rebase without a TTY: generate a todo file (pick/reword/edit/squash/fixup/drop/
  reorder) from `RebaseTodo.svelte`, set `GIT_SEQUENCE_EDITOR='cp <todo>'` (and
  `GIT_EDITOR=true` / a controlled message file for reword/squash) so git consumes our plan
  non-interactively; stream progress; surface conflicts via the Phase 3 machinery.
- Existing filter-repo timestamp rewriting (`rewrite.rs`, `rewrite_history`) stays as-is and
  becomes the engine behind `Edit timestamps`; it already auto-bundles.
- Safety: this is the highest-risk phase. `reset --hard`, rebase, and amend all run through
  `safety.rs`: when `safety.autoBackupDestructive` is on, create a recovery bundle +
  capture the pre-op ref state first; always confirm with a plain-language consequence
  ("This moves `main` back 3 commits and discards 2 uncommitted files"); offer Undo via the
  captured reflog/ref snapshot where possible.

## 9. Phase 5 — Working copy & commits

- Commands (`ops_worktree.rs`): `status`, `stage(paths)`, `unstage(paths)`,
  `discard(paths)`, `diff(repo, {path, staged, commitRange})`, `commit(repo, message,
  {amend?, signoff?})`, `stash_save/list/apply/pop/drop`.
- Diff viewer (`DiffView.svelte`): parse `git diff`/`git show` unified hunks; render unified
  by default with a split toggle (decision-backed setting); syntax highlighting via a
  *bundled* library (not CDN — desktop app may be offline). Candidate: `highlight.js`
  (broad, light) or `shiki` (accurate, heavier) — finalized in the Phase 5 plan.
- UI: a working-copy entry at the top of the history ("Uncommitted changes") opens the
  staging view in `CommitDetail`; `CommitComposer` writes the commit.

## 10. Phase 6 — Remote

- Commands (`ops_remote.rs`): `pull(repo, {rebase?})`, `push(repo, remote, ref,
  {forceWithLease?, setUpstream?})` — streamed.
- Credentials: rely on the system git credential helper / SSH agent; set
  `GIT_TERMINAL_PROMPT=0` and, if auth is needed, surface a `CredentialsPrompt` and retry
  via a scoped `GIT_ASKPASS` helper. Never store secrets in app state or the Store.
- Safety: push defaults to `--force-with-lease` (never bare `--force`) and confirms.

## 11. Cross-cutting — safety & undo

A single `safety.rs` layer wraps every destructive op:
1. If `safety.autoBackupDestructive` (default on, per-op disableable), create a recovery
   bundle (existing `create_bundle`, `--all`) and snapshot affected ref SHAs.
2. Always show a confirmation stating the concrete consequence.
3. After the op, offer Undo: restore refs from the snapshot (and/or `git reflog`) when the
   operation is reflog-reversible. Bundles/safety-refs remain browsable in the sidebar
   Backups section (existing `list_bundles`, `list_safety_refs`, `fetch_bundle`, `delete_*`).

## 12. Performance

- One batched `load_graph` call; client-side lane computation; "Load more" paging (default
  window e.g. 500–1000 commits) rather than loading entire huge histories at once.
- Virtualized history rendering (visible rows + overscan).
- Debounced refresh; optional filesystem watching (Tauri fs / `notify`) to auto-refresh on
  external git changes — deferred/optional, not Phase 1.
- Lane recompute is O(n·lanes) and runs off the loaded set; memoize by `(headSha, refsHash,
  count)`.

## 13. Testing strategy

- Lane engine: Vitest unit tests (add Vitest), table-driven over the §5.2 edge cases —
  built test-first. This is the single most test-critical unit.
- Date/format utils: keep/extend existing-style checks.
- Rust: integration tests that build throwaway fixture repos in a temp dir
  (`git init`, scripted commits/branches/merges) and assert command output — especially
  `load_graph` parsing and each destructive op + its backup.
- Type/compile gates: `npm run check` (svelte-check) + `cargo check` must stay clean.
- Manual: Claude_Preview for the `?translucent` browser path (graph render, interactions),
  then a real `tauri build` smoke test for git operations against a scratch repo.

## 14. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Lane algorithm wrong on gnarly histories (spaghetti) | Pure, TDD'd, exhaustive edge-case tests before any rendering |
| Shell rewrite destabilizes existing features | Keep `dates.ts`, `EditTabs`, bundles intact; absorb into shell behind feature-complete tests; verify each phase |
| Destructive ops lose user work | Configurable auto-backup ON by default + confirmations + reflog undo (§11) |
| Performance on large repos | Paging + virtualization + memoized lane compute |
| Non-interactive interactive-rebase fragility | Controlled env vars (`GIT_SEQUENCE_EDITOR`, `GIT_EDITOR`, `GIT_TERMINAL_PROMPT=0`), generated todo, conflict surfacing |
| Offline syntax highlighting | Bundle the highlighter via npm, never CDN |
| Credential prompts hanging git | `GIT_TERMINAL_PROMPT=0` + explicit `CredentialsPrompt`/`GIT_ASKPASS` |

## 15. Build order & milestones

Each phase is shippable and verified before the next starts:

1. Phase 1 — data layer + lane engine (TDD) + renderers + 3-pane shell + settings. The
   visible "Fork-like graph" milestone; timestamp editing preserved.
2. Phase 2 — navigation & refs (checkout, branch/tag CRUD, fetch).
3. Phase 3 — merge / cherry-pick / revert + conflict view.
4. Phase 4 — history rewriting (rebase/interactive, reset, amend) + safety layer hardened.
5. Phase 5 — working copy (status/stage/diff/commit/stash) + diff viewer.
6. Phase 6 — remote (pull/push + credentials).

## 16. Open items to finalize during per-phase planning

- Exact graph palette hex values (colorblind-aware, light/dark) + dot sizing.
- Diff highlighter library choice (highlight.js vs shiki).
- Whether to add filesystem auto-refresh in Phase 1 or defer.
- Splitter/resize behavior + persistence of pane sizes.
