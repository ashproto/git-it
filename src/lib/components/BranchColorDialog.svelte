<script lang="ts">
  // "Set branch colour" picker — 8 palette swatches + a validated custom hex field
  // + reset-to-lane-default. Writes a per-repo override via the store.
  import { branchColorDialog } from "../branchColorDialog.svelte";
  import { appState } from "../store.svelte";
  import { LANE_PALETTE } from "../graph/colors";

  let closeBtn = $state<HTMLButtonElement | undefined>();
  let hex = $state("");

  const name = $derived(branchColorDialog.refName);
  const HEX_RE = /^#[0-9a-fA-F]{6}$/;
  const validHex = $derived(HEX_RE.test(hex.trim()));
  // The override currently saved for this ref (highlights the active swatch).
  const current = $derived(
    name ? (appState.branchColors[appState.repo]?.[name] ?? null) : null,
  );

  // Seed the hex field from the current override each time the dialog targets a new
  // ref, then focus the close button (predictable, always-present focus target).
  let seededFor = $state<string | null>(null);
  $effect(() => {
    if (!branchColorDialog.open) {
      seededFor = null;
      return;
    }
    const n = name;
    if (n === seededFor) return;
    seededFor = n;
    hex = (n ? appState.branchColors[appState.repo]?.[n] : "") ?? "";
    Promise.resolve().then(() => closeBtn?.focus());
  });

  function pick(c: string) {
    if (name) appState.setBranchColor(name, c.toLowerCase());
    branchColorDialog.close();
  }
  function applyHex() {
    if (name && validHex) appState.setBranchColor(name, hex.trim().toLowerCase());
    branchColorDialog.close();
  }
  function reset() {
    if (name) appState.clearBranchColor(name);
    branchColorDialog.close();
  }
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      branchColorDialog.close();
    }
  }
</script>

{#if branchColorDialog.open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) branchColorDialog.close();
    }}
    onkeydown={handleKeydown}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Set branch colour">
      <div class="header">
        <h3>Branch colour</h3>
        <button
          type="button"
          class="close-btn"
          bind:this={closeBtn}
          aria-label="Close"
          onclick={() => branchColorDialog.close()}
        >✕</button>
      </div>
      <p class="ref-name mono" title={name ?? ""}>{name ?? ""}</p>

      <p class="group-label">Palette</p>
      <div class="swatches">
        {#each LANE_PALETTE as c}
          <button
            type="button"
            class="swatch"
            class:active={current?.toLowerCase() === c.toLowerCase()}
            style={`background:${c}`}
            title={c}
            aria-label={`Use ${c}`}
            onclick={() => pick(c)}
          ></button>
        {/each}
      </div>

      <p class="group-label">Custom</p>
      <div class="hex-row">
        <span class="hex-preview" style={`background:${validHex ? hex.trim() : "transparent"}`}></span>
        <input
          class="hex-input mono"
          bind:value={hex}
          placeholder="#RRGGBB"
          spellcheck="false"
          maxlength="7"
          aria-label="Custom hex colour"
          onkeydown={(e) => {
            if (e.key === "Enter" && validHex) applyHex();
          }}
        />
        <button type="button" class="apply" disabled={!validHex} onclick={applyHex}>Apply</button>
      </div>

      <div class="actions">
        <button type="button" class="reset" onclick={reset}>Reset to lane default</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 3000;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .dialog {
    width: 340px;
    max-width: calc(100vw - 32px);
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    display: flex;
    flex-direction: column;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    line-height: 1;
  }
  .close-btn:hover {
    background: var(--btn-hover);
    color: var(--text);
  }
  .close-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .ref-name {
    margin: 0 2px 10px;
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .group-label {
    margin: 4px 2px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 6px;
    margin: 0 2px 8px;
  }
  .swatch {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 6px;
    border: 1px solid var(--border);
    cursor: pointer;
    padding: 0;
  }
  .swatch.active {
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }
  .swatch:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .hex-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 2px 12px;
  }
  .hex-preview {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 1px solid var(--border);
    flex-shrink: 0;
  }
  .hex-input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--input-bg, var(--btn-bg));
    color: var(--text);
    font-size: 12.5px;
  }
  .hex-input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .apply {
    padding: 5px 12px;
    border-radius: 6px;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    cursor: pointer;
    flex-shrink: 0;
  }
  .apply:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
  .reset {
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  .reset:hover {
    background: var(--btn-hover);
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
