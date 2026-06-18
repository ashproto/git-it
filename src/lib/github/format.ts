/** Compact a count for a stat tile: 1234 → "1.2k", 44882 → "44.9k", 2.5e6 → "2.5M". */
export function formatCompact(n: number): string {
  if (!Number.isFinite(n)) return "0";
  const abs = Math.abs(n);
  if (abs < 1000) return String(n);
  if (abs < 1_000_000) return trimOne(n / 1000) + "k";
  return trimOne(n / 1_000_000) + "M";
}

function trimOne(x: number): string {
  return (Math.round(x * 10) / 10).toString();
}
