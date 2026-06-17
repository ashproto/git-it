<script lang="ts">
  import type { Snippet } from "svelte";
  import { tick, onDestroy } from "svelte";
  import { slide } from "svelte/transition";
  import { quintOut } from "svelte/easing";

  type Props = {
    title: string;
    collapsed?: boolean;
    bare?: boolean;
    // fill: the panel grows to fill its flex parent and its body scrolls (used by
    // the commits graph so it occupies the full timeline height). Ignored when collapsed.
    fill?: boolean;
    // height: a FIXED pixel height for the (expanded) panel; the body becomes the
    // scrolling region. Used by the slide-up commit-details pane so its height stays
    // stable while the diff loads async — otherwise content-driven height shrinks then
    // grows (jitter on open / jump on commit-switch). Ignored when collapsed (→ auto,
    // so a collapsed panel is just its header with no dead space).
    height?: number;
    headerActions?: Snippet;
    children: Snippet;
  };
  let { title, collapsed = $bindable(false), bare = false, fill = false, height, headerActions, children }: Props = $props();

  // Apply the fixed height only when expanded (collapsed → auto-height header only).
  const sized = $derived(height != null && !collapsed);

  // A "normal content panel" (no fill, no fixed height) animates via a body slide — its
  // on-screen height === its content height, so the slide is correct + cheap. A fill or
  // sized panel (the commit graph / the details drawer) fills the available space (or a
  // fixed height), NOT its content, so a body slide would leave a gap or jump; those use
  // the MEASURED animation below instead.
  const animateBody = $derived(!fill && height == null);
  const SLIDE_MS = 200;
  const EASE = "cubic-bezier(0.22, 1, 0.36, 1)"; // ~quintOut

  // `animating` does two jobs while a collapse is in flight: (1) it keeps a fill/sized
  // body rendered through its measured animation, and (2) +page.svelte drops the panel's
  // frosted-glass backdrop-filter while it's set — re-blurring a resizing panel every
  // frame is the main cause of a low-framerate collapse in the translucent Tauri build.
  let animating = $state(false);
  let animTimer: ReturnType<typeof setTimeout> | undefined;

  // Refs + a sequence token for the measured animation (fill/sized panels). The token
  // lets a rapid re-toggle supersede an in-flight animation cleanly.
  let sectionEl = $state<HTMLElement>();
  let headerEl = $state<HTMLElement>();
  let animSeq = 0;
  // Tears down the in-flight measured animation's listener + safety timer. Held so
  // onDestroy can cancel it if the panel unmounts mid-animation (e.g. the details
  // drawer is removed when the commit is deselected), avoiding a post-unmount leak.
  let cancelAnim: (() => void) | undefined;

  function toggle() {
    if (animateBody) {
      // Normal panel: declarative body slide + a transient blur-off window.
      collapsed = !collapsed;
      animating = true;
      clearTimeout(animTimer);
      animTimer = setTimeout(() => (animating = false), SLIDE_MS + 40);
    } else {
      void measuredToggle();
    }
  }

  // Measure the panel's REAL height at both ends and transition the panel's own height
  // between them, so it shrinks/grows together with its (clipped) content — no gap, no
  // jump, works for both fill and sized panels.
  async function measuredToggle() {
    const el = sectionEl;
    const hdr = headerEl;
    if (!el || !hdr) {
      collapsed = !collapsed; // no refs (shouldn't happen) → just toggle instantly
      return;
    }
    const seq = ++animSeq;
    const collapsing = !collapsed;
    const startH = el.offsetHeight;
    collapsed = !collapsed; // flip intent; the body stays mounted via `animating` (see {#if})
    animating = true;
    await tick(); // DOM is at the target layout but not yet painted (microtask)
    if (seq !== animSeq) return; // a newer toggle superseded this one

    el.style.height = ""; // clear any prior lock so the natural target measures correctly
    // Collapsed target = the header alone (+ the panel's 1px top/bottom borders);
    // expanded target = the natural filled/sized height (body is mounted now).
    const endH = collapsing ? hdr.offsetHeight + 2 : el.offsetHeight;
    if (startH === endH) {
      void finishMeasured(el, seq);
      return;
    }
    // Lock to start, then animate to end — all synchronous, so the browser only paints
    // once it's locked to `startH` (no flash to the natural height first).
    el.style.height = `${startH}px`;
    el.style.overflow = "hidden";
    void el.offsetHeight; // force reflow
    el.style.transition = `height ${SLIDE_MS}ms ${EASE}`;
    el.style.height = `${endH}px`;

    const onEnd = (e: TransitionEvent) => {
      if (e.target !== el || e.propertyName !== "height") return;
      cancelAnim?.();
      void finishMeasured(el, seq);
    };
    el.addEventListener("transitionend", onEnd);
    clearTimeout(animTimer);
    cancelAnim = () => {
      el.removeEventListener("transitionend", onEnd);
      clearTimeout(animTimer);
    };
    animTimer = setTimeout(() => {
      cancelAnim?.();
      void finishMeasured(el, seq);
    }, SLIDE_MS + 120); // safety net if transitionend never fires
  }

  // Settle the panel back to its natural layout AFTER the measured animation. Order
  // matters: drop `animating` first (which unmounts a collapsed body) THEN clear the
  // locked height, so a collapsed panel never momentarily springs back to full height.
  async function finishMeasured(el: HTMLElement, seq: number) {
    if (seq !== animSeq) return;
    animating = false;
    await tick();
    if (seq !== animSeq) return;
    el.style.height = "";
    el.style.overflow = "";
    el.style.transition = "";
    cancelAnim = undefined;
  }

  // If the panel unmounts mid-animation (e.g. the details drawer is removed when the
  // commit is deselected), neutralize any pending callback (bump the token so a queued
  // finishMeasured short-circuits) and tear down its listener + safety timer.
  onDestroy(() => {
    animSeq++;
    clearTimeout(animTimer);
    cancelAnim?.();
  });
