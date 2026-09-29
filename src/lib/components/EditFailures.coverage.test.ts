// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { prerequisites } from "../prerequisites.svelte";
import { rebaseEditor } from "../rebaseEditor.svelte";
import { SAMPLE_GRAPH } from "../graph/sample";
import ApplyPanel from "./ApplyPanel.svelte";
import EditTabs from "./EditTabs.svelte";
import RebaseTodo from "./RebaseTodo.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let target: HTMLDivElement;
let dispose: (() => Promise<void>) | undefined;
function button(text: string): HTMLButtonElement {
  const found = Array.from(target.querySelectorAll("button")).find((b) => b.textContent?.trim() === text);
  if (!found) throw new Error(`Missing button: ${text}`);
  return found;
}
function numberValues(values: Array<number | string>) {
  const inputs = target.querySelectorAll<HTMLInputElement>('input[type="number"]');
  values.forEach((value, i) => {
    inputs[i].value = String(value);
    inputs[i].dispatchEvent(new Event("input", { bubbles: true }));
  });
}
// Runs a rewrite whose backend streams `lines` (stderr arrives prefixed "[stderr] "), then rejects.
async function failRewrite(lines: string[], rejection: unknown) {
  vi.spyOn(api, "rewriteHistory").mockImplementation(async (_repo, _mappings, _options, onLine) => {
    for (const line of lines) onLine(line);
    throw rejection;
  });
  appState.setNewDate(SAMPLE_GRAPH[0].sha, new Date("2026-01-01T00:00:00Z"));
  const panel = mount(ApplyPanel, { target });
  dispose = () => unmount(panel);
  await tick();
  button("Rewrite history").click();
  await vi.waitFor(() => expect(errors.current).not.toBeNull());
}
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/edits";
  appState.setGraphCommits(SAMPLE_GRAPH);
  appState.clearNewDates();
  appState.clearLog();
  vi.spyOn(prerequisites, "load").mockResolvedValue(undefined);
  vi.spyOn(prerequisites, "canEditHistory", "get").mockReturnValue(true);
  vi.stubGlobal("confirm", vi.fn(() => true));
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  rebaseEditor.close();
  errors.reset();
  appState.repo = "";
  appState.isRewriting = false;
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("reports the streamed stderr rather than the exit code, keeping progress logging and queued dates", async () => {
  await failRewrite(["Scanning commits", "[stderr] fatal: rewrite denied"], "git-filter-repo exited with code Some(1)");
  expect(errors.current).toMatchObject({ kind: "error", title: "Rewrite failed", headline: "rewrite denied", output: "fatal: rewrite denied" });
  expect(errors.pending).toBe(1);
  expect(appState.logLines).toContain("Scanning commits");
  expect(appState.logLines).toContain("[stderr] fatal: rewrite denied");
  expect(appState.logLines).toContain("[rewrite] stopped: git-filter-repo exited with code Some(1)");
  const log = appState.logLines.join("\n");
  expect(log.match(/exited with code/g)).toHaveLength(1);
  // Accepted: the complaint logs once as it streams and once more in the error entry.
  expect(log.match(/rewrite denied/g)).toHaveLength(2);
  expect(appState.newDates.size).toBe(1);
  expect(appState.isRewriting).toBe(false);
});
it("headlines a streamed Python traceback with its exception, not fast-import's epilogue", async () => {
  await failRewrite(
    [
      "[stderr] Traceback (most recent call last):",
      '[stderr]   File "x.py", line 1, in <module>',
      "[stderr] ValueError: bad date",
      "[stderr] fatal: stream ends early",
      "[stderr] fast-import: dumping crash report to .git/fast_import_crash_12345",
    ],
    "git-filter-repo exited with code Some(1)",
  );
  expect(errors.current).toMatchObject({
    kind: "error",
    title: "Rewrite failed",
    headline: "ValueError: bad date",
    output: 'Traceback (most recent call last):\n  File "x.py", line 1, in <module>\nValueError: bad date',
  });
  expect(appState.logLines).toContain("[stderr] fatal: stream ends early");
});
it("reports the rejection once when the rewrite fails before anything streams", async () => {
  await failRewrite([], new Error("fatal: rewrite denied"));
  expect(errors.current).toMatchObject({ kind: "error", title: "Rewrite failed", headline: "rewrite denied", output: "fatal: rewrite denied" });
  expect(errors.pending).toBe(1);
  expect(appState.logLines).toContain("[rewrite] stopped.");
  expect(appState.logLines.join("\n").match(/rewrite denied/g)).toHaveLength(1);
  expect(appState.newDates.size).toBe(1);
  expect(appState.isRewriting).toBe(false);
});
it("reports only the current run's stderr on a retried rewrite", async () => {
  await failRewrite(["[stderr] fatal: rewrite denied"], "git-filter-repo exited with code Some(1)");
  errors.dismiss();
  vi.mocked(api.rewriteHistory).mockRejectedValueOnce(new Error("fatal: repository busy"));
  button("Rewrite history").click();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ title: "Rewrite failed", headline: "repository busy", output: "fatal: repository busy" }));
});
it.each([false, true])("notices ApplyPanel guard, prerequisites=%s", async (ready) => {
  vi.spyOn(prerequisites, "canEditHistory", "get").mockReturnValue(ready);
  const rewrite = vi.spyOn(api, "rewriteHistory");
  const panel = mount(ApplyPanel, { target });
  dispose = () => unmount(panel);
  await tick();
  // Exercise the defensive handler as if a stale enabled control dispatched it.
  button("Rewrite history").disabled = false;
  button("Rewrite history").click();
  expect(rewrite).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't rewrite history", headline: ready ? "No new dates queued. Use an edit mode to preview changes first." : "Commit-time editing is unavailable — the bundled git-filter-repo couldn't run." });
});
it.each([
  ["selection", "Select one or more commits first."],
  ["exact", "Enter a complete, valid date."],
  ["count", "Compress needs at least 2 selected commits."],
  ["incomplete", "Enter a complete, valid date."],
  ["order", "Compress: end must be after start."],
  ["unreadable", "Compress: couldn't read dates for the selected commits."],
])("notices date validation: %s", async (kind, headline) => {
  const panel = mount(EditTabs, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  appState.selected = new Set(kind === "selection" ? [] : kind === "count" ? ["a"] : ["a", "b"]);
  if (kind === "exact") {
    button("Exact").click();
    await tick();
    numberValues([""]);
  } else if (kind !== "selection") {
    button("Compress").click();
    await tick();
    numberValues([2026, 9, 26, 12, 0, 0, 2026, 9, 25, 12, 0, 0]);
    if (kind === "incomplete") numberValues([""]);
    if (kind === "order") numberValues([2026, 9, 24]);
    if (kind === "unreadable") appState.commits = [];
  }
  await tick();
  button("Preview").click();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't preview date changes", headline });
  expect(appState.status).toBe(headline);
  expect(appState.statusIsError).toBe(false);
});
it("notices all-drop interactive rebase without closing its editor", async () => {
  vi.spyOn(api, "rebaseTodoPreview").mockResolvedValue([{ sha: "a", short: "a", selector: "", subject: "Commit" }]);
  rebaseEditor.openWith("base");
  const panel = mount(RebaseTodo, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.querySelector("select")).not.toBeNull());
  const action = target.querySelector<HTMLSelectElement>("select")!;
  action.value = "drop";
  action.dispatchEvent(new Event("change", { bubbles: true }));
  await tick();
  button("Start rebase").click();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't start the interactive rebase", headline: "Interactive rebase: at least one non-drop step is required." });
  expect(rebaseEditor.open).toBe(true);
});
