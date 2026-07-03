// The one place that knows every overlay/dialog singleton, so global shortcuts
// (⌘F search / filters) can ask "is anything covering the screen?" without each
// handler chasing individual stores (and drifting when a new overlay is added).
import { dialogs } from "./dialogs.svelte";
import { settingsPanel } from "./settingsPanel.svelte";
import { manageRepo } from "./manageRepo.svelte";
import { amendDialog } from "./amendDialog.svelte";
import { branchColorDialog } from "./branchColorDialog.svelte";
import { rebaseEditor } from "./rebaseEditor.svelte";
import { githubActions } from "./githubActions.svelte";

export function anyOverlayOpen(): boolean {
  return (
    dialogs.state.kind !== "none" ||
    settingsPanel.open ||
    manageRepo.open ||
    amendDialog.open ||
    branchColorDialog.open ||
    rebaseEditor.open ||
    githubActions.pending !== null
  );
}
