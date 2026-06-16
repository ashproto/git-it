import { describe, it, expect } from "vitest";
import { buildFileTree, fileFolderPaths, type FileTreeNode } from "./fileTree";

// Test item type: a minimal { path } object, plus an arbitrary extra field to
// prove the generic carries the whole item through to the leaf.
type Item = { path: string; tag?: string };
const build = (paths: string[]) => buildFileTree(paths.map((p) => ({ path: p })), (i) => i.path);

// Narrowing helpers for terse assertions.
const folder = (n: FileTreeNode<Item>) => {
  if (n.kind !== "folder") throw new Error(`expected folder, got ${n.kind}`);
  return n;
};
const file = (n: FileTreeNode<Item>) => {
  if (n.kind !== "file") throw new Error(`expected file, got ${n.kind}`);
  return n;
};
const names = (nodes: FileTreeNode<Item>[]) => nodes.map((n) => n.name);

describe("buildFileTree", () => {
  it("returns an empty array for empty input", () => {
    expect(build([])).toEqual([]);
  });

  it("places a single top-level file as a file leaf carrying the item", () => {
    const item: Item = { path: "README.md", tag: "x" };
    const tree = buildFileTree([item], (i) => i.path);
    expect(tree.length).toBe(1);
    const leaf = file(tree[0]);
    expect(leaf.kind).toBe("file");
    expect(leaf.name).toBe("README.md");
    expect(leaf.path).toBe("README.md");
    expect(leaf.item).toBe(item); // same reference carried through
  });

  it("folderizes a nested path into nested folders with a file leaf", () => {
    const tree = build(["src/lib/foo.ts"]);
    const src = folder(tree[0]);
    expect(src.name).toBe("src");
    expect(src.path).toBe("src");
    const lib = folder(src.children[0]);
    expect(lib.name).toBe("lib");
    expect(lib.path).toBe("src/lib"); // accumulated prefix
    const foo = file(lib.children[0]);
    expect(foo.name).toBe("foo.ts");
    expect(foo.path).toBe("src/lib/foo.ts");
  });

  it("merges multiple files sharing a folder instead of duplicating it", () => {
    const tree = build(["src/a.ts", "src/b.ts"]);
    expect(tree.length).toBe(1); // single shared `src` folder
    const src = folder(tree[0]);
    expect(src.children.map((c) => c.name)).toEqual(["a.ts", "b.ts"]);
    expect(src.children.every((c) => c.kind === "file")).toBe(true);
  });

  it("merges shared intermediate folders across deep paths", () => {
    const tree = build(["src/lib/a.ts", "src/lib/b.ts", "src/main.ts"]);
    const src = folder(tree[0]);
    // lib folder (sorts before the main.ts file), then main.ts
    expect(names(src.children)).toEqual(["lib", "main.ts"]);
    const lib = folder(src.children[0]);
    expect(names(lib.children)).toEqual(["a.ts", "b.ts"]);
  });

  it("orders folders before files at each level", () => {
    // `zfolder` (a folder) must come before `app.ts` (a file) despite z > a.
    const tree = build(["app.ts", "zfolder/x.ts"]);
    expect(tree[0].kind).toBe("folder");
    expect(tree[0].name).toBe("zfolder");
    expect(tree[1].kind).toBe("file");
    expect(tree[1].name).toBe("app.ts");
  });

  it("sorts case-insensitively by name", () => {
    const tree = build(["Banana.ts", "apple.ts", "Cherry.ts"]);
    expect(names(tree)).toEqual(["apple.ts", "Banana.ts", "Cherry.ts"]);
  });

  it("sorts folders case-insensitively too", () => {
    const tree = build(["Zebra/a.ts", "apple/a.ts", "Mango/a.ts"]);
    expect(names(tree)).toEqual(["apple", "Mango", "Zebra"]);
  });

  it("handles deep nesting", () => {
    const tree = build(["a/b/c/d/e.ts"]);
    let node: FileTreeNode<Item> = tree[0];
    for (const seg of ["a", "b", "c", "d"]) {
      const f = folder(node);
      expect(f.name).toBe(seg);
      node = f.children[0];
    }
    const leaf = file(node);
    expect(leaf.name).toBe("e.ts");
    expect(leaf.path).toBe("a/b/c/d/e.ts");
  });
});

describe("fileFolderPaths", () => {
  it("returns an empty array when there are no folders", () => {
    expect(fileFolderPaths(build(["a.ts", "b.ts"]))).toEqual([]);
  });

  it("returns every folder path in the tree", () => {
    const tree = build(["src/lib/a.ts", "src/lib/b.ts", "src/main.ts", "test/t.ts", "top.ts"]);
    const paths = fileFolderPaths(tree).sort();
    expect(paths).toEqual(["src", "src/lib", "test"]);
  });
});
