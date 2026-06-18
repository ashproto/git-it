# GitHub Screen — PR/Issue Detail View (Slice 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clicking a PR or issue opens a full in-app **detail view** (← Back to the list) with the rendered (sanitized-markdown) description, metadata, comment thread, and — for PRs — CI checks, reviews, and changed-file links.

**Architecture:** Two new `gh … view` commands return rich detail DTOs (a pure `statusCheckRollup`→`GhCheck` normalizer is unit-tested). `githubState` gains a `selectedItem` + a detail panel cache. A new `Markdown.svelte` renders descriptions/comments via `marked` + `dompurify` (sanitized). `GithubDetail.svelte` composes it all and reuses the Phase-4 action modal; the PRs/Issues tab components render it in place of the list when an item is selected.

**Tech Stack:** Tauri 2 (Rust) · SvelteKit 5 / Svelte 5 runes · `gh` CLI · `marked` + `dompurify` (new) · Vitest (+ `jsdom` dev-dep for the sanitizer test) + `cargo test`.

**Spec:** `docs/superpowers/specs/2026-06-18-github-detail-and-downloads-design.md` (Part A). **Branch:** `feat/github-detail` (off `main` @ `afadecd`, which has Slice 1).

> Verified `gh` detail shapes (live gh 2.86): `comments[]={author{login},body,createdAt}`, `reviews[]={author{login},state,body,submittedAt}`, `files[]={path,additions,deletions}`, `milestone={title,…}|null`, `statusCheckRollup` = the flat CheckRun/StatusContext leaf array. Reuse `resolve_owner_repo`, `run_gh`, `GithubError`, `RawLabel`/`map_label`, `GhLabel`.

> **Scope note:** Changed files = summary + per-file ± + a link to the file's diff on github.com (no in-app diff renderer). Fenced code blocks render as styled `<pre><code>` (Shiki-in-markdown deferred). Returning from a detail scrolls the list to top (exact scroll-restore deferred as polish). These are intentional v1 trims.

---

## File Structure

**Rust:** `src-tauri/src/github/mod.rs` (DTOs + `map_check` + `pr_detail`/`issue_detail` + tests), `commands.rs`, `lib.rs`.
**TS:** `src/lib/types.ts` (detail types), `src/lib/api.ts` (2 wrappers), `src/lib/githubState.svelte.ts` (selectedItem + detail panels), new `src/lib/github/markdown.ts` (+ `markdown.test.ts`).
**Svelte:** new `src/lib/components/github/Markdown.svelte`, `GithubDetail.svelte`; modify `GithubPulls.svelte`, `GithubIssues.svelte`.

---

## Task 1: Rust — detail commands + DTOs

**Files:** Modify `src-tauri/src/github/mod.rs`, `commands.rs`, `lib.rs`.

- [ ] **Step 1: Add the check normalizer + detail DTOs + the two fns.**

Append to `src-tauri/src/github/mod.rs`:

