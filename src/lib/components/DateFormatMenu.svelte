<script lang="ts">
  import { appState } from "../store.svelte";

  let open = $state(false);
  let rootEl: HTMLDivElement;
  let gearBtn: HTMLButtonElement;

  function toggle() {
    open = !open;
  }
  function close(restoreFocus = false) {
    open = false;
    if (restoreFocus) gearBtn?.focus();
  }
  function onDocPointerDown(e: PointerEvent) {
    if (rootEl && !rootEl.contains(e.target as Node)) close(false);
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close(true);
  }

  // Register global listeners only while open. Capture-phase pointerdown so the
  // outside-click fires even though the header swallows mousedown to drag the
  // window; clicks inside rootEl are ignored, so toggling a checkbox doesn't
  // close the menu. Also close on window blur (native-menu parity).
  $effect(() => {
    if (!open) return;
    const onBlur = () => close(false);
    document.addEventListener("pointerdown", onDocPointerDown, true);
    document.addEventListener("keydown", onKeydown);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("pointerdown", onDocPointerDown, true);
      document.removeEventListener("keydown", onKeydown);
      window.removeEventListener("blur", onBlur);
    };
  });

  // Move focus into the popover when it opens so keyboard users land on the first
  // control; Escape (and re-clicking the gear) returns focus to the gear button.
  $effect(() => {
    if (!open) return;
    rootEl
      ?.querySelector<HTMLInputElement>('.popover input[type="checkbox"]')
      ?.focus();
  });
</script>

<!-- data-no-drag keeps a mousedown anywhere in here from starting a window drag
     (tauriDrag.ts skips [data-no-drag], plus button/input/label individually). -->
<div class="gear" bind:this={rootEl} data-no-drag>
  <button
    type="button"
    class="gear-btn"
    bind:this={gearBtn}
    aria-expanded={open}
    aria-controls={open ? "date-format-popover" : undefined}
    aria-label="Date display settings"
    title="Date display settings"
    onclick={toggle}
  >
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="3"></circle>
      <path
        d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
      ></path>
    </svg>
  </button>

  {#if open}
    <!-- Disclosure pattern: a labeled GROUP of checkboxes, not an ARIA menu
         (a menu requires menuitem* children + arrow-key navigation, which plain
         checkboxes are not). Tab moves between the boxes, Space toggles, Escape
         closes — all correct for a group. -->
    <div
      class="popover"
      id="date-format-popover"
      role="group"
      aria-labelledby="date-format-title"
    >
      <p class="pop-title" id="date-format-title">Date display</p>
      <!-- Controlled (checked + onchange), NOT bind:checked: writes must funnel
           through setDateFormat so the $state object is reassigned (reactivity)
           and persisted. -->
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.dateFormat.hour12}
          onchange={() => appState.setDateFormat({ hour12: !appState.dateFormat.hour12 })}
        />
        <span>12-hour (AM/PM)</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.dateFormat.weekday}
          onchange={() => appState.setDateFormat({ weekday: !appState.dateFormat.weekday })}
        />
        <span>Show weekday</span>
      </label>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.dateFormat.monthName}
          onchange={() => appState.setDateFormat({ monthName: !appState.dateFormat.monthName })}
        />
        <span>Show month name</span>
      </label>
      <hr class="divider" />
      <p class="pop-title">Safety</p>
      <label class="opt">
        <input
          type="checkbox"
          checked={appState.autoBackupDestructive}
          onchange={() => appState.setAutoBackupDestructive(!appState.autoBackupDestructive)}
        />
        <span>Create backup before destructive ops</span>
      </label>
    </div>
  {/if}
</div>

<style>
  .gear {
    position: relative;
    margin-left: auto; /* push to the right edge of the header */
    align-self: center; /* header is baseline-aligned; center the button */
    display: inline-flex;
  }
  .gear-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    cursor: pointer;
  }
  .gear-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }
  .gear-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .popover {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 1000;
    min-width: 190px;
    padding: 8px;
    border-radius: 8px;
    border: 1px solid var(--border);
    /* Falls back to --panel-bg outside glass mode; in glass mode --popover-bg is
       a near-opaque scrim so the settings text stays legible over any wallpaper. */
    background: var(--popover-bg, var(--panel-bg));
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }
  .pop-title {
    margin: 2px 4px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 4px;
    font-size: 12.5px;
    color: var(--text);
    cursor: pointer;
    border-radius: 5px;
  }
  .opt:hover,
  .opt:focus-within {
    background: var(--row-hover);
  }
  .opt input {
    margin: 0;
  }
  .opt input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .divider {
    border: none;
    border-top: 1px solid var(--border);
    margin: 6px 0 4px;
  }
</style>
