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

export const NERV_LANE_PALETTE: readonly string[] = Object.freeze([
  "#F2542D", // orange (lead)
  "#46E88B", // phosphor
  "#5AA9E6", // steel-cyan
  "#9B7FE0", // violet
  "#E8A33D", // amber
  "#E0608A", // pink
  "#EAE6DA", // bone
  "#8A94A0", // haze
]);

export type Scheme = "orange" | "phosphor" | "steel" | "amber" | "violet" | "crimson";
// Each leads with the scheme accent; the rest are shared NERV secondaries (tuned in QA).
export const NERV_SCHEME_PALETTES: Readonly<Record<Scheme, readonly string[]>> = Object.freeze({
  orange: NERV_LANE_PALETTE,
  phosphor: Object.freeze(["#46E88B", "#F2542D", "#5AA9E6", "#9B7FE0", "#E8A33D", "#E0608A", "#EAE6DA", "#8A94A0"]),
  steel: Object.freeze(["#5AA9E6", "#46E88B", "#F2542D", "#9B7FE0", "#E8A33D", "#E0608A", "#EAE6DA", "#8A94A0"]),
  amber: Object.freeze(["#E8A33D", "#46E88B", "#5AA9E6", "#9B7FE0", "#F2542D", "#E0608A", "#EAE6DA", "#8A94A0"]),
  violet: Object.freeze(["#9B7FE0", "#46E88B", "#5AA9E6", "#E8A33D", "#F2542D", "#E0608A", "#EAE6DA", "#8A94A0"]),
  crimson: Object.freeze(["#E0445A", "#46E88B", "#5AA9E6", "#9B7FE0", "#E8A33D", "#EAE6DA", "#8A94A0", "#F2542D"]),
});

export function schemeLanePalette(scheme: Scheme): readonly string[] {
  return NERV_SCHEME_PALETTES[scheme] ?? NERV_LANE_PALETTE;
}

export function laneColor(
  colorIndex: number,
  branchName: string | null,
  overrides: Record<string, string>,
  palette: readonly string[] = LANE_PALETTE,
): string {
  if (branchName && overrides[branchName]) return overrides[branchName];
  // Wrap into range, tolerating negative indices (JS % keeps the operand's sign).
  const i = ((colorIndex % palette.length) + palette.length) % palette.length;
  return palette[i];
}
