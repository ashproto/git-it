import { describe, it, expect } from "vitest";
import { filterItems } from "./listFilter";

const pulls = [
  { number: 12, title: "Fix crash on launch", author: "ash" },
  { number: 34, title: "Add dark mode", author: "bob" },
];
const releases = [{ name: "v1.2 — Spring", tagName: "v1.2.0" }, { name: "Beta", tagName: "v0.9" }];

describe("filterItems", () => {
  it("filters by title substring, case-insensitive", () => {
    expect(filterItems(pulls, "crash", ["title", "author"])).toEqual([pulls[0]]);
  });
  it("filters by author", () => {
    expect(filterItems(pulls, "BOB", ["title", "author"])).toEqual([pulls[1]]);
  });
  it("matches #number and bare number", () => {
    expect(filterItems(pulls, "#34", ["title", "author"])).toEqual([pulls[1]]);
    expect(filterItems(pulls, "12", ["title", "author"])).toEqual([pulls[0]]);
  });
  it("releases: name + tagName fields", () => {
    expect(filterItems(releases, "v1.2", ["name", "tagName"])).toEqual([releases[0]]);
  });
  it("empty query returns the input array", () => {
    expect(filterItems(pulls, " ", ["title"])).toEqual(pulls);
  });
});