```rust
// ---- statusCheckRollup normalization (CheckRun + StatusContext leaves) ----
#[derive(Deserialize)]
struct RawCheck {
    #[serde(rename = "__typename", default)]
    typename: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    conclusion: String,
    #[serde(rename = "detailsUrl", default)]
    details_url: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    state: String,
    #[serde(rename = "targetUrl", default)]
    target_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhCheck {
    pub name: String,
    pub bucket: String, // pass | fail | pending | neutral
    pub url: String,
}

fn map_check(c: RawCheck) -> GhCheck {
    if c.typename == "StatusContext" {
        let bucket = match c.state.as_str() {
            "SUCCESS" => "pass",
            "FAILURE" | "ERROR" => "fail",
            "PENDING" | "EXPECTED" => "pending",
            _ => "neutral",
        };
        GhCheck { name: c.context, bucket: bucket.to_string(), url: c.target_url }
    } else {
        // CheckRun: pass/fail only once completed; otherwise pending.
        let bucket = match (c.status.as_str(), c.conclusion.as_str()) {
            ("COMPLETED", "SUCCESS") => "pass",
            ("COMPLETED", "FAILURE") | ("COMPLETED", "TIMED_OUT") => "fail",
            ("COMPLETED", _) => "neutral",
            _ => "pending",
        };
        GhCheck { name: c.name, bucket: bucket.to_string(), url: c.details_url }
    }
}

// ---- shared detail sub-DTOs ----
#[derive(Deserialize)]
struct RawMilestoneRef {
    title: String,
}
#[derive(Deserialize)]
struct RawComment {
    author: Option<RawUser>,
    #[serde(default)]
    body: String,
    #[serde(rename = "createdAt", default)]
    created_at: String,
}
#[derive(Deserialize)]
struct RawReview {
    author: Option<RawUser>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: String,
    #[serde(rename = "submittedAt", default)]
    submitted_at: String,
}
#[derive(Deserialize)]
struct RawFile {
    path: String,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhReview {
    pub author: String,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhFile {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
}

fn map_comment(c: RawComment) -> GhComment {
    GhComment { author: c.author.map(|a| a.login).unwrap_or_default(), body: c.body, created_at: c.created_at }
}
fn map_review(r: RawReview) -> GhReview {
    GhReview {
        author: r.author.map(|a| a.login).unwrap_or_default(),
        state: r.state,
        body: r.body,
        submitted_at: r.submitted_at,
    }
}

// ---- PR detail ----
#[derive(Deserialize)]
struct RawPullDetail {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    author: Option<RawUser>,
    state: String,
    #[serde(rename = "isDraft", default)]
    is_draft: bool,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    assignees: Vec<RawUser>,
    milestone: Option<RawMilestoneRef>,
    #[serde(rename = "baseRefName", default)]
    base_ref_name: String,
    #[serde(rename = "headRefName", default)]
    head_ref_name: String,
    #[serde(rename = "reviewDecision", default)]
    review_decision: String,
    #[serde(default)]
    mergeable: String,
    #[serde(rename = "mergeStateStatus", default)]
    merge_state_status: String,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(rename = "changedFiles", default)]
    changed_files: u64,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    reviews: Vec<RawReview>,
    #[serde(rename = "statusCheckRollup", default)]
    status_check_rollup: Vec<RawCheck>,
    #[serde(default)]
    comments: Vec<RawComment>,
    #[serde(rename = "createdAt", default)]
    created_at: String,
    #[serde(rename = "updatedAt", default)]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhPullDetail {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub state: String,
    pub is_draft: bool,
    pub labels: Vec<GhLabel>,
    pub assignees: Vec<String>,
    pub milestone: Option<String>,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub review_decision: String,
    pub mergeable: String,
    pub merge_state_status: String,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub files: Vec<GhFile>,
    pub reviews: Vec<GhReview>,
    pub checks: Vec<GhCheck>,
    pub comments: Vec<GhComment>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

fn map_pull_detail(p: RawPullDetail) -> GhPullDetail {
    GhPullDetail {
        number: p.number,
        title: p.title,
        body: p.body,
        author: p.author.map(|a| a.login).unwrap_or_default(),
        state: p.state,
        is_draft: p.is_draft,
        labels: p.labels.into_iter().map(map_label).collect(),
        assignees: p.assignees.into_iter().map(|a| a.login).collect(),
        milestone: p.milestone.map(|m| m.title),
        base_ref_name: p.base_ref_name,
        head_ref_name: p.head_ref_name,
        review_decision: p.review_decision,
        mergeable: p.mergeable,
        merge_state_status: p.merge_state_status,
        additions: p.additions,
        deletions: p.deletions,
        changed_files: p.changed_files,
        files: p.files.into_iter().map(|f| GhFile { path: f.path, additions: f.additions, deletions: f.deletions }).collect(),
        reviews: p.reviews.into_iter().map(map_review).collect(),
        checks: p.status_check_rollup.into_iter().map(map_check).collect(),
        comments: p.comments.into_iter().map(map_comment).collect(),
        created_at: p.created_at,
        updated_at: p.updated_at,
        url: p.url,
    }
}

pub fn pr_detail(repo: &Path, number: u64) -> Result<GhPullDetail, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    let json = run_gh(
        &[
            "pr", "view", &num, "--repo", &slug, "--json",
            "number,title,body,author,state,isDraft,labels,assignees,milestone,baseRefName,headRefName,reviewDecision,mergeable,mergeStateStatus,additions,deletions,changedFiles,files,reviews,statusCheckRollup,comments,createdAt,updatedAt,url",
        ],
        None,
    )?;
    let raw: RawPullDetail =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse pr detail: {e}")))?;
    Ok(map_pull_detail(raw))
}

// ---- Issue detail ----
#[derive(Deserialize)]
struct RawIssueDetail {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    author: Option<RawUser>,
    state: String,
    #[serde(rename = "stateReason", default)]
    state_reason: Option<String>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    assignees: Vec<RawUser>,
    milestone: Option<RawMilestoneRef>,
    #[serde(default)]
    comments: Vec<RawComment>,
    #[serde(rename = "createdAt", default)]
    created_at: String,
    #[serde(rename = "updatedAt", default)]
    updated_at: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhIssueDetail {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub state: String,
    pub state_reason: Option<String>,
    pub labels: Vec<GhLabel>,
    pub assignees: Vec<String>,
    pub milestone: Option<String>,
    pub comments: Vec<GhComment>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

pub fn issue_detail(repo: &Path, number: u64) -> Result<GhIssueDetail, GithubError> {
    let (owner, name) = resolve_owner_repo(repo).ok_or(GithubError::NoRemote)?;
    let slug = format!("{owner}/{name}");
    let num = number.to_string();
    let json = run_gh(
        &[
            "issue", "view", &num, "--repo", &slug, "--json",
            "number,title,body,author,state,stateReason,labels,assignees,milestone,comments,createdAt,updatedAt,url",
        ],
        None,
    )?;
    let raw: RawIssueDetail =
        serde_json::from_str(&json).map_err(|e| GithubError::Other(format!("parse issue detail: {e}")))?;
    Ok(GhIssueDetail {
        number: raw.number,
        title: raw.title,
        body: raw.body,
        author: raw.author.map(|a| a.login).unwrap_or_default(),
        state: raw.state,
        state_reason: raw.state_reason,
        labels: raw.labels.into_iter().map(map_label).collect(),
        assignees: raw.assignees.into_iter().map(|a| a.login).collect(),
        milestone: raw.milestone.map(|m| m.title),
        comments: raw.comments.into_iter().map(map_comment).collect(),
        created_at: raw.created_at,
        updated_at: raw.updated_at,
        url: raw.url,
    })
}
```

