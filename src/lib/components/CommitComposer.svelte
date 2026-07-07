<script lang="ts">
  import { appState } from "../store.svelte";
  import { gitActions } from "../gitActions";

  // Commit message is composed of a single-line summary (subject) and an optional
  // multi-line description (body), joined as `subject\n\nbody` on commit — the git
  // convention. Splitting them into two fields mirrors Fork / SourceTree / GitHub.
  let title = $state("");
  let body = $state("");

  // ── Amend toggle (Fork-style) ───────────────────────────────────────────────
  // When ON, the fields are prefilled from HEAD's message (split into summary +
  // description) for editing, and the Commit button becomes "Amend" (folds the
  // staged changes into HEAD via `git commit --amend`). Turning it OFF restores
  // the user's previous draft.
  let amend = $state(false);
  let draftTitle = $state(""); // the user's draft, saved while amending
  let draftBody = $state("");

  // HEAD commit (the one to amend). null in an empty repo → amend is disabled.
  const headCommit = $derived(
    appState.graphCommits.find((c) => c.refs.some((r) => r.is_head)) ?? null,
  );

  // Split a full commit message into summary (first line) + description (the rest,
  // with the conventional blank separator line trimmed). Joining is the inverse.
  function splitMessage(full: string): { title: string; body: string } {
    const nl = full.indexOf("\n");
    if (nl === -1) return { title: full, body: "" };
    return { title: full.slice(0, nl), body: full.slice(nl + 1).replace(/^\n+/, "") };
  }
  // Strip leading blank lines + trailing whitespace, but NOT leading spaces on the
  // first content line — so an indented body (e.g. a pasted code block) round-trips
  // through amend without losing its indentation. `\s`-only bodies normalize to "".
  function normalizeBody(s: string): string {
    return s.replace(/^\n+/, "").replace(/\s+$/, "");
  }
  function combinedMessage(): string {
    const t = title.trim();
    const b = normalizeBody(body);
    return b ? `${t}\n\n${b}` : t;
  }

  // Prefill from a squash-merge suggestion exactly once (only when both fields are
  // empty), then clear the suggestion so it won't re-seed on a later view open.
  $effect(() => {
    const s = appState.suggestedCommitMessage;
    if (s && title.trim() === "" && body.trim() === "") {
      const parts = splitMessage(s);
      title = parts.title;
      body = parts.body;
      appState.clearSuggestedCommitMessage();
    }
  });

  // Count staged files reactively.
  const stagedCount = $derived(appState.workingChanges.filter((f) => f.staged).length);
  // Amend can edit just the message (no staged files required); a normal commit
  // needs both a summary and at least one staged file.
  const canCommit = $derived(
    amend ? title.trim().length > 0 : title.trim().length > 0 && stagedCount > 0,
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
      draftTitle = title;
      draftBody = body;
      const head = headCommit;
      try {
        const loaded = head ? await gitActions.getCommitMessage(head.sha) : "";
        if (token === amendToken && amend) {
          const parts = splitMessage(loaded);
          title = parts.title;
          body = parts.body;
        }
      } catch {
        // Couldn't read HEAD's message — back out of amend cleanly.
        if (token === amendToken) {
          amend = false;
          title = draftTitle;
          body = draftBody;
          draftTitle = "";
          draftBody = "";
        }
      }
    } else {
      title = draftTitle;
      body = draftBody;
      draftTitle = "";
      draftBody = "";
    }
  }

  async function doCommit() {
    if (!canCommit) return;
    const msg = combinedMessage();
    const ok = amend
      ? await gitActions.amendCommit(msg)
      : await gitActions.commitChanges(msg);
    // Only clear the composer on success — a failed commit keeps the typed message.
    if (!ok) return;
    title = "";
    body = "";
    amend = false;
    draftTitle = "";
    draftBody = "";
    // "Push immediately": push the current branch after a successful commit/amend.
    // A normal push (no force); if it's an amend of an already-pushed commit the
    // push is rejected and reported, leaving the user to force-push deliberately.
    if (appState.pushAfterCommit && hasRemotes) {
      await gitActions.push();
    }
  }

  // ⌘Enter / Ctrl+Enter commits from either field.
  function onFieldKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      doCommit();
    }
  }
</script>

<div class="composer">
  <div class="staged-badge">
    {stagedCount} staged {stagedCount === 1 ? "file" : "files"}
  </div>
  <input
    class="title-box"
    type="text"
    placeholder="Summary (required)"
    bind:value={title}
    onkeydown={onFieldKeydown}
  />
  <textarea
    class="msg-box"
    placeholder="Description (optional)"
    bind:value={body}
    rows={3}
    onkeydown={onFieldKeydown}
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
          ? "Enter a summary"
          : stagedCount === 0
            ? "Stage at least one file first"
            : "Enter a summary"
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

  .title-box {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 13px;
    font-weight: 600;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
  }
  .title-box:focus {
    border-color: var(--accent);
  }
  .title-box::placeholder {
    color: var(--text-muted);
    font-weight: 400;
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
    border-radius: var(--radius-md);
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
    border-radius: var(--radius-md);
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
