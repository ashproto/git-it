# Batch 8 — PR Workflow: Review Loop + git↔GitHub Bridges

**Date:** 2026-07-02
**Branch:** `desktop-batch8` (already carries the GitHub-refresh spinner fix `01b168d`)
**Status:** Approved design

## Goal

Turn the GitHub screen's read-only PR view into a complete review surface, and bridge the
git client and GitHub sides of the app. Seven features:

**Tier 1 — review loop**
1. Render the PR diff in-app (existing Shiki `DiffView`).
2. Full review authoring: verdict (Approve / Request changes / Comment) + inline
   line-anchored comments, held as a **local draft** and submitted in **one atomic API call**.
3. Reply to existing review threads + resolve/unresolve them.
4. Reactions + edit/delete own comments — on timeline comments, review comments, and
   PR/issue descriptions.

**Tier 2 — bridges**
5. Create-PR wizard from the current branch (auto-push with upstream if needed).
6. Checkout a PR locally (`gh pr checkout`).
7. "Open PR for this branch" / "Create pull request…" from branch context menus + a
   current-branch PR chip in the toolbar.

## Decisions (user-confirmed)

- **Review depth:** full — verdict + inline line comments with click-a-line authoring.
- **Draft model:** local client-side draft, one-shot submit via
  `POST /repos/{o}/{r}/pulls/{n}/reviews` with the `comments[]` array. No server-side
  PENDING review. Cancel = clear local state. A draft does not survive app restart —
  accepted trade-off.
- **Reactions scope:** everything — issue/PR timeline comments, review comments, and the
  PR/issue description itself. Edit/delete on any content authored by the signed-in user.
- **Create-PR flow:** full wizard — title prefilled from branch commits, body, base-branch
  picker, draft toggle, auto-push first, open the new PR in-app on success.
- **Bridge UI:** context menus (graph + sidebar) + PR detail/list buttons + toolbar chip.

## Architecture

### Backend (Rust, workspace crate `crates/git-core`)

New sibling module `crates/git-core/src/github/review.rs` (keeps `mod.rs` from growing
further; `mod.rs` re-exports). All functions use the existing `run_gh(args, stdin)`
helper; every user-supplied operand is passed as a separate arg (no shell), request
bodies go via **stdin JSON** (`gh api --input -`), and numeric ids are formatted by us.
All new Tauri commands are `#[tauri::command(async)]` (GitHub screen freeze lesson).

| Function | Mechanism |
|---|---|
| `pr_diff(repo, number) -> String` | `gh pr diff <n>` → raw unified patch |
| `pr_submit_review(repo, number, event, body, comments: Vec<DraftComment>)` | `gh api repos/{o}/{r}/pulls/{n}/reviews --method POST --input -` with `{event, body, comments:[{path, line, side, start_line?, start_side?, body}]}` where `event ∈ {APPROVE, REQUEST_CHANGES, COMMENT}` |
| `pr_reply_thread(repo, pr_number, comment_id, body)` | `gh api repos/{o}/{r}/pulls/{n}/comments/{id}/replies --method POST --input -` |
| `pr_resolve_thread(repo, thread_id, resolve: bool)` | GraphQL `resolveReviewThread` / `unresolveReviewThread` mutation (thread node id comes from the existing reviewThreads query) |
| `set_reaction(repo, target: ReactionTarget, id, content, add: bool)` | POST to the per-target reactions endpoint; remove = list reactions, find the signed-in user's reaction id, DELETE it |
| `edit_comment(repo, kind, id_or_number, body)` | issue comment → `PATCH repos/{o}/{r}/issues/comments/{id}`; review comment → `PATCH repos/{o}/{r}/pulls/comments/{id}`; PR/issue description → `gh pr edit`/`gh issue edit` `--body-file -` |
| `delete_comment(repo, kind, id)` | matching `DELETE` endpoints (descriptions are not deletable) |
| `current_login() -> String` | `gh api user --jq .login`, cached per app run in frontend state |
| `pr_create(repo, title, body, base, draft) -> u64` | ensure upstream: if the current branch has none, push via the gh-as-credential-helper pattern from `create_repo` (`git -c credential.helper= -c credential.helper='!gh auth git-credential' push --set-upstream origin HEAD`); then `gh pr create --title … --body-file - --base … [--draft]`; parse the PR number from the URL on stdout |
| `pr_checkout(repo, number)` | `gh pr checkout <n>` (handles fork head repos natively) |

