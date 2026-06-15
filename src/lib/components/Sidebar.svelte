<script lang="ts">
  import { appState } from "../store.svelte";
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
</script>

<aside class="sidebar">
  <section>
    <button class="sec" onclick={() => (openLocal = !openLocal)} aria-expanded={openLocal}>
      <span class="chev" class:open={openLocal} aria-hidden="true">▸</span>
      <span class="label">Local</span>
      <span class="n">{refs.local.length}</span>
    </button>
    {#if openLocal}
      {#each refs.local as r (r.name)}
        <button class="ref" class:head={r.isHead} onclick={() => jumpTo(r.sha)} title={r.name}>
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
      {#each refs.remote as r (r.name)}
        <button class="ref muted" onclick={() => jumpTo(r.sha)} title={r.name}>
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
      {#each refs.tags as r (r.name)}
        <button class="ref" onclick={() => jumpTo(r.sha)} title={r.name}>
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
