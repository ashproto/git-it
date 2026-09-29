// The text a failed commit-time rewrite reports, from git-filter-repo's stderr lines.
// Pure (Tauri-free) so it is unit-tested with vitest.

// git-filter-repo runs fast-export with --use-done-feature, so when filter-repo itself fails
// (a Python traceback, say), fast-import reports the aborted stream after the real failure.
// Left in, that epilogue's `fatal:` line would take the headline from the cause.
function isFastImportEpilogue(line: string): boolean {
  return (
    line === "fatal: stream ends early" ||
    line.startsWith("fast-import: dumping crash report to ")
  );
}

/** The lines as one text, minus fast-import's epilogue when another line comes before it. */
export function rewriteFailureText(stderr: string[]): string {
  const first = stderr.findIndex(isFastImportEpilogue);
  const lines = first > 0 ? stderr.filter((line) => !isFastImportEpilogue(line)) : stderr;
  return lines.join("\n");
}
