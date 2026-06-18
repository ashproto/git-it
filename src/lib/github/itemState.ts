import type { GhPull, GhIssue } from "../types";

export type ItemStateKey = "open" | "merged" | "closed" | "draft" | "not-planned";

export interface StateBadge {
  key: ItemStateKey;
  label: string;
  /** Hex colour (GitHub's state palette); used as a filled badge + row tint. */
  color: string;
}

// GitHub's state colours (chosen to read on both the light and dark glass).
const OPEN = "#1f883d"; // green
const PURPLE = "#8957e5"; // merged PR / completed issue
const RED = "#cf222e"; // closed (unmerged) PR
const GRAY = "#6e7781"; // draft PR / closed-as-not-planned issue

/** Map a pull request to its GitHub-style state badge. */
export function pullStateBadge(pr: Pick<GhPull, "state" | "isDraft">): StateBadge {
  if (pr.isDraft && pr.state === "OPEN") return { key: "draft", label: "Draft", color: GRAY };
  if (pr.state === "MERGED") return { key: "merged", label: "Merged", color: PURPLE };
  if (pr.state === "CLOSED") return { key: "closed", label: "Closed", color: RED };
  return { key: "open", label: "Open", color: OPEN };
}

/** Map an issue to its GitHub-style state badge. Closed splits into completed
 *  (purple) vs not-planned (gray), matching github.com. */
export function issueStateBadge(it: Pick<GhIssue, "state" | "stateReason">): StateBadge {
  if (it.state === "CLOSED") {
    if ((it.stateReason ?? "").toUpperCase() === "NOT_PLANNED")
      return { key: "not-planned", label: "Closed", color: GRAY };
    return { key: "closed", label: "Closed", color: PURPLE };
  }
  return { key: "open", label: "Open", color: OPEN };
}
