// Tiny singleton controller for the Manage Repository modal: open/close only.
// Mirror of settingsPanel.svelte.ts — no seed data needed.
function makeManageRepo() {
  let open = $state(false);
  return {
    get open() {
      return open;
    },
    openPanel() {
      open = true;
    },
    close() {
      open = false;
    },
  };
}

export const manageRepo = makeManageRepo();