- [ ] **Step 2: Tests (check normalization + detail mapping from real shapes).**

Add inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn map_check_handles_both_typenames() {
        let cr: RawCheck = serde_json::from_str(
            r#"{"__typename":"CheckRun","name":"build","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"u"}"#,
        )
        .unwrap();
        assert_eq!(map_check(cr).bucket, "pass");
        let cr2: RawCheck = serde_json::from_str(
            r#"{"__typename":"CheckRun","name":"x","status":"IN_PROGRESS","conclusion":""}"#,
        )
        .unwrap();
        assert_eq!(map_check(cr2).bucket, "pending");
        let sc: RawCheck = serde_json::from_str(
            r#"{"__typename":"StatusContext","context":"ci/circleci","state":"FAILURE","targetUrl":"t"}"#,
        )
        .unwrap();
        let g = map_check(sc);
        assert_eq!(g.name, "ci/circleci");
        assert_eq!(g.bucket, "fail");
        assert_eq!(g.url, "t");
    }

    #[test]
    fn map_pull_detail_shapes_nested_arrays() {
        let json = r#"{
            "number":13,"title":"T","body":"hi","author":{"login":"me"},"state":"OPEN","isDraft":false,
            "labels":[{"name":"bug","color":"f00"}],"assignees":[{"login":"you"}],"milestone":{"title":"M1"},
            "baseRefName":"main","headRefName":"f","reviewDecision":"APPROVED","mergeable":"MERGEABLE",
            "mergeStateStatus":"CLEAN","additions":10,"deletions":2,"changedFiles":1,
            "files":[{"path":"a.rs","additions":10,"deletions":2}],
            "reviews":[{"author":{"login":"rev"},"state":"APPROVED","body":"lgtm","submittedAt":"2026-01-01T00:00:00Z"}],
            "statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"d"}],
            "comments":[{"author":{"login":"c"},"body":"nice","createdAt":"2026-01-02T00:00:00Z"}],
            "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-02T00:00:00Z","url":"u"
        }"#;
        let d = map_pull_detail(serde_json::from_str(json).unwrap());
        assert_eq!(d.author, "me");
        assert_eq!(d.milestone.as_deref(), Some("M1"));
        assert_eq!(d.assignees, vec!["you".to_string()]);
        assert_eq!(d.files[0].path, "a.rs");
        assert_eq!(d.reviews[0].author, "rev");
        assert_eq!(d.checks[0].bucket, "pass");
        assert_eq!(d.comments[0].author, "c");
    }

    #[test]
    fn issue_detail_dtos_tolerate_nulls() {
        // milestone null, no reviews/files; verify the issue detail struct compiles and maps.
        let raw: RawIssueDetail = serde_json::from_str(
            r#"{"number":1,"title":"t","body":"","author":null,"state":"CLOSED","stateReason":"completed",
                "labels":[],"assignees":[],"milestone":null,"comments":[],
                "createdAt":"","updatedAt":"","url":"u"}"#,
        )
        .unwrap();
        assert_eq!(raw.number, 1);
        assert_eq!(raw.state_reason.as_deref(), Some("completed"));
        assert!(raw.milestone.is_none());
    }
```

- [ ] **Step 3: Commands + register.**

`src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn github_pr_detail(repo: String, number: u64) -> Result<github::GhPullDetail, github::GithubError> {
    github::pr_detail(&PathBuf::from(repo), number)
}

