<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";

  let message = $state("");
  let signoff = $state(false);

  // Count staged files reactively.
  const stagedCount = $derived(appState.workingChanges.filter((f) => f.staged).length);
  const canCommit = $derived(message.trim().length > 0 && stagedCount > 0);

  async function doCommit() {
    if (!canCommit) return;
    await gitActions.commitChanges(message.trim(), signoff);
    message = "";
  }
</script>

<div class="composer">
  <div class="staged-badge">
    {stagedCount} staged {stagedCount === 1 ? "file" : "files"}
  </div>
  <textarea
    class="msg-box"
    placeholder="Commit message…"
    bind:value={message}
    rows={3}
    onkeydown={(e) => {
      // ⌘Enter / Ctrl+Enter to commit
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        doCommit();
      }
    }}
  ></textarea>
  <div class="actions">
    <label class="signoff-label">
      <input type="checkbox" bind:checked={signoff} />
      Sign off
    </label>
    <button
      class="commit-btn"
      disabled={!canCommit}
      onclick={doCommit}
      title={!canCommit
        ? stagedCount === 0
          ? "Stage at least one file first"
          : "Enter a commit message"
        : "Commit staged changes (⌘↵)"}
    >
      Commit {stagedCount > 0 ? `(${stagedCount})` : ""}
    </button>
  </div>
</div>

<style>
  .composer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
    background: var(--panel-bg);
  }

  .staged-badge {
    font-size: 11px;
    color: var(--text-muted);
  }

  .msg-box {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 60px;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 12.5px;
    font-family: inherit;
    line-height: 1.5;
    outline: none;
    transition: border-color 0.15s;
  }
  .msg-box:focus {
    border-color: var(--accent);
  }
  .msg-box::placeholder {
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .signoff-label {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-muted);
    cursor: pointer;
    user-select: none;
    -webkit-user-select: none;
  }

  .commit-btn {
    margin-left: auto;
    padding: 5px 16px;
    border-radius: 6px;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.1s,
      opacity 0.1s;
  }
  .commit-btn:hover:not(:disabled) {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }
  .commit-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
