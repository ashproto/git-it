<script lang="ts">
  import { contextMenu, type MenuItem } from "../contextMenu.svelte";

  function choose(item: MenuItem) {
    contextMenu.close();
    item.action?.();
  }

  // Open submenus leftward when the menu itself is near the right edge, so a
  // right-hand flyout can't run off-screen.
  const flipSub = $derived(
    typeof window !== "undefined" && contextMenu.x > window.innerWidth * 0.6,
  );

  // While open, close on outside pointerdown (capture so it beats other handlers),
  // Escape, or window blur.
  $effect(() => {
    if (!contextMenu.open) return;
    const onDown = (e: PointerEvent) => {
      const el = document.getElementById("app-context-menu");
      if (el && !el.contains(e.target as Node)) contextMenu.close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") contextMenu.close();
    };
    const onBlur = () => contextMenu.close();
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
    };
  });
</script>

{#if contextMenu.open}
  <div
    id="app-context-menu"
    class="menu"
    style={`left:${Math.min(contextMenu.x, (typeof window !== "undefined" ? window.innerWidth : 9999) - 200)}px; top:${Math.min(contextMenu.y, (typeof window !== "undefined" ? window.innerHeight : 9999) - (contextMenu.items.length * 30 + 16))}px`}
    role="menu"
    aria-label="Context menu"
  >
    {#each contextMenu.items as item, i (i)}
      {#if item.separator}
        <div class="sep" role="separator"></div>
      {:else if item.submenu}
        <div class="sub-wrap" class:flip={flipSub}>
          <button class="item has-sub" class:danger={item.danger} role="menuitem" disabled={item.disabled}>
            <span>{item.label}</span><span class="chev" aria-hidden="true">›</span>
          </button>
          <div class="menu submenu" role="menu" aria-label={item.label}>
            {#each item.submenu as sub, j (j)}
              {#if sub.separator}
                <div class="sep" role="separator"></div>
              {:else}
                <button
                  class="item"
                  class:danger={sub.danger}
                  role="menuitem"
                  disabled={sub.disabled}
                  onclick={() => choose(sub)}
                >
                  {sub.label}
                </button>
              {/if}
            {/each}
          </div>
        </div>
      {:else}
        <button
          class="item"
          class:danger={item.danger}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => choose(item)}
        >
          {item.label}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 2000;
    min-width: 190px;
    padding: 4px;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.22);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }
  .item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    border: none;
    background: none;
    color: var(--text);
    font-size: 13px;
    border-radius: 5px;
    cursor: pointer;
  }
  .item:hover:not(:disabled) {
    background: var(--row-hover);
  }
  .item:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .item.danger {
    color: var(--danger);
  }
  .sep {
    height: 1px;
    background: var(--border);
    margin: 4px 2px;
  }
  .sub-wrap {
    position: relative;
  }
  .item.has-sub {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .chev {
    color: var(--text-muted, var(--text));
    opacity: 0.7;
  }
  .submenu {
    position: absolute;
    top: -4px;
    left: 100%;
    margin-left: 2px;
    display: none;
  }
  .sub-wrap.flip .submenu {
    left: auto;
    right: 100%;
    margin-left: 0;
    margin-right: 2px;
  }
  .sub-wrap:hover > .submenu,
  .submenu:hover {
    display: block;
  }
</style>
