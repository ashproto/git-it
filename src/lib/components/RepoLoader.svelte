<script lang="ts">
  import { appState } from "../store.svelte";
  import { api, pickRepoFolder } from "../api";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  let count = $state(50);
  let loading = $state(false);

  async function browse() {
    const picked = await pickRepoFolder(appState.repo || undefined);
    if (picked) appState.repo = picked;
  }

  export async function load() {
    if (!appState.repo) {
      appState.status = "Pick a repository first.";
      return;
    }
    loading = true;
    appState.status = "Loading commits…";
    try {
      const valid = await api.isGitRepo(appState.repo);
      if (!valid) {
        appState.status = `${appState.repo} is not a git repo.`;
        return;
      }
      const gc = await api.loadGraph(appState.repo, count, 0);
      appState.setGraphCommits(gc);
      appState.status = `Loaded ${gc.length} commit(s) across all branches.`;
    } catch (e) {
      appState.status = `Load failed: ${e}`;
    } finally {
      loading = false;
    }
  }
</script>

<CollapsiblePanel title="Repository & Load">
  <div class="row">
    <label for="repo">Repo</label>
    <input
      id="repo"
      type="text"
      bind:value={appState.repo}
      placeholder="/path/to/repo"
    />
    <button type="button" onclick={browse}>Browse…</button>
  </div>
  <div class="row">
    <label for="count">Recent</label>
    <input
      id="count"
      type="number"
      min="1"
      max="5000"
      bind:value={count}
      style="width:6rem"
    />
    <span class="note">recent commits across all branches</span>
    <span class="grow"></span>
    <button type="button" onclick={load} disabled={loading} class="primary">
      {loading ? "Loading…" : "Reload"}
    </button>
  </div>
</CollapsiblePanel>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .row:first-child {
    margin-top: 0;
  }
  label {
    font-size: 13px;
    color: var(--text-muted);
    min-width: 50px;
  }
  .note {
    font-size: 12px;
    color: var(--text-muted);
  }
  .grow {
    flex: 1;
  }
  input[type="text"],
  input[type="number"] {
    flex: 1;
    padding: 6px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
  }
  input[type="text"]:disabled {
    opacity: 0.5;
  }
  button {
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  button.primary:hover {
    background: var(--accent-hover);
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