#[tauri::command]
pub fn github_issue_detail(repo: String, number: u64) -> Result<github::GhIssueDetail, github::GithubError> {
    github::issue_detail(&PathBuf::from(repo), number)
}
```

`src-tauri/src/lib.rs` handler list:

```rust
    commands::github_pr_detail,
    commands::github_issue_detail,
```

- [ ] **Step 4: Build + test.** `cd /Users/ashshah/Projects/GIT-GUI/git-it/src-tauri && cargo test` — clean; all pass (112 + 3 new). **Do NOT run any write command.**

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src-tauri/src/github/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(github): pr/issue detail commands + statusCheckRollup normalization"
```

---

## Task 2: TS — detail types, api wrappers, selection state

**Files:** Modify `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/githubState.svelte.ts`.

- [ ] **Step 1: Add the types.**

Append to `src/lib/types.ts`:

```typescript
export type GhCheck = { name: string; bucket: "pass" | "fail" | "pending" | "neutral"; url: string };
export type GhComment = { author: string; body: string; createdAt: string };
export type GhReview = { author: string; state: string; body: string; submittedAt: string };
export type GhFile = { path: string; additions: number; deletions: number };

export type GhPullDetail = {
  number: number; title: string; body: string; author: string; state: string; isDraft: boolean;
  labels: GhLabel[]; assignees: string[]; milestone: string | null;
  baseRefName: string; headRefName: string;
  reviewDecision: string; mergeable: string; mergeStateStatus: string;
  additions: number; deletions: number; changedFiles: number;
  files: GhFile[]; reviews: GhReview[]; checks: GhCheck[]; comments: GhComment[];
  createdAt: string; updatedAt: string; url: string;
};
export type GhIssueDetail = {
  number: number; title: string; body: string; author: string; state: string; stateReason: string | null;
  labels: GhLabel[]; assignees: string[]; milestone: string | null;
  comments: GhComment[]; createdAt: string; updatedAt: string; url: string;
};
```

- [ ] **Step 2: Add the api wrappers.**

In `src/lib/api.ts`, add `GhPullDetail`, `GhIssueDetail` to the `import type` block, then near the other github wrappers:

```typescript
  githubPrDetail: (repo: string, number: number) =>
    invoke<GhPullDetail>("github_pr_detail", { repo, number }),
  githubIssueDetail: (repo: string, number: number) =>
    invoke<GhIssueDetail>("github_issue_detail", { repo, number }),
```

- [ ] **Step 3: Add selection + detail panels to `githubState`.**

In `src/lib/githubState.svelte.ts`:

(a) extend the `import type` block with `GhPullDetail, GhIssueDetail`.

(b) inside `makeGithubState()`, after the insight panels, add:

```typescript
  const prDetail = makePanel<GhPullDetail>();
  const issueDetail = makePanel<GhIssueDetail>();
  let selectedItem = $state<{ kind: "pr" | "issue"; number: number } | null>(null);

  function loadPrDetail(repo: string, number: number) {
    return prDetail.load(`${repo}|${number}|${reloadNonce}`, () => api.githubPrDetail(repo, number));
  }
  function loadIssueDetail(repo: string, number: number) {
    return issueDetail.load(`${repo}|${number}|${reloadNonce}`, () => api.githubIssueDetail(repo, number));
  }
```

(c) in `ensure(repo)`, in the reset block, also clear the selection + detail panels:

```typescript
    selectedItem = null;
    prDetail.reset();
    issueDetail.reset();
```

(d) change `setActiveTab` to clear the selection when switching tabs:

```typescript
    setActiveTab(t: GithubTab) {
      activeTab = t;
      selectedItem = null;
    },
```

(e) in the returned object, add:

```typescript
    get selectedItem() {
      return selectedItem;
    },
    openItem(kind: "pr" | "issue", number: number) {
      selectedItem = { kind, number };
    },
    closeItem() {
      selectedItem = null;
    },
    get prDetail() {
      return prDetail;
    },
    get issueDetail() {
      return issueDetail;
    },
    loadPrDetail,
    loadIssueDetail,
```

- [ ] **Step 4: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 5: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/types.ts src/lib/api.ts src/lib/githubState.svelte.ts
git commit -m "feat(github): detail types/api + selectedItem & detail-panel state"
```

---

## Task 3: Markdown rendering (deps + sanitizer + component)

**Files:** `package.json` (deps); Create `src/lib/github/markdown.ts`, `src/lib/github/markdown.test.ts`, `src/lib/components/github/Markdown.svelte`.

- [ ] **Step 1: Install the dependencies.**

Run:
```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm install marked dompurify
npm install -D jsdom
```
Expected: `marked` + `dompurify` in `dependencies` (both ship their own TypeScript types — do NOT add `@types/dompurify`, which conflicts with dompurify v3's bundled types), `jsdom` in `devDependencies`.

- [ ] **Step 2: Write the failing sanitizer test.**

Create `src/lib/github/markdown.test.ts` (runs in the jsdom env so DOMPurify has a DOM):

```typescript
// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { mdToSafeHtml } from "./markdown";

