// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  pickRepoFolder: vi.fn(),
  isGitRepo: vi.fn(),
  initializeRepository: vi.fn(),
  githubCurrentLogin: vi.fn(),
  githubCreateRepo: vi.fn(),
  newRepository: vi.fn(),
  confirm: vi.fn(),
  alert: vi.fn(),
  openRepo: vi.fn(),
  setBusyOp: vi.fn(),
  appState: { status: "Ready", repo: "" },
}));

vi.mock("./api", () => ({
  pickRepoFolder: mocks.pickRepoFolder,
  api: {
    isGitRepo: mocks.isGitRepo,
    initializeRepository: mocks.initializeRepository,
    githubCurrentLogin: mocks.githubCurrentLogin,
    githubCreateRepo: mocks.githubCreateRepo,
  },
}));
vi.mock("./dialogs.svelte", () => ({
  dialogs: {
    newRepository: mocks.newRepository,
    confirm: mocks.confirm,
    alert: mocks.alert,
  },
}));
vi.mock("./store.svelte", () => ({
  appState: {
    get status() { return mocks.appState.status; },
    set status(value: string) { mocks.appState.status = value; },
    get repo() { return mocks.appState.repo; },
    openRepo: mocks.openRepo,
    setBusyOp: mocks.setBusyOp,
  },
}));

async function loadFlows() {
  vi.resetModules();
  return import("./repositoryFlows");
}

beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.appState.status = "Ready";
  mocks.appState.repo = "";
  mocks.pickRepoFolder.mockReset().mockResolvedValue("/projects");
  mocks.isGitRepo.mockReset().mockResolvedValue(true);
  mocks.initializeRepository.mockReset().mockResolvedValue({
    path: "/projects/app",
    initialized: true,
    existingEntries: 0,
  });
  mocks.githubCurrentLogin.mockReset().mockResolvedValue("ashproto");
  mocks.githubCreateRepo.mockReset().mockResolvedValue("https://github.com/ashproto/app");
  mocks.newRepository.mockReset().mockResolvedValue({
    name: "app",
    initialBranch: "main",
    createRemote: false,
    isPrivate: true,
    description: "",
  });
  mocks.confirm.mockReset().mockResolvedValue(true);
  mocks.alert.mockReset().mockResolvedValue(undefined);
  mocks.openRepo.mockReset();
  mocks.setBusyOp.mockReset();
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("repository creation flow", () => {
  it("guards the entire flow against overlapping creation entry points", async () => {
    let finishPicker!: (path: string | null) => void;
    mocks.pickRepoFolder.mockImplementation(
      () => new Promise<string | null>((resolve) => { finishPicker = resolve; }),
    );
    const { createRepositoryFlow } = await loadFlows();

    const first = createRepositoryFlow();
    await createRepositoryFlow();

    expect(mocks.pickRepoFolder).toHaveBeenCalledOnce();
    expect(mocks.appState.status).toBe("A repository is already being created.");
    finishPicker(null);
    await first;
  });

  it("creates and opens a local-only repository without touching GitHub", async () => {
    const { createRepositoryFlow } = await loadFlows();

    await createRepositoryFlow();

    expect(mocks.initializeRepository).toHaveBeenCalledWith("/projects", "app", "main", false);
    expect(mocks.openRepo).toHaveBeenCalledWith("/projects/app");
    expect(mocks.githubCurrentLogin).not.toHaveBeenCalled();
    expect(mocks.githubCreateRepo).not.toHaveBeenCalled();
  });

  it("requires confirmation before initializing a non-empty folder", async () => {
    mocks.initializeRepository
      .mockResolvedValueOnce({ path: "/projects/app", initialized: false, existingEntries: 3 })
      .mockResolvedValueOnce({ path: "/projects/app", initialized: true, existingEntries: 3 });
    const { createRepositoryFlow } = await loadFlows();

    await createRepositoryFlow();

    expect(mocks.confirm).toHaveBeenCalledWith(expect.objectContaining({
      title: "Folder is not empty",
      confirmLabel: "Initialize Here",
    }));
    expect(mocks.initializeRepository).toHaveBeenNthCalledWith(2, "/projects", "app", "main", true);
    expect(mocks.openRepo).toHaveBeenCalledWith("/projects/app");
  });

  it("preflights GitHub before local mutation and creates the remote against the canonical path", async () => {
    mocks.newRepository.mockResolvedValue({
      name: "app",
      initialBranch: "main",
      createRemote: true,
      isPrivate: false,
      description: "A test repository",
    });
    const { createRepositoryFlow } = await loadFlows();

    await createRepositoryFlow();

    expect(mocks.githubCurrentLogin.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.initializeRepository.mock.invocationCallOrder[0],
    );
    expect(mocks.githubCreateRepo).toHaveBeenCalledWith(
      "/projects/app",
      "app",
      false,
      "A test repository",
    );
  });

  it("keeps and opens the local repository when GitHub setup fails", async () => {
    mocks.newRepository.mockResolvedValue({
      name: "app",
      initialBranch: "main",
      createRemote: true,
      isPrivate: true,
      description: "",
    });
    mocks.githubCreateRepo.mockRejectedValue({ kind: "Other", message: "name already exists" });
    const { createRepositoryFlow } = await loadFlows();

    await createRepositoryFlow();

    expect(mocks.openRepo).toHaveBeenCalledWith("/projects/app");
    expect(mocks.alert).toHaveBeenCalledWith(expect.objectContaining({
      title: "Local repository created",
      message: expect.stringContaining("name already exists"),
    }));
  });
});
