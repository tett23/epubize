import { expect, test } from "@playwright/test";
import { calledCommands, defaultState, emit, installBackend } from "./backend";

test.describe("トップ", () => {
  test("作品と未取得の購読を並べる", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await expect(page.getByRole("link", { name: "合成データの作品" })).toBeVisible();
    await expect(page.getByText("(1 / 2)")).toBeVisible();
    await expect(page.getByRole("heading", { name: "未取得" })).toBeVisible();
    await expect(page.getByRole("link", { name: "narou-n0002bb" })).toBeVisible();
    await expect(page.getByText("毎日 03:00 に fetch all")).toBeVisible();
  });

  test("作品の URL を購読に加え、対応していない URL ではエラーを出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    const input = page.getByPlaceholder("作品の URL");

    await input.fill("https://ncode.syosetu.com/n0003cc/");
    await input.press("Enter");
    await expect(page.getByRole("status")).toHaveText("narou-n0003cc を購読に加えました");
    await expect(input).toHaveValue("");
    await expect(page.getByRole("link", { name: "narou-n0003cc" })).toBeVisible();

    await input.fill("https://example.com/novel/1");
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("対応していない URL です");
    await expect(input).toHaveValue("https://example.com/novel/1");
  });

  test("fetch all で取得を積み、取得の結果を表示する", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("button", { name: "fetch all", exact: true }).click();
    await expect(page.getByText("2 作品の目次、本文、挿絵の取得を積みました")).toBeVisible();
    expect(await calledCommands(page)).toContain("fetch_all");

    await emit(page, "fetch-done", {
      command: "toc",
      url: "https://ncode.syosetu.com/n0001aa/",
      error: "NotFound: fake",
    });
    await expect(page.getByText("toc https://ncode.syosetu.com/n0001aa/ : NotFound: fake")).toBeVisible();
  });

  test("クローラーがなければ fetch all を押せない", async ({ page }) => {
    await installBackend(page, { ...defaultState(), crawlerFound: null });
    await page.goto("/");
    await expect(page.getByText("クローラーが見つかりません")).toBeVisible();
    await expect(page.getByRole("button", { name: "fetch all", exact: true })).toBeDisabled();
  });
});

test.describe("管理画面", () => {
  test("設定を保存すると、トップの定期取得の表示に反映する", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await page.getByLabel("時刻").fill("04:30");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("status")).toHaveText("保存しました");

    await page.getByRole("link", { name: /^epubize/ }).click();
    await expect(page.getByText("毎日 04:30 に fetch all")).toBeVisible();
  });

  test("正しくない値は保存せず、理由を出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await page.getByLabel("実行ファイル").fill("/nonexistent/novel-crawler");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("alert")).toContainText("クローラーの実行ファイルが見つからないか、実行できません");
    const saved = await page.evaluate(
      () =>
        (window as unknown as { __epubize: { state: { settings: { crawlerPath: string | null } } } }).__epubize.state
          .settings.crawlerPath,
    );
    expect(saved).toBeNull();
  });

  test("購読を外すときは確かめ、やめると外さない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    const row = page.getByRole("row").filter({ hasText: "narou-n0002bb" });
    await expect(row.getByText("（未取得）")).toBeVisible();

    await row.getByRole("button", { name: "remove" }).click();
    const confirm = row.getByRole("alertdialog");
    await expect(confirm).toContainText("narou-n0002bb を購読から外します");
    await confirm.getByRole("button", { name: "やめる" }).click();
    await expect(confirm).toBeHidden();
    expect(await calledCommands(page)).not.toContain("remove_subscription");

    await row.getByRole("button", { name: "remove" }).click();
    await row.getByRole("alertdialog").getByRole("button", { name: "実行する" }).click();
    await expect(page.getByRole("link", { name: "narou-n0002bb" })).toBeHidden();
    expect(await calledCommands(page)).toContain("remove_subscription");
  });
});

test.describe("作品と話", () => {
  test("remove episodes は確かめた後に話を消し、表示を読み直す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await expect(page.getByRole("cell", { name: "第2話　合成データの話" })).toBeVisible();

    await page.getByRole("button", { name: "remove episodes" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "実行する" }).click();
    await expect(page.getByRole("cell", { name: "第2話　合成データの話" })).toBeHidden();
    expect(await calledCommands(page)).toContain("remove_episodes");
  });

  test("話のページで、取得済みの挿絵だけを表示し、ほかは読みに行かない", async ({ page }) => {
    const requested: string[] = [];
    page.on("request", (request) => {
      if (request.url().startsWith("https://example.com/")) requested.push(request.url());
    });
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await page.getByRole("link", { name: "第1話　合成データの話" }).click();

    await expect(page.getByAltText("地図")).toHaveAttribute("src", "data:image/png;base64,AA==");
    await expect(page.getByAltText("図", { exact: true })).not.toHaveAttribute("src", /.+/);
    expect(requested).toEqual([]);
  });
});
