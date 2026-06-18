# GitHub screen — non-blocking loads + accent-glow skeletons

**Date:** 2026-06-18
**Status:** Approved (design)

## Problem

Opening the GitHub screen, and clicking between its tabs/sub-tabs, freezes the
entire app for as long as each `gh` call takes (0.5–3 s on the network).

**Root cause (verified):** every `github_*` Tauri command in
`src-tauri/src/commands.rs` is declared `#[tauri::command] pub fn …`
(synchronous). Per the Tauri v2 docs, *non-async commands run on the main
thread*. `github::run_gh()` blocks on a real `gh` subprocess + network round
trip, so the main (UI) thread is blocked until `gh` returns — first paint of the
screen and every subsequent tab click. A spinner alone would not help: a
main-thread-blocked webview cannot run CSS animations either.

The frontend already loads each panel lazily (`makePanel` cache + per-tab
`$effect`), so the fix is **not** more frontend laziness — it is moving `gh` off
the main thread, then filling the now-responsive wait with proper loading
states.

## Goals

1. The GitHub screen never freezes the app — opening it and switching
   tabs/sub-tabs stays interactive while `gh` runs in the background.
2. Every "is loading" state shows an elegant **accent-glow skeleton** shaped
   like the real content it is replacing, not a bare `Loading…` line.
3. When data arrives, content settles into place with a subtle reveal.

Non-goals: changing what data is fetched, the cache/dedupe logic, error/empty
states (already fine), or any non-GitHub command.

## Part A — Backend: take `gh` off the main thread

Change all 18 `github_*` commands in `src-tauri/src/commands.rs` from
`#[tauri::command]` to `#[tauri::command(async)]`. The `(async)` flag runs each
(still-synchronous) command on a worker thread instead of the UI thread.

- Signatures/bodies unchanged; args are already owned (`String`/`u64`/`u32`), so
  there is no borrow issue.
- The frontend `invoke()` calls already `await`, so `src/lib/api.ts` and all
  callers are unchanged.
- **Side benefit:** the Insights tab fires 5 `gh` calls; off the main thread
  they run in parallel instead of serialized behind the UI thread.

**Fallback** (only if `#[tauri::command(async)]` on a synchronous `fn` does not
compile cleanly under this Tauri version): convert the affected commands to
`pub async fn` whose body is
`tauri::async_runtime::spawn_blocking(move || github::…(…)).await`, mapping the
`JoinError` into `GithubError::Other`. Same effect (work leaves the main
thread). The implementer chooses whichever compiles; `cargo build` decides.

Commands to convert (all of them): `github_availability`, `github_repo_stats`,
`github_pulls`, `github_issues`, `github_releases`, `github_runs`,
`github_traffic`, `github_contributors`, `github_activity`,
`github_milestones`, `github_labels`, `github_pr_comment`,
`github_issue_comment`, `github_issue_set_state`, `github_pr_merge`,
`github_issue_create`, `github_pr_detail`, `github_issue_detail`.

No new Rust tests (pure threading-attribute change). The existing 115 cargo
tests must stay green.

## Part B — Frontend: Skeleton system + accent-glow reveal

### Components (new)

**`src/lib/components/github/Skeleton.svelte`** — the primitive block.
- Props: `w?: string` (default `"100%"`), `h?: string` (default `"12px"`),
  `radius?: string` (default `"6px"`), `circle?: boolean` (default `false`;
  when true, forces a 1:1 pill via `border-radius: 50%`).
- Renders `<span class="sk" style="width/height/border-radius" aria-hidden="true"></span>`.
- Base fill `var(--panel-bg)` over a faint `var(--border)`-toned backdrop so it
  reads on glass; an `::after` band sweeps an accent-tinted gradient
  (`color-mix(in srgb, var(--accent) 28%, transparent)`) left→right.
- Honors `@media (prefers-reduced-motion: reduce)`: no sweep, a static dim block.

**`src/lib/components/github/GithubSkeleton.svelte`** — variant-switched layouts
that compose `Skeleton` into the shape of each real panel.
- Prop: `variant: "header" | "overview" | "list" | "releases" | "actions" | "insights" | "detail"`.
- Optional prop: `rows?: number` (default 6) for the `list`/`actions` variants.
- Each variant's wrapper rows/cards get the staggered `rise` animation
  (`rise-in` class with `nth-child`-based delays) so the skeleton itself
  assembles with a gentle stagger.
- Variant shapes (match the real markup so there is no layout jump on swap):
  - `header`: a wide slug bar + a narrower desc bar + a flex row of 5
    tile-sized blocks (`~64×34`) — mirrors `GithubHeader`.
  - `overview`: a 2-column `auto 1fr` grid of 5 `dt/dd` pairs + a row of 3 topic
    pills — mirrors `GithubOverview`.
  - `list`: `rows` rows, each a title bar (~70% width) over a thin meta bar
    (~45%), divider between — mirrors `GithubPulls`/`GithubIssues`.
  - `releases`: a metrics row (4 number/label stacks) + two chart cards each
    with an `h4` bar and 4 bar-rows + an "All releases" stack of 4 header rows —
    mirrors `GithubReleases`.
  - `actions`: `rows` rows, each a small glyph dot + a flex of muted bars —
    mirrors `GithubActions`.
  - `insights`: a `repeat(auto-fit,minmax(280px,1fr))` grid of 4 cards, each
    with a heading bar + 3 content bars — mirrors `GithubInsights`.
  - `detail`: a back-button bar + a title bar + a sub meta row + a tall body
    block + two comment-card blocks — mirrors `GithubDetail`.

