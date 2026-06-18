<script lang="ts">
  import type { GhAvailability } from "../../types";
  let { availability, onretry }: { availability: GhAvailability; onretry: () => void } = $props();
</script>

<div class="setup">
  {#if availability.kind === "NotInstalled"}
    <h2>GitHub CLI not found</h2>
    <p>The GitHub screen uses the <code>gh</code> command-line tool. Install it, then retry.</p>
    <pre>brew install gh</pre>
    <a href="https://cli.github.com" target="_blank" rel="noreferrer">cli.github.com ↗</a>
  {:else if availability.kind === "NotAuthed"}
    <h2>Sign in to GitHub</h2>
    <p>Authenticate the <code>gh</code> CLI once in your terminal, then retry.</p>
    <pre>gh auth login</pre>
  {:else if availability.kind === "NoRemote"}
    <h2>No GitHub remote</h2>
    <p>This repository has no <code>github.com</code> remote, so there's nothing to show here.</p>
  {/if}
  <button class="retry" type="button" onclick={onretry}>Retry</button>
</div>

<style>
  .setup {
    max-width: 460px;
    margin: 48px auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--text);
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  p {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  pre {
    margin: 4px 0;
    padding: 8px 14px;
    background: var(--btn-bg);
    border-radius: 6px;
    font-size: 12.5px;
  }
  a {
    color: var(--accent);
    font-size: 12.5px;
  }
  .retry {
    margin-top: 8px;
    padding: 6px 16px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--btn-bg);
    color: var(--text);
    cursor: pointer;
  }
</style>
