<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";
  import { api } from "../api";
  import { contextMenu } from "../contextMenu.svelte";
  import DiffView from "./DiffView.svelte";
  import CommitComposer from "./CommitComposer.svelte";
  import type { WorkingFile } from "../types";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  // ── Section derivations ──────────────────────────────────────────────────────

  const stagedFiles = $derived(appState.workingChanges.filter((f) => f.staged && !f.conflicted));
  const unstagedFiles = $derived(
    appState.workingChanges.filter((f) => f.unstaged && !f.untracked && !f.conflicted),
  );
  const untrackedFiles = $derived(appState.workingChanges.filter((f) => f.untracked));

  // W1: when "Merge Untracked into Unstaged" is on, the Unstaged section also lists
  // the untracked files (and the separate Untracked section is not rendered). Each
  // row still branches on f.untracked for its glyph + danger action, so per-file
  // correctness is preserved regardless of which section it appears in.
  const unstagedDisplay = $derived(
    appState.unifyUnstaged ? [...unstagedFiles, ...untrackedFiles] : unstagedFiles,
  );

  // ── Selected file + diff ─────────────────────────────────────────────────────

  const selectedFile = $derived(appState.selectedFile);

  // Determine whether the selected file is in the staged section (governs which
  // diff half to fetch and which hunk callbacks to wire).
  const selectedIsStaged = $derived(
    selectedFile !== null && stagedFiles.some((f) => f.path === selectedFile),
  );

  // Untracked files have no index entry, so the regular diff is empty — they need
  // the `git diff --no-index` path (and hunk/line staging doesn't apply to them).
  const selectedIsUntracked = $derived(
    selectedFile !== null && untrackedFiles.some((f) => f.path === selectedFile),
  );

  // Whether the selected file is a tracked, modified (non-untracked) unstaged file.
  const selectedIsUnstaged = $derived(
    selectedFile !== null && unstagedFiles.some((f) => f.path === selectedFile),
  );

  // Whether the selected file is displayed in the Unstaged section. In unified mode
  // that section also lists untracked files, so a selected untracked file counts too.
  // Drives the Unstaged header button's Fork-style "Stage" vs "Stage all".
  const selectedInUnstagedSection = $derived(
    selectedIsUnstaged || (appState.unifyUnstaged && selectedIsUntracked),
  );

  // Raw patch for the selected file. Re-fetches whenever selectedFile changes OR
  // workingChanges is refreshed (after every op, including hunk ops). We key on a
  // monotonic revision counter (workingChangesRev) instead of the file count so that
  // a hunk stage/unstage — which doesn't change the file count but does re-index
  // hunks on the backend — also triggers a re-fetch and prevents stale hunk indices.
  const diffKey = $derived(
    selectedFile !== null
      ? `${selectedFile}::${selectedIsStaged ? "staged" : selectedIsUntracked ? "untracked" : "unstaged"}::${appState.workingChangesRev}::${appState.effectiveDiffContext}`
      : "",
  );

  let diffPatch = $state("");
  let diffLoading = $state(false);

  $effect(() => {
    const key = diffKey; // register dependency
    if (!key || !selectedFile || !isTauri()) {
      diffPatch = "";
      return;
    }
    diffLoading = true;
    api
      .diff(appState.repo, selectedFile, selectedIsStaged, selectedIsUntracked, appState.effectiveDiffContext)
      .then((p) => {
        // Guard stale results: only apply if the key hasn't changed.
        if (key === diffKey) {
          diffPatch = p;
        }
      })
      .catch(() => {
        if (key === diffKey) diffPatch = "";
      })
      .finally(() => {
        if (key === diffKey) diffLoading = false;
      });
  });

  // ── Status glyph helper ──────────────────────────────────────────────────────

  const GLYPH: Record<string, string> = {
    modified: "M",
    added: "A",
    deleted: "D",
    renamed: "R",
    copied: "C",
    untracked: "?",
    conflicted: "!",
    typechange: "T",
  };

  function glyph(f: WorkingFile): string {
    return GLYPH[f.status] ?? "M";
  }

  // ── Row click ───────────────────────────────────────────────────────────────

  function selectFile(path: string) {
    appState.setSelectedFile(path === selectedFile ? null : path);
  }

  // ── Right-click context menu (Fork-style) ────────────────────────────────────
  // Mirrors GraphHistory.onRowContext: preventDefault, select the row (so the diff
  // updates), then open the shared contextMenu with file-state-appropriate actions.
  // discard/clean already show their own confirm — no extra confirm is added here.
  function onRowContext(e: MouseEvent, f: WorkingFile) {
    e.preventDefault();
    // Select the row so the diff pane follows the right-click. selectFile toggles
    // off when re-clicking the selected row, so only set it if not already selected.
    if (selectedFile !== f.path) appState.setSelectedFile(f.path);
    if (f.staged) {
      contextMenu.openAt(e.clientX, e.clientY, [
        { label: "Unstage", action: () => gitActions.unstage([f.path]) },
      ]);
    } else if (f.untracked) {
      contextMenu.openAt(e.clientX, e.clientY, [
        { label: "Stage", action: () => gitActions.stage([f.path]) },
        { separator: true },
        { label: "Remove", danger: true, action: () => gitActions.clean([f.path]) },
      ]);
    } else {
      contextMenu.openAt(e.clientX, e.clientY, [
        { label: "Stage", action: () => gitActions.stage([f.path]) },
        { separator: true },
        { label: "Discard changes", danger: true, action: () => gitActions.discard([f.path]) },
      ]);
    }
  }

  // ── Horizontal resize: file-list ↔ diff (ITEM 5) ─────────────────────────────
  // Mirrors GraphHistory's resize-handle convention. The .files column is LEFT-
  // anchored (the diff pane is flex:1 to its right), so it GROWS when the handle is
  // dragged RIGHT — hence `startW + delta`. The width is persisted in the store
  // (clamped 180–640). Double-click resets to the 300px default.
  function startFilesResize(e: PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = appState.localFilesWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    const onMove = (ev: PointerEvent) =>
      appState.setLocalFilesWidth(startW + (ev.clientX - startX));
    const onUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }
