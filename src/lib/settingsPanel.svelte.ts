// Tiny singleton controller for the SettingsPanel modal: open/close state only.
// Mirror of amendDialog.svelte.ts — no seed data needed.
function makeSettingsPanel() {
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

export const settingsPanel = makeSettingsPanel();
