// Tiny store for the TimeEditDrawer: just open/close state. The drawer reads the
// live selection (appState.selected) — there is no per-open seed to capture.
function makeTimeEditDrawer() {
  let open = $state(false);

  return {
    get open() {
      return open;
    },
    openDrawer() {
      open = true;
    },
    close() {
      open = false;
    },
  };
}

export const timeEditDrawer = makeTimeEditDrawer();