describe("mdToSafeHtml", () => {
  it("renders basic markdown", () => {
    const html = mdToSafeHtml("# Hi\n\n- a\n- b");
    expect(html).toContain("<h1");
    expect(html).toContain("<li>a</li>");
  });
  it("strips <script> tags", () => {
    expect(mdToSafeHtml("hi <script>alert(1)</script>")).not.toContain("<script");
  });
  it("strips event handlers and javascript: urls", () => {
    const html = mdToSafeHtml('<a href="javascript:alert(1)" onclick="x()">x</a>');
    expect(html).not.toContain("javascript:");
    expect(html.toLowerCase()).not.toContain("onclick");
  });
  it("opens links in a new tab", () => {
    expect(mdToSafeHtml("[x](https://example.com)")).toContain('target="_blank"');
  });
});
```

- [ ] **Step 3: Run it — expect FAIL.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github/markdown`
Expected: FAIL — cannot resolve `./markdown`.

- [ ] **Step 4: Implement the helper.**

Create `src/lib/github/markdown.ts`:

```typescript
import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: true });

// Open all links in a new tab + harden rel. (Global hook; added once on import.)
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noreferrer");
  }
});

const CONFIG = {
  ALLOWED_TAGS: [
    "h1", "h2", "h3", "h4", "h5", "h6", "p", "br", "hr", "ul", "ol", "li", "a",
    "code", "pre", "blockquote", "strong", "em", "del", "img", "table", "thead",
    "tbody", "tr", "th", "td", "input", "span", "kbd", "sup", "sub",
  ],
  ALLOWED_ATTR: ["href", "src", "alt", "title", "class", "type", "checked", "disabled", "align"],
  ALLOWED_URI_REGEXP: /^(?:https?:|mailto:|#)/i,
};

/** Render GitHub-flavored markdown to SANITIZED HTML (third-party content). */
export function mdToSafeHtml(src: string): string {
  const raw = marked.parse(src ?? "", { async: false }) as string;
  return DOMPurify.sanitize(raw, CONFIG);
}
```

- [ ] **Step 5: Run it — expect PASS.**

Run: `cd /Users/ashshah/Projects/GIT-GUI/git-it && npx vitest run src/lib/github/markdown`
Expected: all PASS.

- [ ] **Step 6: Create the component.**

Create `src/lib/components/github/Markdown.svelte`:

```svelte
<script lang="ts">
  import { mdToSafeHtml } from "../../github/markdown";
  let { src }: { src: string } = $props();
  const html = $derived(mdToSafeHtml(src));
</script>

<div class="md">{@html html}</div>

<style>
  .md {
    font-size: 13px;
    line-height: 1.6;
    color: var(--text);
    overflow-wrap: break-word;
    word-break: break-word;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    font-size: 14px;
    margin: 12px 0 6px;
  }
  .md :global(p) {
    margin: 6px 0;
  }
  .md :global(a) {
    color: var(--accent);
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 20px;
    margin: 6px 0;
  }
  .md :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11.5px;
    background: var(--btn-bg);
    padding: 1px 4px;
    border-radius: 4px;
  }
  .md :global(pre) {
    background: var(--btn-bg);
    padding: 10px 12px;
    border-radius: 8px;
    overflow-x: auto;
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
  }
  .md :global(blockquote) {
    border-left: 3px solid var(--border);
    margin: 6px 0;
    padding: 2px 0 2px 12px;
    color: var(--text-muted);
  }
  .md :global(img) {
    max-width: 100%;
  }
  .md :global(table) {
    border-collapse: collapse;
    font-size: 12px;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--border);
    padding: 3px 8px;
  }
</style>
```

- [ ] **Step 7: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add package.json package-lock.json src/lib/github/markdown.ts src/lib/github/markdown.test.ts src/lib/components/github/Markdown.svelte
git commit -m "feat(github): sanitized markdown (marked + dompurify) + Markdown component"
```

---

## Task 4: The detail component

**Files:** Create `src/lib/components/github/GithubDetail.svelte`.

- [ ] **Step 1: Build it.**

Create `src/lib/components/github/GithubDetail.svelte`:

```svelte
<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { githubActions } from "../../githubActions.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import Markdown from "./Markdown.svelte";

  let { kind, number }: { kind: "pr" | "issue"; number: number } = $props();

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce;
    if (repo) {
      if (kind === "pr") void githubState.loadPrDetail(repo, number);
      else void githubState.loadIssueDetail(repo, number);
    }
  });

  const panel = $derived(kind === "pr" ? githubState.prDetail : githubState.issueDetail);
  const d = $derived(panel.data);
  function rel(iso: string): string {
    if (!iso) return "";
    const dt = parseISO(iso);
    return dt ? formatCommitDate(dt, appState.dateFormat, appState.relativeDates) : iso;
  }
  function glyph(b: string): string {
    return b === "pass" ? "✓" : b === "fail" ? "✗" : b === "pending" ? "○" : "–";
  }