`ReactionTarget` / comment `kind` enum: `Issue` (also PR description — PRs are issues in
the REST API), `IssueComment`, `ReviewComment`. Reaction `content` restricted to the 8
GitHub values (`+1, -1, laugh, confused, heart, hooray, rocket, eyes`) — reject anything
else before shelling out.

**`pr_detail` grows** (same best-effort pattern as batch 7 — a secondary-fetch failure
never fails the whole detail):
- review-thread node `id` (for resolve) and per-comment REST `databaseId` (for
  reply/react/edit/delete),
- `reactionGroups { content users { totalCount } viewerHasReacted }` on comments, review
  comments, and the PR body (GraphQL),
- own-comment detection: the frontend compares each item's `author` login to
  `current_login` (one mechanism everywhere; works for both REST- and GraphQL-sourced
  items).
- Issue detail gets the same reaction/id fields for its comment list.

### Frontend

**PR diff — new "Files" tab in `GithubDetail.svelte`** (segmented control:
`Conversation | Files`), replacing the current file-links `<details>`:
- New panel in `githubState`: `prDiff` (`makePanel<string>`), keyed
  `${repo}|${number}|${reloadNonce}`.
- New pure util `src/lib/github/prDiff.ts`: `splitPatchByFile(patch) →
  {path, oldPath, patch, additions, deletions}[]` + Vitest tests (rename headers, new
  /deleted files, binary stubs).
- Layout mirrors commit-details master–detail: file list (reuses the `FileTree`
  flat/tree toggle) + `DiffView` for the selected file. No staging callbacks passed, so
  stage/discard affordances stay hidden. Existing unified/split + context controls work
  unchanged. `language` inferred from the file extension the same way commit diffs do.

**Review draft — new `src/lib/reviewDraft.svelte.ts`** (runes store, same style as
`dialogs.svelte.ts`):
- State: `{ repo, prNumber, comments: DraftComment[], summary, verdict }`; guards that a
  draft belongs to one `(repo, prNumber)`; starting a comment on a different PR asks to
  discard the old draft first.
- `DraftComment = { path, line, side: "LEFT"|"RIGHT", body }` (line = file line number on
  that side, taken from the parsed hunks; multi-line ranges out of scope for this batch).
- In the Files tab, hovering a diff row shows a `＋` gutter affordance; click opens an
  inline composer under the row; Save adds to the draft and leaves a marker chip on the
  line (click to re-edit or remove). Rows with draft comments stay marked across
  file switches.
- **Review bar**: once a draft exists (or via a "Start review / Review changes" button in
  the detail header), a bar shows "N pending comments — Finish review". Finish opens a
  panel: summary textarea + verdict radio (Comment default; Approve;
  Request changes) + Submit / Discard. Submit → `pr_submit_review` → clear draft →
  `githubState.bumpReload()`. Verdict-only reviews (0 inline comments) use the same
  panel. GitHub rejects approving your own PR — surface its error message as-is.

**Threads (in `PrTimeline.svelte`)**: each thread gets a Reply composer (reuses the
`GithubCommentBox` pattern inline) and a Resolve / Unresolve button wired to
`pr_resolve_thread` + `bumpReload()`.

**Reactions + edit/delete — new `src/lib/components/github/ReactionBar.svelte` +
`CommentActions.svelte`** used by `PrTimeline`, `GithubDetail` (description), and the
issue comment list:
- ReactionBar renders existing reaction pills (emoji + count, highlighted when
  `viewerHasReacted`); clicking a pill toggles; a `+` opens the 8-emoji picker.
