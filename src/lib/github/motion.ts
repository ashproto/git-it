import { fly } from "svelte/transition";
import type { FlyParams } from "svelte/transition";
import type { TransitionConfig } from "svelte/transition";

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** A subtle "content settles in" transition: a short fade + 6px rise.
 *  Becomes a no-op when the user prefers reduced motion. Use as `in:revealIn`. */
export function revealIn(node: Element, params?: FlyParams): TransitionConfig {
  if (prefersReducedMotion()) return { duration: 0 };
  return fly(node, { y: 6, duration: 220, ...params });
}
