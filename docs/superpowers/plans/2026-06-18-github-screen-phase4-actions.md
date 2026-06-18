# GitHub Screen — Phase 4 (Write Actions) Implementation Plan — FINAL

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the four guarded write actions — **comment on a PR/issue · close/reopen an issue · merge a PR · create an issue** — each behind a confirm/composer modal, refreshing the affected list on success.

**Architecture:** Five write commands in `src-tauri/src/github/mod.rs` that shell out to `gh` with bodies passed via **stdin** (`--body-file -`, never as args) and validated method/state allowlists. A `githubActions.svelte.ts` controller holds the single `pending` action; a `GithubActionModal.svelte` renders the right UI per kind (comment composer / merge method-picker / close-reopen confirm / new-issue composer). Per-row trigger buttons live on the PR and Issue rows; on success the controller bumps the reload nonce so the active list re-fetches.

**Tech Stack:** Tauri 2 (Rust) · SvelteKit 5 / Svelte 5 runes · `gh` CLI · Vitest + `cargo test`.

**Spec:** `docs/superpowers/specs/2026-06-17-github-screen-design.md` §8. Builds on Phases 1–3 (merged to `main`).

**Branch:** create `feat/github-screen-p4` off `main`.

> Verified `gh` write surface (from `--help`, no writes performed during verification): `gh pr comment <n> --repo O/R --body-file -`; `gh issue comment <n> --repo O/R --body-file -`; `gh issue close <n> --repo O/R`; `gh issue reopen <n> --repo O/R` (no `--reason`); `gh pr merge <n> --repo O/R --merge|--squash|--rebase`; `gh issue create --repo O/R --title=… --body-file -` (prints the new issue URL to stdout). Free text via STDIN; `--title=` single-arg form avoids leading-dash misparse. Reuse `resolve_owner_repo`, `run_gh` (already supports `Some(stdin)`), `GithubError`.

---

## File Structure

**Modified (Rust):** `src-tauri/src/github/mod.rs` (5 fns + `merge_flag` helper + test), `commands.rs` (5 wrappers), `lib.rs` (register 5).

**Modified (TS):** `src/lib/types.ts` (`MergeMethod`), `src/lib/api.ts` (5 wrappers), `src/lib/githubState.svelte.ts` (`bumpReload()`).

**New (TS):** `src/lib/githubActions.svelte.ts` (the action controller).

**New (Svelte):** `src/lib/components/github/GithubActionModal.svelte`.

**Modified (Svelte):** `GithubPulls.svelte` (Merge/Comment row buttons), `GithubIssues.svelte` (Comment + Close/Reopen row buttons + a New-issue button), `GithubView.svelte` (mount the modal).

---

## Task 1: Rust — the five write commands

**Files:** Modify `src-tauri/src/github/mod.rs`, `commands.rs`, `lib.rs`.

- [ ] **Step 1: Add the helper, the five fns, and a test.**

Append to `src-tauri/src/github/mod.rs`:

```rust
/// Map a merge-method name to the `gh pr merge` flag. None for an unknown method.
fn merge_flag(method: &str) -> Option<&'static str> {
    match method {
        "merge" => Some("--merge"),
        "squash" => Some("--squash"),
        "rebase" => Some("--rebase"),
        _ => None,
    }
}

/// Comment on a PR. Body is piped via stdin (`--body-file -`), never an arg.
pub fn pr_comment(repo: &Path, number: u64, body: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    run_gh(&["pr", "comment", &num, "--repo", &slug, "--body-file", "-"], Some(body))?;
    Ok(())
}

/// Comment on an issue.
pub fn issue_comment(repo: &Path, number: u64, body: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    run_gh(&["issue", "comment", &num, "--repo", &slug, "--body-file", "-"], Some(body))?;
    Ok(())
}

/// Close or reopen an issue. `state` ∈ {"closed","open"}.
pub fn issue_set_state(repo: &Path, number: u64, state: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    let verb = match state {
        "closed" => "close",
        "open" => "reopen",
        _ => return Err(GithubError::Other(format!("invalid issue state: {state}"))),
    };
    run_gh(&["issue", verb, &num, "--repo", &slug], None)?;
    Ok(())
}

/// Merge a PR with the given method ∈ {"merge","squash","rebase"}.
pub fn pr_merge(repo: &Path, number: u64, method: &str) -> Result<(), GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let flag = merge_flag(method)
        .ok_or_else(|| GithubError::Other(format!("invalid merge method: {method}")))?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    run_gh(&["pr", "merge", &num, "--repo", &slug, flag], None)?;
    Ok(())
}

/// Create an issue. Title via `--title=` (single arg, dash-safe); body via stdin.
/// Returns the new issue's URL (gh prints it to stdout).
pub fn issue_create(repo: &Path, title: &str, body: &str) -> Result<String, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let title_arg = format!("--title={title}");
    let out = run_gh(
        &["issue", "create", "--repo", &slug, &title_arg, "--body-file", "-"],
        Some(body),
    )?;
    Ok(out.trim().to_string())
}
```

