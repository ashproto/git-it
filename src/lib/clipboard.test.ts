import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ reportError: vi.fn() }));
vi.mock("./errors.svelte", () => ({ reportError: mocks.reportError }));
import { copyText } from "./clipboard";

const TITLE = "Couldn't copy the text";
const writeText = vi.fn();

beforeEach(() => {
  mocks.reportError.mockReset();
  writeText.mockReset().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", { clipboard: { writeText } });
});
afterEach(() => {
  vi.unstubAllGlobals();
});

it("copies the text without a report", async () => {
  await expect(copyText("abc123", TITLE)).resolves.toBe(true);
  expect(writeText).toHaveBeenCalledWith("abc123");
  expect(mocks.reportError).not.toHaveBeenCalled();
});

it("writes before its first await, while the user's gesture still counts", async () => {
  const copied = copyText("abc123", TITLE);
  expect(writeText).toHaveBeenCalledWith("abc123");
  await copied;
});

it("reports a rejected write once, under the caller's title", async () => {
  const error = new Error("fatal: copy denied");
  writeText.mockRejectedValue(error);
  await expect(copyText("abc123", TITLE)).resolves.toBe(false);
  expect(mocks.reportError).toHaveBeenCalledOnce();
  expect(mocks.reportError).toHaveBeenCalledWith(TITLE, error);
});

it("reports a missing clipboard as a failure", async () => {
  vi.stubGlobal("navigator", {});
  await expect(copyText("abc123", TITLE)).resolves.toBe(false);
  expect(mocks.reportError).toHaveBeenCalledOnce();
  expect(mocks.reportError).toHaveBeenCalledWith(TITLE, "Clipboard is unavailable.");
});
