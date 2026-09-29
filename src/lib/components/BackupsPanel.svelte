<script lang="ts">
  import { untrack } from "svelte";
  import { appState } from "../store.svelte";
  import { reportError } from "../errors.svelte";
  import LoadError from "./LoadError.svelte";
  import { api } from "../api";
  import type { BundleInfo, SafetyRef } from "../types";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  let { bare = false } = $props();

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  let tab = $state<"bundles" | "safety">("bundles");
  let bundles = $state<BundleInfo[]>([]);
  let safetyRefs = $state<SafetyRef[]>([]);
  // Set when a list's last load failed, so the panel says so instead of "No bundles yet.".
  // Wrapped so that a null/undefined rejection is still truthy.
  let bundlesError = $state<{ error: unknown } | null>(null);
  let safetyError = $state<{ error: unknown } | null>(null);
  // True once a list has loaded for this repository. A refresh keeps the current list on
  // screen; only the repo-switch effect below resets these.
  let bundlesLoaded = $state(false);
  let safetyLoaded = $state(false);
  // Bumped per request; a response from a superseded request is dropped.
  let bundlesSequence = 0;
  let safetySequence = 0;
  let selectedBundle = $state<string | null>(null);
  let selectedRefs = $state<Set<string>>(new Set());

  // Reset selections on repo switch — `selectedBundle` is a bare path and `deleteRefs` takes
  // names, so a selection carried over would delete the PREVIOUS repo's bundle or refs —
  // and refresh the active safety list too.
  $effect(() => {
    const repo = appState.repo;
    selectedBundle = null;
    selectedRefs = new Set();
    bundles = [];
    safetyRefs = [];
    bundlesLoaded = false;
    safetyLoaded = false;
    bundlesError = null;
    safetyError = null;
    if (isTauri() && repo) {
      void refreshBundles();
      if (untrack(() => tab) === "safety") void refreshSafetyRefs();
    }
  });

  async function refreshBundles() {
    if (!isTauri() || !appState.repo) return;
    const repo = appState.repo;
    const sequence = ++bundlesSequence;
    try {
      const result = await api.listBundles(repo);
      if (sequence !== bundlesSequence || repo !== appState.repo) return;
      bundles = result;
      bundlesLoaded = true;
      bundlesError = null;
    } catch (e) {
      if (sequence !== bundlesSequence || repo !== appState.repo) return;
      bundlesError = { error: e };
    }
  }
  async function refreshSafetyRefs() {
    if (!isTauri() || !appState.repo) return;
    const repo = appState.repo;
    const sequence = ++safetySequence;
    try {
      const result = await api.listSafetyRefs(repo);
      if (sequence !== safetySequence || repo !== appState.repo) return;
      safetyRefs = result;
      safetyLoaded = true;
      safetyError = null;
    } catch (e) {
      if (sequence !== safetySequence || repo !== appState.repo) return;
      safetyError = { error: e };
    }
  }

  async function createBundleNow() {
    try {
      const p = await api.createBundle(appState.repo);
      appState.appendLog(`[bundle] saved ${p}`);
      await refreshBundles();
    } catch (e) {
      reportError("Couldn't create the bundle", e);
    }
  }

  async function deleteBundleNow() {
    if (!selectedBundle) return;
    if (!confirm(`Delete ${selectedBundle}?`)) return;
    try {
      await api.deleteBundle(selectedBundle);
      await refreshBundles();
      selectedBundle = null;
    } catch (e) {
      reportError("Couldn't delete the bundle", e);
    }
  }

  async function fetchBundleNow() {
    if (!selectedBundle) return;
    try {
      const out = await api.fetchBundle(appState.repo, selectedBundle);
      appState.appendLog(`[fetch-bundle] ${out}`);
    } catch (e) {
      reportError("Couldn't fetch from the bundle", e);
    }
  }

  function toggleRef(name: string) {
    const next = new Set(selectedRefs);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    selectedRefs = next;
  }

  async function deleteSelectedRefs() {
    if (selectedRefs.size === 0) return;
    const refs = Array.from(selectedRefs);
    if (!confirm(`Delete ${refs.length} ref(s)?`)) return;
    try {
      await api.deleteRefs(appState.repo, refs);
      selectedRefs = new Set();
      await refreshSafetyRefs();
    } catch (e) {
      reportError("Couldn't delete the safety refs", e);
    }
  }

  async function deleteAllRefs() {
    if (safetyRefs.length === 0) return;
    if (!confirm(`Delete ALL ${safetyRefs.length} safety ref(s)?`)) return;
    try {
      await api.deleteRefs(
        appState.repo,
        safetyRefs.map((r) => r.name),
      );
      await refreshSafetyRefs();
    } catch (e) {
      reportError("Couldn't delete all safety refs", e);
    }
  }

  function humanSize(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<CollapsiblePanel title="Backups" collapsed {bare}>
  {#if !isTauri()}
    <p class="note">Desktop app only — backups are not available in the browser preview.</p>
  {:else if !appState.repo}
    <p class="note">Open a repository to see backups.</p>
  {:else}
    <div class="mode-tabs">
      <button
        type="button"
        class:active={tab === "bundles"}
        onclick={() => {
          tab = "bundles";
          refreshBundles();
        }}>Bundles</button
      >
      <button
        type="button"
        class:active={tab === "safety"}
        onclick={() => {
          tab = "safety";
          refreshSafetyRefs();
        }}>Safety refs</button
      >
    </div>
    {#if tab === "bundles"}
    <div class="row">
      <button type="button" class="primary" onclick={createBundleNow}>Create bundle now</button>
      <button type="button" onclick={refreshBundles}>Refresh</button>
      <span class="spacer"></span>
      <button type="button" onclick={fetchBundleNow} disabled={!selectedBundle || bundlesError !== null}>Fetch refs</button>
      <button type="button" onclick={deleteBundleNow} disabled={!selectedBundle || bundlesError !== null}>Delete</button>
    </div>
    {#if bundlesError}
      <LoadError what="the bundles" error={bundlesError.error} onRetry={refreshBundles} />
    {:else if !bundlesLoaded}
      <p class="note">Loading bundles…</p>
    {:else}
    <ul class="list">
      {#each bundles as b}
        <li class:selected={selectedBundle === b.path}>
          <button type="button" class="row-btn" onclick={() => (selectedBundle = b.path)}>
            <span class="mono">{b.name}</span>
            <span class="size">{humanSize(b.size)}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No bundles yet. Click "Create bundle now".</li>
      {/each}
    </ul>
    {/if}
  {:else}
    <div class="row">
      <button type="button" onclick={refreshSafetyRefs}>Refresh</button>
      <span class="spacer"></span>
      <button
        type="button"
        onclick={deleteSelectedRefs}
        disabled={selectedRefs.size === 0 || safetyError !== null}>Delete selected</button
      >
      <button type="button" onclick={deleteAllRefs} disabled={safetyRefs.length === 0 || safetyError !== null}
        >Delete all</button
      >
    </div>
    {#if safetyError}
      <LoadError what="the safety refs" error={safetyError.error} onRetry={refreshSafetyRefs} />
    {:else if !safetyLoaded}
      <p class="note">Loading safety refs…</p>
    {:else}
    <ul class="list">
      {#each safetyRefs as r}
        <li class:selected={selectedRefs.has(r.name)}>
          <button type="button" class="row-btn" onclick={() => toggleRef(r.name)}>
            <span class="kind">{r.kind}</span>
            <span class="mono">{r.name}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No safety refs from past rewrites.</li>
      {/each}
    </ul>
    {/if}
    {/if}
  {/if}
</CollapsiblePanel>

<style>
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
  .mode-tabs {
    display: flex;
    gap: 2px;
    background: var(--input-bg);
    border-radius: var(--radius-md);
    padding: 2px;
    width: fit-content;
    margin-bottom: 8px;
  }
  .mode-tabs button {
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .mode-tabs button.active {
    background: var(--btn-bg);
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }
  .row:first-of-type {
    margin-top: 0;
  }
  .spacer {
    flex: 1;
  }
  button {
    padding: 5px 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .list {
    list-style: none;
    margin: 8px 0 0 0;
    padding: 0;
    max-height: 200px;
    overflow: auto;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
  }
  .list li {
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12px;
  }
  .list li:last-child {
    border-bottom: none;
  }
  .list li.selected {
    background: var(--row-selected);
  }
  .list li.empty {
    color: var(--text-muted);
    font-style: italic;
    padding: 6px 10px;
  }
  .row-btn {
    width: 100%;
    display: flex;
    gap: 10px;
    padding: 6px 10px;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: inherit;
    font-family: inherit;
    text-align: left;
  }
  .row-btn:hover {
    background: var(--row-hover);
  }
  .size {
    margin-left: auto;
    color: var(--text-muted);
  }
  .kind {
    color: var(--text-muted);
    min-width: 70px;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
