# Batch 8 — PR Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the in-app PR review loop (diff view, review authoring with inline comments, thread reply/resolve, reactions, edit/delete) and bridge git↔GitHub (create-PR wizard, checkout PR, branch↔PR navigation).

**Architecture:** New Rust module `crates/git-core/src/github/review.rs` wraps `gh` (REST via `gh api --input -`, GraphQL for thread resolution) behind `#[tauri::command(async)]` commands; frontend reuses the existing Shiki `DiffView` for the PR diff, holds review drafts in a client-side runes store, and submits reviews in one atomic REST call. Spec: `docs/superpowers/specs/2026-07-02-pr-workflow-batch8-design.md` — read it first.

**Tech Stack:** Tauri 2, SvelteKit 5 (runes), Rust workspace (`git-core`), `gh` CLI ≥2.86, Vitest, cargo test.

**Branch:** `desktop-batch8` (exists). Work directly on it; commit per task.

**Non-negotiable conventions (from this repo):**
- Every new Tauri command is `#[tauri::command(async)]` (sync commands freeze the UI — batch "loading/async" lesson).
- All `gh`/`git` invocations pass user operands as separate args, never through a shell; request bodies via **stdin** (`run_gh(args, Some(json))` with `--input -`); guard git operands with `--end-of-options` or `--` where a flag/operand confusion is possible.
- Rust: follow the existing patterns in `crates/git-core/src/github/mod.rs` — `resolve_owner_repo(repo)` for `{owner}/{name}`, `classify_gh_error` via `run_gh`, `#[serde(rename_all = "camelCase")]` on any struct crossing to TS, panic-safe parsing (`serde_json` pointers + `unwrap_or`).
- Frontend: TS bindings in `src/lib/api.ts` (`invoke<...>("command_name", {...})`), panels via `githubState` `makePanel<T>()` keyed `${repo}|…|${reloadNonce}`, mutations go through `src/lib/githubActions.svelte.ts` and end with `githubState.bumpReload()`.
- Gates after every task: `npm run check` (0 errors/0 warnings) and the relevant test runner; full gates (`npx vitest run`, `cargo test --workspace`) at least at each slice boundary.

---

## Slice 1 — PR diff in-app

### Task 1: Backend `pr_diff` + `review.rs` scaffold

