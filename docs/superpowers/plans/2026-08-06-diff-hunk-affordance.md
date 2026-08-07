# Diff Hunk/Block Affordance + Line Discard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a nested hover affordance (hunk ring + change-block ring with a floating Stage/Discard/Unstage toolbar) to the working-copy diff, add hunk- and line-level Discard, and replace the line-selection visual that is invisible in the NERV theme.

**Architecture:** Pure block/ordinal math moves to a vitest-covered `src/lib/diff/blocks.ts`. `DiffView.svelte` emits one `<tbody>` per hunk (so the hunk ring is a plain CSS `outline`) and draws the block/selection ring with four *separate CSS custom properties* feeding one `box-shadow` on every `<td>` — these merge instead of overriding, which is what makes a ring around a run of `<tr>`s possible without JS measurement. Only the floating toolbar needs JS, and only for its vertical offset. In Rust, `git_apply` gains a `cached` flag; dropping `--cached` turns the existing reverse-apply into a worktree discard.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, vitest, Rust (cargo workspace), Tauri 2.

## Global Constraints

- **Never `:global()` in `src/lib/theme/nerv.css`** — it is not a Svelte `<style>` block, so the browser silently drops the selector. Use bare `:root[data-theme="nerv"]`.
- **Classic must render byte-identically** except where this plan explicitly changes shared markup. All NERV-only rules stay `data-theme`-scoped.
- **Shell-out safety:** any new `git` invocation with user-controlled operands places `--` / `--end-of-options` before them. (No new operand-bearing invocations here — patches go over stdin.)
- **Business logic that can be pure lives in `src/lib/*.ts` with vitest coverage**, not inside a `.svelte` component.
- **Full green gate before claiming done:** `npm run check` (0 errors) + `npm test` + `cargo test`.
- **Tauri-only paths** (real staging/discarding via `invoke`) cannot be exercised by vitest or the browser preview. They require `npm run tauri dev`.
- Commit subjects: **lowercase first letter, ≤100 chars** (commitlint).
- Branch is already `feat/diff-hunk-affordance`, cut from `next`.

---

### Task 1: Pure block/ordinal math

**Files:**
- Create: `src/lib/diff/blocks.ts`
- Test: `src/lib/diff/blocks.test.ts`

**Interfaces:**
- Consumes: `DiffHunk`, `DiffLineKind` from `src/lib/diff/types.ts`
- Produces:
  - `interface Block { rows: number[]; ords: number[] }`
  - `ordinalsOf(hunk: DiffHunk): (number | null)[]`
  - `blocksOf(hunk: DiffHunk): Block[]`
  - `blockAt(hunk: DiffHunk, row: number): number | null`
  - `ordsInRange(hunk: DiffHunk, from: number, to: number): number[]`

  Vocabulary note for later tasks: a **row** is an index into `hunk.lines`. An **ordinal** is the 0-based position of a line among only the `+`/`-` lines of that hunk — the argument `stage_lines` / `unstage_lines` / `discard_lines` take.

- [ ] **Step 1: Write the failing test**

Create `src/lib/diff/blocks.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { blocksOf, ordinalsOf, blockAt, ordsInRange } from "./blocks";
import type { DiffHunk, DiffLineKind } from "./types";

/**
 * Build a hunk from a compact spec string, one char per line:
 * " " context, "+" add, "-" del.  e.g. "  -+  -++ "
 */
const hunk = (spec: string): DiffHunk => ({
  header: "@@ -1,1 +1,1 @@",
  lines: [...spec].map((ch, i) => ({
    kind: (ch === "+" ? "add" : ch === "-" ? "del" : "context") as DiffLineKind,
    text: `line${i}`,
    oldNo: ch === "+" ? null : i,
    newNo: ch === "-" ? null : i,
  })),
});

// Two blocks: rows 2-3 (ords 0,1) and rows 6-8 (ords 2,3,4).
const TWO_BLOCKS = hunk("  -+  -++ ");

describe("ordinalsOf", () => {
  it("numbers only changed lines, in order, leaving context null", () => {
    expect(ordinalsOf(hunk("  -+ "))).toEqual([null, null, 0, 1, null]);
  });

  it("returns all-null for a hunk of pure context", () => {
    expect(ordinalsOf(hunk("   "))).toEqual([null, null, null]);
  });
});

describe("blocksOf", () => {
  it("groups an adjacent del/add pair into ONE block", () => {
    expect(blocksOf(hunk(" -+ "))).toEqual([{ rows: [1, 2], ords: [0, 1] }]);
  });

  it("splits blocks on context lines and keeps ordinals continuous across them", () => {
    expect(blocksOf(TWO_BLOCKS)).toEqual([
      { rows: [2, 3], ords: [0, 1] },
      { rows: [6, 7, 8], ords: [2, 3, 4] },
    ]);
  });

  it("handles blocks touching the first and last row of the hunk", () => {
    expect(blocksOf(hunk("+  +"))).toEqual([
      { rows: [0], ords: [0] },
      { rows: [3], ords: [1] },
    ]);
  });

  it("returns no blocks for a hunk of pure context", () => {
    expect(blocksOf(hunk("   "))).toEqual([]);
  });
});

describe("blockAt", () => {
  it("returns the index of the block containing the row", () => {
    expect(blockAt(TWO_BLOCKS, 3)).toBe(0);
    expect(blockAt(TWO_BLOCKS, 7)).toBe(1);
  });

  it("returns null for a context row", () => {
    expect(blockAt(TWO_BLOCKS, 5)).toBeNull();
  });
});

describe("ordsInRange", () => {
  it("collects only changed lines when the range spans context rows", () => {
    // rows 2..8 covers both blocks AND the context rows 4,5 between them
    expect(ordsInRange(TWO_BLOCKS, 2, 8)).toEqual([0, 1, 2, 3, 4]);
  });

  it("handles a single-row range", () => {
    expect(ordsInRange(TWO_BLOCKS, 2, 2)).toEqual([0]);
  });

  it("normalizes a reversed range", () => {
    expect(ordsInRange(TWO_BLOCKS, 8, 2)).toEqual([0, 1, 2, 3, 4]);
  });

  it("returns empty when the range covers only context", () => {
    expect(ordsInRange(TWO_BLOCKS, 4, 5)).toEqual([]);
  });

  it("covers every ordinal when given the whole hunk", () => {
    expect(ordsInRange(TWO_BLOCKS, 0, 9)).toEqual([0, 1, 2, 3, 4]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
npx vitest run src/lib/diff/blocks.test.ts
```

Expected: FAIL — `Failed to resolve import "./blocks"`.

- [ ] **Step 3: Write the implementation**

Create `src/lib/diff/blocks.ts`:

