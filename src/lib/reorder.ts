// Pure list-reorder helpers for drag-to-reorder UIs (repo tabs). Tauri-free,
// unit-tested in reorder.test.ts.

/**
 * Remove the item at `from`, then insert it so it lands at index `to` of the
 * RESULT. Same-index or out-of-range calls return an unchanged copy. Never
 * mutates the input.
 */
export function reorder<T>(list: T[], from: number, to: number): T[] {
  const next = [...list];
  if (from === to) return next;
  if (from < 0 || from >= list.length || to < 0 || to >= list.length) return next;
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

/**
 * Map a visual insertion GAP (0..n, counting the slots between/around n tabs)
 * to the `to` index `reorder()` expects. Gaps `from` and `from + 1` surround
 * the dragged item itself, so both map to `from` (identity drop).
 */
export function gapToIndex(from: number, gap: number): number {
  return gap > from ? gap - 1 : gap;
}