### Wiring (replace bare notes with skeletons)

In each component, the *first-load* branch (`status === "loading" && !data`, or
the availability/stats-loading gate) renders the matching skeleton instead of a
`<p class="note">…</p>`. Error and empty branches are unchanged. Refresh
(data already present) keeps showing stale data — unchanged.

- `GithubView.svelte`: `availLoading && !avail` → `<GithubSkeleton variant="header" />`
  (plus a tabs placeholder is unnecessary — show the header skeleton only).
  Overview's `statsLoading && !stats` → `<GithubSkeleton variant="overview" />`.
- `GithubHeader.svelte`: when `stats` is null, render 5
  `<GithubSkeleton variant="header">`-style tile blocks in place of the
  `StatTile`s (the slug/desc already render from `owner/repo`). Simplest: keep
  the header structure, and where `{#if stats}` is false, show 5 small
  `Skeleton` tiles so the numbers shimmer in independently of availability.
- `GithubPulls.svelte` / `GithubIssues.svelte`: loading → `<GithubSkeleton variant="list" />`.
- `GithubReleases.svelte`: loading → `<GithubSkeleton variant="releases" />`.
- `GithubActions.svelte`: loading → `<GithubSkeleton variant="actions" />`.
- `GithubInsights.svelte`: each of the 4 cards' loading branch → a small
  in-card skeleton (e.g. `<GithubSkeleton variant="insights" />` renders all
  four; but since cards load independently, give each card its own inline
  skeleton block of 3 bars). Implementer: add a tiny `card`-local skeleton
  rather than the whole-grid variant so each card fills as its call resolves.
- `GithubDetail.svelte`: loading → `<GithubSkeleton variant="detail" />`.

### The "settle" reveal

When a panel transitions loading→ok, wrap the real content in a Svelte
`in:` transition so it fades + rises ~6px over ~220 ms (`fly`/`fade` from
`svelte/transition`, `{ duration: 220, y: 6 }`), gated by `prefers-reduced-motion`
via a `reduced` check (use a module helper `revealIn` that returns an empty
transition when reduced-motion is set, else `fly`). Apply to the top-level
content wrapper of each tab/detail/overview. Keep it subtle — this is a settle,
not a slide-show.

### Animation tokens (style C — accent glow + reveal)

Global keyframes live in `Skeleton.svelte` (scoped) / a shared block:
- `@keyframes gh-sweep { from { transform: translateX(-120%) } to { transform: translateX(120%) } }`
  on the `::after` band, `1.5s ease-in-out infinite`.
- `@keyframes gh-rise { from { opacity:0; transform: translateY(8px) } to { opacity:1; transform: translateY(0) } }`
  on `.rise-in`, `0.5s ease forwards`, with `nth-child(1..n)` delays of
  `0 / .08 / .16 / .24 / .32 / .4s`.
- Accent uses the existing `--accent` token (auto light/dark + per-repo branch
  colour). Reduced-motion disables both keyframes.

## Testing & verification

- **Vitest:** add `src/lib/components/github/Skeleton.test.ts` (jsdom) — mounts
  `Skeleton` and asserts it renders a `.sk` element with the given inline width;
  mounts `GithubSkeleton` with each `variant` and asserts it renders without
  error and produces ≥1 `.sk`. Existing 190 tests stay green.
- **Gates:** `npm run check` (svelte-check 0/0), `npm test` (vitest), `cargo test`
  (115), `npm run build`.
- **Tauri smoke test** (the real proof — the screen is `isTauri`-gated, so the
  browser preview shows the "desktop app only" note): `npm run tauri build`,
  open the GitHub screen on an owned repo, confirm: no freeze on open; tabs and
  Insights sub-panels respond instantly; skeletons animate during `gh`; content
  settles in; detail view opens without blocking.
- **Adversarial review:** dimensions = (1) Tauri threading correctness (does the
  attribute actually move work off the main thread? any borrow/Send issue?),
  (2) skeleton/CSS quality + layout-jump on swap, (3) accessibility
  (`aria-hidden`, reduced-motion, no focus traps), (4) regression (error/empty
  states intact, no double-fetch, reveal transitions don't break the existing
  detail Back button). Fix Critical/Important.

## Files

| File | Change |
|---|---|
| `src-tauri/src/commands.rs` | A — 19 commands → `#[tauri::command(async)]` |
| `src/lib/components/github/Skeleton.svelte` | B — new primitive |
| `src/lib/components/github/GithubSkeleton.svelte` | B — new variant layouts |
| `src/lib/components/github/Skeleton.test.ts` | B — new render tests |
| `src/lib/components/github/GithubView.svelte` | B — header/overview skeletons |
| `src/lib/components/github/GithubHeader.svelte` | B — stat-tile skeletons |
| `src/lib/components/github/GithubPulls.svelte` | B — list skeleton + reveal |
| `src/lib/components/github/GithubIssues.svelte` | B — list skeleton + reveal |
| `src/lib/components/github/GithubReleases.svelte` | B — releases skeleton + reveal |
| `src/lib/components/github/GithubActions.svelte` | B — actions skeleton + reveal |
| `src/lib/components/github/GithubInsights.svelte` | B — per-card skeletons |
| `src/lib/components/github/GithubDetail.svelte` | B — detail skeleton + reveal |
| `src/lib/github/motion.ts` (opt) | B — `revealIn` reduced-motion-aware transition helper |
