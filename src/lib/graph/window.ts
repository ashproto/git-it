export interface VisibleWindow {
  /** First commit index to render (inclusive). */
  start: number;
  /** One past the last commit index to render (exclusive). */
  end: number;
}

/**
 * Compute the half-open range `[start, end)` of commit rows that should be in
 * the DOM for a fixed-row-height virtual list. Pure: the only inputs are the
 * measured scroll geometry; the caller maps the range back to spacer heights.
 *
 * The list is modelled as `total` rows of exactly `rowHeight` px stacked from
 * y=0. `scrolledPx` is how far the *first* row has scrolled above the first
 * visible line (so the top visible row index is `floor(scrolledPx / rowHeight)`);
 * it can be negative when a sticky header still occupies the top of the
 * scroller and the list hasn't reached that edge. `buffer` overscans that many
 * rows above and below so fast scrolling never exposes an unrendered gap.
 */
export function commitWindow(
  scrolledPx: number,
  viewportPx: number,
  rowHeight: number,
  total: number,
  buffer: number,
): VisibleWindow {
  if (total <= 0 || rowHeight <= 0) return { start: 0, end: 0 };
  const safeViewport = Math.max(0, viewportPx);
  const firstVisible = Math.floor(scrolledPx / rowHeight);
  const lastVisible = Math.ceil((scrolledPx + safeViewport) / rowHeight);
  const start = Math.min(total, Math.max(0, firstVisible - buffer));
  const end = Math.min(total, Math.max(start, lastVisible + buffer));
  return { start, end };
}
