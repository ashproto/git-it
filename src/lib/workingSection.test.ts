import { describe, it, expect } from "vitest";
import { resolveSection, type SectionPresence, type WorkingSection } from "./workingSection";

const present = (p: Partial<SectionPresence> = {}): SectionPresence => ({
  staged: false,
  unstaged: false,
  untracked: false,
  ...p,
});

describe("resolveSection", () => {
  it("keeps the clicked section when the file is still in it", () => {
    // The case the whole type exists for: a partially-staged file is in BOTH lists,
    // and clicking the Unstaged row must not resolve to "staged".
    const both = present({ staged: true, unstaged: true });
    expect(resolveSection("unstaged", both)).toBe("unstaged");
    expect(resolveSection("staged", both)).toBe("staged");
  });

  it("follows the file to Staged after all of it has been staged", () => {
    expect(resolveSection("unstaged", present({ staged: true }))).toBe("staged");
  });

  it("follows the file to Unstaged after it has been unstaged", () => {
    expect(resolveSection("staged", present({ unstaged: true }))).toBe("unstaged");
  });

  it("keeps a merged untracked row in the Unstaged list it is rendered in", () => {
    // "Merge Untracked into Unstaged" renders untracked rows in the Unstaged list, so
    // the row reports "unstaged" AND presence.unstaged is true. Resolving to "untracked"
    // here would name a section that has no rows in that mode, leaving the selected row
    // unhighlighted and the header button acting on the whole list.
    expect(resolveSection("unstaged", present({ unstaged: true, untracked: true }))).toBe(
      "unstaged",
    );
  });

  it("resolves an untracked file to its own section when the lists are separate", () => {
    expect(resolveSection("untracked", present({ untracked: true }))).toBe("untracked");
  });

  it("prefers unstaged over staged when the file somehow left the clicked section and is in both", () => {
    expect(resolveSection("untracked", present({ staged: true, unstaged: true }))).toBe("unstaged");
  });

  it("returns null when the path is in no section", () => {
    expect(resolveSection("unstaged", present())).toBeNull();
    expect(resolveSection(null, present())).toBeNull();
  });

  it("resolves without a clicked section by falling back to whatever holds the file", () => {
    expect(resolveSection(null, present({ staged: true }))).toBe("staged");
    expect(resolveSection(null, present({ untracked: true }))).toBe("untracked");
  });

  it("never returns a section the file is absent from", () => {
    const SECTIONS: (WorkingSection | null)[] = ["staged", "unstaged", "untracked", null];
    for (const clicked of SECTIONS) {
      for (const s of [true, false]) {
        for (const u of [true, false]) {
          for (const t of [true, false]) {
            const p = present({ staged: s, unstaged: u, untracked: t });
            const got = resolveSection(clicked, p);
            if (got === null) {
              expect(s || u || t, `clicked=${clicked} presence=${s}/${u}/${t}`).toBe(false);
            } else {
              expect(p[got], `clicked=${clicked} presence=${s}/${u}/${t} -> ${got}`).toBe(true);
            }
          }
        }
      }
    }
  });
});
