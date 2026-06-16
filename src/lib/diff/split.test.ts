import { describe, it, expect } from "vitest";
import { splitDiffFiles } from "./split";

const MULTI = `diff --git a/src/foo.ts b/src/foo.ts
index 1111111..2222222 100644
--- a/src/foo.ts
+++ b/src/foo.ts
@@ -1,2 +1,2 @@
-old line
+new line
 context
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/new.txt
@@ -0,0 +1 @@
+hello
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 4444444..0000000
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/old/name.ts b/new/name.ts
similarity index 100%
rename from old/name.ts
rename to new/name.ts`;

describe("splitDiffFiles", () => {
  it("returns an empty list for an empty patch", () => {
    expect(splitDiffFiles("")).toEqual([]);
  });

  it("splits a multi-file diff into one entry per file, in order", () => {
    const files = splitDiffFiles(MULTI);
    expect(files.map((f) => f.path)).toEqual([
      "src/foo.ts",
      "new.txt",
      "gone.txt",
      "new/name.ts",
    ]);
  });

  it("derives the status of each file", () => {
    const files = splitDiffFiles(MULTI);
    expect(files.map((f) => f.status)).toEqual(["modified", "added", "deleted", "renamed"]);
  });

  it("keeps each file's own raw patch text (starts with its diff --git header)", () => {
    const files = splitDiffFiles(MULTI);
    for (const f of files) expect(f.patch.startsWith("diff --git ")).toBe(true);
    expect(files[0].patch).toContain("-old line");
    expect(files[0].patch).toContain("+new line");
    // the foo chunk must NOT bleed into the next file
    expect(files[0].patch).not.toContain("new.txt");
  });

  it("carries old/new paths for a rename", () => {
    const rename = splitDiffFiles(MULTI)[3];
    expect(rename.oldPath).toBe("old/name.ts");
    expect(rename.newPath).toBe("new/name.ts");
  });

  it("handles a single-file patch", () => {
    const single = splitDiffFiles(MULTI.split("diff --git a/new.txt")[0]);
    expect(single).toHaveLength(1);
    expect(single[0].path).toBe("src/foo.ts");
  });
});