</script>

{#if bare}
  <div class="cp-bare">{@render children()}</div>
{:else}
  <section
    bind:this={sectionEl}
    class="panel"
    class:collapsed
    class:fill={fill && !collapsed}
    class:sized
    class:cp-animating={animating}
    style={height != null ? `--cp-h:${height}px` : undefined}
  >
    <header bind:this={headerEl} class="panel-header">
      <button type="button" class="toggle" onclick={toggle} aria-expanded={!collapsed}>
        <span class="chevron" class:open={!collapsed} aria-hidden="true">▶</span>
        <h2>{title}</h2>
      </button>
      {#if headerActions}
        <div class="actions">{@render headerActions()}</div>
      {/if}
    </header>
    {#if !collapsed || (animating && !animateBody)}
      <!-- Normal panels: a local body slide plays the open/close. Fill/sized panels:
           the body has no slide (duration 0) — the panel's measured height animation
           (measuredToggle) does the motion, and `animating` keeps the body mounted &
           clipped throughout, so its content stays visible as the panel resizes. -->
      <div class="body" transition:slide={{ duration: animateBody ? SLIDE_MS : 0, easing: quintOut }}>
        {@render children()}
      </div>
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
  /* sized mode: a FIXED-height panel (height set inline) whose body is the scroll
     region. Keeps the slide-up details pane a stable height while its diff loads.
     flex:none so a flex parent can't shrink the explicit height away (e.g. next to
     the graph's flex:1, which would otherwise starve it to ~0). */
  .panel.sized {
    /* Height comes from a CSS var (set inline) rather than an inline `height`, so the
       measured collapse animation can override `style.height` imperatively and then
       clear it without fighting Svelte's reactive style binding. */
    height: var(--cp-h);
    flex: none;
    display: flex;
    flex-direction: column;
  }
  .panel.sized .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .cp-bare {
    padding: 2px 0;
  }
</style>
