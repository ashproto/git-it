<script lang="ts">
  // Slim ⌘F search bar pinned above the commit list (pushes content, no overlay).
  // Pure presentation: the host owns the query/matches state; this component
  // debounces input, renders the "N of M" counter and next/prev/close controls.
  type Props = {
    open: boolean;
    count: number;
    // 0-based position of the active match within the match list.
    active: number;
    // Zero matches but older history is still loadable → offer to page more in.
    canLoadMore?: boolean;
    loadingMore?: boolean;
    onQuery: (q: string) => void;
    onNext: () => void;
    onPrev: () => void;
    onClose: () => void;
    onLoadMore?: () => void;
  };
  let {
    open,
    count,
    active,
    canLoadMore = false,
    loadingMore = false,
    onQuery,
    onNext,
    onPrev,
    onClose,
    onLoadMore,
  }: Props = $props();

  let inputEl = $state<HTMLInputElement>();
  let value = $state("");
  let timer: ReturnType<typeof setTimeout> | undefined;
  const DEBOUNCE_MS = 150;

  // Reset + autofocus whenever the bar opens (the effect runs after the
  // {#if open} content is in the DOM, so inputEl is bound by then).
  $effect(() => {
    if (open) {
      value = "";
      inputEl?.focus();
    }
  });

  // Host hook: ⌘F while the bar is already open re-focuses (and selects) the field.
  export function focusInput() {
    inputEl?.focus();
    inputEl?.select();
  }

  function flushQuery() {
    clearTimeout(timer);
    timer = undefined;
    onQuery(value);
  }
  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(flushQuery, DEBOUNCE_MS);
  }
  // Clear any pending debounce on unmount.
  $effect(() => () => clearTimeout(timer));

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (timer !== undefined) flushQuery(); // don't step through stale matches
      if (e.shiftKey) onPrev();
      else onNext();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }

  // Single close path (Escape + the ✕ button): kill any pending debounce so it
  // can't fire after close and repopulate the (cleared) query with stale text.
  function close() {
    clearTimeout(timer);
    timer = undefined;
    onClose();
  }
</script>

{#if open}
  <div class="search-bar" role="search">
    <svg
      class="icon"
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2.2"
      stroke-linecap="round"
      aria-hidden="true"
    >
      <circle cx="11" cy="11" r="7"></circle>
      <line x1="20" y1="20" x2="16" y2="16"></line>
    </svg>
    <input
      bind:this={inputEl}
      bind:value
      oninput={onInput}
      onkeydown={onKeydown}
      placeholder="Search commits — subject, author, SHA"
      aria-label="Search commits"
      spellcheck="false"
      autocomplete="off"
    />
    {#if value.trim() !== ""}
      <span class="count" aria-live="polite">
        {count > 0 ? `${active + 1} of ${count}` : "No matches"}
      </span>
    {/if}
    {#if canLoadMore}
      <button class="load-more" onclick={onLoadMore} disabled={loadingMore}>
        {loadingMore ? "Loading…" : "Load more to search older history"}
      </button>
    {/if}
    <button aria-label="Previous match (Shift+Enter)" title="Previous match (Shift+Enter)" onclick={onPrev} disabled={count === 0}>▲</button>
    <button aria-label="Next match (Enter)" title="Next match (Enter)" onclick={onNext} disabled={count === 0}>▼</button>
    <button aria-label="Close search (Esc)" title="Close search (Esc)" onclick={close}>✕</button>
  </div>
{/if}

<style>
  /* Same chrome tokens as the commit-list header: 1px border, 6px radius, 12px text. */
  .search-bar {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--panel-bg);
    font-size: 12px;
  }
  .icon {
    flex: 0 0 auto;
    color: var(--text-muted);
  }
  input {
    flex: 1 1 auto;
    min-width: 120px;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--input-bg);
    color: var(--text);
    font-size: 12px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .count {
    flex: 0 0 auto;
    color: var(--text-muted);
    white-space: nowrap;
  }
  button {
    flex: 0 0 auto;
    padding: 2px 8px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--btn-hover);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .load-more {
    color: var(--accent);
    white-space: nowrap;
  }
</style>
