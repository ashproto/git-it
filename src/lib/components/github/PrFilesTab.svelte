<script lang="ts">
  import { appState } from "../../store.svelte";
  import { githubState } from "../../githubState.svelte";
  import { reviewDraft } from "../../reviewDraft.svelte";
  import { dialogs } from "../../dialogs.svelte";
  import { splitPatchByFile, type PrDiffFile } from "../../github/prDiff";
  import DiffView from "../DiffView.svelte";
  import FileTree from "../FileTree.svelte";
  import { buildFileTree } from "../../fileTree";
  import GithubSkeleton from "./GithubSkeleton.svelte";

  let { number }: { number: number } = $props();

  $effect(() => {
    const repo = appState.repo;
    void githubState.reloadNonce; // re-fetch on Refresh, like the other panels
    if (repo) void githubState.loadPrDiff(repo, number);
  });

  const panel = $derived(githubState.prDiff);
  const files = $derived(splitPatchByFile(panel.data ?? ""));

  // Selected file. Reset to the first file whenever the diff (i.e. the PR)
  // changes; reads the patch so it re-runs on new data, never reads selectedIdx
  // so it can't loop. (Same pattern as CommitFilesDiff.)
  let selectedIdx = $state(0);
  $effect(() => {
    void panel.data;
    selectedIdx = 0;
  });
  const selected = $derived(files[selectedIdx] ?? files[0] ?? null);

  const basename = (p: string) => p.split("/").pop() ?? p;
  const dirname = (p: string) => {
    const i = p.lastIndexOf("/");
    return i >= 0 ? p.slice(0, i + 1) : "";
  };

  // ── Review-comment composer ─────────────────────────────────────────────────
  // Opened by clicking a line's ＋/💬 in the diff gutter; pinned at the bottom
  // of the tab (never injected into diff rows). Saving adds to (or updates) the
  // shared reviewDraft; the ReviewBar submits the whole draft in one shot.
  let composer = $state<{ line: number; side: "LEFT" | "RIGHT"; body: string } | null>(null);

  // Close the composer when the user switches file or the diff (PR) changes —
  // its line/side would point into the wrong file. Reads only the triggers.
  $effect(() => {
    void selectedIdx;
    void panel.data;
    composer = null;
  });

  // Index of the existing draft comment the composer is editing, or -1 when new.
  const editingIdx = $derived(
    composer && selected ? reviewDraft.indexAt(selected.path, composer.line, composer.side) : -1,
  );

  function openComposer(line: number, side: "LEFT" | "RIGHT") {
    const existing = selected ? reviewDraft.commentAt(selected.path, line, side) : undefined;
    composer = { line, side, body: existing?.body ?? "" };
  }

  async function saveComposer() {
    const c = composer;
    const repo = appState.repo;
    if (!c || !selected || !repo || !c.body.trim()) return;
    if (editingIdx >= 0) {
      reviewDraft.updateComment(editingIdx, c.body);
      composer = null;
      return;
    }
    const comment = { path: selected.path, line: c.line, side: c.side, body: c.body };
    try {
      reviewDraft.addComment(repo, number, comment);
    } catch {
      // Draft belongs to another PR/repo — ask before discarding it.
      const ok = await dialogs.confirm({
        title: "Pending review draft",
        message:
          "You have a pending review draft on another pull request. Discard it and start a new one here?",
        confirmLabel: "Discard draft",
        danger: true,
      });
      if (!ok) return;
      reviewDraft.discard();
      reviewDraft.addComment(repo, number, comment);
    }
    composer = null;
  }

  function removeComposer() {
    if (editingIdx >= 0) reviewDraft.removeComment(editingIdx);
    composer = null;
  }

  function focusOnMount(el: HTMLElement) {
    el.focus();
  }
</script>

