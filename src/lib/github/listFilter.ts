// Client-side filtering for the GitHub screen's cached lists (PRs/Issues/Releases).
// Pure and Tauri-free (vitest-covered).

/**
 * Filter `items` by a query: case-insensitive substring match across the given
 * string `fields`. If an item has a numeric `number` property and the trimmed
 * query is `#?digits`, the digits also match `String(item.number)` exactly
 * (so "12" finds #12 but not #120). An empty/whitespace query returns the
 * input array unchanged (same reference).
 */
export function filterItems<T extends Record<string, unknown>>(
  items: T[],
  query: string,
  fields: (keyof T & string)[],
): T[] {
  const q = query.trim();
  if (!q) return items;
  const lower = q.toLowerCase();
  const num = q.match(/^#?(\d+)$/)?.[1] ?? null;
  return items.filter((item) => {
    if (num !== null && typeof item.number === "number" && String(item.number) === num) {
      return true;
    }
    return fields.some((f) => {
      const v = item[f];
      return typeof v === "string" && v.toLowerCase().includes(lower);
    });
  });
}
