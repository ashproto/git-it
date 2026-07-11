<script lang="ts">
  import type { Snippet } from "svelte";
  import { tick, onDestroy } from "svelte";
  import { slide } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { panelAnim } from "../panelAnim.svelte";

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

  // `animating` keeps a fill/sized body mounted through its measured height animation, so
  // its content stays visible while the panel resizes. The shared `panelAnim` ref-count
  // additionally tells the commit graph to FREEZE its row virtualization during ANY panel
  // animation (its per-frame re-window + SVG rebuild would otherwise make the resize
  // choppy). `animActive` keeps the begin/end pair balanced. (Panels no longer carry a CSS
  // backdrop-filter, so there's no per-frame blur to drop here anymore — see +page.svelte.)
  let animating = $state(false);
  let animActive = false;
  let animTimer: ReturnType<typeof setTimeout> | undefined;

  let sectionEl = $state<HTMLElement>();
  let headerEl = $state<HTMLElement>();
  let animSeq = 0;
  let currentAnim: Animation | undefined; // in-flight WAAPI height animation (measured panels)

  function beginAnim() {
    animating = true;
    if (!animActive) {
      animActive = true;
      panelAnim.begin();
    }
  }
  function endAnim() {
    animating = false;
    if (animActive) {
      animActive = false;
      panelAnim.end();
    }
  }

  function toggle() {
    if (animateBody) {
      // Normal panel: declarative body slide. Flag the blur-off / graph-freeze window.
      collapsed = !collapsed;
      beginAnim();
      clearTimeout(animTimer);
      animTimer = setTimeout(endAnim, SLIDE_MS + 40);
    } else {
      void measuredToggle();
    }
  }

  // Fill/sized panels (commit graph / details drawer) fill the available space, not their
  // content, so a body slide would gap or jump. Instead MEASURE the panel's real height at
  // both ends and run a Web-Animations-API height animation between them.
  //
  // CRITICAL: a `fill` panel is `flex: 1` (grow:1, basis:0%) inside its flex column, which
  // means the flexbox algorithm — NOT the `height` property — decides its used height. Any
  // height we animate is simply ignored and the panel SNAPS to its flex-filled size. (This,
  // not CSS-vs-WAAPI, was the real cause of the snap.) So for the duration of the animation
  // we pin `flex: 0 0 auto`, which makes the animated `height` authoritative; the panel's
  // flex class is restored in finishMeasured. The non-fill sidebar panels never hit this
  // path — they animate via the declarative body slide above.
  async function measuredToggle() {
    const el = sectionEl;
    const hdr = headerEl;
    if (!el || !hdr) {
      collapsed = !collapsed; // no refs (shouldn't happen) → toggle instantly
      return;
    }
    const seq = ++animSeq;
    const collapsing = !collapsed;
    // Read the CURRENT height first — for a reverse-toggle mid-animation this is the live
    // (partway) height; WAAPI reflects it in offsetHeight. Cancel the prior animation only
    // AFTER, otherwise cancel reverts to natural height and the new animation snaps.
    const startH = el.offsetHeight;
    currentAnim?.cancel();
    collapsed = !collapsed; // flip intent; body stays mounted via `animating` (see {#if})
    beginAnim();
    await tick(); // DOM at the target layout (pre-paint microtask)
    if (seq !== animSeq) return; // superseded by a newer toggle

    // Measure the natural target with the panel's REAL flex sizing in effect (clear any
    // leftover inline overrides first): expanding a fill panel → its flex-filled height;
    // collapsing → the header alone (+ the panel's 1px top/bottom borders).
    el.style.height = "";
    el.style.flex = "";
    const endH = collapsing ? hdr.offsetHeight + 2 : el.offsetHeight;
    if (startH === endH || typeof el.animate !== "function") {
      finishMeasured(el, seq);
      return;
    }
    // Pin flex so the height animation isn't overridden by flex-grow, and set the start
    // height synchronously (before the browser can paint) so there's no one-frame flash at
    // the flex-filled size before the animation's fill takes hold.
    el.style.flex = "0 0 auto";
    el.style.overflow = "hidden";
    el.style.height = `${startH}px`;
    const anim = el.animate([{ height: `${startH}px` }, { height: `${endH}px` }], {
      duration: SLIDE_MS,
      easing: EASE,
      fill: "both", // hold startH before the first frame and endH after, so neither end flashes
    });
    currentAnim = anim;
    anim.onfinish = () => finishMeasured(el, seq);
    // Safety net: a backgrounded tab pauses WAAPI so onfinish never fires.
    clearTimeout(animTimer);
    animTimer = setTimeout(() => finishMeasured(el, seq), SLIDE_MS + 200);
  }

  // Settle the panel back to its natural layout after the measured animation. Drop the
  // body-mount flag FIRST (so a collapsed body unmounts) then, after the DOM updates,
  // cancel the WAAPI fill and clear the inline overrides (including the pinned flex) so the
  // panel lands on its natural (header / flex-filled) height without a flash.
  function finishMeasured(el: HTMLElement, seq: number) {
    if (seq !== animSeq) return;
    clearTimeout(animTimer);
    endAnim();
    void tick().then(() => {
      if (seq !== animSeq) return;
      currentAnim?.cancel();
      currentAnim = undefined;
      el.style.height = "";
      el.style.overflow = "";
      el.style.flex = "";
    });
  }

  // If the panel unmounts mid-animation (e.g. the details drawer is removed when the
  // commit is deselected): neutralize pending callbacks (bump the token), stop the WAAPI,
  // clear the timer, and balance the panelAnim ref-count.
  onDestroy(() => {
    animSeq++;
    clearTimeout(animTimer);
    currentAnim?.cancel();
    endAnim();
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
    border-radius: var(--radius-lg);
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
  /* A collapsed panel is exactly its header and must NEVER shrink. Because .panel sets
     overflow:hidden, a flex item's automatic min-size collapses to 0, so without this a
     collapsed panel competing for height in a flex column (e.g. the collapsed commit
     graph next to a tall details pane holding a diff) gets squished below its header and
     clips its title/buttons. flex:none (0 0 auto) pins it to its content height; the
     sibling fill/sized panels absorb any deficit (they scroll internally). */
  .panel.collapsed {
    flex: none;
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
    /* Scroll internally when the content exceeds the panel (matches .panel.sized .body).
       Without this, a fill panel holding tall content — e.g. the commit-details pane in
       fill mode (graph collapsed) showing a real diff — would overflow and push a height
       deficit up the flex column, squishing the collapsed sibling panel's header. The
       graph's own fill body wraps a flex:1 .wrap that already self-scrolls, so this is a
       harmless no-op there. */
    overflow: auto;
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
