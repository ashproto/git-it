// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { mount, type Component } from "svelte";
import Skeleton from "./Skeleton.svelte";
import GithubSkeleton from "./GithubSkeleton.svelte";

function render<Props extends Record<string, unknown>>(Comp: Component<Props>, props: Props) {
  const target = document.createElement("div");
  document.body.appendChild(target);
  mount(Comp, { target, props });
  return target;
}

describe("Skeleton", () => {
  it("renders a decorative .sk block with the given width", () => {
    const target = render(Skeleton, { w: "50px", h: "10px" });
    const sk = target.querySelector(".sk") as HTMLElement;
    expect(sk).not.toBeNull();
    expect(sk.style.width).toBe("50px");
    expect(sk.getAttribute("aria-hidden")).toBe("true");
  });

  it("uses a circular radius when circle is set", () => {
    const target = render(Skeleton, { circle: true });
    const sk = target.querySelector(".sk") as HTMLElement;
    expect(sk.style.borderRadius).toBe("50%");
  });
});

describe("GithubSkeleton", () => {
  const variants = [
    "header",
    "overview",
    "list",
    "releases",
    "actions",
    "card",
    "detail",
  ] as const;

  for (const variant of variants) {
    it(`renders the ${variant} variant with shimmer blocks`, () => {
      const target = render(GithubSkeleton, { variant });
      expect(target.querySelectorAll(".sk").length).toBeGreaterThan(0);
      // Single-instance variants announce loading; the `card` variant is
      // rendered in multiples (Insights), so it stays decorative.
      const hasStatus = target.querySelector("[role='status']") !== null;
      expect(hasStatus).toBe(variant !== "card");
    });
  }

  it("renders the requested number of list rows", () => {
    const target = render(GithubSkeleton, { variant: "list", rows: 3 });
    expect(target.querySelectorAll(".row").length).toBe(3);
  });
});
