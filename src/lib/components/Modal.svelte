<script lang="ts">
  import { dialogs } from "../dialogs.svelte";

  let inputValue = $state("");
  let inputEl = $state<HTMLInputElement | undefined>();

  // When a prompt opens, seed + focus the field.
  $effect(() => {
    const s = dialogs.state;
    if (s.kind === "prompt") {
      inputValue = s.value;
      inputEl?.focus();
      inputEl?.select();
    }
  });

  function okPrompt() {
    const v = inputValue.trim();
    dialogs.resolvePrompt(v.length ? v : null);
  }
  function cancelPrompt() {
    dialogs.resolvePrompt(null);
  }
</script>

{#if dialogs.state.kind === "prompt"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) cancelPrompt();
    }}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={dialogs.state.title}>
      <h3>{dialogs.state.title}</h3>
      <label class="lbl" for="modal-input">{dialogs.state.label}</label>
      <input
        id="modal-input"
        bind:this={inputEl}
        bind:value={inputValue}
        placeholder={dialogs.state.placeholder}
        onkeydown={(e) => {
          if (e.key === "Enter") okPrompt();
          else if (e.key === "Escape") cancelPrompt();
        }}
      />
      <div class="actions">
        <button type="button" onclick={cancelPrompt}>Cancel</button>
        <button type="button" class="primary" onclick={okPrompt}>{dialogs.state.confirmLabel}</button>
      </div>
    </div>
  </div>
{:else if dialogs.state.kind === "confirm"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) dialogs.resolveConfirm(false);
    }}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={dialogs.state.title}>
      <h3>{dialogs.state.title}</h3>
      <p class="msg">{dialogs.state.message}</p>
      <div class="actions">
        <button type="button" onclick={() => dialogs.resolveConfirm(false)}>Cancel</button>
        <button
          type="button"
          class="primary"
          class:danger={dialogs.state.danger}
          onclick={() => dialogs.resolveConfirm(true)}
        >
          {dialogs.state.confirmLabel}
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
    width: 360px;
    max-width: calc(100vw - 32px);
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }
  h3 {
    margin: 0 0 10px;
    font-size: 15px;
    font-weight: 600;
  }
  .lbl {
    display: block;
    font-size: 12px;
    color: var(--text-muted);
    margin-bottom: 4px;
  }
  .msg {
    margin: 0 0 14px;
    font-size: 13px;
    color: var(--text);
    line-height: 1.5;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 7px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
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
</style>
