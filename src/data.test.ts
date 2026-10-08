import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: unknown) => invoke(cmd, args),
  isTauri: () => true,
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const { listSubscriptions, listUnadded, removeSubscription, saveSettings } = await import("./data");

function novel(site: string, siteId: string) {
  return { id: 1, site, siteId, url: "", title: "", authorName: "", authorUrl: null };
}

beforeEach(() => {
  invoke.mockReset();
});

describe("listUnadded", () => {
  it("購読のうち、作品として追加していないものを返す", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_subscriptions") {
        return [
          { site: "narou", id: "n0001aa", url: "u1" },
          { site: "narou", id: "n0002bb", url: "u2" },
          { site: "hameln", id: "100001", url: "u3" },
        ];
      }
      if (cmd === "list_novels") {
        return [novel("narou", "n0001aa"), novel("novel18", "n0002bb")];
      }
      throw new Error(cmd);
    });

    // サイトが違えば同じ ID でも別の作品とみなす
    expect(await listUnadded()).toEqual([
      { site: "narou", siteId: "n0002bb", url: "u2" },
      { site: "hameln", siteId: "100001", url: "u3" },
    ]);
  });
});

describe("listSubscriptions", () => {
  it("コマンドの id を siteId に直す", async () => {
    invoke.mockResolvedValue([{ site: "kakuyomu", id: "1000", url: "u" }]);
    expect(await listSubscriptions()).toEqual([{ site: "kakuyomu", siteId: "1000", url: "u" }]);
  });
});

describe("コマンドの引数", () => {
  it("購読を外すときはサイトを siteKey で渡す", async () => {
    invoke.mockResolvedValue(true);
    await removeSubscription("narou", "n0001aa");
    expect(invoke).toHaveBeenCalledWith("remove_subscription", { siteKey: "narou", siteId: "n0001aa" });
  });

  it("設定は settings で包んで渡す", async () => {
    invoke.mockResolvedValue(null);
    const settings = { crawlerPath: null, epubBuilderPath: null, schedule: { enabled: true, at: "03:00" } };
    await saveSettings(settings);
    expect(invoke).toHaveBeenCalledWith("save_settings", { settings });
  });
});
