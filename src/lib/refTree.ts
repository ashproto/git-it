import type { RefEntry } from "./types";

// A ref name like `feature/graph-view` folderizes into a `feature` folder
// containing a `graph-view` leaf (Fork / SourceTree style). Remotes nest one
// level deeper (`origin/feature/x`). Pure + deterministic so it can be unit-tested.

export type RefTreeNode =
  | { kind: "leaf"; name: string; ref: RefEntry }
  | { kind: "folder"; name: string; path: string; children: RefTreeNode[] };

export function buildRefTree(refs: RefEntry[]): RefTreeNode[] {
  const root: RefTreeNode[] = [];

  for (const ref of refs) {
    const segments = ref.name.split("/").filter((s) => s.length > 0);
    if (segments.length === 0) continue;

    let level = root;
    let prefix = "";
    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      prefix = prefix ? `${prefix}/${seg}` : seg;
      if (i === segments.length - 1) {
        level.push({ kind: "leaf", name: seg, ref });
      } else {
        let folder = level.find(
          (n): n is Extract<RefTreeNode, { kind: "folder" }> =>
            n.kind === "folder" && n.name === seg,
        );
        if (!folder) {
          folder = { kind: "folder", name: seg, path: prefix, children: [] };
          level.push(folder);
        }
        level = folder.children;
      }
    }
  }

  sortLevel(root);
  return root;
}

// Folders first, then case-insensitive alphabetical by display name.
function sortLevel(nodes: RefTreeNode[]): void {
  nodes.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "folder" ? -1 : 1;
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
  for (const n of nodes) if (n.kind === "folder") sortLevel(n.children);
}

/** Collect every folder path in the tree (used to expand/collapse all). */
export function folderPaths(nodes: RefTreeNode[]): string[] {
  const out: string[] = [];
  for (const n of nodes) {
    if (n.kind === "folder") {
      out.push(n.path);
      out.push(...folderPaths(n.children));
    }
  }
  return out;
}
