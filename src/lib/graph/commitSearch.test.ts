import { describe, it, expect } from "vitest";
import { matchCommits } from "./commitSearch";
import type { GraphCommit } from "../types";

const C = (sha: string, subject: string, author: string): GraphCommit => ({
  sha,
  parents: [],
  author_name: author,
  author_email: "",
  author_date: "",
  committer_name: "",
  committer_date: "",
  refs: [],
  subject,
  body: "",
});

const commits = [
  C("a1b2c3d4e5", "Fix login bug", "Ash"),
  C("f6e5d4c3b2", "Add search feature", "Bob"),
  C("09876fedcb", "fix Search index", "ash shah"),
];

describe("matchCommits", () => {
  it("matches subject case-insensitively", () => {
    expect(matchCommits(commits, "search")).toEqual([1, 2]);
  });
  it("matches author name", () => {
    expect(matchCommits(commits, "ash")).toEqual([0, 2]);
  });
  it("matches SHA prefix only when the query is hex-like (≥4 chars)", () => {
    expect(matchCommits(commits, "a1b2")).toEqual([0]);
    expect(matchCommits(commits, "a1")).toEqual([]); // too short for SHA, no text match
  });
  it("empty/whitespace query matches nothing", () => {
    expect(matchCommits(commits, "")).toEqual([]);
    expect(matchCommits(commits, "  ")).toEqual([]);
  });
  it("no matches → empty array", () => {
    expect(matchCommits(commits, "zzz")).toEqual([]);
  });
});
