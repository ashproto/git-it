<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";
  import { api } from "../api";
  import { contextMenu, type MenuItem } from "../contextMenu.svelte";
  import { revealItemInDir, openPath } from "@tauri-apps/plugin-opener";
  import DiffView from "./DiffView.svelte";
  import CommitComposer from "./CommitComposer.svelte";
  import FileTree from "./FileTree.svelte";
  import { buildFileTree } from "../fileTree";
  import type { WorkingFile } from "../types";
  import { crossfade, fade } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";

  // Fork/SourceTree-style "file flies to the other section" animation. A row that
  // leaves one section (out:send|global) and re-appears in another (in:receive|global)
  // with the SAME key flies between the two; animate:flip slides the rest. The
  // directives are |global so the move still fires in tree view when a folder
  // subtree unmounts (local transitions are suppressed by a parent unmount).
  // The fallback (a send/receive with NO partner — view open/close, repo switch, a
  // discarded file) is INSTANT (duration 0), so only genuine moves animate and
  // nothing flickers/fades on mount or unmount.
  const [send, receive] = crossfade({
    duration: 220,
    easing: quintOut,
    fallback: () => ({ duration: 0 }),
  });
  const FLIP = { duration: 220, easing: quintOut };

  // Last path segment — shown as the leaf label in tree mode (full path in title).
  const basename = (p: string) => p.split("/").pop() ?? p;

  // Working-tree absolute path for a repo-relative file (repo paths are absolute,
  // forward-slashed on macOS).
  const absPath = (rel: string) => `${appState.repo.replace(/\/$/, "")}/${rel}`;

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

  // Colour the status glyph by what the change MEANS (not by which section it's in,
  // which the section headers already convey): add = green, modify = yellow,
  // remove = red. Untracked keeps its own neutral tone (it's a distinct "?" state).
  function glyphClass(f: WorkingFile): string {
    if (f.untracked) return "s-untracked";
    switch (f.status) {
      case "added":
      case "copied":
        return "s-add";
      case "deleted":
      case "conflicted":
        return "s-del";
      // modified / renamed / typechange / anything else → a change to existing content.
      default:
        return "s-mod";
    }
  }

  // ── Row click ───────────────────────────────────────────────────────────────

  function selectFile(path: string) {
    appState.setSelectedFile(path === selectedFile ? null : path);
  }

  // ── Right-click context menu (Fork-style) ────────────────────────────────────
  // Mirrors GraphHistory.onRowContext: preventDefault, select the row (so the diff
  // updates), then open the shared contextMenu with file-state-appropriate actions.
  // discard/clean already show their own confirm — no extra confirm is added here.
  // apps_for_file is async, so IPC completion is no longer FIFO — a monotonic token
  // drops a stale menu-open if a newer right-click superseded it mid-await.
  let ctxSeq = 0;
  async function onRowContext(
    e: MouseEvent,
    f: WorkingFile,
    section: "staged" | "unstaged" | "untracked",
  ) {
    e.preventDefault();
    const seq = ++ctxSeq;
    const { clientX, clientY } = e;
    // Select the row so the diff pane follows the right-click. selectFile toggles
    // off when re-clicking the selected row, so only set it if not already selected.
    if (selectedFile !== f.path) appState.setSelectedFile(f.path);

    // File actions (Tauri only). A deleted file has no on-disk target, so Open /
    // Open With are disabled and Show in Finder opens the containing folder instead
    // (revealItemInDir canonicalizes the path first and would error on a gone file).
    const fileItems: MenuItem[] = [];
    if (isTauri()) {
      const abs = absPath(f.path);
      const gone = f.status === "deleted";
      const parent = abs.slice(0, abs.lastIndexOf("/"));
      fileItems.push({ label: "Open", disabled: gone, action: () => void openPath(abs) });
      if (!gone) {
        let apps: { name: string; path: string }[] = [];
        try {
          apps = await api.appsForFile(abs);
        } catch {
          apps = [];
        }
        if (apps.length > 0) {
          fileItems.push({
            label: "Open With",
            submenu: apps.map((a) => ({ label: a.name, action: () => void openPath(abs, a.path) })),
          });
        }
      }
      fileItems.push({
        label: "Show in Finder",
        action: () => void (gone ? openPath(parent) : revealItemInDir(abs)),
      });
      fileItems.push({ separator: true });
    }

    // Branch on the SECTION the row lives in — NOT on f.staged. A partially-staged
    // file (staged AND further-unstaged) appears in both the Staged and Unstaged
    // lists, so the menu must match where it was clicked and offer that section's
    // direction (else the Unstaged row would wrongly show only "Unstage").
    let sectionItems: MenuItem[];
    if (section === "staged") {
      sectionItems = [{ label: "Unstage", action: () => gitActions.unstage([f.path]) }];
    } else if (section === "untracked") {
      sectionItems = [
        { label: "Stage", action: () => gitActions.stage([f.path]) },
        { separator: true },
        { label: "Remove", danger: true, action: () => gitActions.clean([f.path]) },
      ];
    } else {
      // Unstaged section. In unified mode it may hold an untracked file (→ Remove);
      // otherwise it's a tracked file with unstaged edits (→ Discard).
      const removeItem = f.untracked
        ? { label: "Remove", danger: true, action: () => gitActions.clean([f.path]) }
        : { label: "Discard changes", danger: true, action: () => gitActions.discard([f.path]) };
      sectionItems = [
        { label: "Stage", action: () => gitActions.stage([f.path]) },
        { separator: true },
        removeItem,
      ];
    }

    // A newer right-click landed while we awaited apps_for_file — let it win.
    if (seq !== ctxSeq) return;
    contextMenu.openAt(clientX, clientY, [...fileItems, ...sectionItems]);
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

<!-- Tree-mode leaf rows: one snippet per section so each closes over its own section
     string for onRowContext. Each renders the SAME clickable row content as flat mode
     (glyph + name + selection + context-menu), but indented and showing the basename
     (full path in title). `ind` is the tree indent in px. FileTree wraps this content in
     the keyed <li> that carries the flip + crossfade fly (so a file still flies between
     sections AND the surrounding tree closes the gap), so these render only the inner
     <div>, with no transition directives of their own. -->

{#snippet stagedRow(f: WorkingFile, ind: number)}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="file-row"
    class:selected={selectedFile === f.path}
    role="row"
    tabindex="0"
    style={`padding-left:${ind}px`}
    onmousedown={() => selectFile(f.path)}
    oncontextmenu={(e) => onRowContext(e, f, "staged")}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") selectFile(f.path);
    }}
  >
    <span class="glyph {glyphClass(f)}">{glyph(f)}</span>
    <span class="path mono" title={f.path}>{basename(f.path)}</span>
  </div>
{/snippet}

