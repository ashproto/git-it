<script lang="ts">
  import BackupsPanel from "./BackupsPanel.svelte";
  import ReflogPanel from "./ReflogPanel.svelte";
  import StashPanel from "./StashPanel.svelte";
  import RemotePanel from "./RemotePanel.svelte";

  let open = $state(true);
</script>

<div class="recovery-group">
  <button class="sec" onclick={() => (open = !open)} aria-expanded={open}>
    <span class="chev" class:open aria-hidden="true">▸</span>
    <span class="label">History &amp; Recovery</span>
  </button>
  {#if open}
    <div class="panels">
      <BackupsPanel />
      <ReflogPanel />
      <StashPanel />
      <RemotePanel />
    </div>
  {/if}
</div>

<style>
  .recovery-group {
    display: flex;
    flex-direction: column;
    margin-top: 4px;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 6px;
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .sec:hover {
    color: var(--text);
  }
  .chev {
    display: inline-block;
    transition: transform 0.12s ease;
    font-size: 10px;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .label {
    flex: 1;
    text-align: left;
  }
  .panels {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