```ts
import type { DiffHunk } from "./types";

/** A contiguous run of changed (+/-) lines within one hunk. */
export interface Block {
  /** Indices into `hunk.lines`, ascending and contiguous. */
  rows: number[];
  /** The change-line ordinals for those rows — what the staging backend takes. */
  ords: number[];
}

/**
 * Per row of the hunk: its 0-based change-line ordinal, or null for a context row.
 * Ordinals count ONLY +/- lines, in order — the vocabulary `stage_lines`,
 * `unstage_lines` and `discard_lines` expect.
 */
export function ordinalsOf(hunk: DiffHunk): (number | null)[] {
  let n = 0;
  return hunk.lines.map((l) => (l.kind === "context" ? null : n++));
}

/** Contiguous runs of changed lines. A context line breaks a run. */
export function blocksOf(hunk: DiffHunk): Block[] {
  const out: Block[] = [];
  let cur: Block | null = null;
  let ord = 0;
  for (let i = 0; i < hunk.lines.length; i++) {
    if (hunk.lines[i].kind === "context") {
      cur = null;
      continue;
    }
    if (cur === null) {
      cur = { rows: [], ords: [] };
      out.push(cur);
    }
    cur.rows.push(i);
    cur.ords.push(ord++);
  }
  return out;
}

/** Index of the block containing `row`, or null when `row` is a context line. */
export function blockAt(hunk: DiffHunk, row: number): number | null {
  const i = blocksOf(hunk).findIndex((b) => b.rows.includes(row));
  return i === -1 ? null : i;
}

/**
 * Change-line ordinals inside an inclusive row range (either order). Context rows
 * inside the range are skipped, so a range may span unchanged rows and still be
 * contiguous in the only sense the backend cares about.
 */
export function ordsInRange(hunk: DiffHunk, from: number, to: number): number[] {
  const ords = ordinalsOf(hunk);
  const lo = Math.min(from, to);
  const hi = Math.max(from, to);
  const out: number[] = [];
  for (let i = lo; i <= hi; i++) {
    const o = ords[i];
    if (o !== null && o !== undefined) out.push(o);
  }
  return out;
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
npx vitest run src/lib/diff/blocks.test.ts
```

Expected: PASS — 13 tests.

- [ ] **Step 5: Run the type gate**

```bash
npm run check
```

Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add src/lib/diff/blocks.ts src/lib/diff/blocks.test.ts
git commit -m "feat: add pure block/ordinal helpers for diff hunks"
```

---

### Task 2: `discard_hunk` / `discard_lines` in git-core

**Files:**
- Modify: `crates/git-core/src/ops_worktree.rs:264-288` (`git_apply` gains a `cached` param), `:301`, `:315`, `:462`, `:475` (existing call sites), and append two public functions after `unstage_lines` (`:476`)
- Test: `crates/git-core/src/ops_worktree.rs` (the existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `diff`, `split_hunks`, `build_partial_hunk`, `git_apply` in the same file. `TempRepo` test fixture (`:557`) with methods `new()`, `git(&[&str])`, `write(f, s)`, `commit_file(f, s, m)`, `staged_paths()`, field `path`.
- Produces:
  - `pub fn discard_hunk(repo: &Path, path: &str, hunk_index: usize, context: u32) -> Result<(), String>`
  - `pub fn discard_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize], context: u32) -> Result<(), String>`

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests` block in `crates/git-core/src/ops_worktree.rs`, next to `stage_lines_stages_only_selected_lines`:

```rust
    // ── discard (worktree reverse-apply) ─────────────────────────────────────

    #[test]
    fn discard_hunk_reverts_only_that_hunk() {
        let r = TempRepo::new();
        // 20 lines so two separated edits land in two hunks at -U3.
        let base: String = (1..=20).map(|i| format!("line{}\n", i)).collect();
        r.commit_file("f.txt", &base, "init");

        let mut edited: Vec<String> = (1..=20).map(|i| format!("line{}\n", i)).collect();
        edited[1] = "CHANGED2\n".to_string();
        edited[17] = "CHANGED18\n".to_string();
        r.write("f.txt", &edited.concat());

        let d = diff(&r.path, Some("f.txt"), false, 3).unwrap();
        let (_h, hunks) = split_hunks(&d);
        assert_eq!(hunks.len(), 2, "expected two hunks, diff was:\n{}", d);

        discard_hunk(&r.path, "f.txt", 0, 3).unwrap();

        let now = fs::read_to_string(r.path.join("f.txt")).unwrap();
        assert!(now.contains("line2\n"), "hunk 0 should be reverted");
        assert!(!now.contains("CHANGED2"), "hunk 0's change should be gone");
        assert!(now.contains("CHANGED18"), "hunk 1 must be untouched");
    }

    #[test]
    fn discard_lines_reverts_only_selected() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "base\n", "init");
        r.write("f.txt", "base\nline1\nline2\nline3\n");
        // ordinals 0,1,2 == +line1,+line2,+line3 — discard only line2.
        discard_lines(&r.path, "f.txt", 0, &[1], 3).unwrap();
        assert_eq!(
            fs::read_to_string(r.path.join("f.txt")).unwrap(),
            "base\nline1\nline3\n",
            "only the selected line should be reverted"
        );
    }

    /// The load-bearing one: discarding UNSTAGED lines reverts them to the INDEX
    /// state, not to HEAD, so staged work on the same file survives. This is what
    /// makes Discard safe to sit beside Stage.
    #[test]
    fn discard_lines_leaves_staged_changes_intact() {
        let r = TempRepo::new();
        r.commit_file("f.txt", "base\n", "init");
        r.write("f.txt", "base\nkeep\ndrop\n");

        // Stage only `keep` (ordinal 0); `drop` stays unstaged.
        stage_lines(&r.path, "f.txt", 0, &[0], 3).unwrap();
        let staged = diff(&r.path, Some("f.txt"), true, 3).unwrap();
        assert!(staged.contains("+keep"), "precondition: keep must be staged");

        // The unstaged diff now holds exactly one change line (`+drop`) at ordinal 0.
        discard_lines(&r.path, "f.txt", 0, &[0], 3).unwrap();

        assert_eq!(
            fs::read_to_string(r.path.join("f.txt")).unwrap(),
            "base\nkeep\n",
            "drop reverted to the INDEX state, not HEAD"
        );
        let still = diff(&r.path, Some("f.txt"), true, 3).unwrap();
        assert!(still.contains("+keep"), "staged work must survive the discard");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p git-core discard_
```

Expected: FAIL to compile — `cannot find function 'discard_hunk' in this scope`.

- [ ] **Step 3: Add the `cached` flag to `git_apply`**

In `crates/git-core/src/ops_worktree.rs`, replace the signature and body head of `git_apply` (currently at `:262-268`):

```rust
/// Pipe a patch (reconstructed from git's own diff output) to `git apply` via stdin.
/// `cached` selects the target: true → `--cached` (the index, for stage/unstage),
/// false → the WORKING TREE only (for discard). Never uses a temp file or shell.
fn git_apply(repo: &Path, patch: &str, reverse: bool, cached: bool) -> Result<(), String> {
    let mut c = Command::new("git");
    c.current_dir(repo).arg("apply");
    if cached {
        c.arg("--cached");
    }
    if reverse {
        c.arg("--reverse");
    }
```

The rest of the function body (from `c.arg("-")` onward) is unchanged.

- [ ] **Step 4: Update the four existing call sites to pass `cached: true`**

