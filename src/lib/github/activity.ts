/** Map a numeric series to an SVG polyline `points` string in a `w`×`h` box.
 *  The max value sits at the top (y=0); an all-equal series sits on y=0. */
export function sparklinePoints(values: number[], w: number, h: number): string {
  if (values.length === 0) return "";
  const max = Math.max(1, ...values);
  const dx = values.length > 1 ? w / (values.length - 1) : 0;
  return values
    .map((v, i) => `${(i * dx).toFixed(1)},${(h - (v / max) * h).toFixed(1)}`)
    .join(" ");
}
