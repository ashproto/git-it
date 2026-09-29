// @vitest-environment jsdom
// The pull settings are read at module init (sync localStorage seed outside Tauri,
// async Tauri-store hydrate inside it), so every case re-imports a fresh store after
// seeding the storage it should read. jsdom supplies window + localStorage.
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  localStorage.clear();
  vi.resetModules();
});

describe("pull settings persistence", () => {
  it("defaults to merge, no autostash, ask every time", async () => {
    const { appState } = await import("./store.svelte");
    expect(appState.pullStrategy).toBe("merge");
    expect(appState.pullAutostash).toBe(false);
    expect(appState.pullAskEveryTime).toBe(true);
  });

  it("promotes a legacy pullRebase=true to rebase when no strategy was saved", async () => {
    localStorage.setItem("gitit.pullRebase.v1", "true");
    const { appState } = await import("./store.svelte");
    expect(appState.pullStrategy).toBe("rebase");
  });

  it("prefers a saved strategy over the legacy flag", async () => {
    localStorage.setItem("gitit.pullRebase.v1", "true");
    localStorage.setItem("gitit.pullStrategy.v1", "ffOnly");
    const { appState } = await import("./store.svelte");
    expect(appState.pullStrategy).toBe("ffOnly");
  });

  it("persists the three settings and leaves the legacy key in place", async () => {
    localStorage.setItem("gitit.pullRebase.v1", "true");
    const { appState } = await import("./store.svelte");
    appState.setPullStrategy("ffOnly");
    appState.setPullAutostash(true);
    appState.setPullAskEveryTime(false);
    expect(localStorage.getItem("gitit.pullStrategy.v1")).toBe("ffOnly");
    expect(localStorage.getItem("gitit.pullAutostash.v1")).toBe("true");
    expect(localStorage.getItem("gitit.pullAskEveryTime.v1")).toBe("false");
    expect(localStorage.getItem("gitit.pullRebase.v1")).toBe("true");
    expect(appState.pullStrategy).toBe("ffOnly");
    expect(appState.pullAutostash).toBe(true);
    expect(appState.pullAskEveryTime).toBe(false);
  });

  it("migrates a Tauri-store pullRebase=true when the desktop store has no pullStrategy", async () => {
    const values = new Map<string, unknown>([["pullRebase", true]]);
    vi.doMock("@tauri-apps/plugin-store", () => ({
      load: async () => ({
        get: async (key: string) => values.get(key),
        set: async (key: string, v: unknown) => {
          values.set(key, v);
        },
        save: async () => {},
      }),
    }));
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    try {
      const { appState } = await import("./store.svelte");
      await vi.waitFor(() => expect(appState.pullStrategy).toBe("rebase"));
      expect(values.has("pullStrategy")).toBe(false); // hydrating never writes
      expect(values.get("pullRebase")).toBe(true); // legacy key left in place
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
      vi.doUnmock("@tauri-apps/plugin-store");
    }
  });

  // A desktop store whose read completes only when the test says so, to expose the
  // window between launch and hydration.
  function gatedDesktopStore(values: Map<string, unknown>) {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    vi.doMock("@tauri-apps/plugin-store", () => ({
      load: () =>
        gate.then(() => ({
          get: async (key: string) => values.get(key),
          set: async () => {},
          save: async () => {},
        })),
    }));
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    return {
      release,
      cleanup() {
        Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
        vi.doUnmock("@tauri-apps/plugin-store");
      },
    };
  }

  it("exposes a barrier that resolves only once the desktop store has been read", async () => {
    const desktop = gatedDesktopStore(
      new Map<string, unknown>([
        ["pullStrategy", "ffOnly"],
        ["pullAutostash", true],
        ["pullAskEveryTime", false],
      ]),
    );
    try {
      const { appState } = await import("./store.svelte");
      expect(appState.pullStrategy).toBe("merge"); // startup default until the store is read
      let hydrated = false;
      const waited = appState.waitForPullSettingsHydration().then(() => {
        hydrated = true;
      });
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(hydrated).toBe(false);

      desktop.release();
      await waited;
      expect(appState.pullStrategy).toBe("ffOnly");
      expect(appState.pullAutostash).toBe(true);
      expect(appState.pullAskEveryTime).toBe(false);
    } finally {
      desktop.cleanup();
    }
  });

  it("keeps a choice made before hydration finished (touched guard)", async () => {
    const desktop = gatedDesktopStore(new Map<string, unknown>([["pullStrategy", "ffOnly"]]));
    try {
      const { appState } = await import("./store.svelte");
      appState.setPullStrategy("rebase");
      desktop.release();
      await appState.waitForPullSettingsHydration();
      expect(appState.pullStrategy).toBe("rebase");
    } finally {
      desktop.cleanup();
    }
  });
});
