// Tiny store for the AmendDialog: open/close state + seed data.
function makeAmendDialog() {
  let open = $state(false);
  let sha = $state("");
  let subject = $state("");

  return {
    get open() {
      return open;
    },
    get sha() {
      return sha;
    },
    get subject() {
      return subject;
    },
    openWith(commitSha: string, commitSubject: string) {
      sha = commitSha;
      subject = commitSubject;
      open = true;
    },
    close() {
      open = false;
    },
  };
}

export const amendDialog = makeAmendDialog();
