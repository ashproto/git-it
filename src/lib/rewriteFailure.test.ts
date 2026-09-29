import { describe, expect, it } from "vitest";
import { summarizeGitOutput } from "./errorReport";
import { rewriteFailureText } from "./rewriteFailure";

const EPILOGUE = ["fatal: stream ends early", "fast-import: dumping crash report to .git/fast_import_crash_12345"];
const SCRIPT = "/Applications/Git It.app/Contents/Resources/resources/git-filter-repo/git-filter-repo";
// The bundled git-filter-repo's stderr when a commit callback raises: the traceback, then
// fast-import's report of the stream it cut short.
const CALLBACK_FAILURE = [
  "Traceback (most recent call last):",
  `  File "${SCRIPT}", line 4976, in <module>`,
  "    main()",
  `  File "${SCRIPT}", line 4973, in main`,
  "    filter.run()",
  `  File "${SCRIPT}", line 4892, in run`,
  "    self._parser.run(self._input, self._output)",
  `  File "${SCRIPT}", line 1527, in run`,
  "    self._parse_commit()",
  `  File "${SCRIPT}", line 1378, in _parse_commit`,
  "    self._commit_callback(commit, aux_info)",
  `  File "${SCRIPT}", line 4108, in _tweak_commit`,
  "    self._commit_callback(commit, self.callback_metadata(aux_info))",
  '  File "<string>", line 2, in callback',
  "ValueError: bad date",
  ...EPILOGUE,
];

describe("rewriteFailureText", () => {
  it("ends a callback failure at its exception, dropping fast-import's epilogue", () => {
    const text = rewriteFailureText(CALLBACK_FAILURE);
    expect(text).toBe(CALLBACK_FAILURE.slice(0, -EPILOGUE.length).join("\n"));
    expect(summarizeGitOutput(text).headline).toBe("ValueError: bad date");
  });

  it("keeps an epilogue that nothing comes before", () => {
    expect(rewriteFailureText(EPILOGUE)).toBe(EPILOGUE.join("\n"));
  });

  it("keeps a fatal-led stream unchanged", () => {
    expect(rewriteFailureText(["fatal: rewrite denied"])).toBe("fatal: rewrite denied");
  });

  it("keeps a later fatal: line that isn't fast-import's epilogue", () => {
    const stream = ["warning: refname 'main' is ambiguous.", "fatal: rewrite denied"];
    expect(rewriteFailureText(stream)).toBe(stream.join("\n"));
  });
});
