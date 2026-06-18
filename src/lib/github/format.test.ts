import { describe, it, expect } from "vitest";
import { formatCompact } from "./format";

describe("formatCompact", () => {
  it("leaves sub-thousands as-is", () => {
    expect(formatCompact(0)).toBe("0");
    expect(formatCompact(999)).toBe("999");
  });
  it("compacts thousands and millions", () => {
    expect(formatCompact(1000)).toBe("1k");
    expect(formatCompact(1234)).toBe("1.2k");
    expect(formatCompact(44882)).toBe("44.9k");
    expect(formatCompact(1_000_000)).toBe("1M");
    expect(formatCompact(2_500_000)).toBe("2.5M");
  });
  it("is defensive about non-finite input", () => {
    expect(formatCompact(NaN)).toBe("0");
  });
});
