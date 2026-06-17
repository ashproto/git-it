<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";

  let message = $state("");

  // ── Amend toggle (Fork-style) ───────────────────────────────────────────────
  // When ON, the message box is prefilled with HEAD's full message for editing and
  // the Commit button becomes "Amend" (folds the staged changes into HEAD via
  // `git commit --amend`). Turning it OFF restores the user's previous draft.
  let amend = $state(false);
  let draft = $state(""); // the user's commit draft, saved while amending

  // HEAD commit (the one to amend). null in an empty repo → amend is disabled.
  const headCommit = $derived(
    appState.graphCommits.find((c) => c.refs.some((r) => r.is_head)) ?? null,
  );

  // Prefill the message from a squash-merge suggestion exactly once (only when
  // the user hasn't typed anything yet), then clear the suggestion so it won't
  // re-seed on a subsequent open of the working-copy view.
  $effect(() => {
    const s = appState.suggestedCommitMessage;
    if (s && message.trim() === "") {
      message = s;
      appState.clearSuggestedCommitMessage();
    }
  });

  // Count staged files reactively.
  const stagedCount = $derived(appState.workingChanges.filter((f) => f.staged).length);
  // Amend can edit just the message (no staged files required); a normal commit
  // needs both a message and at least one staged file.
  const canCommit = $derived(
    amend ? message.trim().length > 0 : message.trim().length > 0 && stagedCount > 0,
  );

  // ── Push-immediately toggle (SourceTree-style) ──────────────────────────────
  // When ON (and a remote exists), the branch is pushed right after a successful
  // commit/amend. Disabled with no remote. The setting is persisted in the store.
  const hasRemotes = $derived(appState.remotes.length > 0);
  const pushAndEnabled = $derived(appState.pushAfterCommit && hasRemotes);

  // Monotonic token so a slow message-load can't clobber a later toggle (e.g. the
  // user toggles ON then OFF — or ON twice — before HEAD's message resolves).
  let amendToken = 0;
  // Toggle handler (onchange, not $effect, to avoid reactive loops). ON: stash the
  // current draft and load HEAD's message for editing. OFF: restore the draft.
  async function onAmendToggle(e: Event) {
    const on = (e.currentTarget as HTMLInputElement).checked;
    const token = ++amendToken;
    amend = on;
    if (on) {
      draft = message;
      const head = headCommit;
      try {
        const loaded = head ? await gitActions.getCommitMessage(head.sha) : "";
        if (token === amendToken && amend) message = loaded;
      } catch {
        // Couldn't read HEAD's message — back out of amend cleanly.
        if (token === amendToken) {
          amend = false;
          message = draft;
          draft = "";
        }
      }
    } else {
      message = draft;
      draft = "";
    }
  }

  async function doCommit() {
    if (!canCommit) return;
    const ok = amend
      ? await gitActions.amendCommit(message.trim())
      : await gitActions.commitChanges(message.trim());
    // Only clear the composer on success — a failed commit keeps the typed message.
    if (!ok) return;
    message = "";
    amend = false;
    draft = "";
    // "Push immediately": push the current branch after a successful commit/amend.
    // A normal push (no force); if it's an amend of an already-pushed commit the
    // push is rejected and reported, leaving the user to force-push deliberately.
    if (appState.pushAfterCommit && hasRemotes) {
      await gitActions.push();
    }
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
    <label class="signoff-label" class:disabled={!headCommit}>
      <input
        type="checkbox"
        checked={amend}
        disabled={!headCommit}
        onchange={onAmendToggle}
      />
      Amend last commit
    </label>
    <label
      class="signoff-label"
      class:disabled={!hasRemotes}
      title={hasRemotes
        ? "Push to the remote right after committing"
        : "No remote configured for this repository"}
    >
      <input
        type="checkbox"
        checked={appState.pushAfterCommit}
        disabled={!hasRemotes}
        onchange={() => appState.setPushAfterCommit(!appState.pushAfterCommit)}
      />
      Push immediately
    </label>
    <button
      class="commit-btn"
      disabled={!canCommit}
      onclick={doCommit}
      title={!canCommit
        ? amend
          ? "Enter a commit message"
          : stagedCount === 0
            ? "Stage at least one file first"
            : "Enter a commit message"
        : amend
          ? pushAndEnabled
            ? "Amend the last commit and push (⌘↵)"
            : "Amend the last commit (⌘↵)"
          : pushAndEnabled
            ? "Commit staged changes and push (⌘↵)"
            : "Commit staged changes (⌘↵)"}
    >
      {#if amend}{pushAndEnabled ? "Amend & Push" : "Amend"}{:else}{pushAndEnabled
          ? "Commit & Push"
          : "Commit"} {stagedCount > 0 ? `(${stagedCount})` : ""}{/if}
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
    /* Never shrink: the master-detail above (flex:1) yields space first, so the
       composer keeps its full height and is never clipped on short windows. */
    flex-shrink: 0;
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
    /* Cap manual drag so the textarea can't be dragged taller than the (now
       height-capped) view and clip the Commit button beneath it. */
    max-height: 300px;
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
  .signoff-label.disabled {
    opacity: 0.45;
    cursor: not-allowed;
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
