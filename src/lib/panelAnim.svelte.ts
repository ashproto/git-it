// Shared "a collapsible panel is mid-collapse" flag, ref-counted so overlapping
// animations are handled. While active, the commit graph FREEZES its row-virtualization
// recompute — otherwise its ResizeObserver fires every frame of the height animation and
// re-renders the windowed rows + rebuilds the lane SVG, which makes the animation choppy.
// The graph does one recompute when the flag clears (see GraphHistory).
//
// Ref-counting is kept balanced by CollapsiblePanel (begin once per animation, end on
// finish AND in onDestroy), so it can't get stuck "active". GraphHistory additionally
// time-bounds the freeze as a belt-and-suspenders.

let count = $state(0);

export const panelAnim = {
  get active() {
    return count > 0;
  },
  begin() {
    count++;
  },
  end() {
    count = Math.max(0, count - 1);
  },
};
