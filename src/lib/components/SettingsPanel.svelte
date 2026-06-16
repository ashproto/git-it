<script lang="ts">
  import { appState } from "../store.svelte";
  import { settingsPanel } from "../settingsPanel.svelte";

  let closeBtn = $state<HTMLButtonElement | undefined>();

  // Deferred focus on the close button when the panel opens (mirrors AmendDialog).
  $effect(() => {
    if (settingsPanel.open) {
      Promise.resolve().then(() => closeBtn?.focus());
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      settingsPanel.close();
    }
  }
</script>

{#if settingsPanel.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) settingsPanel.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Settings">
      <div class="header">
        <h3>Settings</h3>
        <button
          type="button"
          class="close-btn"
          aria-label="Close settings"
          bind:this={closeBtn}
          onclick={() => settingsPanel.close()}
        >✕</button>
      </div>

      <!-- ── Appearance ─────────────────────────────────────── -->
      <p class="group-label">Appearance</p>

      <div class="seg-row">
        <span class="seg-label">Graph lines</span>
        <div class="seg" role="group" aria-label="Graph lines style">
          <button
            type="button"
            class:active={appState.graphLineStyle === "curved"}
            onclick={() => appState.setGraphLineStyle("curved")}
            aria-pressed={appState.graphLineStyle === "curved"}
          >Curved</button><button
            type="button"
            class:active={appState.graphLineStyle === "angular"}
            onclick={() => appState.setGraphLineStyle("angular")}
            aria-pressed={appState.graphLineStyle === "angular"}
          >Angular</button>
        </div>
      </div>

      <div class="seg-row">
        <span class="seg-label">Repository switcher</span>
        <div class="seg" role="group" aria-label="Repository switcher mode">
          <button
            type="button"
            class:active={appState.repoSwitcherMode === "tabs"}
            onclick={() => appState.setRepoSwitcherMode("tabs")}
            aria-pressed={appState.repoSwitcherMode === "tabs"}
          >Tabs</button><button
            type="button"
            class:active={appState.repoSwitcherMode === "sidebar"}
            onclick={() => appState.setRepoSwitcherMode("sidebar")}
            aria-pressed={appState.repoSwitcherMode === "sidebar"}
          >Sidebar</button>
        </div>
      </div>

      <div class="seg-row">
        <span class="seg-label">Diff view</span>
        <div class="seg" role="group" aria-label="Diff view mode">
          <button
            type="button"
            class:active={!appState.diffSplit}
            onclick={() => appState.setDiffSplit(false)}
            aria-pressed={!appState.diffSplit}
          >Unified</button><button
            type="button"
            class:active={appState.diffSplit}
            onclick={() => appState.setDiffSplit(true)}
            aria-pressed={appState.diffSplit}
          >Split</button>
        </div>
      </div>

      <hr class="divider" />

      <!-- ── Commit dates ───────────────────────────────────── -->
      <p class="group-label">Commit dates</p>

      <label class="opt">
        <input
          type="checkbox"
          checked={appState.relativeDates}
          onchange={() => appState.setRelativeDates(!appState.relativeDates)}
        />
        <span>Show "Today" / "Yesterday" for recent commits</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.dateFormat.hour12}
          onchange={() => appState.setDateFormat({ hour12: !appState.dateFormat.hour12 })}
        />
        <span>12-hour time (AM/PM)</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.dateFormat.weekday}
          onchange={() => appState.setDateFormat({ weekday: !appState.dateFormat.weekday })}
        />
        <span>Show weekday</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.dateFormat.monthName}
          onchange={() => appState.setDateFormat({ monthName: !appState.dateFormat.monthName })}
        />
        <span>Show month name</span>
      </label>

      <hr class="divider" />

      <!-- ── Behavior ───────────────────────────────────────── -->
      <p class="group-label">Behavior</p>

      <label class="opt">
        <input
          type="checkbox"
          checked={appState.autoBackupDestructive}
          onchange={() => appState.setAutoBackupDestructive(!appState.autoBackupDestructive)}
        />
        <span>Create a backup before destructive operations</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.pullRebase}
          onchange={() => appState.setPullRebase(!appState.pullRebase)}
        />
        <span>Pull with rebase (instead of merge)</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.autoShowEditTools}
          onchange={() => appState.setAutoShowEditTools(!appState.autoShowEditTools)}
        />
        <span>Show edit tools below commit details</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.unifyUnstaged}
          onchange={() => appState.setUnifyUnstaged(!appState.unifyUnstaged)}
        />
        <span>Merge Untracked into Unstaged</span>
      </label>
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
    width: 440px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    line-height: 1;
  }

  .close-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }

  .close-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .group-label {
    margin: 6px 4px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  /* Segmented control rows */
  .seg-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 2px;
    gap: 12px;
  }

  .seg-label {
    font-size: 12.5px;
    color: var(--text);
  }

  .seg {
    display: flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    flex-shrink: 0;
  }

  .seg button {
    padding: 3px 9px;
    border: none;
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 11.5px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;
  }

  .seg button + button {
    border-left: 1px solid var(--border);
  }

  .seg button:hover {
    background: var(--btn-hover);
    color: var(--text);
  }

  .seg button.active {
    background: var(--accent);
    color: #fff;
  }

  .seg button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  /* Checkbox rows — mirrors DateFormatMenu .opt */
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 4px;
    font-size: 12.5px;
    color: var(--text);
    cursor: pointer;
    border-radius: 5px;
  }

  .opt:hover,
  .opt:focus-within {
    background: var(--row-hover);
  }

  .opt input {
    margin: 0;
    cursor: pointer;
  }

  .opt input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .divider {
    border: none;
    border-top: 1px solid var(--border);
    margin: 8px 0 4px;
  }
</style>