- [ ] **Step 2: Test the pure helper.**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn merge_flag_maps_known_methods() {
        assert_eq!(merge_flag("merge"), Some("--merge"));
        assert_eq!(merge_flag("squash"), Some("--squash"));
        assert_eq!(merge_flag("rebase"), Some("--rebase"));
        assert_eq!(merge_flag("bogus"), None);
    }
```

- [ ] **Step 3: Add the five command wrappers.**

In `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_pr_comment(repo: String, number: u64, body: String) -> Result<(), github::GithubError> {
    github::pr_comment(&PathBuf::from(repo), number, &body)
}

#[tauri::command]
pub fn github_issue_comment(repo: String, number: u64, body: String) -> Result<(), github::GithubError> {
    github::issue_comment(&PathBuf::from(repo), number, &body)
}

#[tauri::command]
pub fn github_issue_set_state(repo: String, number: u64, state: String) -> Result<(), github::GithubError> {
    github::issue_set_state(&PathBuf::from(repo), number, &state)
}

#[tauri::command]
pub fn github_pr_merge(repo: String, number: u64, method: String) -> Result<(), github::GithubError> {
    github::pr_merge(&PathBuf::from(repo), number, &method)
}

#[tauri::command]
pub fn github_issue_create(repo: String, title: String, body: String) -> Result<String, github::GithubError> {
    github::issue_create(&PathBuf::from(repo), &title, &body)
}
```

- [ ] **Step 4: Register all five.**

In `src-tauri/src/lib.rs` handler list:

```rust
    commands::github_pr_comment,
    commands::github_issue_comment,
    commands::github_issue_set_state,
    commands::github_pr_merge,
    commands::github_issue_create,
```

- [ ] **Step 5: Build + test.** `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test` — clean build; all pass (111 + `merge_flag_maps_known_methods`). **Do NOT run any of the write commands — they mutate real GitHub.**

- [ ] **Step 6: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): write commands — comment, close/reopen, merge, create (gh, stdin bodies)"
```

---

## Task 2: TS — types, api wrappers, reload hook

**Files:** Modify `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/githubState.svelte.ts`.

- [ ] **Step 1: Add the type.**

Append to `src/lib/types.ts`:

```typescript
export type MergeMethod = "merge" | "squash" | "rebase";
```

- [ ] **Step 2: Add the api wrappers.**

In `src/lib/api.ts`, add `MergeMethod` to the `import type { … } from "./types"` block, then add near the other github wrappers:

```typescript
  githubPrComment: (repo: string, number: number, body: string) =>
    invoke<void>("github_pr_comment", { repo, number, body }),
  githubIssueComment: (repo: string, number: number, body: string) =>
    invoke<void>("github_issue_comment", { repo, number, body }),
  githubIssueSetState: (repo: string, number: number, state: "open" | "closed") =>
    invoke<void>("github_issue_set_state", { repo, number, state }),
  githubPrMerge: (repo: string, number: number, method: MergeMethod) =>
    invoke<void>("github_pr_merge", { repo, number, method }),
  githubIssueCreate: (repo: string, title: string, body: string) =>
    invoke<string>("github_issue_create", { repo, title, body }),
```

