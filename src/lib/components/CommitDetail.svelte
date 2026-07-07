<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { parseISO, formatCommitDate } from "../dates";
  import { reflowCommitBody } from "../commitBody";
  import CommitFilesDiff from "./CommitFilesDiff.svelte";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import RefIcon from "./RefIcon.svelte";
  import CommitMessageEdit from "./CommitMessageEdit.svelte";
  import EditTabs from "./EditTabs.svelte";
  import ApplyPanel from "./ApplyPanel.svelte";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  // Layout props forwarded to the underlying CollapsiblePanel:
  // - height: fixed pixel height (stable while the diff loads) when the graph is expanded.
  // - fill: grow to fill instead (used when the graph is collapsed so the details pane
  //   takes the freed space).
  // - collapsed: bindable so the parent (+page) knows whether the details are collapsed.
  let {
    height,
    fill = false,
    collapsed = $bindable(false),
  }: { height?: number; fill?: boolean; collapsed?: boolean } = $props();

  const c = $derived(appState.selectedCommit);
  // Reflow the hard-wrapped commit body into paragraph blocks so it fills the width
  // instead of keeping the author's ~72-col line breaks (see commitBody.ts).
  const bodyBlocks = $derived(c?.body ? reflowCommitBody(c.body) : []);

  // Edit mode: the panel flips between read-only details and the inline edit tools
  // (message reword + date editing). The date editor (EditTabs) targets
  // appState.selected, so this covers single- AND multi-commit edits. Exit edit
  // mode whenever the focused commit changes (i.e. selecting another row).
  let editing = $state(false);
  const selCount = $derived(appState.selected.size);
  // Exit edit mode when navigating to a DIFFERENT commit as a fresh single
  // selection (a plain click). Growing a multi-selection (cmd/shift-click → size
  // > 1) while editing stays in edit mode, so a set can be built up and edited
  // together. prevSha is a plain (non-reactive) cursor of the last focused sha.
  let prevSha: string | null = null;
  $effect(() => {
    const sha = appState.currentSha;
    const multi = appState.selected.size > 1;
    if (sha !== prevSha) {
      prevSha = sha;
      if (!multi) editing = false;
    }
  });

  function fmt(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  function initials(name: string): string {
    return name
      .split(/\s+/)
      .map((s) => s[0] ?? "")
      .slice(0, 2)
      .join("")
      .toUpperCase();
  }

  // Commit diff state
  let diffPatch = $state<string>("");
  let diffError = $state<string | null>(null);
  let diffLoading = $state(false);

  // Re-fetch when selected commit changes
  $effect(() => {
    const sha = appState.currentSha;
    // Read so the effect re-runs when the context / whole-file setting changes.
    const context = appState.effectiveDiffContext;
    if (!sha || !isTauri() || !appState.repo) {
      diffPatch = "";
      diffError = null;
      diffLoading = false;
      return;
    }
    diffLoading = true;
    diffError = null;
    diffPatch = "";
    api.commitDiff(appState.repo, sha, null, context)
      .then((patch) => {
        // Guard: the user may have navigated away during the async gap.
        if (appState.currentSha !== sha) return;
        diffPatch = patch;
        diffLoading = false;
      })
      .catch((e) => {
        if (appState.currentSha !== sha) return;
        diffError = String(e).split("\n")[0];
        diffLoading = false;
      });
  });
</script>

<CollapsiblePanel title={editing ? "Edit commit(s)" : "Commit"} {height} {fill} bind:collapsed>
  {#snippet headerActions()}
    {#if c}
      <button
        type="button"
        class="edit-btn"
        class:active={editing}
        aria-pressed={editing}
        onclick={() => (editing = !editing)}
        title={editing ? "Back to commit details" : "Edit this commit's message and date"}
      >{editing ? "Done" : selCount > 1 ? `Edit ${selCount} commits` : "Edit"}</button>
    {/if}
  {/snippet}
  {#if c}
    {#if editing}
      <!-- Inline edit tools (message reword + date editing). The date editor
           targets appState.selected, so this edits the focused commit or all
           currently-selected commits. -->
      <div class="inline-edit">
        <CommitMessageEdit />
        <EditTabs bare />
        <ApplyPanel bare />
      </div>
    {:else}
    <div class="hdr">
      <div class="avatar" aria-hidden="true">{initials(c.author_name)}</div>
      <div class="ttl">
        <div class="subj">{c.subject}</div>
        <div class="sub">{c.author_name} committed {fmt(c.committer_date)}</div>
      </div>
      <span class="sha mono">{c.sha.slice(0, 10)}</span>
    </div>

    <!-- Metadata grid + description side by side so the body uses the empty space to
         the right of the short key/value rows; it drops below on a narrow panel. -->
    <div class="meta-row">
      <div class="grid">
      <span class="k">Author</span>
      <span class="v">{c.author_name} &lt;{c.author_email}&gt;</span>
      <span class="k">Authored</span>
      <span class="v mono">{fmt(c.author_date)}</span>
      <span class="k">Committed</span>
      <span class="v mono">{fmt(c.committer_date)}</span>
      <span class="k">Parents</span>
      <span class="v mono">{c.parents.map((p) => p.slice(0, 9)).join(", ") || "(root commit)"}</span>
      {#if c.refs.length}
        <span class="k">Refs</span>
        <span class="v badges">
          {#each c.refs as r (r.name + r.kind)}
            <span
              class="badge {r.kind}"
              class:current={r.is_head}
              style={`--ref-color:${appState.colorForRef(r.name, c.sha)}`}
            ><RefIcon kind={r.kind} />{r.name}</span>
          {/each}
        </span>
      {/if}
      </div>

      <!-- Body sits beside the grid (see .meta-row). Reflowed into paragraph blocks so it
           fills the width; blank-line breaks, list items and trailers are preserved. -->
      {#if bodyBlocks.length}
        <div class="body-msg">
          {#each bodyBlocks as blk, i}
            <p class="para" class:tight={blk.tight} class:first={i === 0}>{blk.text}</p>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Diff section -->
    <div class="diff-section">
      {#if !isTauri()}
        <p class="note">Commit diff is available in the desktop app only.</p>
      {:else if diffLoading}
        <p class="note">Loading diff…</p>
      {:else if diffError}
        <p class="note err">Could not load diff: {diffError}</p>
      {:else}
        <!-- The file-count ("N files changed") is shown by CommitFilesDiff's own
             toolbar (next to the tree-view toggle), so it isn't repeated here. -->
        <CommitFilesDiff patch={diffPatch} />
      {/if}
    </div>
    {/if}
  {:else}
    <p class="empty">Select a commit to see its details.</p>
  {/if}
</CollapsiblePanel>

<style>
  .hdr {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
  }
  .avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--row-selected);
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
  }
  .ttl {
    flex: 1;
    min-width: 0;
  }
  .subj {
    font-weight: 600;
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }
  .sha {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  /* Metadata grid + description side by side. flex-wrap drops the body below the grid
     when the panel is too narrow to fit both; align-items:flex-start keeps the grid
     pinned to the top so a tall body doesn't vertically center the metadata. */
  .meta-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 12px 24px;
  }
  .grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 14px;
    font-size: 12.5px;
    /* Keep the metadata column compact so the description gets the remaining width.
       max-width is wide enough that the longest value (a date) never truncates. */
    flex: 0 1 auto;
    min-width: 200px;
    max-width: 340px;
  }
  .k {
    color: var(--text-muted);
  }
  .v {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .badge {
    /* Colour matches the ref's graph lane (or manual override) via --ref-color;
       the RefIcon glyph conveys local / remote / tag. */
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--ref-color, var(--border));
    color: var(--ref-color, var(--text-muted));
  }
  .badge.current {
    font-weight: 600;
    background: color-mix(in srgb, var(--ref-color, var(--accent)) 16%, transparent);
  }
  .note {
    margin: 12px 0 0 0;
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }
  /* Commit message body, beside the grid in .meta-row (so no top margin — the row gap
     spaces it, including the row-gap when it wraps below on a narrow panel). flex:1 1
     260px takes the remaining width but wraps under the grid below ~260px. Each child
     <p> is a reflowed paragraph that wraps NORMALLY to fill the column; break-word stops
     an over-long token from forcing horizontal scroll. */
  .body-msg {
    flex: 1 1 260px;
    min-width: 0;
    overflow-wrap: break-word;
    word-break: break-word;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text);
  }
  .body-msg .para {
    margin: 0;
  }
  /* Paragraph gap between true paragraphs; tight blocks (consecutive list items /
     trailers, no blank line between) sit closer. .first never gets a top margin. */
  .body-msg .para + .para {
    margin-top: 0.75em;
  }
  .body-msg .para.tight {
    margin-top: 0.15em;
  }
  .diff-section {
    margin-top: 14px;
    border-top: 1px solid var(--border-subtle);
  }
  .err {
    color: var(--err, #c0392b);
  }
  .empty {
    margin: 0;
    color: var(--text-muted);
    font-style: italic;
    font-size: 13px;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 11.5px;
  }

  /* Edit toggle in the panel header (Edit ⇄ Done). */
  .edit-btn {
    padding: 3px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s, border-color 0.1s;
  }
  .edit-btn:hover {
    background: var(--btn-hover);
  }
  .edit-btn.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  /* Inline edit body (message reword + date editing), merged from the old
     separate "Edit commit(s)" panel. */
  .inline-edit {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
</style>
