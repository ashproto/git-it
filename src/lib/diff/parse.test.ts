import { describe, it, expect } from "vitest";
import { parseDiff } from "./parse";

// ─── helpers ──────────────────────────────────────────────────────────────────

/** All lines of a given kind from the first hunk of the first file */
function lineKinds(patch: string): string[] {
  const d = parseDiff(patch);
  return d.files[0]?.hunks[0]?.lines.map((l) => l.kind) ?? [];
}

// ─── single-file modify ───────────────────────────────────────────────────────

describe("parseDiff — single-file modify", () => {
  const PATCH = `diff --git a/src/foo.ts b/src/foo.ts
index abc1234..def5678 100644
--- a/src/foo.ts
+++ b/src/foo.ts
@@ -1,4 +1,4 @@
 line1
-line2
+LINE2
 line3
 line4
`;

  it("produces one file", () => {
    expect(parseDiff(PATCH).files).toHaveLength(1);
  });

  it("sets old/new paths", () => {
    const f = parseDiff(PATCH).files[0];
    expect(f.oldPath).toBe("src/foo.ts");
    expect(f.newPath).toBe("src/foo.ts");
  });

  it("detects language from extension", () => {
    expect(parseDiff(PATCH).files[0].language).toBe("typescript");
  });

  it("is not binary", () => {
    expect(parseDiff(PATCH).files[0].binary).toBe(false);
  });

  it("produces one hunk", () => {
    expect(parseDiff(PATCH).files[0].hunks).toHaveLength(1);
  });

  it("correct line kinds: context/del/add/context/context", () => {
    expect(lineKinds(PATCH)).toEqual(["context", "del", "add", "context", "context"]);
  });

  it("correct old line numbers for del/context lines", () => {
    const lines = parseDiff(PATCH).files[0].hunks[0].lines;
    // -1,4: old starts at 1
    expect(lines[0].oldNo).toBe(1); // context line1
    expect(lines[1].oldNo).toBe(2); // del line2
    expect(lines[2].oldNo).toBeNull(); // add LINE2 (no old line number)
    expect(lines[3].oldNo).toBe(3); // context line3
    expect(lines[4].oldNo).toBe(4); // context line4
  });

  it("correct new line numbers for add/context lines", () => {
    const lines = parseDiff(PATCH).files[0].hunks[0].lines;
    // +1,4: new starts at 1
    expect(lines[0].newNo).toBe(1); // context line1
    expect(lines[1].newNo).toBeNull(); // del line2 (no new line number)
    expect(lines[2].newNo).toBe(2); // add LINE2
    expect(lines[3].newNo).toBe(3); // context line3
    expect(lines[4].newNo).toBe(4); // context line4
  });

  it("line text strips the leading sigil", () => {
    const lines = parseDiff(PATCH).files[0].hunks[0].lines;
    expect(lines[0].text).toBe("line1");
    expect(lines[1].text).toBe("line2");
    expect(lines[2].text).toBe("LINE2");
  });
});

// ─── multi-hunk ───────────────────────────────────────────────────────────────

describe("parseDiff — multi-hunk file", () => {
  const PATCH = `diff --git a/util.rs b/util.rs
index 000..111 100644
--- a/util.rs
+++ b/util.rs
@@ -1,3 +1,3 @@
 alpha
-beta
+BETA
 gamma
@@ -10,3 +10,3 @@
 delta
-epsilon
+EPSILON
 zeta
`;

  it("produces two hunks", () => {
    expect(parseDiff(PATCH).files[0].hunks).toHaveLength(2);
  });

  it("second hunk starts at correct old line number", () => {
    const hunk = parseDiff(PATCH).files[0].hunks[1];
    const ctx = hunk.lines.find((l) => l.kind === "context")!;
    expect(ctx.oldNo).toBe(10);
  });

  it("language detected from .rs extension", () => {
    expect(parseDiff(PATCH).files[0].language).toBe("rust");
  });
});

// ─── added file (/dev/null → real path) ──────────────────────────────────────