| Location | Before | After |
|---|---|---|
| `stage_hunk` (`:301`) | `git_apply(repo, &format!("{}{}", header, h), false)` | `git_apply(repo, &format!("{}{}", header, h), false, true)` |
| `unstage_hunk` (`:315`) | `git_apply(repo, &format!("{}{}", header, h), true)` | `git_apply(repo, &format!("{}{}", header, h), true, true)` |
| `stage_lines` (`:462`) | `git_apply(repo, &format!("{}{}", header, partial), false)` | `git_apply(repo, &format!("{}{}", header, partial), false, true)` |
| `unstage_lines` (`:475`) | `git_apply(repo, &format!("{}{}", header, partial), true)` | `git_apply(repo, &format!("{}{}", header, partial), true, true)` |

- [ ] **Step 5: Add the two discard functions**

Append after `unstage_lines` (i.e. after `:476`), before `stash_push`:

```rust
/// Discard one hunk (by index) of `path`'s UNSTAGED diff: reverse-apply it to the
/// WORKING TREE only (no `--cached`). Because the unstaged diff's old side is the
/// INDEX, the lines revert to their staged state — staged changes to the same file
/// are untouched. DESTRUCTIVE / not undoable.
///
/// Hunk indices refer to the CURRENT live diff; after a successful op the remaining
/// diff re-indexes, so callers must re-fetch before issuing another.
pub fn discard_hunk(repo: &Path, path: &str, hunk_index: usize, context: u32) -> Result<(), String> {
    let d = diff(repo, Some(path), false, context)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    git_apply(repo, &format!("{}{}", header, h), true, false)
}

/// Discard selected change-line ordinals of one hunk of `path`'s UNSTAGED diff.
/// `build_partial_hunk(.., reverse: true)` emits a patch whose NEW side matches the
/// working file (unselected `+` become context because they ARE present there;
/// unselected `-` are dropped because they are not) — exactly what a worktree
/// reverse-apply needs. DESTRUCTIVE / not undoable.
pub fn discard_lines(repo: &Path, path: &str, hunk_index: usize, selected: &[usize], context: u32) -> Result<(), String> {
    let d = diff(repo, Some(path), false, context)?;
    let (header, hunks) = split_hunks(&d);
    let h = hunks.get(hunk_index).ok_or("hunk index out of range")?;
    let set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let partial = build_partial_hunk(h, &set, true).ok_or("no lines selected to discard")?;
    git_apply(repo, &format!("{}{}", header, partial), true, false)
}
```

- [ ] **Step 6: Run the new tests**

```bash
cargo test -p git-core discard_
```

Expected: PASS — including the three new tests.

- [ ] **Step 7: Run the whole crate to prove the `git_apply` change broke nothing**

```bash
cargo test -p git-core
```

Expected: PASS — all staging round-trip tests still green.

- [ ] **Step 8: Commit**

```bash
git add crates/git-core/src/ops_worktree.rs
git commit -m "feat: add hunk and line discard to the working-copy ops"
```

---

### Task 3: Plumb discard through Tauri, api, and gitActions

**Files:**
- Modify: `src-tauri/src/commands.rs` (append after `unstage_lines`, `:463`)
- Modify: `src-tauri/src/lib.rs:202` (after `commands::unstage_lines,`)
- Modify: `src/lib/api.ts:200` (after `unstageLines`)
- Modify: `src/lib/gitActions.ts:872` (after `unstageLines`)

**Interfaces:**
- Consumes: `ops_worktree::discard_hunk` / `discard_lines` (Task 2); `runWorktree(label, fn)` (`gitActions.ts:522`, already guards `isTauri()` and `appState.repo`); `dialogs.confirm({ title, message, confirmLabel, danger })`; `appState.effectiveDiffContext`.
- Produces:
  - `api.discardHunk(repo, path, hunkIndex, context?) => Promise<void>`
  - `api.discardLines(repo, path, hunkIndex, selected, context?) => Promise<void>`
  - `gitActions.discardHunk(path: string, hunkIndex: number) => Promise<boolean>`
  - `gitActions.discardLines(path: string, hunkIndex: number, selected: number[]) => Promise<boolean>`

  There is no unit test at this layer — it is `invoke` glue. The gate is that both toolchains compile and the existing suites stay green.

- [ ] **Step 1: Add the Tauri commands**

In `src-tauri/src/commands.rs`, append immediately after the `unstage_lines` command:

```rust
#[tauri::command]
pub fn discard_hunk(repo: String, path: String, hunk_index: usize, context: u32) -> Result<(), String> {
    ops_worktree::discard_hunk(&PathBuf::from(repo), &path, hunk_index, context)
}

#[tauri::command]
pub fn discard_lines(repo: String, path: String, hunk_index: usize, selected: Vec<usize>, context: u32) -> Result<(), String> {
    ops_worktree::discard_lines(&PathBuf::from(repo), &path, hunk_index, &selected, context)
}
```

These are deliberately **synchronous**, matching the neighbouring staging commands: `git apply` on a local repo is fast, and the `#[tauri::command(async)]` rule targets slow external processes (`gh`, network `git`).

- [ ] **Step 2: Register them**

In `src-tauri/src/lib.rs`, in the `generate_handler!` list, add after `commands::unstage_lines,`:

```rust
            commands::discard_hunk,
            commands::discard_lines,
```

- [ ] **Step 3: Verify the Rust side compiles**

```bash
cargo check -p git-it
```

Expected: finishes with no errors.

- [ ] **Step 4: Add the api bindings**

In `src/lib/api.ts`, after the `unstageLines` entry:

```ts
  discardHunk: (repo: string, path: string, hunkIndex: number, context = 3) =>
    invoke<void>("discard_hunk", { repo, path, hunkIndex, context }),
  discardLines: (repo: string, path: string, hunkIndex: number, selected: number[], context = 3) =>
    invoke<void>("discard_lines", { repo, path, hunkIndex, selected, context }),
```

- [ ] **Step 5: Add the gitActions wrappers**

In `src/lib/gitActions.ts`, after the `unstageLines` entry:

```ts
  // Hunk/line discard is DESTRUCTIVE and NOT undoable — same reasoning as the
  // file-level `discard` below: uncommitted work is not in the reflog, so there is
  // no backup bundle to take. Guard BEFORE confirming so the browser preview never
  // shows a dialog it cannot honour.
  discardHunk: async (path: string, hunkIndex: number): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    const confirmed = await dialogs.confirm({
      title: "Discard hunk",
      message: `Permanently discard this hunk of ${path}. This cannot be undone. (Stash instead to keep it.)`,
      confirmLabel: "Discard",
      danger: true,
    });
    if (!confirmed) return false;
    return runWorktree(`Discard hunk in ${path}`, () =>
      api.discardHunk(appState.repo, path, hunkIndex, appState.effectiveDiffContext),
    );
  },
  discardLines: async (path: string, hunkIndex: number, selected: number[]): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    const n = selected.length;
    const confirmed = await dialogs.confirm({
      title: "Discard lines",
      message: `Permanently discard ${n} line${n === 1 ? "" : "s"} in ${path}. This cannot be undone. (Stash instead to keep them.)`,
      confirmLabel: "Discard",
      danger: true,
    });
    if (!confirmed) return false;
    return runWorktree(`Discard ${n} line(s) in ${path}`, () =>
      api.discardLines(appState.repo, path, hunkIndex, selected, appState.effectiveDiffContext),
    );
  },
```

