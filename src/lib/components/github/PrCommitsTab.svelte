<script lang="ts">
  import { appState } from "../../store.svelte";
  import { parseISO, formatCommitDate } from "../../dates";
  import type { GhCommit } from "../../types";

  let { commits }: { commits: GhCommit[] } = $props();

  // Newest-first regardless of fetch order (gh returns them oldest-first).
  const sorted = $derived(
    [...commits].sort((a, b) => (b.committedDate || "").localeCompare(a.committedDate || "")),
  );

  function rel(iso: string): string {
    if (!iso) return "";
    const dt = parseISO(iso);
    return dt ? formatCommitDate(dt, appState.dateFormat, appState.relativeDates) : iso;
  }

  function subject(message: string): string {
    return (message ?? "").split("\n")[0];
  }

  // Copy feedback: which oid just got copied (transient ✓ swap on the SHA chip).
  let copiedOid = $state<string | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  async function copySha(oid: string) {
    try {
      await navigator.clipboard.writeText(oid);
      copiedOid = oid;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedOid = null), 1200);
    } catch {
      /* clipboard unavailable — leave the chip as-is */
    }
  }
</script>

{#if sorted.length === 0}
  <p class="note">No commits.</p>
{:else}
  <ul class="commit-list">
    {#each sorted as c (c.oid)}
      <li class="row">
        <button
          type="button"
          class="sha selectable"
          class:copied={copiedOid === c.oid}
          title="Copy full SHA"
          onclick={() => copySha(c.oid)}
        >{copiedOid === c.oid ? "✓ copied" : c.oid.slice(0, 7)}</button>
        <span class="subject" title={subject(c.message)}>{subject(c.message)}</span>
        <span class="meta"><strong>{c.author}</strong> · {rel(c.committedDate)}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .note {
    color: var(--text-muted);
    font-size: 13px;
    padding: 12px 0;
  }
  .commit-list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 7px 4px;
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }
  .row:last-child {
    border-bottom: none;
  }
  .sha {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent);
    background: none;
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 1px 6px;
    cursor: pointer;
    min-width: 66px;
    text-align: center;
  }
  .sha:hover {
    border-color: var(--accent);
  }
  .sha.copied {
    color: var(--status-add, #2ea043);
    border-color: var(--status-add, #2ea043);
  }
  .subject {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    color: var(--text);
  }
  .meta {
    flex: 0 0 auto;
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .meta strong {
    font-weight: 500;
    color: var(--text-muted);
  }
</style>