<!-- Tree-mode leaf: the SAME .file button as flat mode (basename + counts),
     indented and selecting THAT file. Selection is by index, so map the leaf
     item back to its index by identity. -->
{#snippet prFileRow(f: PrDiffFile, ind: number)}
  {@const i = files.indexOf(f)}
  <button
    type="button"
    class="file"
    class:active={i === selectedIdx}
    style={`padding-left:${ind}px`}
    onclick={() => (selectedIdx = i)}
    title={f.path}
  >
    <span class="fname">{basename(f.path)}</span>
    <span class="fstat"><span class="add">+{f.additions}</span> <span class="del">−{f.deletions}</span></span>
  </button>
{/snippet}

{#if panel.status === "loading" && !panel.data}
  <GithubSkeleton variant="list" rows={5} />
{:else if panel.status === "error"}
  <p class="note err">Could not load the diff ({panel.error?.kind}).</p>
{:else if files.length === 0}
  <p class="empty">No file changes.</p>
{:else}
  <div class="files-toolbar">
    <span class="ft-label">{files.length} file{files.length === 1 ? "" : "s"} changed</span>
    <button
      class="ft-toggle"
      class:active={appState.fileTreeView}
      onclick={() => appState.setFileTreeView(!appState.fileTreeView)}
      title="Toggle folder tree view"
      aria-label="Tree view"
      aria-pressed={appState.fileTreeView}
    >⊟ Tree</button>
  </div>
  <div class="master-detail">
    <ul class="filelist">
      {#if appState.fileTreeView}
        <FileTree nodes={buildFileTree(files, (f) => f.path)} fileRow={prFileRow} />
      {:else}
        {#each files as f, i (f.path + ":" + i)}
          <li>
            <button
              type="button"
              class="file"
              class:active={i === selectedIdx}
              onclick={() => (selectedIdx = i)}
              title={f.path}
            >
              <span class="fname">{basename(f.path)}</span>
              {#if dirname(f.path)}<span class="fdir">{dirname(f.path)}</span>{/if}
              <span class="fstat"><span class="add">+{f.additions}</span> <span class="del">−{f.deletions}</span></span>
            </button>
          </li>
        {/each}
      {/if}
    </ul>
    <div class="diffpane">
      {#if selected}
        {#key selected.patch}
          <DiffView
            patch={selected.patch}
            onLineComment={openComposer}
            hasComment={(l, s) => !!reviewDraft.commentAt(selected.path, l, s)}
          />
        {/key}
      {/if}
    </div>
  </div>

  {#if composer && selected}
    <div class="composer">
      <div class="c-head">
        <span class="c-loc mono">{selected.path}:{composer.line} ({composer.side})</span>
        {#if editingIdx >= 0}<span class="c-editing">editing draft comment</span>{/if}
      </div>
      <textarea
        bind:value={composer.body}
        placeholder="Leave a review comment…"
        rows="3"
        use:focusOnMount
        aria-label="Review comment on {selected.path} line {composer.line}"
        onkeydown={(e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
            e.preventDefault();
            void saveComposer();
          } else if (e.key === "Escape") {
            composer = null;
          }
        }}
      ></textarea>
      <div class="c-row">
        <span class="c-hint">⌘⏎ to save · part of your pending review</span>
        {#if editingIdx >= 0}
          <button type="button" class="c-remove" onclick={removeComposer}>Remove</button>
        {/if}
        <button type="button" class="c-cancel" onclick={() => (composer = null)}>Cancel</button>
        <button type="button" class="c-save" disabled={!composer.body.trim()} onclick={saveComposer}>
          Save
        </button>
      </div>
    </div>
  {/if}
{/if}

<style>
  .files-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px 4px 0;
  }
  .ft-label {
    flex: 1;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .ft-toggle {
    padding: 2px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
  }
  .ft-toggle.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .ft-toggle:hover {
    background: var(--btn-hover);
  }

  .master-detail {
    display: flex;
    height: clamp(320px, 58vh, 820px);
    margin-top: 4px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel-bg);
    padding: 6px;
  }
  .filelist {
    flex: 0 0 240px;
    overflow: auto;
    border-right: 1px solid var(--border-subtle, var(--border));
    list-style: none;
    margin: 0;
    padding: 6px 6px 6px 0;
  }
  .diffpane {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding-left: 10px;
  }
  .file {
    display: flex;
    align-items: baseline;
    gap: 7px;
    width: 100%;
    padding: 4px 8px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .file:hover {
    background: var(--row-hover);
  }
  .file.active {
    background: var(--row-selected);
  }
  .fname {
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 130px;
  }
  .fdir {
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }
  .fstat {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 11.5px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .add {
    color: var(--status-add, #2ea043);
  }
  .del {
    color: var(--err, #c0392b);
  }
  .empty {
    margin: 12px 0 0 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
  .note {
    margin: 8px 2px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .note.err {
    color: var(--err, #c0392b);
  }

  /* ── Review-comment composer strip ─────────────────────────────────────────── */
  .composer {
    margin-top: 8px;
    border: 1px solid var(--accent);
    border-radius: 10px;
    background: var(--panel-bg);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .c-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 12px;
  }
  .c-loc {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11.5px;
  }
  .c-editing {
    color: var(--text-muted);
    font-size: 11px;
    font-style: italic;
  }
  .composer textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 56px;
    font: inherit;
    font-size: 13px;
    line-height: 1.5;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--input-bg, var(--panel-bg));
    color: var(--text);
  }
  .composer textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .c-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .c-hint {
    margin-right: auto;
    font-size: 11px;
    color: var(--text-muted);
  }
  .c-cancel,
  .c-remove {
    padding: 4px 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  .c-cancel:hover,
  .c-remove:hover {
    background: var(--btn-hover);
  }
  .c-remove {
    color: var(--err, #c0392b);
    border-color: var(--err, #c0392b);
  }
  .c-save {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 7px;
    padding: 4px 14px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .c-save:hover:not(:disabled) {
    filter: brightness(1.06);
  }
  .c-save:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
