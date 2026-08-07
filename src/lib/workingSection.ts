/**
 * Which list of the working-copy screen a file was selected in.
 *
 * A partially-staged file (`MM`) appears in BOTH the Staged and the Unstaged list, so a
 * path alone cannot say which half the user opened — and the two halves need different
 * diffs and different actions (Unstage on one side, Stage/Discard on the other). Rows
 * therefore carry their section, and the selection records it.
 *
 * "untracked" is a section, not a file property: with "Merge Untracked into Unstaged"
 * enabled, untracked files are rendered in the Unstaged list and their rows report
 * `"unstaged"`. Whether a file needs the `--no-index` diff is a separate question,
 * answered by the file itself.
 */
export type WorkingSection = "staged" | "unstaged" | "untracked";

/** Which sections currently contain a given path. */
export interface SectionPresence {
  staged: boolean;
  unstaged: boolean;
  untracked: boolean;
}

/**
 * The section the diff pane should actually show for the selected file.
 *
 * Normally this is just the section the user clicked. But the file can leave that
 * section under them — staging all of a partially-staged file removes it from Unstaged,
 * and unstaging removes it from Staged. Rather than stranding the pane on an empty diff
 * with no visible selected row, follow the file to whichever section still holds it.
 *
 * Returns null when the path is in no section at all (it was committed, discarded, or
 * the refresh dropped it), which callers render as "no selection".
 */
export function resolveSection(
  clicked: WorkingSection | null,
  present: SectionPresence,
): WorkingSection | null {
  if (clicked !== null && present[clicked]) return clicked;
  // Preference order matters only for a file that left its section. Unstaged first so
  // that unstaging a file lands on the half that now holds the change; a file that left
  // Unstaged (fully staged) falls through to Staged.
  if (present.unstaged) return "unstaged";
  if (present.untracked) return "untracked";
  if (present.staged) return "staged";
  return null;
}