{#snippet unstagedRow(f: WorkingFile, ind: number)}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="file-row"
    class:selected={selectedFile === f.path}
    role="row"
    tabindex="0"
    style={`padding-left:${ind}px`}
    onmousedown={() => selectFile(f.path)}
    oncontextmenu={(e) => onRowContext(e, f, "unstaged")}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") selectFile(f.path);
    }}
  >
    <span class="glyph {glyphClass(f)}">{glyph(f)}</span>
    <span class="path mono" title={f.path}>{basename(f.path)}</span>
  </div>
{/snippet}

{#snippet untrackedRow(f: WorkingFile, ind: number)}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="file-row"
    class:selected={selectedFile === f.path}
    role="row"
    tabindex="0"
    style={`padding-left:${ind}px`}
    onmousedown={() => selectFile(f.path)}
    oncontextmenu={(e) => onRowContext(e, f, "untracked")}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") selectFile(f.path);
    }}
  >
    <span class="glyph {glyphClass(f)}">{glyph(f)}</span>
    <span class="path mono" title={f.path}>{basename(f.path)}</span>
  </div>
{/snippet}

<div class="wc-view panel">
  {#if !isTauri()}
    <p class="desktop-only">Local changes are only available in the desktop app.</p>
  {:else}
    <!-- The Staged and Unstaged sections are ALWAYS rendered (even when empty), so
         the layout is stable and files visibly move between them (Fork/SourceTree). -->
    <div class="master-detail" style={`--files-w:${appState.localFilesWidth}px`}>
      <div class="files">
      <!-- ── View toggle (flat list ↔ folder tree) — one control for all sections ── -->
      <div class="files-toolbar">
        <span class="ft-label">Local Changes</span>
        <button
          class="ft-toggle"
          class:active={appState.fileTreeView}
          onclick={() => appState.setFileTreeView(!appState.fileTreeView)}
          title="Toggle folder tree view"
          aria-label="Tree view"
          aria-pressed={appState.fileTreeView}
        >⊟ Tree</button>
      </div>
      <!-- Scroll container for just the sections, so the toolbar above and the
           per-section sticky headers below it never overlap (the toolbar is
           OUTSIDE this scroll; section headers stick to the top of THIS box).
           Clicking its empty area (below the rows — target === this container, so
           clicks bubbling up from a file row are ignored) deselects the file. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="files-scroll"
        onmousedown={(e) => {
          if (e.target === e.currentTarget) appState.setSelectedFile(null);
        }}
      >
      <!-- ── Staged (always shown) ───────────────────────────────────────────── -->
      <section class="file-section">
        <header class="section-header">
          <span class="section-title">Staged ({stagedFiles.length})</span>
          <button
            class="hdr-btn"
            disabled={stagedFiles.length === 0}
            onclick={() =>
              selectedIsStaged
                ? gitActions.unstage([selectedFile!])
                : gitActions.unstage(stagedFiles.map((f) => f.path))}
          >{selectedIsStaged ? "Unstage" : "Unstage all"}</button>
        </header>
        {#if appState.fileTreeView}
          <ul class="file-list">
            <FileTree
              nodes={buildFileTree(stagedFiles, (f) => f.path)}
              fileRow={stagedRow}
              animate
              {send}
              {receive}
            />
          </ul>
        {:else}
          <ul class="file-list">
            {#each stagedFiles as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                in:receive|global={{ key: f.path }}
                out:send|global={{ key: f.path }}
                animate:flip={FLIP}
                onmousedown={() => selectFile(f.path)}
                oncontextmenu={(e) => onRowContext(e, f, "staged")}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph {glyphClass(f)}">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
              </li>
            {/each}
          </ul>
        {/if}
        {#if stagedFiles.length === 0}
          <p class="section-empty" in:fade={{ duration: 150 }}>Nothing staged</p>
        {/if}
      </section>

      <!-- ── Unstaged ─────────────────────────────────────────────────────────── -->
      <!-- When unifyUnstaged is on, unstagedDisplay also includes untracked files; each
           row branches on f.untracked for its glyph + danger action (Remove vs Discard). -->
      <section class="file-section">
        <header class="section-header">
          <span class="section-title">Unstaged ({unstagedDisplay.length})</span>
          <!-- Context-aware: when a file in THIS section is selected, stage just it
               (in unified mode that selection may be an untracked file too); else
               stage every file shown in the section. -->
          <button
            class="hdr-btn"
            disabled={unstagedDisplay.length === 0}
            onclick={() =>
              selectedInUnstagedSection
                ? gitActions.stage([selectedFile!])
                : gitActions.stage(unstagedDisplay.map((f) => f.path))}
          >{selectedInUnstagedSection ? "Stage" : "Stage all"}</button>
        </header>
        {#if appState.fileTreeView}
          <ul class="file-list">
            <FileTree
              nodes={buildFileTree(unstagedDisplay, (f) => f.path)}
              fileRow={unstagedRow}
              animate
              {send}
              {receive}
            />
          </ul>
        {:else}
          <ul class="file-list">
            {#each unstagedDisplay as f (f.path)}
              <li
                class="file-row"
                class:selected={selectedFile === f.path}
                role="row"
                tabindex="0"
                in:receive|global={{ key: f.path }}
                out:send|global={{ key: f.path }}
                animate:flip={FLIP}
                onmousedown={() => selectFile(f.path)}
                oncontextmenu={(e) => onRowContext(e, f, "unstaged")}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                }}
              >
                <span class="glyph {glyphClass(f)}">{glyph(f)}</span>
                <span class="path mono" title={f.path}>{f.path}</span>
              </li>
            {/each}
          </ul>
        {/if}
        {#if unstagedDisplay.length === 0}
          <p class="section-empty" in:fade={{ duration: 150 }}>No unstaged changes</p>
        {/if}
      </section>

      <!-- ── Untracked ─────────────────────────────────────────────────────────── -->
      <!-- Rendered whenever NOT unified (even at 0 files) so the FileTree stays mounted:
           staging the LAST untracked file is then a local keyed-row removal that still
           flies to Staged, instead of the whole section unmounting and suppressing the
           fly. The header is hidden and the section border removed when empty, so an
           empty Untracked section is invisible. -->
      {#if !appState.unifyUnstaged}
        <section class="file-section" class:is-empty={untrackedFiles.length === 0}>
          {#if untrackedFiles.length > 0}
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
          {/if}
          {#if appState.fileTreeView}
            <ul class="file-list">
              <FileTree
                nodes={buildFileTree(untrackedFiles, (f) => f.path)}
                fileRow={untrackedRow}
                animate
                {send}
                {receive}
              />
            </ul>
          {:else}
            <ul class="file-list">
              {#each untrackedFiles as f (f.path)}
                <li
                  class="file-row"
                  class:selected={selectedFile === f.path}
                  role="row"
                  tabindex="0"
                  in:receive|global={{ key: f.path }}
                  out:send|global={{ key: f.path }}
                  animate:flip={FLIP}
                  onmousedown={() => selectFile(f.path)}
                  oncontextmenu={(e) => onRowContext(e, f, "untracked")}
                  onkeydown={(e) => {
                    if (e.key === "Enter" || e.key === " ") selectFile(f.path);
                  }}
                >
                  <span class="glyph {glyphClass(f)}">{glyph(f)}</span>
                  <span class="path mono" title={f.path}>{f.path}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
      </div>
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
            onDiscardHunk={selectedIsStaged || selectedIsUntracked
              ? undefined
              : (i) => gitActions.discardHunk(selectedFile!, i)}
            onDiscardLines={selectedIsStaged || selectedIsUntracked
              ? undefined
              : (hi, sel) => gitActions.discardLines(selectedFile!, hi, sel)}
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
    border-radius: var(--radius-dialog);
    background: var(--panel-bg);
    overflow: hidden;
    /* Fill the available vertical space in the main column (the parent flex chain
       is full-height) so there's no dead space under the commit composer. */
    flex: 1;
    min-height: 0;
  }

  .desktop-only {
    margin: 0;
    padding: 16px 14px;
    font-size: 13px;
    color: var(--text-muted);
    font-style: italic;
  }

  /* Placeholder shown inside an empty Staged/Unstaged section. */
  .section-empty {
    margin: 0;
    padding: 10px 12px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  /* ── File sections ──────────────────────────────────────────────────────────── */
  /* Master-detail: file sections on the left, diff on the right. */
  .master-detail {
    display: flex;
    /* Grow to fill the wc-view (which fills the main column); min-height:0 lets it
       shrink on short windows so the commit composer below is never clipped. */
    flex: 1;
    min-height: 0;
  }
  .files {
    flex: 0 0 var(--files-w, 300px);
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  /* Toolbar above the sections: holds the one flat/tree view toggle. It sits
     OUTSIDE the .files-scroll box (below), so it never overlaps the per-section
     sticky headers when the list scrolls. */
  .files-toolbar {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px 5px 12px;
    background: var(--header-bg);
    border-bottom: 1px solid var(--border-subtle);
  }

  /* The only scrolling region of the file column: the sections. Section headers
     stick to the top of THIS box, beneath the (non-scrolling) toolbar. */
  .files-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
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
    color: var(--on-accent);
  }
  .ft-toggle:hover {
    background: var(--btn-hover);
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
  /* An empty (header-less) Untracked section is kept mounted so its FileTree's last-row
     fly still works; drop its border so it's fully invisible when it has no files. */
  .file-section.is-empty {
    border-bottom: none;
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
  .hdr-btn:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  .hdr-btn:disabled {
    opacity: 0.4;
    cursor: default;
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
  /* Only the flat-list rows are <li>s; drop the trailing separator on the last one.
     In tree mode the row is a <div> wrapped in FileTree's <li>, so this li-scoped
     rule doesn't match and every tree file row keeps its separator. */
  li.file-row:last-child {
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
    font-family: var(--font-mono);
    border-radius: 3px;
    padding: 1px 3px;
  }
  /* Status-based colours: add = green, modify = yellow, remove = red. Conveys what
     the change IS (the section header already conveys staged/unstaged). */
  .glyph.s-add {
    color: var(--status-add, #2da44e);
    background: rgba(46, 160, 67, 0.1);
  }
  .glyph.s-mod {
    color: var(--status-mod, #bf8700);
    background: rgba(191, 135, 0, 0.12);
  }
  .glyph.s-del {
    color: var(--status-del, #cf222e);
    background: rgba(207, 34, 46, 0.1);
  }
  .glyph.s-untracked {
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
    font-family: var(--font-mono);
  }

  /* Stacked/narrow layout: the shell is content-sized (no full-height chain), so
     restore a sensible floor for the master-detail (the wide-mode flex:1 fill has
     no effect here). Mirrors the pre-round-6 clamp minimum. */
  @media (max-width: 900px) {
    .master-detail {
      min-height: 320px;
    }
  }
</style>
