<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";
  import { api } from "../api";
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

  // ── Selected file + diff ─────────────────────────────────────────────────────

  const selectedFile = $derived(appState.selectedFile);

  // Determine whether the selected file is in the staged section (governs which
  // diff half to fetch and which hunk callbacks to wire).
  const selectedIsStaged = $derived(
    selectedFile !== null && stagedFiles.some((f) => f.path === selectedFile),
  );

  // Raw patch for the selected file. Re-fetches whenever selectedFile changes OR
  // workingChanges is refreshed (after every op, including hunk ops). We key on a
  // monotonic revision counter (workingChangesRev) instead of the file count so that
  // a hunk stage/unstage — which doesn't change the file count but does re-index
  // hunks on the backend — also triggers a re-fetch and prevents stale hunk indices.
  const diffKey = $derived(
    selectedFile !== null
      ? `${selectedFile}::${selectedIsStaged ? "staged" : "unstaged"}::${appState.workingChangesRev}`
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
      .diff(appState.repo, selectedFile, selectedIsStaged)
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
</script>

<div class="wc-view panel">
  {#if !isTauri()}
    <p class="desktop-only">Working copy is only available in the desktop app.</p>
  {:else if appState.workingChanges.length === 0}
    <p class="empty">Working copy is clean — no changes.</p>
  {:else}
    <div class="sections">
      <!-- ── Staged ───────────────────────────────────────────────────────────── -->
      {#if stagedFiles.length > 0}
        <section class="file-section">
          <header class="section-header">
            <span class="section-title">Staged ({stagedFiles.length})</span>
            <button
              class="hdr-btn"
              onclick={() => gitActions.unstage(stagedFiles.map((f) => f.path))}
            >Unstage all</button>
          </header>
          <ul class="file-list">
            {#each stagedFiles as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                onmousedown={() => selectFile(f.path)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph staged">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
                <span class="row-actions">
                  <button
                    class="row-btn"
                    onclick={(e) => {
                      e.stopPropagation();
                      gitActions.unstage([f.path]);
                    }}>Unstage</button
                  >
                </span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- ── Unstaged ─────────────────────────────────────────────────────────── -->
      {#if unstagedFiles.length > 0}
        <section class="file-section">
          <header class="section-header">
            <span class="section-title">Unstaged ({unstagedFiles.length})</span>
            <button
              class="hdr-btn"
              onclick={() => gitActions.stage(unstagedFiles.map((f) => f.path))}
            >Stage all</button>
          </header>
          <ul class="file-list">
            {#each unstagedFiles as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                onmousedown={() => selectFile(f.path)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph unstaged">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
                <span class="row-actions">
                  <button
                    class="row-btn"
                    onclick={(e) => {
                      e.stopPropagation();
                      gitActions.stage([f.path]);
                    }}>Stage</button
                  >
                  <button
                    class="row-btn danger"
                    onclick={(e) => {
                      e.stopPropagation();
                      gitActions.discard([f.path]);
                    }}>Discard</button
                  >
                </span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- ── Untracked ─────────────────────────────────────────────────────────── -->
      {#if untrackedFiles.length > 0}
        <section class="file-section">
          <header class="section-header">
            <span class="section-title">Untracked ({untrackedFiles.length})</span>
            <button
              class="hdr-btn"
              onclick={() => gitActions.stage(untrackedFiles.map((f) => f.path))}
            >Stage all</button>
          </header>
          <ul class="file-list">
            {#each untrackedFiles as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                onmousedown={() => selectFile(f.path)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph untracked">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
                <span class="row-actions">
                  <button
                    class="row-btn"
                    onclick={(e) => {
                      e.stopPropagation();
                      gitActions.stage([f.path]);
                    }}>Stage</button
                  >
                  <button
                    class="row-btn danger"
                    onclick={(e) => {
                      e.stopPropagation();
                      gitActions.clean([f.path]);
                    }}>Remove</button
                  >
                </span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>

    <!-- ── Diff pane ────────────────────────────────────────────────────────────── -->
    {#if selectedFile}
      <div class="diff-pane">
        {#if diffLoading}
          <p class="diff-loading">Loading diff…</p>
        {:else}
          <DiffView
            patch={diffPatch}
            staged={selectedIsStaged}
            onStageHunk={selectedIsStaged
              ? undefined
              : (i) => gitActions.stageHunk(selectedFile!, i)}
            onUnstageHunk={selectedIsStaged
              ? (i) => gitActions.unstageHunk(selectedFile!, i)
              : undefined}
          />
        {/if}
      </div>
    {/if}
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
  .sections {
    display: flex;
    flex-direction: column;
    gap: 0;
    max-height: 260px;
    overflow-y: auto;
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

  .row-actions {
    flex: 0 0 auto;
    display: flex;
    gap: 4px;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .file-row:hover .row-actions,
  .file-row.selected .row-actions {
    opacity: 1;
  }

  .row-btn {
    padding: 1px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
  }
  .row-btn:hover {
    background: var(--btn-hover);
  }
  .row-btn.danger {
    color: var(--danger);
    border-color: var(--danger);
  }
  .row-btn.danger:hover {
    background: rgba(220, 38, 38, 0.08);
  }

  /* ── Diff pane ──────────────────────────────────────────────────────────────── */
  .diff-pane {
    flex: 1 1 auto;
    min-height: 120px;
    max-height: 360px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
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
