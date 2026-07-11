<script lang="ts">
  import { mdToSafeHtml } from "../../github/markdown";
  let { src }: { src: string } = $props();
  const html = $derived(mdToSafeHtml(src));
</script>

<div class="md">{@html html}</div>

<style>
  .md {
    font-size: 13px;
    line-height: 1.6;
    color: var(--text);
    overflow-wrap: break-word;
    word-break: break-word;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    font-size: 14px;
    margin: 12px 0 6px;
  }
  .md :global(p) {
    margin: 6px 0;
  }
  .md :global(a) {
    color: var(--accent);
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 20px;
    margin: 6px 0;
  }
  .md :global(code) {
    font-family: var(--font-mono);
    font-size: 11.5px;
    background: var(--btn-bg);
    padding: 1px 4px;
    border-radius: var(--radius-sm);
  }
  .md :global(pre) {
    background: var(--btn-bg);
    padding: 10px 12px;
    border-radius: var(--radius-dialog);
    overflow-x: auto;
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
  }
  .md :global(blockquote) {
    border-left: 3px solid var(--border);
    margin: 6px 0;
    padding: 2px 0 2px 12px;
    color: var(--text-muted);
  }
  .md :global(img) {
    max-width: 100%;
  }
  .md :global(table) {
    border-collapse: collapse;
    font-size: 12px;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--border);
    padding: 3px 8px;
  }
</style>
