<script lang="ts">
  import { appState } from "../store.svelte";
  import { contextMenu, type MenuItem } from "../contextMenu.svelte";
  import { dialogs } from "../dialogs.svelte";
  import { gitActions } from "../gitActions";
  import type { RefEntry } from "../types";
  import BackupsPanel from "./BackupsPanel.svelte";

  const refs = $derived(appState.refsByKind);

  let openLocal = $state(true);
  let openRemote = $state(true);
  let openTags = $state(true);

  // Display-only navigation: focus + select the ref's commit and scroll to it.
  // (Checkout etc. arrive in the operations phases.)
  function jumpTo(sha: string) {
    appState.setCurrent(sha);
    appState.selected = new Set([sha]);
    const el = document.getElementById(`gc-row-${sha}`);
    el?.scrollIntoView({ block: "center", behavior: "smooth" });
  }

  async function confirmDeleteBranch(name: string) {
    const ok = await dialogs.confirm({
      title: "Delete branch",
      message: `Delete branch "${name}"? Branches with unmerged commits won't delete unless forced.`,
      confirmLabel: "Delete",
      danger: true,
    });
    if (!ok) return;
    const deleted = await gitActions.deleteBranch(name, false);
    if (!deleted) {
      const force = await dialogs.confirm({
        title: "Force-delete branch?",
        message: `"${name}" was not deleted (likely unmerged). Force-delete and lose its unmerged commits?`,
        confirmLabel: "Force delete",
        danger: true,
      });
      if (force) gitActions.deleteBranch(name, true);
    }
  }

  function onRefContext(event: MouseEvent, r: RefEntry, kind: "local" | "remote" | "tag") {
    event.preventDefault();
    jumpTo(r.sha);
    const items: MenuItem[] = [];
    if (kind === "local") {
      items.push({ label: `Checkout ${r.name}`, action: () => gitActions.checkout(r.name) });
      items.push({
        label: "Rename…",
        action: async () => {
          const n = await dialogs.prompt({ title: "Rename branch", label: "New name", value: r.name });
          if (n && n !== r.name) gitActions.renameBranch(r.name, n);
        },
      });
      items.push({
        label: "Create tag here…",
        action: async () => {
          const n = await dialogs.prompt({ title: "New tag", label: "Tag name", placeholder: "v1.0.0" });
          if (n) gitActions.createTag(n, r.sha);
        },
      });
      items.push({ separator: true });
      items.push({ label: "Delete branch", danger: true, action: () => confirmDeleteBranch(r.name) });
    } else if (kind === "remote") {
      items.push({ label: `Checkout ${r.name} (detached)`, action: () => gitActions.checkout(r.name) });
      items.push({
        label: "Create local branch…",
        action: async () => {
          const n = await dialogs.prompt({ title: `New branch from ${r.name}`, label: "Branch name" });
          if (n) gitActions.createBranch(n, r.sha);
        },
      });
    } else {
      items.push({ label: `Checkout ${r.name} (detached)`, action: () => gitActions.checkout(r.name) });
      items.push({ separator: true });
      items.push({
        label: "Delete tag",
        danger: true,
        action: async () => {
          const ok = await dialogs.confirm({
            title: "Delete tag",
            message: `Delete tag "${r.name}"?`,
            confirmLabel: "Delete",
            danger: true,
          });
          if (ok) gitActions.deleteTag(r.name);
        },
      });
    }

    // Merge this ref into the current branch (contract e: ff/no-ff are distinct
    // items, never combined). Skip when this IS the checked-out branch. Squash is
    // deferred to Phase 5: a conflicting --squash writes no MERGE_HEAD (so the
    // conflict panel/abort can't recover it) and a clean one only stages changes,
    // needing a commit UI to finish — neither exists yet.
    if (!(kind === "local" && r.isHead)) {
      items.push({ separator: true });
      items.push({ label: `Merge ${r.name} into current`, action: () => gitActions.merge(r.name) });
      items.push({ label: `Merge ${r.name} (no-ff)`, action: () => gitActions.merge(r.name, { noFf: true }) });
    }
    contextMenu.openAt(event.clientX, event.clientY, items);
  }
