<script lang="ts">
  import { outputAddsToHeadline } from "../errorReport";
  import { errors } from "../errors.svelte";

  let okEl = $state<HTMLButtonElement | undefined>();
  let showOutput = $state(false);
  let copyState = $state<"idle" | "copied" | "failed">("idle");
  // Plain bookkeeping for the effect below (never read by the template).
  let shownId: number | null = null;
  let focusedId: number | null = null;
  let returnFocusTo: HTMLElement | null = null;

  const report = $derived(errors.current);
  // Offer the full output only when it adds something beyond the headline.
  const hasOutput = $derived(
    !!report && report.kind === "error" && outputAddsToHeadline(report),
  );

  // Reset the disclosure and copy state per report, focus OK once the button exists, and give
  // focus back to wherever it was when the queue empties. `okEl` is read on every run, so if
  // the button binds after this effect first runs, the effect runs again and still focuses it.
  $effect(() => {
    const r = errors.current;
    if (!r) {
      if (shownId !== null) {
        const el = returnFocusTo;
        shownId = focusedId = null;
        returnFocusTo = null;
        if (el?.isConnected) el.focus();
      }
      return;
    }
    if (r.id !== shownId) {
      if (shownId === null) {
        const active = document.activeElement;
        returnFocusTo = active instanceof HTMLElement ? active : null;
      }
      shownId = r.id;
      showOutput = false;
      copyState = "idle";
    }
    if (okEl && focusedId !== r.id) {
      focusedId = r.id;
      okEl.focus();
    }
  });

  async function copyDetails() {
    const r = errors.current;
    if (!r) return;
    try {
      await navigator.clipboard.writeText(`${r.title}\n${r.headline}\n\n${r.output}`);
      copyState = "copied";
    } catch {
      copyState = "failed";
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Keys stop at the overlay so bubbling window-level handlers underneath (the GitHub
    // action modal's Escape, a diff selection) don't also fire. Capture-phase window
    // listeners still see them first.
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      errors.dismiss();
    }
  }
</script>

{#if report}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) errors.dismiss();
    }}
    onkeydown={onKeydown}
  >
    <!-- tabindex -1: clicking the selectable text focuses the dialog rather than <body>, so
         Escape still dismisses it and keys still stop at the overlay. -->
    <div
      class="dialog"
      class:notice={report.kind === "notice"}
      tabindex="-1"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="error-dialog-title"
      aria-describedby="error-dialog-headline"
    >
      <div class="title-row">
        <h3 id="error-dialog-title">{report.title}</h3>
        {#if errors.pending > 1}
          <span class="count">1 of {errors.pending}</span>
        {/if}
      </div>
      <div class="body">
        <div class="headline">
          <span class="glyph" aria-hidden="true">{report.kind === "error" ? "⚠" : "ⓘ"}</span>
          <p id="error-dialog-headline" class="headline-text selectable">{report.headline}</p>
        </div>
        {#if report.hints}
          <pre class="hints selectable">{report.hints}</pre>
        {/if}
        {#if hasOutput}
          <button
            type="button"
            class="disclosure"
            aria-expanded={showOutput}
            onclick={() => (showOutput = !showOutput)}
          >
            <span aria-hidden="true">{showOutput ? "▾" : "▸"}</span>
            {showOutput ? "Hide git output" : "Show git output"}
          </button>
          {#if showOutput}
            <pre class="output selectable">{report.output}</pre>
          {/if}
        {/if}
      </div>
      <div class="actions">
        {#if report.kind === "error"}
          <button type="button" class="copy" onclick={copyDetails}>
            {copyState === "copied" ? "Copied" : copyState === "failed" ? "Copy failed" : "Copy details"}
          </button>
        {/if}
        <button type="button" class="primary" bind:this={okEl} onclick={() => errors.dismiss()}>OK</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    /* One step above the dialog layer (Modal.svelte, 4000): an error that lands while a
       prompt is open covers it without closing it. */
    z-index: 4100;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .dialog {
    width: min(600px, calc(100vw - 32px));
    max-height: 80vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }
  .dialog:focus {
    outline: none;
  }
  .title-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin: 0 0 10px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .count {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  /* The text scrolls; the title and buttons stay put. */
  .body {
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    margin: 0 0 14px;
  }
  .headline {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .glyph {
    flex-shrink: 0;
    font-size: 14px;
    line-height: 1.5;
    color: var(--danger);
  }
  .notice .glyph {
    color: var(--accent);
  }
  .headline-text {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .hints {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    background: var(--btn-bg);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .output {
    margin: 6px 0 0;
    max-height: 260px;
    overflow: auto;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--input-bg);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.45;
    white-space: pre;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  button {
    padding: 6px 14px;
    border-radius: var(--radius-md);
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
    color: var(--on-accent);
  }
  .disclosure {
    margin-top: 10px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    font-size: 12px;
  }
  .disclosure:hover {
    background: none;
    color: var(--text);
  }
</style>
