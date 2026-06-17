<script lang="ts">
  import { appState } from "../store.svelte";
  import { api } from "../api";
  import { parseISO, formatCommitDate } from "../dates";
  import CommitFilesDiff from "./CommitFilesDiff.svelte";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import RefIcon from "./RefIcon.svelte";

  function isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  const c = $derived(appState.selectedCommit);

  function fmt(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat, appState.relativeDates) : iso;
  }
  function initials(name: string): string {
    return name
      .split(/\s+/)
      .map((s) => s[0] ?? "")
      .slice(0, 2)
      .join("")
      .toUpperCase();
  }

  // Commit diff state
  let diffPatch = $state<string>("");
  let diffError = $state<string | null>(null);
  let diffLoading = $state(false);

  // Re-fetch when selected commit changes
  $effect(() => {
    const sha = appState.currentSha;
    // Read so the effect re-runs when the context / whole-file setting changes.
    const context = appState.effectiveDiffContext;
    if (!sha || !isTauri() || !appState.repo) {
      diffPatch = "";
      diffError = null;
      diffLoading = false;
      return;
    }
    diffLoading = true;
    diffError = null;
    diffPatch = "";
    api.commitDiff(appState.repo, sha, null, context)
      .then((patch) => {
        // Guard: the user may have navigated away during the async gap.
        if (appState.currentSha !== sha) return;
        diffPatch = patch;
        diffLoading = false;
      })
      .catch((e) => {
        if (appState.currentSha !== sha) return;
        diffError = String(e).split("\n")[0];
        diffLoading = false;
      });
  });
</script>

<CollapsiblePanel title="Commit">
  {#if c}
    <div class="hdr">
      <div class="avatar" aria-hidden="true">{initials(c.author_name)}</div>
      <div class="ttl">
        <div class="subj">{c.subject}</div>
        <div class="sub">{c.author_name} committed {fmt(c.committer_date)}</div>
      </div>
      <span class="sha mono">{c.sha.slice(0, 10)}</span>
    </div>

    <div class="grid">
      <span class="k">Author</span>
      <span class="v">{c.author_name} &lt;{c.author_email}&gt;</span>
      <span class="k">Authored</span>
      <span class="v mono">{fmt(c.author_date)}</span>
      <span class="k">Committed</span>
      <span class="v mono">{fmt(c.committer_date)}</span>
      <span class="k">Parents</span>
      <span class="v mono">{c.parents.map((p) => p.slice(0, 9)).join(", ") || "(root commit)"}</span>
      {#if c.refs.length}
        <span class="k">Refs</span>
        <span class="v badges">
          {#each c.refs as r (r.name + r.kind)}
            <span
              class="badge {r.kind}"
              class:current={r.is_head}
              style={`--ref-color:${appState.colorForRef(r.name, c.sha)}`}
            ><RefIcon kind={r.kind} />{r.name}</span>
          {/each}
        </span>
      {/if}
    </div>

    <!-- Diff section -->
    <div class="diff-section">
      {#if !isTauri()}
        <p class="note">Commit diff is available in the desktop app only.</p>
      {:else if diffLoading}
        <p class="note">Loading diff…</p>
      {:else if diffError}
        <p class="note err">Could not load diff: {diffError}</p>
      {:else}
        <!-- The file-count ("N files changed") is shown by CommitFilesDiff's own
             toolbar (next to the tree-view toggle), so it isn't repeated here. -->
        <CommitFilesDiff patch={diffPatch} />
      {/if}
    </div>
  {:else}
    <p class="empty">Select a commit to see its details.</p>
  {/if}
</CollapsiblePanel>

<style>
  .hdr {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
  }
  .avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--row-selected);
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
  }
  .ttl {
    flex: 1;
    min-width: 0;
  }
  .subj {
    font-weight: 600;
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }
  .sha {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 14px;
    font-size: 12.5px;
  }
  .k {
    color: var(--text-muted);
  }
  .v {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .badge {
    /* Colour matches the ref's graph lane (or manual override) via --ref-color;
       the RefIcon glyph conveys local / remote / tag. */
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--ref-color, var(--border));
    color: var(--ref-color, var(--text-muted));
  }
  .badge.current {
    font-weight: 600;
    background: color-mix(in srgb, var(--ref-color, var(--accent)) 16%, transparent);
  }
  .note {
    margin: 12px 0 0 0;
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }
  .diff-section {
    margin-top: 14px;
    border-top: 1px solid var(--border-subtle);
  }
  .err {
    color: var(--err, #c0392b);
  }
  .empty {
    margin: 0;
    color: var(--text-muted);
    font-style: italic;
    font-size: 13px;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11.5px;
  }
</style>
