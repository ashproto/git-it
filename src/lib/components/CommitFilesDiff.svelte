<script lang="ts">
  import { splitDiffFiles } from "../diff/split";
  import DiffView from "./DiffView.svelte";

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

{#if files.length === 0}
  <p class="empty">No file changes.</p>
{:else}
  <div class="master-detail">
    <ul class="filelist">
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
  .master-detail {
    display: flex;
    height: clamp(320px, 58vh, 820px);
    margin-top: 8px;
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
  .status {
    flex-shrink: 0;
    width: 12px;
    text-align: center;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    font-weight: 700;
  }
  .status.added {
    color: #3fb950;
  }
  .status.modified {
    color: var(--accent);
  }
  .status.deleted {
    color: var(--danger);
  }
  .status.renamed {
    color: #a371f7;
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
