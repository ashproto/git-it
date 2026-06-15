// Tiny store for the RebaseTodo interactive-rebase editor: open/close state + base SHA.
function makeRebaseEditor() {
  let open = $state(false);
  let base = $state("");

  return {
    get open() {
      return open;
    },
    get base() {
      return base;
    },
    openWith(baseSha: string) {
      base = baseSha;
      open = true;
    },
    close() {
      open = false;
    },
  };
}

export const rebaseEditor = makeRebaseEditor();
