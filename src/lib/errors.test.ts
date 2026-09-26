import { afterEach, describe, expect, it } from "vitest";
import { errors, reportError, reportNotice } from "./errors.svelte";
import { appState } from "./store.svelte";

const DIVERGENT = "From /tmp/origin\nfatal: Need to specify how to reconcile divergent branches.\n";
const OVERWRITTEN =
  "error: Your local changes to the following files would be overwritten by merge:\n\tf\nAborting\n";

afterEach(() => {
  errors.reset();
  appState.status = "Ready";
  appState.clearLog();
});

describe("reportError", () => {
  it("queues a popup led by git's real error", () => {
    reportError("Pull failed", DIVERGENT);
    expect(errors.current).toMatchObject({
      kind: "error",
      title: "Pull failed",
      headline: "Need to specify how to reconcile divergent branches.",
      output: DIVERGENT,
    });
    expect(errors.pending).toBe(1);
  });

  it("puts the real error in the status bar, in the error state", () => {
    reportError("Pull failed", DIVERGENT);
    expect(appState.status).toBe("Pull failed: Need to specify how to reconcile divergent branches.");
    expect(appState.statusIsError).toBe(true);
  });

  it("uses only the headline's first line in the status bar", () => {
    reportError("Pull failed", OVERWRITTEN);
    expect(appState.status).toBe(
      "Pull failed: Your local changes to the following files would be overwritten by merge:",
    );
  });

  it("logs the full output", () => {
    reportError("Pull failed", DIVERGENT);
    expect(appState.logLines[appState.logLines.length - 1]).toBe(`[error] Pull failed\n${DIVERGENT}`);
  });

  it("shows queued reports in order", () => {
    reportError("Pull failed", DIVERGENT);
    reportError("Push failed", "error: failed to push some refs to 'origin'");
    expect(errors.pending).toBe(2);
    expect(errors.current?.title).toBe("Pull failed");
    errors.dismiss();
    expect(errors.current?.title).toBe("Push failed");
    errors.dismiss();
    expect(errors.current).toBeNull();
  });

  it("drops a report identical to the last one queued", () => {
    reportError("Pull failed", DIVERGENT);
    reportError("Pull failed", DIVERGENT);
    expect(errors.pending).toBe(1);
  });

  it("records a quiet report without a popup", () => {
    reportError("Update check failed", "offline", { quiet: true });
    expect(errors.current).toBeNull();
    expect(errors.last?.title).toBe("Update check failed");
    expect(appState.statusIsError).toBe(true);
  });

  it("reopens the last error after it was dismissed", () => {
    reportError("Pull failed", DIVERGENT);
    errors.dismiss();
    errors.reopenLast();
    expect(errors.current?.title).toBe("Pull failed");
  });

  it("gives a reopened report a fresh id", () => {
    reportError("Pull failed", DIVERGENT);
    const first = errors.current?.id;
    errors.dismiss();
    errors.reopenLast();
    expect(errors.current?.id).not.toBe(first);
  });

  it("clears the error state on the next ordinary status message", () => {
    reportError("Pull failed", DIVERGENT);
    appState.status = "Fetch — done.";
    expect(appState.statusIsError).toBe(false);
  });
});

describe("reportNotice", () => {
  it("queues an info popup without touching the error state", () => {
    reportNotice("Can't reword this commit", "Can't reword a merge commit here.");
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't reword this commit",
      headline: "Can't reword a merge commit here.",
      hints: "",
      output: "",
    });
    expect(appState.status).toBe("Can't reword a merge commit here.");
    expect(appState.statusIsError).toBe(false);
    expect(errors.last).toBeNull();
  });
});