describe("parseDiff — added file", () => {
  const PATCH = `diff --git a/new.py b/new.py
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/new.py
@@ -0,0 +1,2 @@
+hello
+world
`;

  it("oldPath is /dev/null", () => {
    expect(parseDiff(PATCH).files[0].oldPath).toBe("/dev/null");
  });

  it("newPath is the real path", () => {
    expect(parseDiff(PATCH).files[0].newPath).toBe("new.py");
  });

  it("all lines are add", () => {
    const kinds = parseDiff(PATCH).files[0].hunks[0].lines.map((l) => l.kind);
    expect(kinds).toEqual(["add", "add"]);
  });

  it("language detected from .py", () => {
    expect(parseDiff(PATCH).files[0].language).toBe("python");
  });

  it("add lines have no oldNo", () => {
    const lines = parseDiff(PATCH).files[0].hunks[0].lines;
    expect(lines[0].oldNo).toBeNull();
    expect(lines[1].oldNo).toBeNull();
  });

  it("add lines have correct newNo", () => {
    const lines = parseDiff(PATCH).files[0].hunks[0].lines;
    expect(lines[0].newNo).toBe(1);
    expect(lines[1].newNo).toBe(2);
  });
});

// ─── deleted file (real path → /dev/null) ────────────────────────────────────

describe("parseDiff — deleted file", () => {
  const PATCH = `diff --git a/gone.json b/gone.json
deleted file mode 100644
index abc1234..0000000
--- a/gone.json
+++ /dev/null
@@ -1,2 +0,0 @@
-{"a":1}
-{"b":2}
`;

  it("newPath is /dev/null", () => {
    expect(parseDiff(PATCH).files[0].newPath).toBe("/dev/null");
  });

  it("oldPath is the real path", () => {
    expect(parseDiff(PATCH).files[0].oldPath).toBe("gone.json");
  });

  it("all lines are del", () => {
    const kinds = parseDiff(PATCH).files[0].hunks[0].lines.map((l) => l.kind);
    expect(kinds).toEqual(["del", "del"]);
  });

  it("language detected from .json via old path", () => {
    expect(parseDiff(PATCH).files[0].language).toBe("json");
  });
});

// ─── binary file ──────────────────────────────────────────────────────────────

describe("parseDiff — binary file", () => {
  const PATCH = `diff --git a/icon.png b/icon.png
index abc1234..def5678 100644
Binary files a/icon.png and b/icon.png differ
`;

  it("binary:true", () => {
    expect(parseDiff(PATCH).files[0].binary).toBe(true);
  });

  it("no hunks", () => {
    expect(parseDiff(PATCH).files[0].hunks).toHaveLength(0);
  });

  it("paths correct", () => {
    const f = parseDiff(PATCH).files[0];
    expect(f.oldPath).toBe("icon.png");
    expect(f.newPath).toBe("icon.png");
  });
});

// ─── rename header ────────────────────────────────────────────────────────────

describe("parseDiff — rename", () => {
  const PATCH = `diff --git a/old-name.ts b/new-name.ts
similarity index 95%
rename from old-name.ts
rename to new-name.ts
index abc..def 100644
--- a/old-name.ts
+++ b/new-name.ts
@@ -1,2 +1,2 @@
 same
-old line
+new line
`;

  it("oldPath from --- header", () => {
    expect(parseDiff(PATCH).files[0].oldPath).toBe("old-name.ts");
  });

  it("newPath from +++ header", () => {
    expect(parseDiff(PATCH).files[0].newPath).toBe("new-name.ts");
  });

  it("language detected from new path extension", () => {
    expect(parseDiff(PATCH).files[0].language).toBe("typescript");
  });
});

// ─── language detection by extension ─────────────────────────────────────────

