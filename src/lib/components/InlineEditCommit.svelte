<script lang="ts">
  // Inline "Edit commit(s)" panel — shown below the commit details when the
  // autoShowEditTools setting is on and a commit is selected. It's the only
  // editor in that mode (the standalone button + context-menu item are hidden,
  // and no modal opens). All three children read appState directly, so there is
  // no prop plumbing; the message editor updates in place (no onDone).
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import CommitMessageEdit from "./CommitMessageEdit.svelte";
  import EditTabs from "./EditTabs.svelte";
  import ApplyPanel from "./ApplyPanel.svelte";
</script>

<CollapsiblePanel title="Edit commit(s)">
  <div class="inline-edit">
    <CommitMessageEdit />
    <EditTabs bare />
    <ApplyPanel bare />
  </div>
</CollapsiblePanel>

<style>
  .inline-edit {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
</style>
