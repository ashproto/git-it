<script lang="ts">
  import { githubActions } from "../../githubActions.svelte";
  import type { GhCommentKind, GhReactionGroup } from "../../types";

  // Reaction pills + an add-reaction popover for one comment/description.
  // `target` null means the payload carried no usable id — no reactions
  // possible, so the whole bar renders nothing.
  let {
    reactions,
    kind,
    target,
  }: { reactions: GhReactionGroup[]; kind: GhCommentKind; target: number | null } = $props();

  // REST content name → emoji (the 8 reactions GitHub supports).
  const EMOJI: Record<string, string> = {
    "+1": "👍",
    "-1": "👎",
    laugh: "😄",
    confused: "😕",
    heart: "❤️",
    hooray: "🎉",
    rocket: "🚀",
    eyes: "👀",
  };
  const ALL = ["+1", "-1", "laugh", "confused", "heart", "hooray", "rocket", "eyes"];

  let busy = $state(false); // single in-flight toggle per bar
  let pickerOpen = $state(false);
  let error = $state<string | null>(null);
  let root: HTMLElement | undefined = $state();

  const shown = $derived(reactions.filter((r) => r.count > 0 && EMOJI[r.content]));

  async function toggle(content: string) {
    if (busy || target == null) return;
    pickerOpen = false;
    busy = true;
    error = null;
    const res = await githubActions.toggleReaction(kind, target, content);
    busy = false;
    if (!res.ok) error = res.error ?? "Could not update reaction.";
  }

  // Close the picker on outside click or Escape while it's open.
  $effect(() => {
    if (!pickerOpen) return;
    const onDown = (e: MouseEvent) => {
      if (root && !root.contains(e.target as Node)) pickerOpen = false;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") pickerOpen = false;
    };
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
    };
  });
</script>

{#if target != null}
  <div class="rbar" bind:this={root}>
    {#each shown as r (r.content)}
      <button
        type="button"
        class="pill"
        class:mine={r.viewerReacted}
        disabled={busy}
        aria-pressed={r.viewerReacted}
        aria-label={`Toggle ${r.content} reaction`}
        onclick={() => void toggle(r.content)}
      >
        <span class="e">{EMOJI[r.content]}</span>{r.count}
      </button>
    {/each}
    <button
      type="button"
      class="pill add"
      disabled={busy}
      aria-label="Add reaction"
      aria-expanded={pickerOpen}
      title="Add reaction"
      onclick={() => (pickerOpen = !pickerOpen)}
    >☺＋</button>
    {#if pickerOpen}
      <div class="picker" role="menu" aria-label="Pick a reaction">
        {#each ALL as c (c)}
          <button
            type="button"
            class="pick"
            disabled={busy}
            aria-label={`React with ${c}`}
            onclick={() => void toggle(c)}
          >{EMOJI[c]}</button>
        {/each}
      </div>
    {/if}
    {#if error}<span class="rerr">{error}</span>{/if}
  </div>
{/if}

<style>
  .rbar {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-top: 6px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    line-height: 1;
    color: var(--text-muted);
    background: var(--btn-bg);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .pill:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--text-muted);
  }
  .pill:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .pill.mine {
    color: var(--accent);
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .pill .e {
    font-size: 12px;
  }
  .pill.add {
    padding: 2px 7px;
  }
  .picker {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 0;
    z-index: 20;
    display: flex;
    gap: 2px;
    padding: 4px 6px;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 999px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
  }
  .pick {
    background: none;
    border: none;
    border-radius: var(--radius-md);
    font-size: 14px;
    line-height: 1;
    padding: 3px 4px;
    cursor: pointer;
  }
  .pick:hover:not(:disabled) {
    background: var(--btn-hover, var(--btn-bg));
  }
  .pick:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .rerr {
    font-size: 11.5px;
    color: var(--status-del, #d22323);
  }
</style>
