<script lang="ts">
  import { dialogs } from "../dialogs.svelte";

  let inputValue = $state("");
  let inputEl = $state<HTMLInputElement | undefined>();
  let credUsernameEl = $state<HTMLInputElement | undefined>();

  // When a prompt opens, seed + focus the field.
  // When a credentials dialog opens, focus the username input.
  $effect(() => {
    const s = dialogs.state;
    if (s.kind === "prompt") {
      inputValue = s.value;
      inputEl?.focus();
      inputEl?.select();
    } else if (s.kind === "credentials") {
      credUsernameEl?.focus();
    }
  });

  function okPrompt() {
    const v = inputValue.trim();
    dialogs.resolvePrompt(v.length ? v : null);
  }
  function cancelPrompt() {
    dialogs.resolvePrompt(null);
  }

  function handleDestructiveKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      dialogs.resolveDestructive(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      dialogs.resolveDestructive(false);
    }
  }

  function handleBranchDeleteKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      dialogs.resolveBranchDelete(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      dialogs.resolveBranchDelete(false);
    }
  }

  function handleCreateRepoKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (dialogs.state.kind === "createRepo" && dialogs.state.name.trim()) {
        dialogs.resolveCreateRepo(true);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      dialogs.resolveCreateRepo(false);
    }
  }

  // Credentials dialog helpers
  function submitCredentials() {
    const s = dialogs.state;
    if (s.kind !== "credentials") return;
    dialogs.resolveCredentials({ username: s.username, password: s.password });
  }
  function cancelCredentials() {
    dialogs.resolveCredentials(null);
  }
  function handleCredentialsKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      submitCredentials();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelCredentials();
    }
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
{:else if dialogs.state.kind === "destructive"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) dialogs.resolveDestructive(false);
    }}
    onkeydown={handleDestructiveKey}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={dialogs.state.title}>
      <h3>{dialogs.state.title}</h3>
      <p class="msg consequence">{dialogs.state.consequence}</p>
      <label class="backup-row">
        <input
          type="checkbox"
          checked={dialogs.state.backup}
          onchange={(e) => dialogs.setDestructiveBackup((e.currentTarget as HTMLInputElement).checked)}
        />
        Create backup bundle
      </label>
      <div class="actions">
        <button type="button" onclick={() => dialogs.resolveDestructive(false)}>Cancel</button>
        <button
          type="button"
          class="primary danger"
          onclick={() => dialogs.resolveDestructive(true)}
        >
          {dialogs.state.confirmLabel}
        </button>
      </div>
    </div>
  </div>
{:else if dialogs.state.kind === "branchDelete"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) dialogs.resolveBranchDelete(false);
    }}
    onkeydown={handleBranchDeleteKey}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={dialogs.state.title}>
      <h3>{dialogs.state.title}</h3>
      <p class="msg">Delete branch "{dialogs.state.branch}"?</p>
      <label class="backup-row">
        <input
          type="checkbox"
          checked={dialogs.state.force}
          onchange={(e) => dialogs.setBranchDeleteForce((e.currentTarget as HTMLInputElement).checked)}
        />
        Force delete (discard unmerged commits)
      </label>
      {#if dialogs.state.upstream !== null}
        <label class="backup-row">
          <input
            type="checkbox"
            checked={dialogs.state.deleteRemote}
            onchange={(e) => dialogs.setBranchDeleteRemote((e.currentTarget as HTMLInputElement).checked)}
          />
          Also delete {dialogs.state.upstream}
        </label>
      {/if}
      <div class="actions">
        <button type="button" onclick={() => dialogs.resolveBranchDelete(false)}>Cancel</button>
        <button
          type="button"
          class="primary danger"
          onclick={() => dialogs.resolveBranchDelete(true)}
        >
          Delete
        </button>
      </div>
    </div>
  </div>
{:else if dialogs.state.kind === "createRepo"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) dialogs.resolveCreateRepo(false);
    }}
    onkeydown={handleCreateRepoKey}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={dialogs.state.title}>
      <h3>{dialogs.state.title}</h3>
      <p class="msg">Creates the repository on your GitHub account, adds it as origin, and pushes.</p>
      <label class="lbl" for="create-repo-name">Repository name</label>
      <input
        id="create-repo-name"
        value={dialogs.state.name}
        oninput={(e) => dialogs.setCreateRepoName((e.currentTarget as HTMLInputElement).value)}
      />
      <div class="visibility-row" role="radiogroup" aria-label="Visibility">
        <label class="radio-opt">
          <input
            type="radio"
            name="create-repo-visibility"
            checked={dialogs.state.isPrivate}
            onchange={() => dialogs.setCreateRepoPrivate(true)}
          />
          Private
        </label>
        <label class="radio-opt">
          <input
            type="radio"
            name="create-repo-visibility"
            checked={!dialogs.state.isPrivate}
            onchange={() => dialogs.setCreateRepoPrivate(false)}
          />
          Public
        </label>
      </div>
      <label class="lbl" for="create-repo-desc">Description</label>
      <input
        id="create-repo-desc"
        value={dialogs.state.description}
        placeholder="Description (optional)"
        oninput={(e) => dialogs.setCreateRepoDescription((e.currentTarget as HTMLInputElement).value)}
      />
      <div class="actions">
        <button type="button" onclick={() => dialogs.resolveCreateRepo(false)}>Cancel</button>
        <button
          type="button"
          class="primary"
          disabled={!dialogs.state.name.trim()}
          onclick={() => dialogs.resolveCreateRepo(true)}
        >
          Create &amp; push
        </button>
      </div>
    </div>
  </div>
{:else if dialogs.state.kind === "credentials"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) cancelCredentials();
    }}
    onkeydown={handleCredentialsKey}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={dialogs.state.title}>
      <h3>{dialogs.state.title}</h3>
      {#if dialogs.state.message}
        <p class="msg">{dialogs.state.message}</p>
      {/if}
      <label class="lbl" for="cred-username">Username</label>
      <input
        id="cred-username"
        bind:this={credUsernameEl}
        type="text"
        autocomplete="username"
        value={dialogs.state.username}
        oninput={(e) => dialogs.setCredField("username", (e.currentTarget as HTMLInputElement).value)}
      />
      <label class="lbl" for="cred-password">Password / Token</label>
      <input
        id="cred-password"
        type="password"
        autocomplete="current-password"
        value={dialogs.state.password}
        oninput={(e) => dialogs.setCredField("password", (e.currentTarget as HTMLInputElement).value)}
      />
      <p class="hint">Used once for this operation — never stored.</p>
      <div class="actions">
        <button type="button" onclick={cancelCredentials}>Cancel</button>
        <button type="button" class="primary" onclick={submitCredentials}>Sign in</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    /* This generic dialog layer (prompt / confirm / create-repo / credentials …) is
       opened FROM other modals — e.g. the Manage Repository modal (z 3000) launches
       the Add-remote / Create-on-GitHub dialogs — so it must sit ABOVE them, not tie
       and lose on DOM order. Keep this the highest modal z-index in the app. */
    z-index: 4000;
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
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .visibility-row {
    display: flex;
    gap: 16px;
    margin-bottom: 14px;
  }
  .radio-opt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    cursor: pointer;
    user-select: none;
  }
  .radio-opt input[type="radio"] {
    width: auto;
    margin: 0;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
  .consequence {
    color: var(--text-muted);
  }
  .backup-row {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    margin-bottom: 14px;
    cursor: pointer;
    user-select: none;
  }
  .backup-row input[type="checkbox"] {
    width: auto;
    margin: 0;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
  .hint {
    margin: 0 0 14px;
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.4;
  }
</style>