- [ ] **Step 6: Verify the frontend type-checks and tests stay green**

```bash
npm run check && npm test
```

Expected: 0 errors; all existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/api.ts src/lib/gitActions.ts
git commit -m "feat: expose hunk and line discard to the frontend"
```

---

### Task 4: Hunk + block hover rings in DiffView (unified view)

**Files:**
- Modify: `src/lib/components/DiffView.svelte` — script (imports, props, hover state, helpers), markup (`<tbody>` per hunk at `:435`, unified `{#each}` at `:486`), styles

**Interfaces:**
- Consumes: `blocksOf`, `blockAt` from `../diff/blocks` (Task 1)
- Produces (used by Tasks 5–8):
  - props `onDiscardHunk?: (i: number) => void`, `onDiscardLines?: (hunkIndex: number, selected: number[]) => void`
  - `const hasActions: boolean` — true when any staging/discard callback was supplied
  - `let hov: { fi: number; hi: number; block: number | null } | null`
  - `function hoverRow(fi: number, hi: number, ri: number): void`
  - `function ringRange(fi: number, hi: number): { from: number; to: number } | null`
  - CSS classes `ring`, `ring-first`, `ring-last` on `<tr>`; `hunk-hover` on `<tbody>`

- [ ] **Step 1: Add the imports, props, and hover state**

In the `<script>` of `src/lib/components/DiffView.svelte`, add to the imports at the top:

```ts
  import { blocksOf, blockAt } from "../diff/blocks";
```

Add to the `Props` interface (after `onUnstageLines`):

```ts
    onDiscardHunk?: (i: number) => void;
    onDiscardLines?: (hunkIndex: number, selected: number[]) => void;
```

Add both to the destructuring on the `let { ... }: Props = $props();` line, and after it add:

```ts
  // The whole hover/selection affordance only exists where actions do. Commit diffs
  // (CommitFilesDiff) and PR review diffs (PrFilesTab) pass no callbacks and stay inert.
  const hasActions = $derived(
    !!(onStageHunk || onUnstageHunk || onStageLines || onUnstageLines || onDiscardHunk || onDiscardLines),
  );

  // Hovered unit. `block` is the index into blocksOf(hunk), or null on a context row
  // (which targets the whole hunk instead).
  let hov = $state<{ fi: number; hi: number; block: number | null } | null>(null);

  function hoverRow(fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return;
    hov = { fi, hi, block: blockAt(h, ri) };
  }

  function clearHover() {
    hov = null;
  }

  /**
   * The row range the inner ring should cover for this hunk, or null for none.
   * Only a hovered BLOCK gets the inner ring; a hovered context row gets the
   * outer `<tbody>` outline instead (see the `hunk-hover` class).
   */
  function ringRange(fi: number, hi: number): { from: number; to: number } | null {
    if (!hov || hov.fi !== fi || hov.hi !== hi || hov.block === null) return null;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return null;
    const b = blocksOf(h)[hov.block];
    return b ? { from: b.rows[0], to: b.rows[b.rows.length - 1] } : null;
  }
```

- [ ] **Step 2: Emit one `<tbody>` per hunk**

In the markup, the table currently wraps everything in a single `<tbody>` (`:435`) with `{#each file.hunks ...}` inside it. Restructure so the `{#each}` is outside and each hunk gets its own `<tbody>`.

Replace:

```svelte
            <tbody>
              {#each file.hunks as hunk, hi (hi)}
```

with:

```svelte
            {#each file.hunks as hunk, hi (hi)}
              {@const ring = ringRange(fi, hi)}
              <tbody
                class="hunk"
                class:hunk-hover={hasActions && hov?.fi === fi && hov?.hi === hi}
              >
```

and replace the matching closing tags at the end of the loop:

```svelte
              {/each}
            </tbody>
```

with:

```svelte
              </tbody>
            {/each}
```

`{@const ring = ...}` must be the first child of the `{#each}` block — it is computed **once per hunk** rather than per row, which keeps this O(rows) rather than O(rows²).

- [ ] **Step 3: Apply the ring classes to unified rows**

In the unified branch, change the `{#each}` to expose the row index and add the ring classes plus the hover handler. Replace the opening of the row loop and the `<tr>`:

```svelte
                  {#each indexHunkLines(hunk) as { line, beforeIdx, afterIdx } (line.oldNo ?? `a${line.newNo}`)}
                    {@const toks = lineTokens(fi, hi, line, beforeIdx, afterIdx)}
                    {@const ord = line.kind !== "context" ? ords.get(line) : undefined}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <tr
                      class="diff-row {line.kind}"
                      class:selected={ord !== undefined && isSelected(fi, hi, ord)}
```

with:

```svelte
                  {#each indexHunkLines(hunk) as { line, beforeIdx, afterIdx }, ri (ri)}
                    {@const toks = lineTokens(fi, hi, line, beforeIdx, afterIdx)}
                    {@const ord = line.kind !== "context" ? ords.get(line) : undefined}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <tr
                      class="diff-row {line.kind}"
                      class:selected={ord !== undefined && isSelected(fi, hi, ord)}
                      class:ring={ring !== null && ri >= ring.from && ri <= ring.to}
                      class:ring-first={ring !== null && ri === ring.from}
                      class:ring-last={ring !== null && ri === ring.to}
                      onmouseenter={() => hoverRow(fi, hi, ri)}
```

Everything else on the `<tr>` (the `style`, `onclick`, `onkeydown`) stays for now — Task 6 replaces it.

- [ ] **Step 4: Add the ring CSS**

Add to the `<style>` block of `DiffView.svelte`, after the `.gutter` rules:

```css
  /* ── Hover / selection ring ──────────────────────────────────────────────────
     A hunk is a <tbody> and takes a plain outline. A BLOCK is only a run of <tr>s
     with no wrapping element, so its ring is built from four box-shadow insets
     spread across the run's cells. They must be four SEPARATE custom properties
     feeding one box-shadow: written as four rules on `box-shadow` directly, the
     last would simply win. As custom properties they compose. */
  .diff-table td {
    box-shadow: var(--rt, 0 0 transparent), var(--rb, 0 0 transparent),
                var(--rl, 0 0 transparent), var(--rr, 0 0 transparent);
  }
  tr.ring-first td { --rt: inset 0 1.5px 0 var(--diff-ring); }
  tr.ring-last td { --rb: inset 0 -1.5px 0 var(--diff-ring); }
  tr.ring td:first-child { --rl: inset 1.5px 0 0 var(--diff-ring); }
  tr.ring td:last-child { --rr: inset -1.5px 0 0 var(--diff-ring); }

  /* Outer ring — subtler than the block ring, and drawn on the hunk's own element. */
  tbody.hunk.hunk-hover {
    outline: 1px solid color-mix(in srgb, var(--diff-ring) 34%, transparent);
    outline-offset: -1px;
  }
```

- [ ] **Step 5: Add the `--diff-ring` token**

In `src/routes/+page.svelte`, add to the Classic `:global(:root)` block, next to the other diff tokens (after the `--diff-add-fg` / `--diff-del-fg` line):

```css
    --diff-ring: var(--accent);
```

NERV inherits this through its own `--accent` and needs no separate declaration; the token exists so `nerv.css` can retune a colliding scheme later without touching component markup.

- [ ] **Step 6: Verify types and tests**

```bash
npm run check && npm test
```

Expected: 0 errors; all tests pass.

- [ ] **Step 7: Verify by hand in the desktop app**

```bash
npm run tauri dev
```

Open a repo with uncommitted changes → Local Changes → select a modified file. Confirm:
1. Hovering a **changed** line draws a solid ring around that contiguous block *and* a fainter outline around the whole hunk.
2. Hovering a **context** line draws only the faint hunk outline.
3. Moving the pointer out of the diff clears both.
4. The ring's left/right edges follow the block when you scroll horizontally on a long line.
5. Open a commit from the graph — its diff shows **no** rings at all.

- [ ] **Step 8: Commit**

```bash
git add src/lib/components/DiffView.svelte src/routes/+page.svelte
git commit -m "feat: add nested hunk and block hover rings to the diff view"
```

---

### Task 5: Floating action toolbar + WorkingCopyView wiring

**Files:**
- Modify: `src/lib/components/DiffView.svelte` — script (target resolution, toolbar position, action dispatch), markup (wrap `.diff-table-wrap` in a positioned outer div; render the toolbar), styles
- Modify: `src/lib/components/WorkingCopyView.svelte:549-564` (pass the discard callbacks)

**Interfaces:**
- Consumes: `hov`, `hasActions`, `ringRange` (Task 4); `blocksOf` (Task 1); `gitActions.discardHunk` / `discardLines` (Task 3)
- Produces (used by Tasks 6–8):
  - `interface ActionTarget { fi: number; hi: number; scope: "sel" | "blk" | "hunk"; ords: number[] | null; anchorRow: number }`
  - `function activeTarget(): ActionTarget | null`
  - `function runAction(kind: "stage" | "unstage" | "discard"): void`
  - `let wrapEls: Record<number, HTMLDivElement | undefined>` and `let toolTop: number`

- [ ] **Step 1: Add target resolution and action dispatch**

In the `<script>` of `DiffView.svelte`, after `ringRange`:

```ts
  interface ActionTarget {
    fi: number;
    hi: number;
    scope: "sel" | "blk" | "hunk";
    /** null for a whole-hunk target — the hunk ops take no ordinals. */
    ords: number[] | null;
    /** Row the toolbar anchors to. */
    anchorRow: number;
  }

  /** What the visible toolbar acts on right now. Task 6 adds the selection branch. */
  function activeTarget(): ActionTarget | null {
    if (!hasActions || !hov) return null;
    const h = parsed.files[hov.fi]?.hunks[hov.hi];
    if (!h) return null;
    if (hov.block !== null) {
      const b = blocksOf(h)[hov.block];
      if (b) return { fi: hov.fi, hi: hov.hi, scope: "blk", ords: b.ords, anchorRow: b.rows[0] };
    }
    return { fi: hov.fi, hi: hov.hi, scope: "hunk", ords: null, anchorRow: 0 };
  }

  function runAction(kind: "stage" | "unstage" | "discard") {
    const t = activeTarget();
    if (!t) return;
    if (t.scope === "hunk") {
      if (kind === "stage") onStageHunk?.(t.hi);
      else if (kind === "unstage") onUnstageHunk?.(t.hi);
      else onDiscardHunk?.(t.hi);
    } else {
      const ords = t.ords ?? [];
      if (!ords.length) return;
      if (kind === "stage") onStageLines?.(t.hi, ords);
      else if (kind === "unstage") onUnstageLines?.(t.hi, ords);
      else onDiscardLines?.(t.hi, ords);
    }
    hov = null;
  }
```

- [ ] **Step 2: Add toolbar positioning**

Still in the `<script>`:

```ts
  // The toolbar lives OUTSIDE the horizontally-scrolling table wrap so `right` pins
  // it to the visible edge. Only its vertical offset needs measuring, and using
  // getBoundingClientRect differences means scrollTop needs no separate arithmetic.
  let wrapEls = $state<Record<number, HTMLDivElement | undefined>>({});
  let toolTop = $state(0);
  let toolVisible = $state(false);

  function measureTool() {
    const t = activeTarget();
    const wrap = t ? wrapEls[t.fi] : undefined;
    if (!t || !wrap) {
      toolVisible = false;
      return;
    }
    const row = wrap.querySelector<HTMLElement>(
      `tr[data-h="${t.hi}"][data-i="${t.anchorRow}"]`,
    );
    if (!row) {
      toolVisible = false;
      return;
    }
    const top = row.getBoundingClientRect().top - wrap.getBoundingClientRect().top;
    // Hide rather than float a toolbar pointing at a row scrolled out of view.
    if (top < 0 || top > wrap.clientHeight - 8) {
      toolVisible = false;
      return;
    }
    toolTop = top;
    toolVisible = true;
  }

  // Runs after the DOM settles, so the anchor row is guaranteed to exist.
  $effect(() => {
    hov;
    measureTool();
  });
```

- [ ] **Step 3: Add `data-h` / `data-i` to unified rows**

`measureTool` finds its anchor by attribute. On the unified `<tr>` edited in Task 4, add:

```svelte
                      data-h={hi}
                      data-i={ri}
```

- [ ] **Step 4: Wrap the table and render the toolbar**

Replace:

```svelte
        <div class="diff-table-wrap">
          <table class="diff-table mono" class:split={appState.diffSplit}>
```

with:

```svelte
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="diff-table-outer" onmouseleave={clearHover}>
        <div
          class="diff-table-wrap"
          bind:this={wrapEls[fi]}
          onscroll={measureTool}
        >
          <table class="diff-table mono" class:split={appState.diffSplit}>
```

**Hover must clear on THIS element, not on the `<tbody>`.** The toolbar lives outside the
table, so a `mouseleave` on the `<tbody>` fires the moment the pointer travels from a row
toward the toolbar — clearing `hov`, unmounting the toolbar, and making it unclickable.
`.diff-table-outer` contains both the table and the toolbar, so the pointer can reach the
buttons without leaving it. Moving between hunks still retargets correctly, because each
row's `onmouseenter` updates `hov` on entry.

and replace the matching close:

```svelte
          </table>
        </div>
```

with:

```svelte
          </table>
        </div>
        {#if hasActions && toolVisible}
          {@const t = activeTarget()}
          {#if t && t.fi === fi}
            {@const label = t.scope === "hunk" ? "hunk" : String((t.ords ?? []).length)}
            <div class="diff-tools" style="top: {toolTop}px">
              {#if !staged && onStageHunk}
                <button onclick={() => runAction("stage")}>Stage {label}</button>
              {/if}
              {#if !staged && onDiscardHunk}
                <button class="danger" onclick={() => runAction("discard")}>Discard {label}</button>
              {/if}
              {#if staged && onUnstageHunk}
                <button onclick={() => runAction("unstage")}>Unstage {label}</button>
              {/if}
            </div>
          {/if}
        {/if}
        </div>
```

- [ ] **Step 5: Add the toolbar CSS**

Add to `DiffView.svelte`'s `<style>`:

```css
  /* Positioning context for the floating toolbar. It must NOT be the scroll
     container: an absolutely-positioned child of a scroller is laid out against
     the content, so `right` would drift as the diff scrolls sideways. */
  .diff-table-outer {
    position: relative;
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .diff-tools {
    position: absolute;
    right: 14px;
    z-index: 6;
    display: flex;
    gap: 4px;
    padding: 3px;
    background: var(--header-bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .diff-tools button {
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent);
    background: var(--btn-bg);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    white-space: nowrap;
    cursor: pointer;
  }
  .diff-tools button:hover {
    background: var(--accent);
    color: var(--on-accent);
  }
  .diff-tools button.danger {
    border-color: var(--danger);
    color: var(--danger);
  }
  .diff-tools button.danger:hover {
    background: var(--danger);
    color: var(--on-accent);
  }
```

Also confirm the existing `.diff-table-wrap` rule reads as below — it now sits inside `.diff-table-outer`, so it needs `min-width: 0` for the row axis (its main-axis minimum is already 0 by virtue of being a scroll container):

```css
  .diff-table-wrap {
    overflow: auto;
    flex: 1 1 auto;
    min-width: 0;
  }
```

- [ ] **Step 6: Pass the discard callbacks from WorkingCopyView**

In `src/lib/components/WorkingCopyView.svelte`, in the `<DiffView ... />` call, add after `onUnstageLines`:

```svelte
            onDiscardHunk={selectedIsStaged || selectedIsUntracked
              ? undefined
              : (i) => gitActions.discardHunk(selectedFile!, i)}
            onDiscardLines={selectedIsStaged || selectedIsUntracked
              ? undefined
              : (hi, sel) => gitActions.discardLines(selectedFile!, hi, sel)}
```

Discard is unstaged-and-tracked only: on the staged side it would mean unstage-and-revert, and untracked files have no index entry to revert to.

- [ ] **Step 7: Verify types and tests**

```bash
npm run check && npm test
```

Expected: 0 errors; all tests pass.

- [ ] **Step 8: Verify by hand**

```bash
npm run tauri dev
```

1. Hover a changed line in an **unstaged** file → toolbar shows `Stage N` + `Discard N` pinned to the right edge of the diff pane, level with the block.
2. Hover a context line → toolbar shows `Stage hunk` + `Discard hunk`.
3. Select a **staged** file → toolbar shows `Unstage N` only, no Discard.
4. Click `Discard N` → confirm dialog appears; confirming reverts exactly those lines in the file and leaves the rest.
5. Stage some lines of a file, then discard *different* lines of the same file → the staged lines are still staged.
6. Scroll the diff horizontally → the toolbar stays pinned to the right edge. Scroll vertically until the block leaves view → the toolbar disappears rather than floating.

- [ ] **Step 9: Commit**

```bash
git add src/lib/components/DiffView.svelte src/lib/components/WorkingCopyView.svelte
git commit -m "feat: add floating stage/discard toolbar to the diff view"
```

---

### Task 6: Replace the line-selection model

**Files:**
- Modify: `src/lib/components/DiffView.svelte` — remove the `selected` Map, `toggleLine`, `isSelected`, `selCount`, `applyLines`, `hunkOrdinals`; add the range selection; remove the hunk-header buttons and the old selection CSS

**Interfaces:**
- Consumes: `ordsInRange` (Task 1); `activeTarget`, `runAction`, `measureTool` (Task 5)
- Produces (used by Tasks 7–8):
  - `let sel: { fi: number; hi: number; anchor: number; from: number; to: number } | null`
  - `function lockSelection(fi: number, hi: number, ri: number): void`
  - `function extendSelection(fi: number, hi: number, ri: number): void`
  - `function clearSelection(): void`

- [ ] **Step 1: Delete the old selection machinery**

In the `<script>`, delete this whole block (currently `:207-246`) — the `selected` state, its clearing `$effect`, `hunkOrdinals`, `keyOf`, `isSelected`, `toggleLine`, `selCount`, and `applyLines`:

```ts
  // ─── Line-level selection ─────────────────────────────────────────────────────

  // Map keyed by "${fi}:${hi}" → Set of change-line ordinals (0-based among +/- lines).
  let selected = $state<Map<string, Set<number>>>(new Map());
  ...
  function applyLines(fi: number, hi: number) { ... }
```

Also update the import of `ordsInRange`:

```ts
  import { blocksOf, blockAt, ordsInRange } from "../diff/blocks";
```

- [ ] **Step 2: Add the range selection**

In its place:

```ts
  // ─── Line selection ───────────────────────────────────────────────────────────
  // A contiguous row range inside ONE hunk. `anchor` is the row the range grew
  // from, so extending down and then back up pivots correctly. Ranges may span
  // context rows; `ordsInRange` keeps only the change lines.
  let sel = $state<{ fi: number; hi: number; anchor: number; from: number; to: number } | null>(null);

  // A new patch re-indexes every hunk, so any range we were holding is meaningless.
  $effect(() => {
    patch;
    sel = null;
  });

  function lockSelection(fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h || h.lines[ri]?.kind === "context") return;
    sel = { fi, hi, anchor: ri, from: ri, to: ri };
    hov = null;
    // A double-click is also the browser's select-word gesture, and `.diff-cell`
    // deliberately opts back into user-select so diff code stays copyable.
    window.getSelection()?.removeAllRanges();
  }

  function extendSelection(fi: number, hi: number, ri: number) {
    if (!sel || sel.fi !== fi || sel.hi !== hi) return;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return;
    const from = Math.min(sel.anchor, ri);
    const to = Math.max(sel.anchor, ri);
    if (!ordsInRange(h, from, to).length) return;
    sel = { fi, hi, anchor: sel.anchor, from, to };
    window.getSelection()?.removeAllRanges();
  }

  function clearSelection() {
    sel = null;
  }

  function onRowClick(e: MouseEvent, fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    if (e.shiftKey) {
      extendSelection(fi, hi, ri);
      return;
    }
    clearSelection();
  }
```

- [ ] **Step 3: Make hover, ring, and target respect the lock**

Replace the three functions so a locked selection outranks hover absolutely.

`hoverRow` — add the guard as its first line:

```ts
  function hoverRow(fi: number, hi: number, ri: number) {
    if (!hasActions || sel) return;   // locked: hover is dead
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return;
    hov = { fi, hi, block: blockAt(h, ri) };
  }
```

`clearHover`:

```ts
  function clearHover() {
    if (sel) return;
    hov = null;
  }
```

`ringRange` — the selection draws the ring when one exists:

```ts
  function ringRange(fi: number, hi: number): { from: number; to: number } | null {
    if (sel) {
      return sel.fi === fi && sel.hi === hi ? { from: sel.from, to: sel.to } : null;
    }
    if (!hov || hov.fi !== fi || hov.hi !== hi || hov.block === null) return null;
    const h = parsed.files[fi]?.hunks[hi];
    if (!h) return null;
    const b = blocksOf(h)[hov.block];
    return b ? { from: b.rows[0], to: b.rows[b.rows.length - 1] } : null;
  }
```

`activeTarget` — add the selection branch at the top:

```ts
  function activeTarget(): ActionTarget | null {
    if (!hasActions) return null;
    if (sel) {
      const h = parsed.files[sel.fi]?.hunks[sel.hi];
      if (!h) return null;
      const ords = ordsInRange(h, sel.from, sel.to);
      return ords.length
        ? { fi: sel.fi, hi: sel.hi, scope: "sel", ords, anchorRow: sel.from }
        : null;
    }
    if (!hov) return null;
    const h = parsed.files[hov.fi]?.hunks[hov.hi];
    if (!h) return null;
    if (hov.block !== null) {
      const b = blocksOf(h)[hov.block];
      if (b) return { fi: hov.fi, hi: hov.hi, scope: "blk", ords: b.ords, anchorRow: b.rows[0] };
    }
    return { fi: hov.fi, hi: hov.hi, scope: "hunk", ords: null, anchorRow: 0 };
  }
```

`runAction` — clear the selection too, and label the selection scope. Change its last line from `hov = null;` to:

```ts
    hov = null;
    sel = null;
```

The `measureTool` `$effect` must also track `sel`:

```ts
  $effect(() => {
    hov;
    sel;
    measureTool();
  });
```

And the `hunk-hover` outer ring must not draw while locked. In the `<tbody>`:

```svelte
                class:hunk-hover={hasActions && !sel && hov?.fi === fi && hov?.hi === hi}
```

- [ ] **Step 4: Swap the row handlers**

On the unified `<tr>`, replace the old selection handlers:

```svelte
                      style={line.kind !== "context" ? "cursor: pointer" : ""}
                      onclick={() => { if (ord !== undefined) toggleLine(fi, hi, ord); }}
                      onkeydown={(e) => { if (ord !== undefined && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); toggleLine(fi, hi, ord); } }}
```

with:

```svelte
                      style={hasActions && line.kind !== "context" ? "cursor: pointer" : ""}
                      onclick={(e) => onRowClick(e, fi, hi, ri)}
                      ondblclick={() => lockSelection(fi, hi, ri)}
```

Also delete the now-unused `class:selected={...}` attribute and the `{@const ord = ...}` line above the `<tr>`, plus the `{@const ords = hunkOrdinals(hunk)}` line that precedes the unified `{#each}`.

- [ ] **Step 5: Add the Escape handler**

Add near the top of the markup, before `<div class="diff-view">`:

```svelte
<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && sel) clearSelection();
  }}
/>
```

- [ ] **Step 6: Remove the hunk-header buttons**

The floating toolbar supersedes them; leaving both would be two live paths to the same action. In **both** the split and unified branches of the hunk-header row, delete every `{#if onStageHunk || onUnstageHunk}` / `{#if selCount(fi, hi) > 0}` block and their buttons, leaving only:

```svelte
                    <td class="hunk-header-cell">
                      <span class="hunk-range">{hunk.header}</span>
                    </td>
```

(and the `colspan="4"` variant for split). The `.hunk-btn` CSS rules become dead — delete them too.

- [ ] **Step 7: Remove the old selection CSS**

Delete this block from `<style>`:

```css
  /* ── Line-level selection highlight ─────────────────────────────────────────── */
  .diff-row.selected {
    box-shadow: inset 2px 0 0 var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }
  .split-cell.selected {
    box-shadow: inset 2px 0 0 var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }
```

This is the rule that was invisible in NERV. Note `.diff-row.selected` also set `background`, which would have fought the new `box-shadow` ring on the same cells.

- [ ] **Step 8: Verify types and tests**

```bash
npm run check && npm test
```

Expected: 0 errors (no references left to `toggleLine`, `selCount`, `applyLines`, `isSelected`, `hunkOrdinals`); all tests pass.

- [ ] **Step 9: Verify by hand**

```bash
npm run tauri dev
```

1. Double-click a changed line → ring locks around that one line; toolbar reads `Stage 1` / `Discard 1` and stays put when the pointer moves away.
2. Shift+click a line further down the same hunk → ring grows; count updates.
3. Shift+click *above* the anchor → the range pivots around the original anchor.
4. Extend across context rows → the ring covers them but the count only rises for changed lines.
5. Move the pointer over another block → **nothing happens**; no second ring, no hunk outline.
6. `Esc`, or a single click anywhere → selection clears and hover resumes.
7. Double-click no longer leaves a blue word-selection behind.
8. Switch to the NERV theme → the locked ring is clearly visible on both added and deleted lines.

- [ ] **Step 10: Commit**

```bash
git add src/lib/components/DiffView.svelte
git commit -m "feat: replace diff line selection with a contiguous range and ring"
```

---

### Task 7: Split view support

**Files:**
- Modify: `src/lib/components/DiffView.svelte` — `SplitRow` interface and `toSplitRows` (`:250-310`), split-view markup (`:523-589`)

**Interfaces:**
- Consumes: everything from Tasks 4–6
- Produces: `SplitRow` gains `rows: number[]` — the hunk line indices that split row represents (one for a context row, one or two for a paired change row)

- [ ] **Step 1: Track hunk line indices in `toSplitRows`**

A split row is a *pairing* of hunk lines, so its index is not a hunk line index. Everything in Tasks 4–6 speaks hunk line indices, so each split row must carry the ones it represents.

Add to the `SplitRow` interface:

```ts
    /** Hunk line indices this row represents: 1 for context, 1-2 for a change pair. */
    rows: number[];
```

In `toSplitRows`, the context branch becomes:

```ts
      if (line.kind === "context") {
        rows.push({ oldLine: line, newLine: line, oldBeforeIdx: bi, oldAfterIdx: ai, newBeforeIdx: bi, newAfterIdx: ai, rows: [i] });
        bi++;
        ai++;
        i++;
        continue;
      }
```

The change-run collection records each line's index:

```ts
      const dels: Array<{ line: DiffLine; bi: number; idx: number }> = [];
      const adds: Array<{ line: DiffLine; ai: number; idx: number }> = [];

      while (i < lines.length && (lines[i].kind === "del" || lines[i].kind === "add")) {
        if (lines[i].kind === "del") {
          dels.push({ line: lines[i], bi, idx: i });
          bi++;
        } else {
          adds.push({ line: lines[i], ai, idx: i });
          ai++;
        }
        i++;
      }

      const maxLen = Math.max(dels.length, adds.length);
      for (let j = 0; j < maxLen; j++) {
        const d = dels[j] ?? null;
        const a = adds[j] ?? null;
        rows.push({
          oldLine: d?.line ?? null,
          newLine: a?.line ?? null,
          oldBeforeIdx: d?.bi ?? 0,
          oldAfterIdx: 0,
          newBeforeIdx: 0,
          newAfterIdx: a?.ai ?? 0,
          rows: [d?.idx, a?.idx].filter((x): x is number => x !== undefined),
        });
      }
```

- [ ] **Step 2: Apply ring classes and handlers to split rows**

Replace the split `{#each}` opening and its `<tr>`:

```svelte
                  {#each toSplitRows(hunk) as row, ri (ri)}
                    {@const oldOrd = row.oldLine?.kind === "del" ? ords.get(row.oldLine) : undefined}
                    {@const newOrd = row.newLine?.kind === "add" ? ords.get(row.newLine) : undefined}
                    <tr class="diff-row split-row">
```

with:

```svelte
                  {#each toSplitRows(hunk) as row, ri (ri)}
                    {@const head = row.rows[0] ?? 0}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <tr
                      class="diff-row split-row"
                      class:ring={ring !== null && row.rows.some((r) => r >= ring.from && r <= ring.to)}
                      class:ring-first={ring !== null && row.rows.includes(ring.from)}
                      class:ring-last={ring !== null && row.rows.includes(ring.to)}
                      data-h={hi}
                      data-i={head}
                      style={hasActions && row.rows.length > 0 && (row.oldLine?.kind === "del" || row.newLine?.kind === "add") ? "cursor: pointer" : ""}
                      onmouseenter={() => hoverRow(fi, hi, head)}
                      onclick={(e) => onRowClick(e, fi, hi, head)}
                      ondblclick={() => lockSelection(fi, hi, head)}
                    >
```

A paired row selects as a unit — `head` is its first hunk line, and `lockSelection` anchors there. Selecting the addition without its deletion remains possible in Unified view.

- [ ] **Step 3: Remove the old per-cell selection from split cells**

On both split `<td class="diff-cell split-cell">` elements, delete these attributes:

```svelte
                        class:selected={oldOrd !== undefined && isSelected(fi, hi, oldOrd)}
                        style={row.oldLine?.kind === "del" ? "cursor: pointer" : ""}
                        onclick={() => { if (oldOrd !== undefined) toggleLine(fi, hi, oldOrd); }}
                        onkeydown={(e) => { if (oldOrd !== undefined && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); toggleLine(fi, hi, oldOrd); } }}
```

(and the `newOrd` equivalents on the new-side cell). Also delete the now-unused `{@const ords = hunkOrdinals(hunk)}` above the split `{#each}` and the two `svelte-ignore` comments that preceded the cells.

- [ ] **Step 4: Make the ring reach the gutters in split view**

The split table has four columns; `td:first-child` / `td:last-child` already resolve to the outer gutter and the new-side cell, so the ring spans the full row without extra CSS. Confirm no rule scopes the ring to `.diff-table:not(.split)`.

- [ ] **Step 5: Verify types and tests**

```bash
npm run check && npm test
```

Expected: 0 errors; all tests pass.

- [ ] **Step 6: Verify by hand**

```bash
npm run tauri dev
```

Switch the diff toolbar to **Split**, then:
1. Hover a paired change row → ring spans **both** columns.
2. Hover a context row → hunk outline only.
3. Double-click + shift+click → range locks across paired rows; count matches the changed lines covered.
4. `Stage N` / `Discard N` act on exactly those lines; flip to Unified and confirm the result.
5. Column widths are unchanged from before this task (the `<colgroup>` still governs sizing under `<tbody>`-per-hunk).

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/DiffView.svelte
git commit -m "feat: support hunk and block rings in split diff view"
```

---

### Task 8: Keyboard parity and final gate

**Files:**
- Modify: `src/lib/components/DiffView.svelte` — focus handling on rows, arrow-key extension

**Interfaces:**
- Consumes: `hoverRow`, `lockSelection`, `extendSelection`, `clearSelection`, `hasActions` (Tasks 4–6)
- Produces: nothing consumed downstream — this is the last task.

- [ ] **Step 1: Make focus behave as hover**

Task 6 removed the rows' `onkeydown`, which is what made the old selection keyboard-reachable. Restore parity for the new model. On the unified `<tr>` (and the split `<tr>`), add:

```svelte
                      tabindex={hasActions ? 0 : undefined}
                      onfocus={() => hoverRow(fi, hi, ri)}
                      onkeydown={(e) => onRowKeydown(e, fi, hi, ri)}
```

For the split `<tr>`, use `head` in place of `ri` in both handlers, matching Task 7.

- [ ] **Step 2: Add the key handler**

In the `<script>`, after `onRowClick`:

```ts
  function onRowKeydown(e: KeyboardEvent, fi: number, hi: number, ri: number) {
    if (!hasActions) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      lockSelection(fi, hi, ri);
      return;
    }
    if (e.shiftKey && (e.key === "ArrowDown" || e.key === "ArrowUp") && sel) {
      e.preventDefault();
      const delta = e.key === "ArrowDown" ? 1 : -1;
      const edge = sel.to === sel.anchor ? sel.from : sel.to;
      const h = parsed.files[fi]?.hunks[hi];
      if (!h) return;
      const next = Math.max(0, Math.min(h.lines.length - 1, edge + delta));
      extendSelection(fi, hi, next);
    }
  }