- CommentActions (hover row): Edit / Delete shown only when the comment's author ==
  `current_login` (fetched once into `githubState`). Edit swaps the rendered markdown
  for a textarea prefilled with the raw body (raw body must be carried in the detail
  payload alongside the rendered HTML) with Save/Cancel; Delete confirms via the
  existing `dialogs` destructive pattern. All mutations → `bumpReload()`.

**Tier 2 wiring:**
- `createPr` dialog kind in `dialogs.svelte.ts` + rendering in `Modal.svelte` (same
  pattern as `createRepo`): title (prefilled from the branch's commits unique to it vs.
  the base — `git log base..HEAD --format=%s`, single commit → subject, else branch
  name humanized), body textarea, base picker (local branches + default branch first),
  draft toggle. Submit runs through `gitActions.run()` (busy states) and on success
  switches to the GitHub screen and opens the new PR's detail.
- Branch context menus (graph + sidebar — the same shared menu builder from batch 7's
  fast-forward item): if the loaded `pulls` list has an **open** PR whose `headRefName`
  matches the branch → "Open pull request #N"; otherwise → "Create pull request…"
  (current branch: opens the wizard; other branches: checkout first is NOT implied —
  the wizard always targets the branch it was invoked on via `--head`).
- PR detail header + PR list row context/button: **Checkout** → `pr_checkout` via
  `gitActions.run()` + `refreshRefs()`.
- Toolbar chip: when the current branch has an open PR (derived from `pulls` panel data
  already cached — no new polling), show `#N` + review/CI glyph; click → GitHub screen →
  that PR's detail. Hidden when the pulls panel hasn't loaded — purely opportunistic.

## Error handling

- All new commands return `GithubError` through the existing classify path; write
  failures surface via the status bar / inline error text exactly like batch 7 writes.
- `pr_create` partial failure is reported distinctly: "Branch pushed, but creating the
  pull request failed: …" (mirrors the create_repo wizard convention).
- `pr_submit_review` validation errors from GitHub (e.g. stale line positions after a
  force-push, own-PR approval) are shown verbatim in the review panel; the draft is
  **kept** so the user can fix and resubmit.
- Reaction/edit/delete failures leave the timeline untouched (no optimistic UI in v1;
  every mutation re-fetches via `bumpReload`).

## Testing

- **Vitest:** `splitPatchByFile` (renames, new/deleted, binary, multi-hunk);
  `reviewDraft` store (add/edit/remove/cross-PR guard/clear-on-submit); line-number
  mapping from parsed hunks to `{line, side}`; title-prefill helper; branch→PR matching
  helper; `prTimeline` additions (threads carry ids/reactions through).
- **Rust:** arg-construction + JSON-body serialization for `pr_submit_review`,
  `set_reaction` content allowlist, `pr_create` URL→number parsing, reply/resolve arg
  shapes — same table style as existing github tests.
- **Gates:** `npm run check` 0/0 · `npx vitest run` · `cargo test --workspace` · final
  adversarial review before merge.
- **Tauri-only manual:** live review submit + resolve + reactions on an owned repo
  (`ashproto/git-it` itself is private but fully usable for this), create-PR wizard from
  a test branch, PR checkout.

## Slices (implementation order)

1. **PR diff tab** — backend `pr_diff`, `splitPatchByFile`, Files tab UI. (Foundation for 2.)
2. **Review authoring** — `pr_submit_review`, draft store, line-comment UI, review bar.
3. **Threads** — reply + resolve/unresolve (backend + PrTimeline UI).
4. **Reactions + edit/delete** — pr_detail/issue_detail payload growth, ReactionBar,
   CommentActions, backend mutations.
5. **Create-PR wizard** — `pr_create`, dialog, prefill, success navigation.
6. **Bridges** — `pr_checkout`, context-menu items, toolbar chip.

Each slice: implementer subagent → spec + quality review → gates, per
subagent-driven-development; whole-batch adversarial review before merge.

## Out of scope (deferred)

- Multi-line (range) inline comments; suggested-changes blocks.
- Server-side pending reviews / draft persistence across restarts.
- Notifications inbox, search, label/assignee editing, run logs (Tier 3 backlog).
- Optimistic UI for mutations.