</script>

<aside class="sidebar">
  {#if refs.head.length}
    <button class="ref detached" onclick={() => jumpTo(refs.head[0].sha)} title="Detached HEAD">
      <span class="dot head" aria-hidden="true"></span>
      <span class="rn">HEAD (detached)</span>
    </button>
  {/if}
  <section>
    <button class="sec" onclick={() => (openLocal = !openLocal)} aria-expanded={openLocal}>
      <span class="chev" class:open={openLocal} aria-hidden="true">▸</span>
      <span class="label">Local</span>
      <span class="n">{refs.local.length}</span>
    </button>
    {#if openLocal}
      {#each refs.local as r (`${r.name}@${r.sha}`)}
        <button
          class="ref"
          class:head={r.isHead}
          onclick={() => jumpTo(r.sha)}
          oncontextmenu={(e) => onRefContext(e, r, "local")}
          title={r.name}
        >
          <span class="dot local" aria-hidden="true"></span>
          <span class="rn">{r.name}</span>
        </button>
      {/each}
      {#if refs.local.length === 0}<p class="none">No local branches</p>{/if}
    {/if}
  </section>

  <section>
    <button class="sec" onclick={() => (openRemote = !openRemote)} aria-expanded={openRemote}>
      <span class="chev" class:open={openRemote} aria-hidden="true">▸</span>
      <span class="label">Remotes</span>
      <span class="n">{refs.remote.length}</span>
    </button>
    {#if openRemote}
      {#each refs.remote as r (`${r.name}@${r.sha}`)}
        <button
          class="ref muted"
          onclick={() => jumpTo(r.sha)}
          oncontextmenu={(e) => onRefContext(e, r, "remote")}
          title={r.name}
        >
          <span class="dot remote" aria-hidden="true"></span>
          <span class="rn">{r.name}</span>
        </button>
      {/each}
      {#if refs.remote.length === 0}<p class="none">No remotes</p>{/if}
    {/if}
  </section>

  <section>
    <button class="sec" onclick={() => (openTags = !openTags)} aria-expanded={openTags}>
      <span class="chev" class:open={openTags} aria-hidden="true">▸</span>
      <span class="label">Tags</span>
      <span class="n">{refs.tags.length}</span>
    </button>
    {#if openTags}
      {#each refs.tags as r (`${r.name}@${r.sha}`)}
        <button
          class="ref"
          onclick={() => jumpTo(r.sha)}
          oncontextmenu={(e) => onRefContext(e, r, "tag")}
          title={r.name}
        >
          <span class="dot tag" aria-hidden="true"></span>
          <span class="rn">{r.name}</span>
        </button>
      {/each}
      {#if refs.tags.length === 0}<p class="none">No tags</p>{/if}
    {/if}
  </section>

  <div class="backups">
    <BackupsPanel />
  </div>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
  }
  section {
    display: flex;
    flex-direction: column;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 6px;
    margin-top: 4px;
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .sec:hover {
    color: var(--text);
  }
  .chev {
    display: inline-block;
    transition: transform 0.12s ease;
    font-size: 10px;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .label {
    flex: 1;
    text-align: left;
  }
  .n {
    color: var(--text-muted);
    font-weight: 500;
  }
  .ref {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px 5px 18px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
  }
  .ref:hover {
    background: var(--row-hover);
  }
  .ref.head .rn {
    color: var(--accent);
    font-weight: 600;
  }
  .ref.muted .rn {
    color: var(--text-muted);
  }
  .rn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot.local {
    background: #378add;
  }
  .dot.remote {
    background: #888780;
  }
  .dot.tag {
    background: #ba7517;
  }
  .dot.head {
    background: var(--err);
  }
  .ref.detached .rn {
    color: var(--err);
    font-weight: 600;
  }
  .none {
    margin: 0 0 4px 18px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
  .backups {
    margin-top: 10px;
  }
</style>
