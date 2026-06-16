// Coordinates programmatic "scroll to a commit" requests that originate outside
// the graph view (e.g. the Sidebar's jump-to-ref). Because the graph virtualizes
// its rows, a target commit may not be in the DOM — so callers can't just
// getElementById + scrollIntoView. Instead they ask here; GraphHistory resolves
// the request by commit index and scrolls its container, which renders the row.
//
// `nonce` is a monotonic counter so that repeated jumps to the *same* sha still
// trigger the listening effect (a bare sha wouldn't change and would be missed).
function makeGraphView() {
  let requestSha = $state<string | null>(null);
  let nonce = $state(0);

  return {
    get requestSha() {
      return requestSha;
    },
    get nonce() {
      return nonce;
    },
    scrollToCommit(sha: string) {
      requestSha = sha;
      nonce++;
    },
  };
}

export const graphView = makeGraphView();
