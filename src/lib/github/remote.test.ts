import { describe, it, expect } from "vitest";
import { parseGithubRemote } from "./remote";

describe("parseGithubRemote", () => {
  it("parses all github url forms to owner/repo", () => {
    for (const url of [
      "https://github.com/cli/cli.git",
      "https://github.com/cli/cli",
      "git@github.com:cli/cli.git",
      "ssh://git@github.com/cli/cli.git",
    ]) {
      expect(parseGithubRemote(url)).toEqual({ owner: "cli", repo: "cli" });
    }
  });

  it("returns null for non-github or malformed urls", () => {
    expect(parseGithubRemote("https://gitlab.com/x/y.git")).toBeNull();
    expect(parseGithubRemote("https://github.com/onlyowner")).toBeNull();
    expect(parseGithubRemote("https://github.com/cli/cli/extra")).toBeNull();
    expect(parseGithubRemote("")).toBeNull();
  });
});
