import { describe, it, expect } from "vitest";
import { formatCommitDate, relativeDayLabel, formatTzOffset } from "./dates";

// Base date: June 15 2026, 14:30:05 local time.
const d = new Date(2026, 5, 15, 14, 30, 5);
const prefs = { hour12: false, weekday: false, monthName: false };

describe("relativeDayLabel", () => {
  it("returns 'Today' when now is the same day", () => {
    expect(relativeDayLabel(d, d)).toBe("Today");
  });

  it("returns 'Yesterday' when now is the next calendar day", () => {
    const nextDay = new Date(2026, 5, 16);
    expect(relativeDayLabel(d, nextDay)).toBe("Yesterday");
  });

  it("returns null when now is two or more days later", () => {
    const twoDaysLater = new Date(2026, 5, 17);
    expect(relativeDayLabel(d, twoDaysLater)).toBeNull();
  });
});

describe("formatCommitDate — relative mode", () => {
  it("renders 'Today …' when now is the same day", () => {
    const result = formatCommitDate(d, prefs, true, d);
    expect(result).toBe(`Today 14:30:05 ${formatTzOffset(d)}`);
  });

  it("renders 'Yesterday …' when now is the next calendar day", () => {
    const now = new Date(2026, 5, 16, 9, 0, 0);
    const result = formatCommitDate(d, prefs, true, now);
    expect(result).toBe(`Yesterday 14:30:05 ${formatTzOffset(d)}`);
  });

  it("falls back to absolute format when now is 2+ days later", () => {
    const now = new Date(2026, 5, 17, 9, 0, 0);
    const result = formatCommitDate(d, prefs, true, now);
    expect(result).toBe(`2026-06-15 14:30:05 ${formatTzOffset(d)}`);
  });
});

describe("formatCommitDate — relative=false (default)", () => {
  it("always returns absolute format even when now=d", () => {
    const result = formatCommitDate(d, prefs, false, d);
    expect(result).toBe(`2026-06-15 14:30:05 ${formatTzOffset(d)}`);
    expect(result).not.toMatch(/^Today/);
  });

  it("returns absolute when called with only 2 args (default relative=false)", () => {
    // Override `now` isn't needed here — just confirm signature is backward-compatible
    const result = formatCommitDate(d, prefs);
    expect(result).toBe(`2026-06-15 14:30:05 ${formatTzOffset(d)}`);
  });
});

describe("formatCommitDate — existing absolute format unchanged", () => {
  it("pins the 24h absolute output byte-for-byte", () => {
    expect(formatCommitDate(d, { hour12: false, weekday: false, monthName: false })).toBe(
      `2026-06-15 14:30:05 ${formatTzOffset(d)}`,
    );
  });

  it("renders 12h time correctly", () => {
    expect(formatCommitDate(d, { hour12: true, weekday: false, monthName: false })).toBe(
      `2026-06-15 2:30:05 PM ${formatTzOffset(d)}`,
    );
  });

  it("pins the weekday + month-name absolute format", () => {
    // Independently rebuild the expected string from the same fixed abbreviations
    // the formatter uses, to lock the combined absolute layout against regressions.
    const WD = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MO = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const exp = `${WD[d.getDay()]} ${MO[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()} 14:30:05 ${formatTzOffset(d)}`;
    expect(formatCommitDate(d, { hour12: false, weekday: true, monthName: true })).toBe(exp);
  });
});
