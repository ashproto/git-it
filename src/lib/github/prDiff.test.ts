import { describe, it, expect } from "vitest";
import { splitPatchByFile } from "./prDiff";

const TWO_FILES = `diff --git a/src/a.ts b/src/a.ts
index 111..222 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -1,2 +1,3 @@
 line1
+added
 line2
diff --git a/README.md b/README.md
index 333..444 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-old
+new
`;

describe("splitPatchByFile", () => {
  it("splits a multi-file patch, keeping each file's full patch text", () => {
    const files = splitPatchByFile(TWO_FILES);
    expect(files.map((f) => f.path)).toEqual(["src/a.ts", "README.md"]);
    expect(files[0].patch).toContain("@@ -1,2 +1,3 @@");
    expect(files[0].patch.startsWith("diff --git")).toBe(true);
    expect(files[1].patch).toContain("-old");
  });
  it("counts additions/deletions per file", () => {
    const files = splitPatchByFile(TWO_FILES);
    expect(files[0]).toMatchObject({ additions: 1, deletions: 0 });
    expect(files[1]).toMatchObject({ additions: 1, deletions: 1 });
  });
  it("handles renames (path = new side)", () => {
    const p = `diff --git a/old.ts b/new.ts
similarity index 90%
rename from old.ts
rename to new.ts
--- a/old.ts
+++ b/new.ts
@@ -1 +1 @@
-x
+y
`;
    const f = splitPatchByFile(p)[0];
    expect(f.path).toBe("new.ts");
    expect(f.oldPath).toBe("old.ts");
  });
  it("handles new and deleted files (/dev/null sides)", () => {
    const p = `diff --git a/gone.ts b/gone.ts
deleted file mode 100644
--- a/gone.ts
+++ /dev/null
@@ -1 +0,0 @@
-bye
`;
    expect(splitPatchByFile(p)[0].path).toBe("gone.ts");
  });
  it("returns [] for empty/whitespace input and keeps binary stubs", () => {
    expect(splitPatchByFile("")).toEqual([]);
    const bin = `diff --git a/img.png b/img.png
Binary files a/img.png and b/img.png differ
`;
    expect(splitPatchByFile(bin)[0]).toMatchObject({ path: "img.png", additions: 0, deletions: 0 });
  });
  it("does not count +++/--- header lines as changes", () => {
    const files = splitPatchByFile(TWO_FILES);
    // file 0 has one "+++" header and one "---" header; only body lines counted
    expect(files[0].additions).toBe(1);
    expect(files[0].deletions).toBe(0);
  });
  it("handles a new file (--- /dev/null → path from +++ side)", () => {
    const p = `diff --git a/fresh.ts b/fresh.ts
new file mode 100644
--- /dev/null
+++ b/fresh.ts
@@ -0,0 +1 @@
+hi
`;
    const f = splitPatchByFile(p)[0];
    expect(f.path).toBe("fresh.ts");
    expect(f.oldPath).toBeNull();
    expect(f).toMatchObject({ additions: 1, deletions: 0 });
  });
  it("returns [] for whitespace-only input", () => {
    expect(splitPatchByFile("\n  \n")).toEqual([]);
  });
  it("oldPath is null for a plain modification", () => {
    const files = splitPatchByFile(TWO_FILES);
    expect(files[0].oldPath).toBeNull();
    expect(files[1].oldPath).toBeNull();
  });
});
