// A file path like `src/lib/foo.ts` folderizes into a `src` folder containing a
// `lib` folder containing a `foo.ts` file leaf (Fork / SourceTree style). Generic
// over the item type so both the working-copy and commit-file lists can reuse it.
// Pure + deterministic so it can be unit-tested. Modelled on refTree.ts.

export type FileTreeNode<T> =
  | { kind: "file"; name: string; path: string; item: T }
  | { kind: "folder"; name: string; path: string; children: FileTreeNode<T>[] };

export function buildFileTree<T>(items: T[], getPath: (item: T) => string): FileTreeNode<T>[] {
  const root: FileTreeNode<T>[] = [];

  for (const item of items) {
    const path = getPath(item);
    const segments = path.split("/").filter((s) => s.length > 0);
    if (segments.length === 0) continue;

    let level = root;
    let prefix = "";
    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      prefix = prefix ? `${prefix}/${seg}` : seg;
      if (i === segments.length - 1) {
        level.push({ kind: "file", name: seg, path, item });
      } else {
        let folder = level.find(
          (n): n is Extract<FileTreeNode<T>, { kind: "folder" }> =>
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
function sortLevel<T>(nodes: FileTreeNode<T>[]): void {
  nodes.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "folder" ? -1 : 1;
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
  for (const n of nodes) if (n.kind === "folder") sortLevel(n.children);
}

// A single visual row of the tree once flattened for rendering: either a folder
// header or a file leaf, tagged with its nesting `depth` (top level = 0). Folders
// carry a display `name`; files carry the original `item`. Flattening lets the
// component render the whole tree as ONE keyed list, which is what makes
// `animate:flip` work across the entire tree (a file staged out of a folder slides
// and its neighbours close the gap, instead of the structure popping at the end).
export type FlatFileRow<T> =
  | { kind: "folder"; path: string; name: string; depth: number }
  | { kind: "file"; path: string; item: T; depth: number };

/**
 * Flatten a nested file tree into the ordered list of rows to render, skipping the
 * children of any folder whose path is in `collapsed`. Pure + deterministic.
 */
export function flattenFileTree<T>(
  nodes: FileTreeNode<T>[],
  collapsed: Set<string>,
  depth = 0,
  out: FlatFileRow<T>[] = [],
): FlatFileRow<T>[] {
  for (const node of nodes) {
    if (node.kind === "folder") {
      out.push({ kind: "folder", path: node.path, name: node.name, depth });
      if (!collapsed.has(node.path)) {
        flattenFileTree(node.children, collapsed, depth + 1, out);
      }
    } else {
      out.push({ kind: "file", path: node.path, item: node.item, depth });
    }
  }
  return out;
}

/** Collect every folder path in the tree (used to expand/collapse all). */
export function fileFolderPaths<T>(nodes: FileTreeNode<T>[]): string[] {
  const out: string[] = [];
  for (const n of nodes) {
    if (n.kind === "folder") {
      out.push(n.path);
      out.push(...fileFolderPaths(n.children));
    }
  }
  return out;
}