- [ ] **Step 3: Add `bumpReload` to `githubState`.**

In `src/lib/githubState.svelte.ts`, in the returned object (next to `refresh`), add:

```typescript
    /** Force the active tab's load-effect to re-fetch (used after a write action). */
    bumpReload() {
      reloadNonce++;
    },
```

- [ ] **Step 4: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/types.ts src/lib/api.ts src/lib/githubState.svelte.ts
git commit -m "feat(github): MergeMethod type, write api wrappers, githubState.bumpReload"
```

---

## Task 3: TS — the action controller

**Files:** Create `src/lib/githubActions.svelte.ts`.

- [ ] **Step 1: Create the controller.**

Create `src/lib/githubActions.svelte.ts`:

```typescript
import { appState } from "./store.svelte";
import { api } from "./api";
import { githubState } from "./githubState.svelte";
import type { GithubError, MergeMethod } from "./types";

export type PendingAction =
  | { kind: "comment"; target: "pr" | "issue"; number: number; title: string }
  | { kind: "merge"; number: number; title: string }
  | { kind: "setState"; number: number; title: string; to: "open" | "closed" }
  | { kind: "create" }
  | null;

function errText(e: unknown): string {
  const g = e as GithubError | undefined;
  if (g && typeof g === "object" && "kind" in g) {
    return g.kind === "Other" ? (g as { message: string }).message : g.kind;
  }
  return String(e);
}

function makeGithubActions() {
  let pending = $state<PendingAction>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  function open(a: Exclude<PendingAction, null>) {
    pending = a;
    busy = false;
    error = null;
  }
  function cancel() {
    if (busy) return; // don't close mid-request
    pending = null;
    error = null;
  }

  async function submit(fields: { body?: string; title?: string; method?: MergeMethod }) {
    const a = pending;
    const repo = appState.repo;
    if (!a || !repo) return;
    busy = true;
    error = null;
    try {
      if (a.kind === "comment" && a.target === "pr") {
        await api.githubPrComment(repo, a.number, fields.body ?? "");
      } else if (a.kind === "comment") {
        await api.githubIssueComment(repo, a.number, fields.body ?? "");
      } else if (a.kind === "merge") {
        await api.githubPrMerge(repo, a.number, fields.method ?? "squash");
      } else if (a.kind === "setState") {
        await api.githubIssueSetState(repo, a.number, a.to);
      } else if (a.kind === "create") {
        await api.githubIssueCreate(repo, fields.title ?? "", fields.body ?? "");
      }
      pending = null;
      githubState.bumpReload(); // refresh the affected list
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }

  return {
    get pending() {
      return pending;
    },
    get busy() {
      return busy;
    },
    get error() {
      return error;
    },
    open,
    cancel,
    submit,
  };
}

export const githubActions = makeGithubActions();
```

- [ ] **Step 2: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/githubActions.svelte.ts
git commit -m "feat(github): githubActions controller (pending action + submit + refresh)"
```

---

## Task 4: Svelte — the action modal

**Files:** Create `src/lib/components/github/GithubActionModal.svelte`.

- [ ] **Step 1: Create the modal.**

Create `src/lib/components/github/GithubActionModal.svelte`:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { githubActions } from "../../githubActions.svelte";
  import type { MergeMethod } from "../../types";

  const pending = $derived(githubActions.pending);

  let body = $state("");
  let title = $state("");
  let method = $state<MergeMethod>("squash");
  let seen: unknown = null;

  // Reset the fields whenever a new action is opened.
  $effect(() => {
    const p = githubActions.pending;
    untrack(() => {
      if (p !== seen) {
        seen = p;
        body = "";
        title = "";
        method = "squash";
      }
    });
  });

  const METHODS: MergeMethod[] = ["merge", "squash", "rebase"];

  function backdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) githubActions.cancel();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && githubActions.cancel()} />