</script>

<div class="detail">
  <div class="topbar">
    <button class="back" type="button" onclick={() => githubState.closeItem()}>← Back</button>
  </div>

  {#if panel.status === "loading" && !d}
    <p class="note">Loading…</p>
  {:else if panel.status === "error"}
    <p class="note err">Could not load ({panel.error?.kind}).</p>
  {:else if d}
    <header class="hd">
      <h2><span class="num">#{d.number}</span> {d.title}</h2>
      <div class="sub">
        <span class="state {d.state.toLowerCase()}">{kind === "pr" && d.isDraft ? "DRAFT" : d.state}</span>
        <span>{d.author}</span>
        {#if kind === "pr"}<span class="mono">{d.headRefName} → {d.baseRefName}</span>{/if}
        <span class="when">opened {rel(d.createdAt)}</span>
        <a class="ext" href={d.url} target="_blank" rel="noreferrer">Open on github.com ↗</a>
      </div>
      <div class="acts">
        <button type="button" onclick={() => githubActions.open({ kind: "comment", target: kind, number: d.number, title: d.title })}>Comment</button>
        {#if kind === "pr" && d.state === "OPEN" && !d.isDraft}
          <button type="button" onclick={() => githubActions.open({ kind: "merge", number: d.number, title: d.title })}>Merge</button>
        {/if}
        {#if kind === "issue"}
          {#if d.state === "OPEN"}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: d.number, title: d.title, to: "closed" })}>Close</button>
          {:else}
            <button type="button" onclick={() => githubActions.open({ kind: "setState", number: d.number, title: d.title, to: "open" })}>Reopen</button>
          {/if}
        {/if}
      </div>
    </header>

    {#if d.labels.length || d.assignees.length || d.milestone}
      <div class="meta">
        {#each d.labels as l (l.name)}<span class="label" style={`--lc:#${l.color || "888"}`}>{l.name}</span>{/each}
        {#if d.assignees.length}<span class="m">assigned: {d.assignees.join(", ")}</span>{/if}
        {#if d.milestone}<span class="m">milestone: {d.milestone}</span>{/if}
      </div>
    {/if}

    <section class="body">
      {#if d.body.trim()}<Markdown src={d.body} />{:else}<p class="note">No description.</p>{/if}
    </section>

    {#if kind === "pr"}
      <div class="pr-extra">
        <div class="readiness">
          {#if d.reviewDecision}<span class="pill">{d.reviewDecision.replace(/_/g, " ").toLowerCase()}</span>{/if}
          {#if d.mergeable}<span class="pill">{d.mergeable.toLowerCase()}</span>{/if}
          <span class="diffstat"><span class="add">+{d.additions}</span> <span class="del">−{d.deletions}</span> · {d.changedFiles} files</span>
        </div>
        {#if d.checks.length}
          <details class="block"><summary>Checks ({d.checks.length})</summary>
            <ul class="checks">
              {#each d.checks as c, i (c.name + i)}
                <li><span class="g {c.bucket}">{glyph(c.bucket)}</span><a href={c.url} target="_blank" rel="noreferrer">{c.name}</a></li>
              {/each}
            </ul>
          </details>
        {/if}
        {#if d.reviews.length}
          <details class="block"><summary>Reviews ({d.reviews.length})</summary>
            <ul class="reviews">
              {#each d.reviews as r, i (r.author + i)}
                <li><strong>{r.author}</strong> <span class="rv">{r.state.replace(/_/g, " ").toLowerCase()}</span> <span class="when">{rel(r.submittedAt)}</span></li>
              {/each}
            </ul>
          </details>
        {/if}
        {#if d.files.length}
          <details class="block"><summary>Files ({d.files.length})</summary>
            <ul class="files">
              {#each d.files as f (f.path)}
                <li><a href={`${d.url}/files`} target="_blank" rel="noreferrer" class="mono">{f.path}</a><span class="fstat"><span class="add">+{f.additions}</span> <span class="del">−{f.deletions}</span></span></li>
              {/each}
            </ul>
          </details>
        {/if}
      </div>
    {/if}

    <section class="comments">
      <h3>{d.comments.length} {d.comments.length === 1 ? "comment" : "comments"}</h3>
      {#if d.comments.length === 0}
        <p class="note">No comments yet.</p>
      {:else}
        {#each d.comments as c, i (c.author + i)}
          <article class="comment">
            <div class="chead"><strong>{c.author}</strong> <span class="when">{rel(c.createdAt)}</span></div>
            <Markdown src={c.body} />
          </article>
        {/each}
      {/if}
    </section>
  {/if}
</div>

<style>
  .detail { display: flex; flex-direction: column; gap: 12px; }
  .topbar { position: sticky; top: 0; }
  .back {
    background: none; border: 1px solid var(--border); border-radius: 7px;
    color: var(--text); padding: 4px 12px; font-size: 12.5px; cursor: pointer;
  }
  .back:hover { border-color: var(--accent); }
  .hd h2 { margin: 0; font-size: 17px; }
  .num { color: var(--text-muted); }
  .sub { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; font-size: 12.5px; color: var(--text-muted); margin-top: 6px; }
  .state { font-size: 11px; padding: 1px 8px; border-radius: 999px; border: 1px solid var(--border); text-transform: uppercase; }
  .state.open { color: var(--status-add, #2ea043); border-color: currentColor; }
  .state.closed { color: var(--err, #c0392b); border-color: currentColor; }
  .state.merged { color: var(--accent); border-color: currentColor; }
  .ext { color: var(--accent); text-decoration: none; }
  .acts { display: flex; gap: 8px; margin-top: 10px; }
  .acts button {
    padding: 4px 14px; border: 1px solid var(--border); border-radius: 7px;
    background: var(--btn-bg); color: var(--text); font-size: 12.5px; cursor: pointer;
  }
  .acts button:hover { border-color: var(--accent); }
  .meta { display: flex; flex-wrap: wrap; gap: 6px 10px; align-items: center; }
  .label { font-size: 10.5px; padding: 0 7px; border-radius: 999px; border: 1px solid var(--lc); color: var(--lc); }
  .m { font-size: 12px; color: var(--text-muted); }
  .body { border: 1px solid var(--border); border-radius: 10px; padding: 12px 14px; background: var(--panel-bg); }
  .pr-extra { display: flex; flex-direction: column; gap: 8px; }
  .readiness { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; font-size: 12px; }
  .pill { padding: 1px 8px; border: 1px solid var(--border); border-radius: 999px; text-transform: capitalize; color: var(--text-muted); }
  .diffstat { color: var(--text-muted); }
  .add { color: var(--status-add, #2ea043); }
  .del { color: var(--err, #c0392b); }
  .block { border: 1px solid var(--border); border-radius: 8px; padding: 6px 10px; }
  .block summary { cursor: pointer; font-size: 12.5px; }
  .checks, .reviews, .files { list-style: none; margin: 8px 0 0; padding: 0; }
  .checks li, .files li { display: flex; align-items: center; gap: 8px; padding: 2px 0; font-size: 12px; }
  .files li { justify-content: space-between; }
  .checks a, .files a { color: var(--text); text-decoration: none; }
  .checks a:hover, .files a:hover { color: var(--accent); }
  .g { width: 14px; text-align: center; font-weight: 700; }
  .g.pass { color: var(--status-add, #2ea043); }
  .g.fail { color: var(--err, #c0392b); }
  .g.pending { color: var(--status-mod, #d29922); }
  .reviews li { font-size: 12px; padding: 2px 0; }
  .rv { color: var(--text-muted); text-transform: capitalize; }
  .comments h3 { font-size: 13px; margin: 6px 0; }
  .comment { border: 1px solid var(--border); border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; background: var(--panel-bg); }
  .chead { font-size: 12.5px; margin-bottom: 4px; }
  .when { color: var(--text-muted); }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11.5px; }
  .fstat { font-size: 11.5px; }
  .note { margin: 8px 2px; color: var(--text-muted); font-size: 12.5px; }
  .note.err { color: var(--err, #c0392b); }
</style>
```

- [ ] **Step 2: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 3: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubDetail.svelte
git commit -m "feat(github): GithubDetail (description, checks, reviews, files, comments)"
```

---

## Task 5: Wire row-click → detail in the PR/Issue tabs

**Files:** Modify `src/lib/components/github/GithubPulls.svelte`, `src/lib/components/github/GithubIssues.svelte`.

- [ ] **Step 1: GithubPulls — render the detail in place + make the title open it.**

In `src/lib/components/github/GithubPulls.svelte`, add the import:

```typescript
  import GithubDetail from "./GithubDetail.svelte";
```

Add a derived for the open detail (after `const panel = …`):

```typescript
  const detailNumber = $derived(
    githubState.selectedItem?.kind === "pr" ? githubState.selectedItem.number : null,
  );
```

Wrap the WHOLE existing template (the `{#if panel.status …}` block + filters) so the detail replaces it when open. At the very top of the markup add:

```svelte
{#if detailNumber !== null}
  <GithubDetail kind="pr" number={detailNumber} />
{:else}
```
…and add the matching `{/if}` at the very end of the markup (after the final existing `{/if}`).

Change the PR title from an `<a>` to a button that opens the detail, with a small external link kept. Replace:

```svelte
        <a class="title" href={pr.url} target="_blank" rel="noreferrer">
          <span class="num">#{pr.number}</span>{pr.title}
          {#if pr.isDraft}<span class="badge">draft</span>{/if}
        </a>
```

with:

```svelte
        <button class="title" type="button" onclick={() => githubState.openItem("pr", pr.number)}>
          <span class="num">#{pr.number}</span>{pr.title}
          {#if pr.isDraft}<span class="badge">draft</span>{/if}
        </button>
        <a class="ext" href={pr.url} target="_blank" rel="noreferrer" title="Open on github.com">↗</a>
```

Add CSS (after `.title` styles): make the button look like the old link, and the `.ext` small.

```css
  .title {
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    color: var(--text);
    font-weight: 500;
    font-size: 13.5px;
  }
  .ext {
    color: var(--text-muted);
    text-decoration: none;
    font-size: 12px;
    margin-left: 6px;
  }
  .ext:hover {
    color: var(--accent);
  }
```

(If the file already has a `.title` rule from the `<a>`, replace its selector body with the above.)

- [ ] **Step 2: GithubIssues — same wiring.**

In `src/lib/components/github/GithubIssues.svelte`, add the import:

```typescript
  import GithubDetail from "./GithubDetail.svelte";
```

Add:

```typescript
  const detailNumber = $derived(
    githubState.selectedItem?.kind === "issue" ? githubState.selectedItem.number : null,
  );
```

Wrap the markup with:

```svelte
{#if detailNumber !== null}
  <GithubDetail kind="issue" number={detailNumber} />
{:else}
```
…and the matching `{/if}` at the end.

Replace the issue title `<a>` with the button + external link (same pattern as PRs):

```svelte
        <button class="title" type="button" onclick={() => githubState.openItem("issue", it.number)}>
          <span class="num">#{it.number}</span>{it.title}
        </button>
        <a class="ext" href={it.url} target="_blank" rel="noreferrer" title="Open on github.com">↗</a>
```

Add the same `.title` (button) + `.ext` CSS as in Step 1.

- [ ] **Step 3: Type-check.** `cd /Users/ashshah/Projects/GIT-GUI/git-it && npm run check` — 0/0.

- [ ] **Step 4: Commit.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
git add src/lib/components/github/GithubPulls.svelte src/lib/components/github/GithubIssues.svelte
git commit -m "feat(github): open the in-app detail view when a PR/issue row is clicked"
```

---

## Task 6: Full verification

**Files:** none.

- [ ] **Step 1: Gates.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run check
npx vitest run
cd src-tauri && cargo test
```
Expected: svelte-check 0/0 · vitest all pass (+ the new `markdown` suite, jsdom env) · cargo all pass (+ the 3 new mapping tests).

- [ ] **Step 2: Build + smoke.**

```bash
cd /Users/ashshah/Projects/GIT-GUI/git-it
npm run tauri build
```
On a repo with PRs/issues (e.g. `cli/cli`): click a PR → detail shows rendered description, checks (✓/✗/○), reviews, changed-file links, and the comment thread; the Comment/Merge buttons open the action modal; ← Back returns to the list. Click an issue → description + comments + Close/Reopen/Comment. Confirm a description containing markdown (headings/lists/code/links) renders formatted and that links open in the browser. (Quick safety check: an issue/PR whose body contains literal HTML must render inert — no script execution.)

- [ ] **Step 3: Proceed to review + merge.**

---

## Done criteria (Slice 2)
- Clicking a PR/issue opens a full in-app detail (sanitized markdown description + comments, PR checks/reviews/file links), with Back to the list and the action modal reused. The check normalizer + detail mapping + the markdown sanitizer are unit-tested.
- Gates green. This completes the post-completion enhancement batch (Slices 1 + 2).

## After all tasks
Adversarial review of the branch diff (`superpowers:code-reviewer`) — focus on the **markdown sanitization** (the new attack surface), the detail-panel cache/selection lifecycle, and the row-click wiring. Fix Critical/Important, then **superpowers:finishing-a-development-branch** to ff-merge `feat/github-detail` → `main`.
