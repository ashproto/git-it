import { describe, it, expect } from "vitest";
import { inferPlatform, aggregateDownloads } from "./downloads";
import type { GhRelease } from "../types";

function rel(tag: string, name: string, assets: [string, number][]): GhRelease {
  return {
    tagName: tag,
    name,
    draft: false,
    prerelease: false,
    publishedAt: null,
    htmlUrl: "",
    totalDownloads: 0,
    assets: assets.map(([n, c]) => ({ name: n, size: 1, downloadCount: c, downloadUrl: "" })),
  };
}

describe("inferPlatform", () => {
  it("classifies by filename markers", () => {
    expect(inferPlatform("gh_2.95.0_macOS_arm64.zip")).toBe("macOS");
    expect(inferPlatform("app-darwin-amd64.tar.gz")).toBe("macOS");
    expect(inferPlatform("tool_1.0_windows_amd64.msi")).toBe("Windows");
    expect(inferPlatform("setup.exe")).toBe("Windows");
    expect(inferPlatform("gh_2.95.0_linux_amd64.deb")).toBe("Linux");
    expect(inferPlatform("app.AppImage")).toBe("Linux");
    expect(inferPlatform("myproject-1.0-source.tar.gz")).toBe("Source");
    expect(inferPlatform("gh_2.95.0_checksums.txt")).toBe("Other");
  });
});

describe("aggregateDownloads", () => {
  it("returns zeros/nulls/empties for no releases", () => {
    const s = aggregateDownloads([]);
    expect(s.grandTotal).toBe(0);
    expect(s.releaseCount).toBe(0);
    expect(s.avgPerRelease).toBe(0);
    expect(s.topRelease).toBeNull();
    expect(s.byRelease).toEqual([]);
    expect(s.byPlatform).toEqual([]);
    expect(s.topAssets).toEqual([]);
  });

  it("rolls up totals, releases, platforms, and top assets", () => {
    const s = aggregateDownloads([
      rel("v2", "Two", [["app_macOS_arm64.dmg", 100], ["app_windows.exe", 50]]),
      rel("v1", "One", [["app_linux.deb", 30]]),
    ]);
    expect(s.grandTotal).toBe(180);
    expect(s.releaseCount).toBe(2);
    expect(s.avgPerRelease).toBe(90);
    expect(s.topRelease).toEqual({ tag: "v2", total: 150 });
    expect(s.byRelease).toEqual([
      { tag: "v2", name: "Two", total: 150 },
      { tag: "v1", name: "One", total: 30 },
    ]);
    expect(s.byPlatform).toEqual([
      { platform: "macOS", total: 100 },
      { platform: "Windows", total: 50 },
      { platform: "Linux", total: 30 },
    ]);
    expect(s.topAssets[0]).toEqual({ name: "app_macOS_arm64.dmg", release: "v2", count: 100 });
    expect(s.topAssets).toHaveLength(3);
  });

  it("excludes zero-download assets from top assets and platform totals", () => {
    const s = aggregateDownloads([rel("v1", "One", [["a.dmg", 0], ["b.exe", 5]])]);
    expect(s.topAssets).toEqual([{ name: "b.exe", release: "v1", count: 5 }]);
    expect(s.byPlatform).toEqual([{ platform: "Windows", total: 5 }]);
  });
});