describe("parseDiff — language detection", () => {
  function patchFor(filename: string): string {
    return `diff --git a/${filename} b/${filename}
--- a/${filename}
+++ b/${filename}
@@ -1 +1 @@
-x
+y
`;
  }

  const cases: [string, string][] = [
    ["app.ts", "typescript"],
    ["app.tsx", "tsx"],
    ["app.js", "javascript"],
    ["app.jsx", "jsx"],
    ["main.rs", "rust"],
    ["script.py", "python"],
    ["data.json", "json"],
    ["page.html", "html"],
    ["style.css", "css"],
    ["comp.svelte", "svelte"],
    ["README.md", "markdown"],
    ["run.sh", "bash"],
    ["config.yml", "yaml"],
    ["config.yaml", "yaml"],
    ["Cargo.toml", "toml"],
    ["main.go", "go"],
    ["lib.c", "c"],
    ["lib.h", "c"],
    ["lib.cpp", "cpp"],
    ["App.java", "java"],
    ["app.rb", "ruby"],
    ["script.php", "php"],
    ["query.sql", "sql"],
    ["mystery.xyz", "text"],
  ];

  for (const [filename, expectedLang] of cases) {
    it(`${filename} → ${expectedLang}`, () => {
      expect(parseDiff(patchFor(filename)).files[0].language).toBe(expectedLang);
    });
  }
});

// ─── no-newline marker ────────────────────────────────────────────────────────

describe("parseDiff — no newline at end of file marker", () => {
  const PATCH = `diff --git a/no-nl.txt b/no-nl.txt
--- a/no-nl.txt
+++ b/no-nl.txt
@@ -1,2 +1,2 @@
 context
-old line
\\ No newline at end of file
+new line
\\ No newline at end of file
`;

  it("ignores the no-newline marker lines", () => {
    const lines = parseDiff(PATCH).files[0].hunks[0].lines;
    expect(lines).toHaveLength(3);
    expect(lines.every((l) => !l.text.startsWith("\\"))).toBe(true);
  });

  it("correct kinds without marker", () => {
    const kinds = parseDiff(PATCH).files[0].hunks[0].lines.map((l) => l.kind);
    expect(kinds).toEqual(["context", "del", "add"]);
  });
});

// ─── malformed / empty patch ──────────────────────────────────────────────────

describe("parseDiff — malformed/empty patch", () => {
  it("empty string → empty files, no crash", () => {
    expect(() => parseDiff("")).not.toThrow();
    expect(parseDiff("").files).toHaveLength(0);
  });

  it("random garbage → empty files, no crash", () => {
    expect(() => parseDiff("this is not a diff at all\ngarbage")).not.toThrow();
    expect(parseDiff("this is not a diff at all\ngarbage").files).toHaveLength(0);
  });

  it("diff header with no hunks → one file, no hunks", () => {
    const patch = `diff --git a/empty.txt b/empty.txt
index abc..def 100644
`;
    expect(() => parseDiff(patch)).not.toThrow();
    const files = parseDiff(patch).files;
    expect(files).toHaveLength(1);
    expect(files[0].hunks).toHaveLength(0);
  });

  it("truncated hunk header → no crash", () => {
    const patch = `diff --git a/t.ts b/t.ts
--- a/t.ts
+++ b/t.ts
@@ -1`;
    expect(() => parseDiff(patch)).not.toThrow();
  });

  it("hunk with no body lines → empty lines array", () => {
    const patch = `diff --git a/t.ts b/t.ts
--- a/t.ts
+++ b/t.ts
@@ -1,0 +1,0 @@
`;
    const lines = parseDiff(patch).files[0].hunks[0].lines;
    expect(lines).toHaveLength(0);
  });
});

// ─── multi-file patch ─────────────────────────────────────────────────────────

describe("parseDiff — multi-file patch", () => {
  const PATCH = `diff --git a/a.ts b/a.ts
--- a/a.ts
+++ b/a.ts
@@ -1 +1 @@
-old
+new
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -1 +1 @@
-uno
+dos
`;

  it("produces two files", () => {
    expect(parseDiff(PATCH).files).toHaveLength(2);
  });

  it("each file has correct language", () => {
    const langs = parseDiff(PATCH).files.map((f) => f.language);
    expect(langs).toEqual(["typescript", "rust"]);
  });
});
