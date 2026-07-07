<script lang="ts">
  import { amendDialog } from "../amendDialog.svelte";
  import { gitActions } from "../gitActions";

  let message = $state("");
  let resetAuthorDate = $state(false);
  let resetCommitterDate = $state(false);
  let textareaEl = $state<HTMLTextAreaElement | undefined>();

  // When the dialog opens, seed message from the commit subject and focus.
  $effect(() => {
    if (amendDialog.open) {
      message = amendDialog.subject;
      resetAuthorDate = false;
      resetCommitterDate = false;
      // Defer focus to the next microtask so the DOM is painted.
      Promise.resolve().then(() => textareaEl?.focus());
    }
  });

  async function doAmend() {
    await gitActions.amend(message.trim() || null, resetAuthorDate, resetCommitterDate);
    amendDialog.close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      amendDialog.close();
    }
  }
</script>

{#if amendDialog.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) amendDialog.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Amend commit">
      <h3>Amend commit</h3>
      <p class="sub mono">{amendDialog.sha.slice(0, 9)}</p>

      <label class="lbl" for="amend-msg">Commit message</label>
      <textarea
        id="amend-msg"
        bind:this={textareaEl}
        bind:value={message}
        rows="4"
        placeholder="Commit message"
      ></textarea>

      <div class="checks">
        <label class="opt">
          <input
            type="checkbox"
            bind:checked={resetAuthorDate}
          />
          <span>Reset author date to now</span>
        </label>
        <label class="opt">
          <input
            type="checkbox"
            bind:checked={resetCommitterDate}
          />
          <span>Reset committer date to now</span>
        </label>
      </div>

      <div class="actions">
        <button type="button" onclick={() => amendDialog.close()}>Cancel</button>
        <button type="button" class="primary danger" onclick={doAmend}>Amend</button>
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
    width: 400px;
    max-width: calc(100vw - 32px);
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
    gap: 10px;
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
  .lbl {
    display: block;
    font-size: 12px;
    color: var(--text-muted);
    margin-bottom: -6px;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 7px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
    resize: vertical;
  }
  textarea:focus {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .checks {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 2px;
    font-size: 12.5px;
    color: var(--text);
    cursor: pointer;
    border-radius: 5px;
  }
  .opt:hover {
    background: var(--row-hover);
  }
  .opt input {
    margin: 0;
    cursor: pointer;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 2px;
  }
  button {
    padding: 6px 14px;
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
    color: #fff;
  }
  button.primary.danger {
    background: var(--danger);
    border-color: var(--danger);
  }
  button.primary.danger:hover {
    background: var(--danger-hover);
    border-color: var(--danger-hover);
  }
</style>
