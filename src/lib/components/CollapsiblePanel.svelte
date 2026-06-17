<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title: string;
    collapsed?: boolean;
    bare?: boolean;
    // fill: the panel grows to fill its flex parent and its body scrolls (used by
    // the commits graph so it occupies the full timeline height). Ignored when collapsed.
    fill?: boolean;
    headerActions?: Snippet;
    children: Snippet;
  };
  let { title, collapsed = $bindable(false), bare = false, fill = false, headerActions, children }: Props = $props();

  function toggle() {
    collapsed = !collapsed;
  }
</script>

{#if bare}
  <div class="cp-bare">{@render children()}</div>
{:else}
  <section class="panel" class:collapsed class:fill={fill && !collapsed}>
    <header class="panel-header">
      <button type="button" class="toggle" onclick={toggle} aria-expanded={!collapsed}>
        <span class="chevron" class:open={!collapsed} aria-hidden="true">▶</span>
        <h2>{title}</h2>
      </button>
      {#if headerActions}
        <div class="actions">{@render headerActions()}</div>
      {/if}
    </header>
    {#if !collapsed}
      <div class="body">{@render children()}</div>
    {/if}
  </section>
{/if}

<style>
  .panel {
    background: var(--panel-bg);
    border-radius: 10px;
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 14px;
    gap: 12px;
    background: var(--panel-bg);
  }
  .panel.collapsed .panel-header {
    border-bottom: none;
  }
  .panel:not(.collapsed) .panel-header {
    border-bottom: 1px solid var(--border-subtle);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 2px 0;
    color: inherit;
    flex: 1;
    min-width: 0;
    text-align: left;
  }
  .toggle:hover .chevron {
    color: var(--text);
  }
  .chevron {
    font-size: 10px;
    color: var(--text-muted);
    transition: transform 0.12s ease;
    width: 12px;
    display: inline-block;
  }
  .chevron.open {
    transform: rotate(90deg);
  }
  h2 {
    font-size: 13px;
    margin: 0;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: 600;
  }
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .body {
    padding: 10px 14px 14px 14px;
  }
  /* fill mode: the panel grows to fill its flex parent and the body becomes the
     flex region whose child (e.g. the graph .wrap) scrolls. */
  .panel.fill {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .panel.fill .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .cp-bare {
    padding: 2px 0;
  }
</style>