```

- [ ] **Step 3: Verify types and tests**

```bash
npm run check && npm test
```

Expected: 0 errors; all tests pass.

- [ ] **Step 4: Run the full gate**

```bash
npm run check && npm test && cargo test
```

Expected: 0 type errors; all frontend tests pass; all Rust tests pass across `git-core`, `git-it`, and `git-it-agent`.

- [ ] **Step 5: Final manual sweep**

```bash
npm run tauri dev
```

1. **Keyboard:** Tab into the diff → the focused row raises its ring and toolbar. `Enter` locks it. `Shift`+`↓` extends. `Esc` clears. Tab reaches the toolbar buttons while they are visible.
2. **Themes:** Classic light, Classic dark, and NERV in **all six** schemes (orange, phosphor, steel, amber, violet, crimson). The ring must stay legible on both added and deleted rows. If crimson or phosphor collides, override `--diff-ring` in `src/lib/theme/nerv.css` using a bare `:root[data-theme="nerv"][data-scheme="crimson"]` selector — **never `:global()`**.
3. **Inert hosts:** a commit diff and a PR Files diff show no rings, no toolbar, and no pointer cursor.
4. **Untracked file:** no rings (no staging callbacks are passed).
5. **Re-index safety:** discard a block, then immediately act on another block in the same file — the second op must hit the right lines, proving the refresh re-fetched the diff.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/DiffView.svelte
git commit -m "feat: add keyboard parity for diff ring selection"
```

---

## Notes for the implementer

- **Why four custom properties for one ring.** `box-shadow` does not merge across rules — a later `box-shadow` declaration replaces an earlier one entirely. Setting `--rt`/`--rb`/`--rl`/`--rr` from four *different* selectors and composing them in one `box-shadow` on `td` is what lets a top edge, a bottom edge, and two side edges coexist on the same cell. If you refactor this into direct `box-shadow` rules, the ring will silently lose edges.
- **Why the toolbar sits outside the scroll container.** An absolutely-positioned child of an `overflow: auto` element is positioned against that element's padding box but *scrolls with its content*. Putting the toolbar in `.diff-table-outer` (which does not scroll) is what keeps `right: 14px` pinned to the visible edge on a horizontally scrolling diff.
- **Hunk indices are live.** Every op re-indexes the remaining diff, which is why `runAction` clears both `hov` and `sel` and the `$effect` on `patch` clears `sel` again when the refresh lands. Do not cache hunk indices across an operation.
- **`ordsInRange` is deliberately lossy about context.** A range that spans unchanged rows yields only the changed ordinals. The ring covering more rows than the count suggests is intended, not a bug.
