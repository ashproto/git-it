// Singleton controller for the "Set branch colour" picker modal. Holds which ref
// is being edited; the dialog reads/writes the override via the store.
function makeBranchColorDialog() {
  let open = $state(false);
  let refName = $state<string | null>(null);
  return {
    get open() {
      return open;
    },
    get refName() {
      return refName;
    },
    openFor(name: string) {
      refName = name;
      open = true;
    },
    close() {
      open = false;
      refName = null;
    },
  };
}

export const branchColorDialog = makeBranchColorDialog();
