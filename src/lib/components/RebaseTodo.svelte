<script lang="ts">
  import { rebaseEditor } from "../rebaseEditor.svelte";
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { gitActions } from "../gitActions";
  import type { ReflogEntry, RebaseStep } from "../types";

  function inTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  // Reactive rows: each entry is { entry: ReflogEntry, action, rewordText }.
  // We maintain this as a plain $state array so up/down reordering is a splice.
  type Row = {
    entry: ReflogEntry;
    action: string;
    rewordText: string;
  };

  let rows = $state<Row[]>([]);
  let loading = $state(false);
  let loadErr = $state<string | null>(null);

  // Fetch the preview whenever the dialog opens or the base changes.
  $effect(() => {
    if (!rebaseEditor.open) {
      rows = [];
      loadErr = null;
      return;
    }
    const base = rebaseEditor.base;
    if (!inTauri() || !appState.repo) {
      rows = [];
      return;
    }
    loading = true;
    loadErr = null;
    api
      .rebaseTodoPreview(appState.repo, base)
      .then((entries) => {
        // API returns oldest-first (base..HEAD log --reverse).
        rows = entries.map((e) => ({ entry: e, action: "pick", rewordText: e.subject }));
        loading = false;
      })
      .catch((e: unknown) => {
        loadErr = String(e).split("\n")[0];
        loading = false;
      });
  });

  function moveUp(i: number) {
    if (i === 0) return;
    const next = [...rows];
    [next[i - 1], next[i]] = [next[i], next[i - 1]];
    rows = next;
  }

  function moveDown(i: number) {
    if (i === rows.length - 1) return;
    const next = [...rows];
    [next[i], next[i + 1]] = [next[i + 1], next[i]];
    rows = next;
  }

  function setAction(i: number, action: string) {
    const next = [...rows];
    next[i] = { ...next[i], action };
    rows = next;
  }

  function setRewordText(i: number, text: string) {
    const next = [...rows];
    next[i] = { ...next[i], rewordText: text };
    rows = next;
  }

  function currentBranchName(): string {
    return appState.refsByKind.local.find((r) => r.isHead)?.name ?? "HEAD";
  }

  async function startRebase() {
    const nonDrop = rows.filter((r) => r.action !== "drop");
    if (nonDrop.length === 0) {
      appState.status = "Interactive rebase: at least one non-drop step is required.";
      return;
    }
    const steps: RebaseStep[] = rows.map((r) => ({
      action: r.action,
      sha: r.entry.sha,
      message: r.action === "reword" ? r.rewordText : null,
    }));
    const branch = currentBranchName();
    const consequence = `Rewrites ${steps.length} commit(s) on ${branch}; hashes change.`;
    await gitActions.rebaseInteractive(rebaseEditor.base, steps, consequence);
    rebaseEditor.close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      rebaseEditor.close();
    }
  }
</script>

{#if rebaseEditor.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) rebaseEditor.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Interactive rebase">
      <h3>Interactive rebase</h3>
      <p class="sub mono">from {rebaseEditor.base.slice(0, 9)}</p>

      {#if !inTauri()}
        <p class="note">Interactive rebase is only available in the desktop app.</p>
      {:else if loading}
        <p class="note">Loading commits…</p>
      {:else if loadErr}
        <p class="err">{loadErr}</p>
      {:else if rows.length === 0}
        <p class="note">No commits between the selected commit and HEAD.</p>
      {:else}
        <div class="list">
          {#each rows as row, i (row.entry.sha + i)}
            <div class="row">
              <div class="reorder">
                <button
                  type="button"
                  class="ord"
                  disabled={i === 0}
                  onclick={() => moveUp(i)}
                  title="Move up"
                >▲</button>
                <button
                  type="button"
                  class="ord"
                  disabled={i === rows.length - 1}
                  onclick={() => moveDown(i)}
                  title="Move down"
                >▼</button>
              </div>

              <select
                class="action-sel"
                value={row.action}
                onchange={(e) => setAction(i, (e.currentTarget as HTMLSelectElement).value)}
              >
                <option value="pick">pick</option>
                <option value="reword">reword</option>
                <option value="edit">edit</option>
                <option value="squash">squash</option>
                <option value="fixup">fixup</option>
                <option value="drop">drop</option>
              </select>

              <span class="sha mono">{row.entry.short}</span>

              {#if row.action === "reword"}
                <input
                  type="text"
                  class="reword-in"
                  value={row.rewordText}
                  oninput={(e) => setRewordText(i, (e.currentTarget as HTMLInputElement).value)}
                  placeholder="New commit message"
                />
              {:else}
                <span class="msg" class:dropped={row.action === "drop"}>{row.entry.subject}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

      <div class="actions">
        <button type="button" onclick={() => rebaseEditor.close()}>Cancel</button>
        <button
          type="button"
          class="primary danger"
          disabled={!inTauri() || loading || rows.length === 0}
          onclick={startRebase}
        >
          Start rebase
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 3000;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .dialog {
    width: 620px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 64px);
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow: hidden;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .note {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    font-style: italic;
  }
  .err {
    margin: 0;
    font-size: 13px;
    color: var(--danger, #dc2626);
  }
  .list {
    flex: 1 1 auto;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    /* Limit height so the footer always stays visible */
    max-height: calc(100vh - 240px);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-subtle, var(--border));
    background: var(--panel-bg);
    min-height: 34px;
  }
  .row:hover {
    background: var(--row-hover);
  }
  .reorder {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 0 0 auto;
  }
  .ord {
    padding: 0 4px;
    font-size: 9px;
    line-height: 1.2;
    border-radius: 3px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    cursor: pointer;
  }
  .ord:hover:not(:disabled) {
    background: var(--btn-hover);
    color: var(--text);
  }
  .ord:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
  .action-sel {
    flex: 0 0 74px;
    padding: 3px 4px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 12px;
    font-family: var(--font-mono);
    cursor: pointer;
  }
  .sha {
    flex: 0 0 58px;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .msg {
    flex: 1 1 auto;
    font-size: 12.5px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg.dropped {
    color: var(--text-muted);
    text-decoration: line-through;
  }
  .reword-in {
    flex: 1 1 auto;
    padding: 3px 7px;
    border-radius: 5px;
    border: 1px solid var(--accent);
    background: var(--input-bg);
    color: var(--text);
    font-size: 12.5px;
    font-family: inherit;
  }
  .reword-in:focus {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 2px;
    flex: 0 0 auto;
  }
  button {
    padding: 6px 14px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  button.primary.danger {
    background: var(--danger, #dc2626);
    border-color: var(--danger, #dc2626);
  }
  button.primary.danger:hover:not(:disabled) {
    background: var(--danger-hover, #b91c1c);
    border-color: var(--danger-hover, #b91c1c);
  }
</style>
