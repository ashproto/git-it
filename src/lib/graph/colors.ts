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

export function laneColor(
  colorIndex: number,
  branchName: string | null,
  overrides: Record<string, string>,
): string {
  if (branchName && overrides[branchName]) return overrides[branchName];
  return LANE_PALETTE[colorIndex % LANE_PALETTE.length];
}
