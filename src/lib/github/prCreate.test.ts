import { describe, it, expect } from "vitest";
import { humanizeBranch, prefillFromSubjects } from "./prCreate";

describe("humanizeBranch", () => {
  it("strips the prefix up to the last slash and de-kebabs", () => {
    expect(humanizeBranch("feature/fix-foo-bar")).toBe("Fix foo bar");
  });

  it("replaces underscores with spaces", () => {
    expect(humanizeBranch("fix_thing")).toBe("Fix thing");
  });

  it("capitalizes only the first letter of a plain branch name", () => {
    expect(humanizeBranch("main")).toBe("Main");
  });

  it("handles nested slashes by keeping only the last segment", () => {
    expect(humanizeBranch("user/ash/add-new-thing")).toBe("Add new thing");
  });

  it("collapses runs of separators/whitespace to single spaces", () => {
    expect(humanizeBranch("fix--double__sep")).toBe("Fix double sep");
  });

  it("returns empty string for an empty branch name", () => {
    expect(humanizeBranch("")).toBe("");
  });

  it("returns empty string when the branch ends in a slash", () => {
    expect(humanizeBranch("feature/")).toBe("");
  });
});

describe("prefillFromSubjects", () => {
  it("0 subjects → humanized branch title, empty body", () => {
    expect(prefillFromSubjects([], "feature/fix-foo-bar")).toEqual({
      title: "Fix foo bar",
      body: "",
    });
  });

  it("1 subject → that subject as title, empty body", () => {
    expect(prefillFromSubjects(["Fix the flux capacitor"], "feature/fix-foo")).toEqual({
      title: "Fix the flux capacitor",
      body: "",
    });
  });

  it("n subjects → humanized title + bullet list reversed to oldest-first", () => {
    // Input order is newest-first (git log order); the body should read oldest-first.
    expect(
      prefillFromSubjects(["third change", "second change", "first change"], "feat/big_thing"),
    ).toEqual({
      title: "Big thing",
      body: "- first change\n- second change\n- third change",
    });
  });

  it("empty branch string falls back gracefully to an empty title", () => {
    expect(prefillFromSubjects([], "")).toEqual({ title: "", body: "" });
  });
});
