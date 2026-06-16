import { describe, it, expect } from "vitest";
import { buildRefTree, folderPaths, type RefTreeNode } from "./refTree";
import type { RefEntry } from "./types";

const r = (name: string, isHead = false): RefEntry => ({ name, sha: "s_" + name, isHead });

// Compact shape for assertions: folders as { name: [...children] }, leaves as the name string.
function shape(nodes: RefTreeNode[]): unknown {
  return nodes.map((n) =>
    n.kind === "leaf" ? n.name : { [n.name]: shape(n.children) },
  );
}

describe("buildRefTree", () => {
  it("returns an empty tree for no refs", () => {
    expect(buildRefTree([])).toEqual([]);
  });

  it("keeps a slashless ref as a single leaf", () => {
    expect(shape(buildRefTree([r("main")]))).toEqual(["main"]);
  });

  it("groups refs sharing a prefix under one folder", () => {
    const tree = buildRefTree([r("feature/a"), r("feature/b")]);
    expect(shape(tree)).toEqual([{ feature: ["a", "b"] }]);
  });

  it("nests multiple slash levels", () => {
    const tree = buildRefTree([r("feature/sub/c")]);
    expect(shape(tree)).toEqual([{ feature: [{ sub: ["c"] }] }]);
    // folder paths carry the full prefix for collapse keys
    const folder = tree[0] as Extract<RefTreeNode, { kind: "folder" }>;
    expect(folder.path).toBe("feature");
    const sub = folder.children[0] as Extract<RefTreeNode, { kind: "folder" }>;
    expect(sub.path).toBe("feature/sub");
  });

  it("sorts folders before leaves, then case-insensitive alphabetical", () => {
    const tree = buildRefTree([r("main"), r("Zeta/x"), r("feature/a"), r("dev")]);
    // folders (feature, Zeta) before leaves (dev, main); each group alphabetical
    expect(shape(tree)).toEqual([{ feature: ["a"] }, { Zeta: ["x"] }, "dev", "main"]);
  });

  it("nests remote refs one level deeper under the remote name", () => {
    const tree = buildRefTree([r("origin/main"), r("origin/feature/x")]);
    expect(shape(tree)).toEqual([{ origin: [{ feature: ["x"] }, "main"] }]);
  });

  it("preserves the original ref object (and isHead) on the leaf", () => {
    const tree = buildRefTree([r("feature/graph-view", true)]);
    const folder = tree[0] as Extract<RefTreeNode, { kind: "folder" }>;
    const leaf = folder.children[0];
    expect(leaf.kind).toBe("leaf");
    if (leaf.kind === "leaf") {
      expect(leaf.ref.name).toBe("feature/graph-view");
      expect(leaf.ref.isHead).toBe(true);
      expect(leaf.name).toBe("graph-view");
    }
  });

  it("folderPaths lists every folder prefix", () => {
    const tree = buildRefTree([r("a/b/c"), r("a/d"), r("e")]);
    expect(folderPaths(tree).sort()).toEqual(["a", "a/b"]);
  });
});
