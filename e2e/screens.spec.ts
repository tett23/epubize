import { expect, test } from "@playwright/test";
import { calledCommands, calls, defaultState, emit, installBackend } from "./backend";

test.describe("トップの操作", () => {
  test("見出しに環境を出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await expect(page.getByRole("link", { name: /^epubize/ })).toContainText("development");
  });

  test("fetch all metadata で目次だけの取得を積む", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("button", { name: "fetch all metadata" }).click();
    await expect(page.getByText("2 作品の目次の取得を積みました")).toBeVisible();
    expect(await calledCommands(page)).toContain("fetch_all_metadata");
    expect(await calledCommands(page)).not.toContain("fetch_all");
  });

  test("定期取得が行われたら知らせる", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await expect(page.getByText("毎日 03:00 に fetch all")).toBeVisible();
    await emit(page, "scheduled-fetch", { Ok: 2 });
    await expect(
      page.getByText("定期取得: キューを空にして、2 作品の目次、本文、挿絵の取得を積みました"),
    ).toBeVisible();
    await emit(page, "scheduled-fetch", { Err: "novels.json を読めません" });
    await expect(page.getByText("定期取得: novels.json を読めません")).toBeVisible();
  });

  test("未取得の add は、その作品の目次の取得を積む", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    const row = page.getByRole("row").filter({ hasText: "narou-n0002bb" });
    await row.getByRole("button", { name: "add" }).click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "add_novel")?.args)
      .toEqual({
        siteKey: "narou",
        siteId: "n0002bb",
      });
  });

  test("取得が終わると、作品の一覧を読み直す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await expect(page.getByText("(1 / 2)")).toBeVisible();
    await page.evaluate(() => {
      const w = window as unknown as {
        __epubize: { state: { novels: { episodes: { bodyFetchedAt: string | null }[] }[] } };
      };
      for (const e of w.__epubize.state.novels[0].episodes) e.bodyFetchedAt = "2026-01-04T00:00:00Z";
    });
    await emit(page, "fetch-done", { command: "episode", url: "https://ncode.syosetu.com/n0001aa/2/", error: null });
    await expect(page.getByText("(2 / 2)")).toBeVisible();
  });
});

test.describe("latest", () => {
  test("公開日ごとにまとめ、refetch でその話の取得を積む", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "latest" }).click();
    await expect(page.getByRole("columnheader", { name: /^2026-01-0[12]$/ })).toHaveCount(2);

    const row = page.getByRole("row").filter({ hasText: "第2話" });
    await expect(row.getByRole("button", { name: "send to Kindle" })).toHaveCount(0);
    await row.getByRole("button", { name: "refetch" }).click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "refetch_episode")?.args)
      .toEqual({
        episodeId: 2,
      });

    // 本文を取得済みの話には send to Kindle を出すが、まだ押せない
    const fetched = page.getByRole("row").filter({ hasText: "第1話" });
    await expect(fetched.getByRole("button", { name: "send to Kindle" })).toBeDisabled();
  });
});

test.describe("作品のページ", () => {
  test("作品の情報と章ごとの目次を出し、fetch でその作品の取得を積む", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await expect(page.getByText("完結済み")).toBeVisible();
    await expect(page.getByText("合成データのあらすじ。")).toBeVisible();
    await expect(page.getByRole("columnheader", { name: "第一章" })).toBeVisible();
    // 本文を取得済みの話だけをリンクにする
    await expect(page.getByRole("link", { name: "第1話　合成データの話" })).toBeVisible();
    await expect(page.getByRole("link", { name: "第2話　合成データの話" })).toHaveCount(0);

    await page.getByRole("button", { name: "fetch", exact: true }).click();
    await expect
      .poll(async () => (await calls(page)).find((c) => c.cmd === "fetch_novel")?.args)
      .toEqual({
        novelId: 1,
      });
  });

  test("整形の設定を変えると保存する", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await page.getByLabel("horizontal").check();
    await page.getByLabel("insertIndent").uncheck();
    await expect
      .poll(async () => (await calls(page)).filter((c) => c.cmd === "set_normalize_options").at(-1)?.args)
      .toMatchObject({ novelId: 1, options: { direction: "horizontal", insertIndent: false } });
  });

  test("作品がなければ、取得の仕方を案内する", async ({ page }) => {
    await installBackend(page, { ...defaultState(), novels: [] });
    await page.goto("/");
    await expect(page.getByText("作品はまだありません。未取得の作品の add か、fetch all で取得します。")).toBeVisible();
    // 購読は全て未取得に並ぶ
    await expect(page.getByRole("link", { name: "narou-n0001aa" })).toBeVisible();
  });
});

test.describe("話のページ", () => {
  test("前後の話は、本文を取得済みのときだけリンクにする", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await page.getByRole("link", { name: "第1話　合成データの話" }).click();
    await expect(page.getByRole("link", { name: "Prev" })).toHaveCount(0);
    await expect(page.getByRole("link", { name: "Next" })).toHaveCount(0);
    await expect(page.getByText("Next")).toBeVisible();
  });

  test("取得済みの次の話へ移れる", async ({ page }) => {
    const state = defaultState();
    state.novels[0].episodes[1].bodyFetchedAt = "2026-01-03T00:00:00Z";
    state.bodies[2] = { body: "　二話目の合成データ。", images: {} };
    await installBackend(page, state);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await page.getByRole("link", { name: "第1話　合成データの話" }).click();
    await page.getByRole("link", { name: "Next" }).click();
    await expect(page.getByRole("heading", { name: "2: 第2話　合成データの話" })).toBeVisible();
    await expect(page.getByText("二話目の合成データ。").first()).toBeVisible();
  });

  test("組方向を変えると、プレビューの書字方向が変わる", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await page.getByRole("link", { name: "第1話　合成データの話" }).click();
    const preview = page.locator(".novel-body");
    await expect(preview).toHaveCSS("writing-mode", "vertical-rl");
    await page.getByLabel("horizontal").check();
    await expect(preview).toHaveCSS("writing-mode", "horizontal-tb");
  });
});

test.describe("管理画面の定期取得", () => {
  test("無効にすると、トップに時刻を出さない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await page.getByLabel("アプリが動いている間、毎日 fetch all を行う").uncheck();
    await expect(page.getByLabel("時刻")).toBeDisabled();
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("status")).toHaveText("保存しました");
    await page.getByRole("link", { name: /^epubize/ }).click();
    await expect(page.getByText(/毎日 .* に fetch all/)).toHaveCount(0);
  });

  test("データの置き場所と、クローラーの使用中のものを出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await expect(page.getByText("/tmp/epubize/settings.json")).toBeVisible();
    await expect(page.getByText("/usr/local/bin/novel-crawler").first()).toBeVisible();
  });
});
