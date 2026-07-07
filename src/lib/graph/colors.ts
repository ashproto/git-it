export const LANE_PALETTE: string[] = [
  "#378ADD",
  "#1D9E75",
  "#D85A30",
  "#7F77DD",
  "#BA7517",
  "#D4537E",
  "#639922",
  "#5F5E5A",
];

export const NERV_LANE_PALETTE: string[] = [
  "#F2542D", // orange (lead)
  "#46E88B", // phosphor
  "#5AA9E6", // steel-cyan
  "#9B7FE0", // violet
  "#E8A33D", // amber
  "#E0608A", // pink
  "#EAE6DA", // bone
  "#8A94A0", // haze
];

export function laneColor(
  colorIndex: number,
  branchName: string | null,
  overrides: Record<string, string>,
  palette: string[] = LANE_PALETTE,
): string {
  if (branchName && overrides[branchName]) return overrides[branchName];
  // Wrap into range, tolerating negative indices (JS % keeps the operand's sign).
  const i = ((colorIndex % palette.length) + palette.length) % palette.length;
  return palette[i];
}
