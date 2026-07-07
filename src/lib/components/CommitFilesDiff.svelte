<script lang="ts">
  import { splitDiffFiles } from "../diff/split";
  import DiffView from "./DiffView.svelte";
  import FileTree from "./FileTree.svelte";
  import { buildFileTree } from "../fileTree";
  import { appState } from "../store.svelte";
  import type { DiffFileEntry } from "../diff/split";

  let { patch }: { patch: string } = $props();

  const files = $derived(splitDiffFiles(patch));

  // Selected file. Reset to the first file whenever the patch (i.e. the commit)
  // changes; reads `patch` so it re-runs on a new commit, never reads selectedIdx
  // so it can't loop.
  let selectedIdx = $state(0);
  $effect(() => {
    void patch;
    selectedIdx = 0;
  });
  const selected = $derived(files[selectedIdx] ?? files[0] ?? null);

  const STATUS_LABEL: Record<string, string> = {
    added: "A",
    deleted: "D",
    modified: "M",
    renamed: "R",
  };
  const basename = (p: string) => p.split("/").pop() ?? p;
  const dirname = (p: string) => {
    const i = p.lastIndexOf("/");
    return i >= 0 ? p.slice(0, i + 1) : "";
  };
</script>

<!-- Tree-mode leaf: the SAME .file button as flat mode (status badge + basename),
     indented and selecting THAT file. Selection is by index, so map the leaf item
     back to its index by identity. FileTree provides the wrapping <li>, so this
     renders only the button. -->
{#snippet commitFileRow(f: DiffFileEntry, ind: number)}
  {@const i = files.indexOf(f)}
  <button
    type="button"
    class="file"
    class:active={i === selectedIdx}
    style={`padding-left:${ind}px`}
    onclick={() => (selectedIdx = i)}
    title={f.path}
  >
    <span class="status {f.status}" title={f.status}>{STATUS_LABEL[f.status]}</span>
    <span class="fname">{basename(f.path)}</span>
  </button>
{/snippet}

{#if files.length === 0}
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
        <FileTree nodes={buildFileTree(files, (f) => f.path)} fileRow={commitFileRow} />
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
              <span class="status {f.status}" title={f.status}>{STATUS_LABEL[f.status]}</span>
              <span class="fname">{basename(f.path)}</span>
              {#if dirname(f.path)}<span class="fdir">{dirname(f.path)}</span>{/if}
            </button>
          </li>
        {/each}
      {/if}
    </ul>
    <div class="diffpane">
      {#if selected}
        {#key selected.patch}
          <DiffView patch={selected.patch} />
        {/key}
      {/if}
    </div>
  </div>
{/if}

<style>
  .files-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
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
  }
  .filelist {
    flex: 0 0 230px;
    overflow: auto;
    border-right: 1px solid var(--border-subtle);
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
    border-radius: var(--radius-md);
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
  .status {
    flex-shrink: 0;
    width: 12px;
    text-align: center;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
  }
  /* Match the Local Changes scheme: add = green, modify = yellow, remove = red.
     A rename is a change to an existing file → yellow (modify). */
  .status.added {
    color: var(--status-add, #2da44e);
  }
  .status.modified {
    color: var(--status-mod, #bf8700);
  }
  .status.deleted {
    color: var(--status-del, #cf222e);
  }
  .status.renamed {
    color: var(--status-mod, #bf8700);
  }
  .fname {
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 150px;
  }
  .fdir {
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .empty {
    margin: 12px 0 0 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
