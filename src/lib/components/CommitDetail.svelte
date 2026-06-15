<script lang="ts">
  import { appState } from "../store.svelte";
  import { parseISO, formatCommitDate } from "../dates";

  const c = $derived(appState.selectedCommit);

  function fmt(iso: string): string {
    const d = parseISO(iso);
    return d ? formatCommitDate(d, appState.dateFormat) : iso;
  }
  function initials(name: string): string {
    return name
      .split(/\s+/)
      .map((s) => s[0] ?? "")
      .slice(0, 2)
      .join("")
      .toUpperCase();
  }
</script>

<div class="detail panel">
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
            <span class="badge {r.kind}" class:current={r.is_head}>{r.name}</span>
          {/each}
        </span>
      {/if}
    </div>

    <p class="note">
      File changes and the line-by-line diff arrive with the working-copy phase.
    </p>
  {:else}
    <p class="empty">Select a commit to see its details.</p>
  {/if}
</div>

<style>
  .detail {
    padding: 12px 14px;
  }
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
    font-size: 11px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--text-muted);
  }
  .badge.current {
    border-color: var(--accent);
    color: var(--accent);
  }
  .badge.head {
    border-color: var(--text-muted);
    color: var(--text);
    font-weight: 600;
  }
  .badge.tag {
    border-color: var(--err);
    color: var(--err);
  }
  .badge.remote {
    opacity: 0.7;
  }
  .note {
    margin: 12px 0 0 0;
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
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
