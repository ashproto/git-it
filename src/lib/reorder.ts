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
 * Insertion index for a pointer at `pointerX` given ascending tab `midpoints`
 * (x of each tab's centre). Returns the first index whose midpoint is right of
 * the pointer, clamped to the last index when the pointer is past every tab.
 */
export function insertionIndex(midpoints: number[], pointerX: number): number {
  for (let i = 0; i < midpoints.length; i++) {
    if (pointerX < midpoints[i]) return i;
  }
  return Math.max(0, midpoints.length - 1);
}
