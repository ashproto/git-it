// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { appState } from "./store.svelte";
import { errors } from "./errors.svelte";

const mocks = vi.hoisted(() => ({ pick: vi.fn(), isGitRepo: vi.fn() }));
vi.mock("./api", async (original) => ({
  ...await original<typeof import("./api")>(),
  pickRepoFolder: mocks.pick,
  api: { ...((await original<typeof import("./api")>()).api), isGitRepo: mocks.isGitRepo },
}));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
import { createRepositoryFlow, openRepositoryFlow } from "./repositoryFlows";

beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.pick.mockReset().mockResolvedValue("/tmp/recent");
  mocks.isGitRepo.mockReset().mockResolvedValue(true);
  appState.repo = "";
  appState.status = "Ready";
});
afterEach(() => {
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

it.each(["picker", "validation"])("reports a rejected %s once", async (site) => {
  const open = vi.spyOn(appState, "openRepo").mockImplementation(() => {});
  (site === "picker" ? mocks.pick : mocks.isGitRepo).mockRejectedValue(new Error("fatal: access denied"));
  await expect(openRepositoryFlow()).resolves.toBeUndefined();
  expect(open).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't open the repository", headline: "access denied" });
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
});

it("notices an ordinary non-repository result", async () => {
  mocks.isGitRepo.mockResolvedValue(false);
  await openRepositoryFlow();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Not a git repository", headline: "/tmp/recent is not a git repo." });
  expect(appState.status).toBe("/tmp/recent is not a git repo.");
  expect(appState.statusIsError).toBe(false);
});

it("treats picker cancellation as a no-op and still opens valid repositories", async () => {
  const open = vi.spyOn(appState, "openRepo").mockImplementation(() => {});
  mocks.pick.mockResolvedValueOnce(null);
  await openRepositoryFlow();
  expect(errors.pending).toBe(0);
  expect(open).not.toHaveBeenCalled();
  await openRepositoryFlow();
  expect(open).toHaveBeenCalledWith("/tmp/recent");
});

it("notices overlapping creation without opening another picker", async () => {
  let finish!: (value: string | null) => void;
  mocks.pick.mockImplementationOnce(() => new Promise<string | null>((resolve) => { finish = resolve; }));
  const first = createRepositoryFlow();
  try {
    await createRepositoryFlow();
    expect(mocks.pick).toHaveBeenCalledOnce();
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't create a repository yet", headline: "A repository is already being created." });
  } finally {
    finish(null);
    await first;
  }
});

it("keeps desktop-preview guards status-only", async () => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  await openRepositoryFlow();
  expect(appState.status).toBe("Opening a repo needs the desktop app.");
  await createRepositoryFlow();
  expect(appState.status).toBe("Creating a repo needs the desktop app.");
  expect(errors.pending).toBe(0);
});