**Files:**
- Create: `crates/git-core/src/github/review.rs`
- Modify: `crates/git-core/src/github/mod.rs` (add `pub mod review;` — check how `github` is declared; if it's `github/mod.rs`, add the module line near the top and re-export nothing — callers use `github::review::…`)
- Modify: `src-tauri/src/lib.rs` (new command + `invoke_handler` registration)
- Modify: `src/lib/api.ts`

- [ ] **Step 1: failing Rust test** — in `review.rs` bottom `#[cfg(test)] mod tests`:

```rust
#[test]
fn pr_diff_args_shape() {
    let args = pr_diff_args("ashproto/git-it", 42);
    assert_eq!(args, vec!["pr", "diff", "42", "--repo", "ashproto/git-it"]);
}
```

- [ ] **Step 2:** `cargo test -p git-core pr_diff_args_shape` → FAIL (function missing).

- [ ] **Step 3: implement.** `review.rs` gets a small args-builder (testable pure fn) + the runner:

```rust
use std::path::Path;
use super::{resolve_owner_repo, run_gh, GithubError};

fn pr_diff_args(slug: &str, number: u64) -> Vec<String> {
    vec!["pr".into(), "diff".into(), number.to_string(), "--repo".into(), slug.into()]
}

pub fn pr_diff(repo: &Path, number: u64) -> Result<String, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let args = pr_diff_args(&slug, number);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_gh(&refs, None)
}
```

(If `resolve_owner_repo`/`run_gh`/`GithubError` aren't `pub(crate)`-visible from a submodule, adjust their visibility in `mod.rs` — smallest change that compiles.)

- [ ] **Step 4:** `cargo test -p git-core` → PASS.

- [ ] **Step 5: Tauri command + TS binding.** In `src-tauri/src/lib.rs`, mirror the existing `github_pr_detail` command exactly (async, `PathBuf` repo, `Result<_, GithubError>`):

```rust
#[tauri::command(async)]
fn github_pr_diff(repo: std::path::PathBuf, number: u64) -> Result<String, git_core::github::GithubError> {
    git_core::github::review::pr_diff(&repo, number)
}
```

Register in the `invoke_handler` list. In `api.ts`:

```ts
githubPrDiff: (repo: string, number: number) =>
  invoke<string>("github_pr_diff", { repo, number }),
```

- [ ] **Step 6:** `cargo test --workspace` + `npm run check` → green. Commit: `feat(github): pr_diff backend (gh pr diff) + review.rs module`.

### Task 2: `splitPatchByFile` util (TDD) + Files tab UI

**Files:**
- Create: `src/lib/github/prDiff.ts`, `src/lib/github/prDiff.test.ts`
- Create: `src/lib/components/github/PrFilesTab.svelte`
- Modify: `src/lib/githubState.svelte.ts` (new `prDiff` panel + include in `anyLoading`)
- Modify: `src/lib/components/github/GithubDetail.svelte` (Conversation | Files segmented control)

- [ ] **Step 1: failing tests** — `prDiff.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { splitPatchByFile } from "./prDiff";

const TWO_FILES = `diff --git a/src/a.ts b/src/a.ts
index 111..222 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -1,2 +1,3 @@
 line1
+added
 line2
diff --git a/README.md b/README.md
index 333..444 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-old
+new
`;

describe("splitPatchByFile", () => {
  it("splits a multi-file patch, keeping each file's full patch text", () => {
    const files = splitPatchByFile(TWO_FILES);
    expect(files.map((f) => f.path)).toEqual(["src/a.ts", "README.md"]);
    expect(files[0].patch).toContain("@@ -1,2 +1,3 @@");
    expect(files[0].patch.startsWith("diff --git")).toBe(true);
    expect(files[1].patch).toContain("-old");
  });
  it("counts additions/deletions per file", () => {
    const files = splitPatchByFile(TWO_FILES);
    expect(files[0]).toMatchObject({ additions: 1, deletions: 0 });
    expect(files[1]).toMatchObject({ additions: 1, deletions: 1 });
  });
  it("handles renames (path = new side)", () => {
    const p = `diff --git a/old.ts b/new.ts
similarity index 90%
rename from old.ts
rename to new.ts
--- a/old.ts
+++ b/new.ts
@@ -1 +1 @@
-x
+y
`;
    const f = splitPatchByFile(p)[0];
    expect(f.path).toBe("new.ts");
    expect(f.oldPath).toBe("old.ts");
  });
  it("handles new and deleted files (/dev/null sides)", () => {
    const p = `diff --git a/gone.ts b/gone.ts
deleted file mode 100644
--- a/gone.ts
+++ /dev/null
@@ -1 +0,0 @@
-bye
`;
    expect(splitPatchByFile(p)[0].path).toBe("gone.ts");
  });
  it("returns [] for empty/whitespace input and keeps binary stubs", () => {
    expect(splitPatchByFile("")).toEqual([]);
    const bin = `diff --git a/img.png b/img.png
Binary files a/img.png and b/img.png differ
`;
    expect(splitPatchByFile(bin)[0]).toMatchObject({ path: "img.png", additions: 0, deletions: 0 });
  });
});
```

- [ ] **Step 2:** `npx vitest run src/lib/github/prDiff.test.ts` → FAIL.

- [ ] **Step 3: implement** `prDiff.ts`:

```ts
export type PrDiffFile = {
  path: string;      // new-side path ("b/" side; old side for deletions)
  oldPath: string | null; // set when renamed
  patch: string;     // full per-file patch incl. its "diff --git" header
  additions: number;
  deletions: number;
};

/** Split `gh pr diff` output into per-file patches. Boundaries are lines
 *  starting with "diff --git ". Paths come from the +++/--- headers when
 *  present (strip "a/"/"b/" prefix; "/dev/null" → use the other side),
 *  falling back to parsing the "diff --git a/X b/Y" line (binary files
 *  have no ---/+++ lines). additions/deletions count "+"/"-" body lines
 *  (not "+++"/"---" headers). */
export function splitPatchByFile(patch: string): PrDiffFile[] { /* … */ }
```

Implementation notes: iterate lines; start a new record on `diff --git `; per record capture `rename from`/`rename to` lines for `oldPath`/`path`; else derive from `+++ b/…` / `--- a/…`; count `+`/`-` lines that are not `+++`/`---`. Keep it dependency-free (do NOT reuse `src/lib/diff/parse.ts` — that parses hunks for rendering; this only splits).

- [ ] **Step 4:** tests PASS.

- [ ] **Step 5: `prDiff` panel.** In `githubState.svelte.ts`, next to `prDetail`: `const prDiff = makePanel<string>();`, loader

```ts
function loadPrDiff(repo: string, number: number) {
  return prDiff.load(`${repo}|${number}|${reloadNonce}`, () => api.githubPrDiff(repo, number));
}
```

expose `get prDiff()` + `loadPrDiff`, reset it in `ensure()` and in `openItem()` (alongside `prDetail.reset()`), and add `prDiff.status === "loading"` to `anyLoading`.

- [ ] **Step 6: Files tab.** `PrFilesTab.svelte` props: `{ number: number }`. On mount/`$effect`: `githubState.loadPrDiff(appState.repo, number)` (also read `void githubState.reloadNonce` so Refresh re-fetches — same pattern as the README effect in `GithubOverview.svelte`). Derive `files = splitPatchByFile(githubState.prDiff.data ?? "")`. Layout: master–detail like `CommitFilesDiff.svelte` (copy its structure): left file list (reuse `FileTree.svelte` + the flat/tree toggle and `fileTreeView` setting, entries showing `+n −n`), right `<DiffView patch={selected.patch} language={langFromPath(selected.path)} />` — no staging props. Reuse the extension→language helper `CommitFilesDiff` uses (extract to a shared util if it's currently inline). Loading state: `GithubSkeleton` while `prDiff.status === "loading"` and no data; error state shows `prDiff.error` message like other panels.

In `GithubDetail.svelte`, for PRs only: a segmented control `Conversation | Files (N)` above the timeline (default Conversation; `let detailTab = $state<"conversation" | "files">("conversation")`, reset to Conversation when `d.number` changes); Files renders `<PrFilesTab number={d.number} />`; **remove** the old file-links `<details>` block. Keep the pinned checks strip visible on both tabs.

- [ ] **Step 7:** `npm run check` + `npx vitest run` → green. Commit: `feat(github): PR Files tab — per-file diff rendered in DiffView`.

---

## Slice 2 — Review authoring

### Task 3: Backend `pr_submit_review`

**Files:**
- Modify: `crates/git-core/src/github/review.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`

- [ ] **Step 1: failing tests** (in `review.rs` tests):

```rust
#[test]
fn review_body_serializes_comments() {
    let body = review_request_json(
        "REQUEST_CHANGES",
        "needs work",
        &[DraftComment { path: "src/a.ts".into(), line: 12, side: "RIGHT".into(), body: "why?".into() }],
    );
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["event"], "REQUEST_CHANGES");
    assert_eq!(v["comments"][0]["path"], "src/a.ts");
    assert_eq!(v["comments"][0]["line"], 12);
    assert_eq!(v["comments"][0]["side"], "RIGHT");
}

#[test]
fn review_body_omits_empty_comments_and_body() {
    let body = review_request_json("APPROVE", "", &[]);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v.get("comments").is_none());
    assert!(v.get("body").is_none());
    assert_eq!(v["event"], "APPROVE");
}

#[test]
fn submit_review_rejects_bad_event() {
    assert!(validate_review_event("LGTM").is_err());
    assert!(validate_review_event("APPROVE").is_ok());
}
```

- [ ] **Step 2:** run → FAIL. **Step 3: implement:**

```rust
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DraftComment {
    pub path: String,
    pub line: u64,
    pub side: String, // "LEFT" | "RIGHT"
    pub body: String,
}

fn validate_review_event(event: &str) -> Result<(), GithubError> {
    match event {
        "APPROVE" | "REQUEST_CHANGES" | "COMMENT" => Ok(()),
        _ => Err(GithubError::Other(format!("Invalid review event: {event}"))),
    }
}

fn review_request_json(event: &str, body: &str, comments: &[DraftComment]) -> String { /* build serde_json::json! map; skip "body" when empty, skip "comments" when empty; comments entries use snake_case REST keys: path, line, side, body */ }

pub fn pr_submit_review(repo: &Path, number: u64, event: &str, body: &str, comments: Vec<DraftComment>) -> Result<(), GithubError> {
    validate_review_event(event)?;
    // also validate every comment side is LEFT/RIGHT before shelling out
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let path = format!("repos/{owner}/{name}/pulls/{number}/reviews");
    let json = review_request_json(event, body, &comments);
    run_gh(&["api", &path, "--method", "POST", "--input", "-"], Some(&json)).map(|_| ())
}
```

Note: GitHub requires a `body` for `REQUEST_CHANGES`/`COMMENT` events — do NOT enforce client-side beyond the UI; let GitHub's error surface verbatim (spec: draft is kept on failure).

- [ ] **Step 4:** `cargo test -p git-core` PASS. **Step 5:** Tauri command `github_pr_submit_review(repo, number, event: String, body: String, comments: Vec<DraftComment>)` (async, registered) + `api.ts`:

```ts
githubPrSubmitReview: (repo: string, number: number, event: string, body: string, comments: DraftComment[]) =>
  invoke<void>("github_pr_submit_review", { repo, number, event, body, comments }),
```

with `export type DraftComment = { path: string; line: number; side: "LEFT" | "RIGHT"; body: string };` added to `src/lib/types.ts`.

- [ ] **Step 6:** gates; commit `feat(github): one-shot review submission (verdict + inline comments)`.

### Task 4: `reviewDraft` store (TDD) + line-comment UI + review bar

**Files:**
- Create: `src/lib/reviewDraft.svelte.ts`, `src/lib/reviewDraft.test.ts`
- Create: `src/lib/components/github/ReviewBar.svelte`
- Modify: `src/lib/components/DiffView.svelte` (optional comment-affordance props)
- Modify: `src/lib/components/github/PrFilesTab.svelte`, `GithubDetail.svelte`
- Modify: `src/lib/githubActions.svelte.ts`

- [ ] **Step 1: failing store tests** (`reviewDraft.test.ts` — plain Vitest; `.svelte.ts` runes files are testable like the existing `dialogs` usage; if runes-in-test friction appears, structure the store as a plain class with `$state` fields like `githubState` and test the exported instance):

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { reviewDraft } from "./reviewDraft.svelte";

beforeEach(() => reviewDraft.discard());

describe("reviewDraft", () => {
  it("starts empty and records the PR it belongs to on first add", () => {
    expect(reviewDraft.count).toBe(0);
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(reviewDraft.count).toBe(1);
    expect(reviewDraft.belongsTo("/r", 5)).toBe(true);
    expect(reviewDraft.belongsTo("/r", 6)).toBe(false);
  });
  it("rejects adds for a different PR while a draft exists", () => {
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(() => reviewDraft.addComment("/r", 6, { path: "b.ts", line: 1, side: "RIGHT", body: "x" })).toThrow();
  });
  it("edits and removes by index; discard clears everything", () => {
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    reviewDraft.updateComment(0, "better");
    expect(reviewDraft.comments[0].body).toBe("better");
    reviewDraft.removeComment(0);
    expect(reviewDraft.count).toBe(0);
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    reviewDraft.discard();
    expect(reviewDraft.count).toBe(0);
    expect(reviewDraft.belongsTo("/r", 5)).toBe(false);
  });
  it("finds the draft comment on a given line/side", () => {
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(reviewDraft.commentAt("a.ts", 3, "RIGHT")?.body).toBe("hm");
    expect(reviewDraft.commentAt("a.ts", 4, "RIGHT")).toBeUndefined();
  });
});
```

- [ ] **Step 2:** FAIL. **Step 3: implement** the store (module-level `$state` in `makeReviewDraft()` like `githubState`; state `{repo, prNumber, comments: DraftComment[], summary: string, verdict: "COMMENT"|"APPROVE"|"REQUEST_CHANGES"}`; `count`, `belongsTo`, `addComment` (throws on cross-PR add — the UI catches and shows the discard-first dialog), `updateComment`, `removeComment`, `commentAt`, `setSummary`, `setVerdict`, `discard`). **Step 4:** PASS.

- [ ] **Step 5: DiffView affordance.** Add optional props:

```ts
onLineComment?: (line: number, side: "LEFT" | "RIGHT") => void;
commentMarkers?: (line: number, side: "LEFT" | "RIGHT") => boolean;
```

DiffView's row model already carries old/new line numbers (used by line-staging). When `onLineComment` is set: render a `＋` button in the line-number gutter cell on row hover (unified: side = "LEFT" only for pure deletion rows, else "RIGHT" with the new line number; split: the hovered column decides). When `commentMarkers(line, side)` is true render a small accent 💬 marker chip on that row. **No behavior change when the props are absent** (working-copy/commit diffs untouched — verify by leaving their call sites alone).

- [ ] **Step 6: composer + markers in `PrFilesTab`.** Clicking `＋` (or a marker) opens one inline composer panel positioned under the diff (simplest robust v1: a fixed composer strip at the bottom of the Files tab showing `path:line`, textarea, Save / Remove / Cancel — no DOM row injection). Save → `reviewDraft.addComment(appState.repo, number, {...})` or `updateComment`; wrap the cross-PR throw in a `dialogs` confirm ("Discard the review draft for PR #N?" → `reviewDraft.discard()` then re-add).

- [ ] **Step 7: `ReviewBar.svelte`.** Rendered in `GithubDetail` header area for PRs: hidden state = a "Review changes" button; with a draft = "N pending comment(s)" + "Finish review". Expanded panel: summary textarea, verdict radios (Comment / Approve / Request changes), Submit + Discard. Submit calls a new `githubActions.submitReview()`:

```ts
async submitReview(number: number) {
  const ok = await api.githubPrSubmitReview(appState.repo, number, reviewDraft.verdict, reviewDraft.summary, [...reviewDraft.comments])
    .then(() => true)
    .catch((e) => { submitError = message(e); return false; });  // keep the draft!
  if (ok) { reviewDraft.discard(); githubState.bumpReload(); }
  return ok;
}
```

(Show `submitError` verbatim inside the panel — GitHub's own message covers stale lines / own-PR approval.)

- [ ] **Step 8:** gates (`npm run check`, `npx vitest run`); commit `feat(github): review authoring — draft store, inline line comments, review bar`.

---

## Slice 3 — Threads: reply + resolve

### Task 5: Backend ids + reply/resolve; frontend thread actions

**Files:**
- Modify: `crates/git-core/src/github/mod.rs` (`pr_detail` GraphQL: thread `id`, comment `databaseId`), `review.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`, `src/lib/types.ts`
- Modify: `src/lib/github/prTimeline.ts` (+`.test.ts`) only if types force it (pass-through)
- Modify: `src/lib/components/github/PrTimeline.svelte`, `src/lib/githubActions.svelte.ts`

- [ ] **Step 1:** extend the existing reviewThreads GraphQL query in `pr_detail` to fetch thread `id` and per-comment `databaseId`; extend `GhReviewThread`/`GhInlineComment`:

```ts
export type GhInlineComment = { author: string; body: string; path: string; line: number; createdAt: string; databaseId: number | null };
export type GhReviewThread = { id: string; resolved: boolean; path: string; line: number; comments: GhInlineComment[] };
```

Rust structs mirror (camelCase serde). Keep parsing panic-safe: missing id → skip resolve affordance, `databaseId` default `None`. Update `prTimeline.test.ts` fixtures compile-clean (add the new fields).

- [ ] **Step 2: backend fns + tests** in `review.rs` (same TDD rhythm as Task 3 — args/JSON builders tested):

```rust
pub fn pr_reply_thread(repo: &Path, pr_number: u64, comment_id: u64, body: &str) -> Result<(), GithubError>
// gh api repos/{o}/{n}/pulls/{pr}/comments/{comment_id}/replies --method POST --input -  {"body": …}

pub fn pr_resolve_thread(repo: &Path, thread_id: &str, resolve: bool) -> Result<(), GithubError>
// gh api graphql -f query='mutation($id: ID!) { resolveReviewThread(input:{threadId:$id}) { thread { id } } }' -f id=<thread_id>
// resolve=false → unresolveReviewThread. Reject thread_id that doesn't match ^[A-Za-z0-9_=+/-]+$ before shelling out.
```

Tests: JSON body shape for reply; mutation string picks resolve vs unresolve; invalid thread id rejected.

- [ ] **Step 3:** Tauri commands `github_pr_reply_thread`, `github_pr_resolve_thread` (async, registered) + api.ts bindings (`githubPrReplyThread`, `githubPrResolveThread`).

- [ ] **Step 4: UI.** In `PrTimeline.svelte`, inside each rendered thread (both the review-attached and standalone `reviewThread` events): a "Reply" affordance expanding a small composer (textarea + Reply button, ⌘⏎ — copy the interaction from `GithubCommentBox.svelte`; reply target = the thread's **first** comment's `databaseId`; hide Reply when it's null), and a "Resolve" / "Unresolve" text-button (hidden when `thread.id` empty). Both go through new `githubActions.replyThread(prNumber, commentId, body)` / `resolveThread(threadId, resolve)` that call the api then `githubState.bumpReload()`.

- [ ] **Step 5:** gates + `cargo test --workspace`; commit `feat(github): review-thread reply + resolve/unresolve`.

---

## Slice 4 — Reactions + edit/delete own comments

### Task 6: Backend — login, reaction/comment ids in payloads, mutations

**Files:**
- Modify: `crates/git-core/src/github/mod.rs` (`pr_detail`, `issue_detail` payload growth; `current_login`), `review.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`, `src/lib/types.ts`

- [ ] **Step 1: payload growth.** Add to `types.ts`:

```ts
export type GhReactionGroup = { content: string; count: number; viewerReacted: boolean };
```

Extend `GhComment` → `{ author, body, createdAt, id: number | null, reactions: GhReactionGroup[] }`, `GhInlineComment` gains `reactions: GhReactionGroup[]`, `GhPullDetail`/`GhIssueDetail` gain `bodyReactions: GhReactionGroup[]`. Rust mirrors with defaults (`Vec::new()`, `None`) so older/partial gh output never panics. Sources: timeline comments come from `gh pr view --json comments` which includes `id` (numeric databaseId) and `reactionGroups`; verify against live gh output (`gh pr view 1 --repo ashproto/git-it --json comments | head`) and adapt the field mapping — `reactionGroups[].users.totalCount` → `count`, `viewerHasReacted` → `viewerReacted`, content values are UPPER_SNAKE (`THUMBS_UP`) in GraphQL-backed fields; normalize to the REST names (`+1`, `-1`, `laugh`, `confused`, `heart`, `hooray`, `rocket`, `eyes`) in Rust with a single `normalize_reaction(content) -> Option<String>` fn (+ unit test for all 8 + unknown→None). PR/issue **body** reactions: fetch via the same GraphQL call that gets reviewThreads (add `reactionGroups` on the PR/issue), best-effort.

- [ ] **Step 2: `current_login`** in `mod.rs`: `run_gh(&["api", "user", "--jq", ".login"], None)` → trimmed String. Command `github_current_login`, api binding `githubCurrentLogin`, and a lazily-loaded `login` field on `githubState` (fetched once in `ensure()` when availability is Ok; `get login()`).

- [ ] **Step 3: mutations in `review.rs`** (TDD on the pure builders):

```rust
// serde: "issueComment" | "reviewComment" | "prBody" | "issueBody"
pub enum CommentKind { IssueComment, ReviewComment, PrBody, IssueBody }

pub fn set_reaction(repo, kind: CommentKind, target: u64, content: &str, add: bool) -> Result<(), GithubError>
// content allowlist: +1 -1 laugh confused heart hooray rocket eyes → else Err (test all 8 + reject).
// add:    POST {base}/reactions {"content": …}
// remove: GET {base}/reactions?per_page=100 → find entry with user.login == current_login(repo-independent) && content match → DELETE repos/{o}/{n}/…/reactions/{id}; not found → Ok(()) (idempotent).
// base: Issue → repos/{o}/{n}/issues/{target} (works for PRs too); IssueComment → repos/{o}/{n}/issues/comments/{target}; ReviewComment → repos/{o}/{n}/pulls/comments/{target}

pub fn edit_comment(repo, kind: CommentKind, target: u64, body: &str) -> Result<(), GithubError>
// IssueComment → gh api PATCH repos/{o}/{n}/issues/comments/{id} --input - {"body":…}
// ReviewComment → PATCH repos/{o}/{n}/pulls/comments/{id}
// Issue (= PR/issue description) → NOT handled here; use gh issue edit / gh pr edit with --body-file - (needs is_pr flag → make the TS side pass kind "prBody" | "issueBody" and add those variants)

pub fn delete_comment(repo, kind: CommentKind, target: u64) -> Result<(), GithubError>
// DELETE on the two comment endpoints; Err for body kinds (not deletable)
```

Endpoint mapping: reactions on descriptions (`PrBody`/`IssueBody`) both use `repos/{o}/{n}/issues/{target}/reactions` — PRs are issues in REST — with `target` = the PR/issue **number**; the comment kinds use the comment **id** against their respective comment endpoints.

- [ ] **Step 4:** Tauri commands `github_set_reaction`, `github_edit_comment`, `github_delete_comment` + api bindings (`githubSetReaction(repo, kind, target, content, add)` etc.). Raw markdown body: the detail payloads must carry the **raw** body (they already do — `body` fields are raw markdown rendered client-side via `mdToSafeHtml`; confirm and reuse).

- [ ] **Step 5:** `cargo test --workspace` + gates; commit `feat(github): reactions + comment edit/delete backend, ids/reactions in detail payloads`.

### Task 7: Frontend — ReactionBar + CommentActions

**Files:**
- Create: `src/lib/components/github/ReactionBar.svelte`, `src/lib/components/github/CommentActions.svelte`
- Modify: `PrTimeline.svelte`, `GithubDetail.svelte`, `githubActions.svelte.ts`

- [ ] **Step 1: `ReactionBar.svelte`.** Props: `{ reactions: GhReactionGroup[]; onToggle: (content: string, add: boolean) => void; disabled?: boolean }`. Render pills for groups with `count > 0` (emoji map: `+1`👍 `-1`👎 `laugh`😄 `confused`😕 `heart`❤️ `hooray`🎉 `rocket`🚀 `eyes`👀) — accent-tinted when `viewerReacted`; click toggles. Trailing `+` button opens a small popover with all 8 (reuse the `contextMenu.svelte.ts` positioning if it fits; else a simple absolutely-positioned div with outside-click close).

- [ ] **Step 2: `CommentActions.svelte`.** Props: `{ author: string; onEdit: () => void; onDelete: () => void; canDelete?: boolean }`; renders ✏️/🗑 icon buttons on hover only when `githubState.login === author` (and `login` non-empty). Delete goes through `dialogs` destructive confirm.

- [ ] **Step 3: wire into `PrTimeline`** (timeline comments: kind `issueComment`, target `comment.id` — skip actions when null; inline thread comments: kind `reviewComment`, target `databaseId`) and `GithubDetail` (description: ReactionBar with kind `prBody`/`issueBody`, target = number; Edit description only when own — body edit uses the same inline-textarea swap; no delete). Edit mode: local `$state` editingKey; textarea prefilled with raw `body`; Save → `githubActions.editComment(...)` → `bumpReload()`. New `githubActions` wrappers: `toggleReaction`, `editComment`, `deleteComment` — thin api calls + `bumpReload()`, errors to the existing inline error surface.

- [ ] **Step 4:** issue detail comment list gets the same treatment (it renders via `GithubDetail`'s issue path).

- [ ] **Step 5:** gates; commit `feat(github): reaction pills + edit/delete own comments across PR/issue detail`.

---

## Slice 5 — Create-PR wizard

### Task 8: Backend `pr_create` + prefill helper

**Files:**
- Modify: `crates/git-core/src/github/review.rs`, `crates/git-core/src/ops.rs` (subjects helper), `src-tauri/src/lib.rs`, `src/lib/api.ts`

- [ ] **Step 1 (TDD):** URL→number parser test:

```rust
#[test]
fn parses_pr_number_from_create_output() {
    assert_eq!(pr_number_from_url("https://github.com/o/r/pull/123\n"), Some(123));
    assert_eq!(pr_number_from_url("garbage"), None);
}
```

- [ ] **Step 2: implement `pr_create`:**

```rust
pub fn pr_create(repo: &Path, title: &str, body: &str, base: &str, draft: bool) -> Result<u64, GithubError>
```

Flow: (1) check upstream: `git rev-parse --abbrev-ref --symbolic-full-name @{upstream}` in `repo` — on failure push first using the exact credential-helper pattern from `create_repo` (`git -c credential.helper= -c credential.helper=!gh auth git-credential push --set-upstream origin HEAD`, `GIT_TERMINAL_PROMPT=0`); push failure → `Err("Could not push the branch: …")`. (2) `gh pr create --title <t> --body-file - --base <base> [--draft] --repo <slug>` with body via stdin. (3) parse the PR URL from stdout → number (`pr_number_from_url`); unparseable → Err with the raw output. Partial-failure message when push succeeded but create failed: `"Branch pushed, but creating the pull request failed: …"`. Reject `base`/`title` starting with `-` (flag-injection guard, same as create_repo's name guard).

- [ ] **Step 3: subjects prefill helper** in `ops.rs` (near `fast_forward_branch`):

```rust
pub fn branch_subjects(repo: &Path, base: &str, limit: u32) -> Result<Vec<String>, String>
// git log --format=%s --max-count=<limit> --end-of-options <base>..HEAD  → Vec of subject lines (empty on error is fine — prefill is best-effort)
```

+ arg-shape unit test. Tauri commands `github_pr_create(repo, title, body, base, draft) -> u64` and `branch_subjects(repo, base, limit)`; api bindings `githubPrCreate`, `branchSubjects`.

- [ ] **Step 4:** `cargo test --workspace`; commit `feat(github): pr_create backend (auto-push + gh pr create) + subjects prefill`.

### Task 9: Wizard dialog + navigation

**Files:**
- Modify: `src/lib/dialogs.svelte.ts`, `src/lib/components/Modal.svelte`, `src/lib/gitActions.ts`, `src/lib/githubState.svelte.ts` (openItem after create), plus the entry point in Task 10

- [ ] **Step 1: dialog kind** `createPr` in `dialogs.svelte.ts`, mirroring `createRepo` exactly (state + setters + resolve):

```ts
| {
    kind: "createPr";
    title: string; body: string; base: string; draft: boolean;
    branch: string; bases: string[];
    resolve: (v: { title: string; body: string; base: string; draft: boolean } | null) => void;
  }
```

with `openCreatePr(branch: string, bases: string[], prefillTitle: string, prefillBody: string)` returning the promise. Render in `Modal.svelte` under the existing dialog switch: text input (title), textarea (body), `<select>` (base — `bases`, first entry preselected), draft checkbox, Create/Cancel. Disable Create while title empty.

- [ ] **Step 2: prefill + orchestration** in `gitActions.ts` (same layer as `createGithubRepo`):

```ts
async createPullRequest() {
  const branch = appState.currentBranch; // however Sidebar derives it today — reuse that source
  const bases = /* local branches minus current; default branch first (appState default-branch source used by README rewrite) */;
  const subjects = await api.branchSubjects(appState.repo, bases[0], 20).catch(() => []);
  const title = subjects.length === 1 ? subjects[0] : humanizeBranch(branch); // "fix/foo-bar" → "Fix foo bar"
  const body = subjects.length > 1 ? subjects.map((s) => `- ${s}`).join("\n") : "";
  const v = await dialogs.openCreatePr(branch, bases, title, body);
  if (!v) return;
  await this.run(`Create PR from ${branch}`, async () => {
    const number = await api.githubPrCreate(appState.repo, v.title, v.body, v.base, v.draft);
    appState.setActiveScreen("github");            // whatever the existing screen-switch mechanism is (GithubView nav)
    githubState.setActiveTab("pulls");
    githubState.bumpReload();
    githubState.openItem("pr", number);
  });
}
```

`humanizeBranch` = strip `feature/`-style prefix, replace `-_` with spaces, capitalize first letter — tiny exported util + 3-case Vitest test in a new `src/lib/github/prCreate.ts` (+`.test.ts`) together with the title/body prefill logic (pure: `prefillFromSubjects(subjects, branch) → {title, body}`).

- [ ] **Step 3:** gates; commit `feat(github): create-PR wizard dialog + prefill + open-in-app on success`.

---

## Slice 6 — Bridges: checkout, menus, toolbar chip

### Task 10: `pr_checkout` + entry points

**Files:**
- Modify: `crates/git-core/src/github/review.rs` (`pr_checkout`), `src-tauri/src/lib.rs`, `src/lib/api.ts`
- Create: `src/lib/github/branchPr.ts` + `.test.ts` (branch→PR matcher)
- Modify: `src/lib/components/Sidebar.svelte`, the graph context-menu site (grep `Fast-forward to` for the shared menu builder), `GithubDetail.svelte`, `GithubPulls.svelte`, `gitActions.ts`, the toolbar in `src/routes/+page.svelte`

- [ ] **Step 1: backend.** `pr_checkout(repo, number)` → `gh pr checkout <n> --repo <slug>`; **must run with the repo as cwd** — `run_gh` doesn't set cwd, so add a `run_gh_in(repo, args, stdin)` variant (identical, plus `.current_dir(repo)`) and use it here (checkout mutates the working copy). Args-shape test. Tauri command `github_pr_checkout` + `githubPrCheckout` binding.

- [ ] **Step 2: matcher util** (TDD):

```ts
// branchPr.ts
export function prForBranch(pulls: GhPull[] | null, branch: string): GhPull | null
// open PRs only (state === "OPEN" — check the actual state casing in GhPull data), headRefName === branch, else null; null/empty pulls → null
```

Tests: match, no-match, closed PR ignored, null list.

- [ ] **Step 3: menus.** In the branch context menu (Sidebar `onRefContext` menu items + the graph's equivalent — they share the builder from batch 7; find it via `grep -rn "Fast-forward to" src/lib`): for **local** branches append: `prForBranch(githubState.pulls.data, ref.name)` → "Open pull request #N" (switch to GitHub screen → pulls tab → `openItem("pr", n)`), else if the branch is the **current** branch → "Create pull request…" (`gitActions.createPullRequest()`). Only when `githubState.hasGithubRemote`.

- [ ] **Step 4: checkout buttons.** `GithubDetail.svelte` PR header: "Checkout" button → `gitActions.run("Checkout PR #N", () => api.githubPrCheckout(repo, n))` then `refreshRefs()` (reuse the existing post-checkout refresh the double-click checkout uses). Same action in `GithubPulls.svelte` row (small icon button or context item — match the row's existing affordance style).

- [ ] **Step 5: toolbar chip.** In the toolbar region of `+page.svelte` (near pull/push buttons): when on the git screens and `prForBranch(githubState.pulls.data, currentBranch)` returns a PR, render a chip `#N` + a status glyph (reuse the review/CI glyph mapping from `GithubPulls`/`itemState.ts`); click = the same navigate-to-detail as the menu item. Hidden when pulls not loaded (data null) — no new polling. NOTE: the pulls panel only loads when the GitHub screen was visited; that's the accepted v1 behavior (chip appears opportunistically).

- [ ] **Step 6:** gates; commit `feat(github): PR checkout + branch↔PR menus + current-branch PR chip`.

---

### Task 11: Full gates + whole-batch adversarial review

- [ ] `npm run check` → 0/0 · `npx vitest run` → all green · `cargo test --workspace` → all green · `npm run build` → ok.
- [ ] Dispatch a final adversarial code-review subagent over `git diff main...desktop-batch8` (focus: injection via gh args, draft-loss paths, reaction toggle idempotency, DiffView regression risk for staging call sites, z-index/modal regressions). Fix Critical/Important findings, re-run gates.
- [ ] Update memory files; STOP before merging — user decides merge + tests the `tauri build` (live review submit, reactions, wizard, checkout are Tauri-only paths).