{#if pending}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" role="presentation" onclick={backdrop}>
    <div class="modal" role="dialog" aria-modal="true">
      {#if pending.kind === "comment"}
        <h3>Comment on {pending.target === "pr" ? "PR" : "issue"} #{pending.number}</h3>
        <p class="sub">{pending.title}</p>
        <textarea bind:value={body} rows="5" placeholder="Leave a comment…"></textarea>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy || !body.trim()} onclick={() => githubActions.submit({ body })}>{githubActions.busy ? "Posting…" : "Comment"}</button>
        </div>
      {:else if pending.kind === "merge"}
        <h3>Merge PR #{pending.number}</h3>
        <p class="sub">{pending.title}</p>
        <div class="methods">
          {#each METHODS as m (m)}
            <label><input type="radio" name="merge-method" value={m} checked={method === m} onchange={() => (method = m)} /> {m}</label>
          {/each}
        </div>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy} onclick={() => githubActions.submit({ method })}>{githubActions.busy ? "Merging…" : `Merge (${method})`}</button>
        </div>
      {:else if pending.kind === "setState"}
        <h3>{pending.to === "closed" ? "Close" : "Reopen"} issue #{pending.number}</h3>
        <p class="sub">{pending.title}</p>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy} onclick={() => githubActions.submit({})}>{githubActions.busy ? "Working…" : pending.to === "closed" ? "Close issue" : "Reopen issue"}</button>
        </div>
      {:else if pending.kind === "create"}
        <h3>New issue</h3>
        <input class="title-in" bind:value={title} placeholder="Title" />
        <textarea bind:value={body} rows="6" placeholder="Describe the issue…"></textarea>
        <div class="actions">
          <button type="button" onclick={() => githubActions.cancel()} disabled={githubActions.busy}>Cancel</button>
          <button class="primary" type="button" disabled={githubActions.busy || !title.trim()} onclick={() => githubActions.submit({ title, body })}>{githubActions.busy ? "Creating…" : "Create issue"}</button>
        </div>
      {/if}
      {#if githubActions.error}<p class="err">Failed: {githubActions.error}</p>{/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    width: min(460px, 92vw);
    background: var(--panel-bg, var(--popover-bg));
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  .sub {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  textarea,
  .title-in {
    width: 100%;
    box-sizing: border-box;
    background: var(--input-bg, var(--btn-bg));
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
    font: inherit;
    font-size: 13px;
    resize: vertical;
  }
  .methods {
    display: flex;
    gap: 14px;
    font-size: 13px;
  }
  .methods label {
    display: flex;
    align-items: center;
    gap: 5px;
    text-transform: capitalize;
    cursor: pointer;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .actions button {
    padding: 6px 14px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  .actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .actions .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .err {
    margin: 0;
    color: var(--err, #c0392b);
    font-size: 12.5px;
  }
</style>
```

- [ ] **Step 2: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubActionModal.svelte
git commit -m "feat(github): GithubActionModal (comment/merge/close-reopen/create UIs)"
```

---

## Task 5: Svelte — wire the triggers + mount the modal

**Files:** Modify `GithubPulls.svelte`, `GithubIssues.svelte`, `GithubView.svelte`.

- [ ] **Step 1: PR row actions.**

In `src/lib/components/github/GithubPulls.svelte`, add the import (after the existing imports in `<script>`):

```typescript
  import { githubActions } from "../../githubActions.svelte";
```

Then, inside the `<li class="row">`, add a row-actions block right after the closing `</div>` of `.meta` (i.e. between the `.meta` div and the `{#if pr.labels.length}` block):

```svelte
        <div class="row-actions">
          {#if pr.state === "OPEN"}
            <button type="button" onclick={() => githubActions.open({ kind: "merge", number: pr.number, title: pr.title })}>Merge</button>
          {/if}
          <button type="button" onclick={() => githubActions.open({ kind: "comment", target: "pr", number: pr.number, title: pr.title })}>Comment</button>
        </div>
```

Add the `.row-actions` CSS inside the component's `<style>` (after the `.more` rule):

```css
  .row-actions {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }
  .row-actions button {
    padding: 2px 10px;
    font-size: 11.5px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--btn-bg);
    color: var(--text);
    cursor: pointer;
  }
  .row-actions button:hover {
    border-color: var(--accent);
  }
```

- [ ] **Step 2: Issue row actions + New-issue button.**

In `src/lib/components/github/GithubIssues.svelte`, add the import:

```typescript
  import { githubActions } from "../../githubActions.svelte";
```

Add a New-issue button to the filter bar — change the `<div class="filters">…</div>` block to append the button after the `{#each}`:

```svelte
<div class="filters">
  {#each FILTERS as f (f)}
    <button class="chip" class:active={githubState.issueState === f} type="button" onclick={() => githubState.setIssueState(f)}>{f}</button>
  {/each}
  <button class="new-issue" type="button" onclick={() => githubActions.open({ kind: "create" })}>New issue</button>
</div>
```

Inside the `<li class="row">`, add a row-actions block right after the `.meta` div (between `.meta` and the `{#if it.labels.length}` block):

```svelte
        <div class="row-actions">
          <button type="button" onclick={() => githubActions.open({ kind: "comment", target: "issue", number: it.number, title: it.title })}>Comment</button>
          {#if it.state === "OPEN"}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: it.number, title: it.title, to: "closed" })}>Close</button>
          {:else}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: it.number, title: it.title, to: "open" })}>Reopen</button>
          {/if}
        </div>
```

Add the CSS inside the component's `<style>` (after the `.more` rule):

```css
  .new-issue {
    margin-left: auto;
    padding: 3px 12px;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    cursor: pointer;
  }
  .row-actions {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }
  .row-actions button {
    padding: 2px 10px;
    font-size: 11.5px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--btn-bg);
    color: var(--text);
    cursor: pointer;
  }
  .row-actions button:hover {
    border-color: var(--accent);
  }
```

- [ ] **Step 3: Mount the modal.**

In `src/lib/components/github/GithubView.svelte`, add the import (with the other component imports):

```typescript
  import GithubActionModal from "./GithubActionModal.svelte";
```

Then add the modal just before the closing `</section>` (so it overlays regardless of the active tab):

```svelte
  <GithubActionModal />
</section>
```

- [ ] **Step 4: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubPulls.svelte src/lib/components/github/GithubIssues.svelte src/lib/components/github/GithubView.svelte
git commit -m "feat(github): row action triggers + New-issue button + mount the action modal"
```

---

## Task 6: Full verification + smoke

**Files:** none.

- [ ] **Step 1: Gates.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run check
npx vitest run
cd src-tauri && cargo test
```
Expected: svelte-check 0/0 · vitest all pass · cargo all pass (+ `merge_flag` test).

- [ ] **Step 2: Build + smoke against a repo you OWN (writes are real!).**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run tauri build
```
On a throwaway/test repo you own:
- **Comment:** open the Issues tab → a row's "Comment" → type → Comment → confirm the comment lands on github.com.
- **Close/Reopen:** close an open issue → it leaves the "open" filter → reopen from the "closed" filter.
- **New issue:** the "New issue" button → title + body → Create → the issue appears in the list.
- **Merge:** on a test PR, the "Merge" button → pick squash → Merge → the PR leaves the "open" filter and shows merged.
- Verify Cancel + Escape close the modal; a failed action (e.g. merge a PR with failing checks) shows the error and keeps the modal open.

- [ ] **Step 3: Proceed to review + merge.**

---

## Done criteria (Phase 4 — and the whole GitHub screen)

- All four write actions work through the modal, with bodies passed via stdin, validated method/state, a confirm/composer per kind, error display, and refresh-after-success.
- Gates green; the `merge_flag` allowlist is unit-tested; the live write paths are manual-only (they mutate real GitHub — never run in tests).
- **The GitHub screen is feature-complete** (Phases 1–4). Only the deferred items remain (last-active sub-tab persistence; GitHub Projects).

## After all tasks

Adversarial review of the branch diff (`superpowers:code-reviewer`) — pay special attention to: stdin body handling, the method/state allowlists, no-shell arg passing, the modal's busy/cancel guards, and refresh-after-action correctness. Fix Critical/Important, then **superpowers:finishing-a-development-branch** to ff-merge `feat/github-screen-p4` → `main`.