</script>

<div class="wc-view panel">
  {#if !isTauri()}
    <p class="desktop-only">Local changes are only available in the desktop app.</p>
  {:else if appState.workingChanges.length === 0}
    <p class="empty">No local changes — your working copy is clean.</p>
  {:else}
    <div class="master-detail" style={`--files-w:${appState.localFilesWidth}px`}>
      <div class="files">
      <!-- ── Staged ───────────────────────────────────────────────────────────── -->
      {#if stagedFiles.length > 0}
        <section class="file-section">
          <header class="section-header">
            <span class="section-title">Staged ({stagedFiles.length})</span>
            <button
              class="hdr-btn"
              onclick={() =>
                selectedIsStaged
                  ? gitActions.unstage([selectedFile!])
                  : gitActions.unstage(stagedFiles.map((f) => f.path))}
            >{selectedIsStaged ? "Unstage" : "Unstage all"}</button>
          </header>
          <ul class="file-list">
            {#each stagedFiles as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                onmousedown={() => selectFile(f.path)}
                oncontextmenu={(e) => onRowContext(e, f)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph staged">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- ── Unstaged ─────────────────────────────────────────────────────────── -->
      <!-- When unifyUnstaged is on, unstagedDisplay also includes untracked files; each
           row branches on f.untracked for its glyph + danger action (Remove vs Discard). -->
      {#if unstagedDisplay.length > 0}
        <section class="file-section">
          <header class="section-header">
            <span class="section-title">Unstaged ({unstagedDisplay.length})</span>
            <!-- Context-aware: when a file in THIS section is selected, stage just it
                 (in unified mode that selection may be an untracked file too); else
                 stage every file shown in the section. -->
            <button
              class="hdr-btn"
              onclick={() =>
                selectedInUnstagedSection
                  ? gitActions.stage([selectedFile!])
                  : gitActions.stage(unstagedDisplay.map((f) => f.path))}
            >{selectedInUnstagedSection ? "Stage" : "Stage all"}</button>
          </header>
          <ul class="file-list">
            {#each unstagedDisplay as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                onmousedown={() => selectFile(f.path)}
                oncontextmenu={(e) => onRowContext(e, f)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph" class:unstaged={!f.untracked} class:untracked={f.untracked}
                  >{glyph(f)}</span
                >
                <span class="path mono" title={f.path}>{f.path}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- ── Untracked ─────────────────────────────────────────────────────────── -->
      {#if !appState.unifyUnstaged && untrackedFiles.length > 0}
        <section class="file-section">
          <header class="section-header">
            <span class="section-title">Untracked ({untrackedFiles.length})</span>
            <button
              class="hdr-btn"
              onclick={() =>
                selectedIsUntracked
                  ? gitActions.stage([selectedFile!])
                  : gitActions.stage(untrackedFiles.map((f) => f.path))}
            >{selectedIsUntracked ? "Stage" : "Stage all"}</button>
          </header>
          <ul class="file-list">
            {#each untrackedFiles as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                onmousedown={() => selectFile(f.path)}
                oncontextmenu={(e) => onRowContext(e, f)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph untracked">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
      </div>

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="files-resize-handle"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize file list"
        title="Drag to resize · double-click to reset"
        onpointerdown={startFilesResize}
        ondblclick={() => appState.setLocalFilesWidth(300)}
      ></div>

      <!-- ── Diff pane (right column) ─────────────────────────────────────────── -->
      <div class="diff-pane">
        {#if !selectedFile}
          <p class="diff-loading">Select a file to view its diff.</p>
        {:else if diffLoading}
          <p class="diff-loading">Loading diff…</p>
        {:else}
          <DiffView
            patch={diffPatch}
            staged={selectedIsStaged}
            onStageHunk={selectedIsStaged || selectedIsUntracked
              ? undefined
              : (i) => gitActions.stageHunk(selectedFile!, i)}
            onUnstageHunk={selectedIsStaged
              ? (i) => gitActions.unstageHunk(selectedFile!, i)
              : undefined}
            onStageLines={selectedIsStaged || selectedIsUntracked
              ? undefined
              : (hi, sel) => gitActions.stageLines(selectedFile!, hi, sel)}
            onUnstageLines={selectedIsStaged
              ? (hi, sel) => gitActions.unstageLines(selectedFile!, hi, sel)
              : undefined}
          />
        {/if}
      </div>
    </div>
  {/if}

  <!-- Commit composer always mounted at the bottom -->
  <CommitComposer />
</div>

<style>
  .wc-view {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel-bg);
    overflow: hidden;
  }

  .desktop-only,
  .empty {
    margin: 0;
    padding: 16px 14px;
    font-size: 13px;
    color: var(--text-muted);
    font-style: italic;
  }

  /* ── File sections ──────────────────────────────────────────────────────────── */
  /* Master-detail: file sections on the left, diff on the right. */
  .master-detail {
    display: flex;
    height: clamp(320px, 56vh, 760px);
  }
  .files {
    flex: 0 0 var(--files-w, 300px);
    min-width: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  /* Vertical drag handle on the file-list ↔ diff boundary. A thin grabbable strip
     whose centred 1px line (::before) is the column divider; it thickens to the
     accent colour on hover. Mirrors GraphHistory's .col-resize-handle. */
  .files-resize-handle {
    flex: 0 0 6px;
    align-self: stretch;
    position: relative;
    cursor: col-resize;
    touch-action: none;
    z-index: 2;
  }
  .files-resize-handle::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 1px;
    background: var(--border);
    transform: translateX(-50%);
    transition: background 0.1s, width 0.1s;
  }
  .files-resize-handle:hover::before {
    background: var(--accent);
    width: 2px;
  }

  .file-section {
    border-bottom: 1px solid var(--border-subtle);
  }

  .section-header {
    display: flex;
    align-items: center;
    padding: 5px 10px 5px 12px;
    background: var(--header-bg);
    border-bottom: 1px solid var(--border-subtle);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .section-title {
    flex: 1;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .hdr-btn {
    padding: 2px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
  }
  .hdr-btn:hover {
    background: var(--btn-hover);
  }

  /* ── File list ──────────────────────────────────────────────────────────────── */
  .file-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .file-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px 4px 12px;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    font-size: 12.5px;
    min-height: 28px;
  }
  .file-row:last-child {
    border-bottom: none;
  }
  .file-row:hover {
    background: var(--row-hover);
  }
  .file-row.selected {
    background: var(--row-selected);
  }

  .glyph {
    flex: 0 0 auto;
    width: 16px;
    text-align: center;
    font-size: 11px;
    font-weight: 700;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    border-radius: 3px;
    padding: 1px 3px;
  }
  .glyph.staged {
    color: var(--diff-add-fg, #2da44e);
    background: rgba(46, 160, 67, 0.1);
  }
  .glyph.unstaged {
    color: var(--err);
    background: rgba(180, 83, 9, 0.1);
  }
  .glyph.untracked {
    color: var(--text-muted);
    background: var(--btn-bg);
  }

  .path {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--text);
  }

  /* ── Diff pane ──────────────────────────────────────────────────────────────── */
  .diff-pane {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }

  .diff-loading {
    margin: 0;
    padding: 12px 14px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
