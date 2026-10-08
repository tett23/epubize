import { expect, test } from "@playwright/test";
import { calledCommands, defaultState, emit, installBackend } from "./backend";

test("すでに購読している作品を足すと、そう出す", async ({ page }) => {
  await installBackend(page);
  await page.goto("/");
  const input = page.getByPlaceholder("作品の URL");
  await input.fill("https://ncode.syosetu.com/n0001aa/");
  await input.press("Enter");
  await expect(page.getByRole("status")).toHaveText("narou-n0001aa はすでに購読しています");
});

test("ボタンの処理が失敗したら、ボタンの横に理由を出し、押し直せる", async ({ page }) => {
  await installBackend(page, { ...defaultState(), failures: { fetch_novel: "クローラーが見つかりません" } });
  await page.goto("/");
  await page.getByRole("link", { name: "合成データの作品" }).click();
  const fetch = page.getByRole("button", { name: "fetch", exact: true });
  await fetch.click();
  await expect(page.getByRole("alert")).toHaveText("クローラーが見つかりません");
  await expect(fetch).toBeEnabled();
});

test("fetch all が失敗したら理由を出す", async ({ page }) => {
  await installBackend(page, { ...defaultState(), failures: { fetch_all: "novels.json を読めません" } });
  await page.goto("/");
  await page.getByRole("button", { name: "fetch all", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText("novels.json を読めません");
});

test("作品の一覧を読めなければ理由を出す", async ({ page }) => {
  await installBackend(page, { ...defaultState(), failures: { list_novels: "database is locked" } });
  await page.goto("/");
  await expect(page.getByRole("alert").first()).toContainText("database is locked");
});

test("取得の結果は新しい順に 20 件まで出す", async ({ page }) => {
  await installBackend(page);
  await page.goto("/");
  for (let i = 1; i <= 25; i++) {
    await emit(page, "fetch-done", { command: "episode", url: `https://example.com/${i}`, error: null });
  }
  const items = page.getByRole("listitem");
  await expect(items).toHaveCount(20);
  await expect(items.first()).toHaveText("episode https://example.com/25 ok");
  await expect(items.last()).toHaveText("episode https://example.com/6 ok");
});

test("設定のファイルを読めなければ、既定値で表示して理由を出す", async ({ page }) => {
  await installBackend(page, { ...defaultState(), settingsLoadError: "settings.json を読めません: expected value" });
  await page.goto("/");
  await page.getByRole("link", { name: "settings" }).click();
  await expect(page.getByRole("alert")).toContainText("設定のファイルを読めないため、既定値を表示しています");
  await expect(page.getByLabel("時刻")).toHaveValue("03:00");
});

test("購読を外すのに失敗したら理由を出し、一覧に残す", async ({ page }) => {
  await installBackend(page, {
    ...defaultState(),
    failures: { remove_subscription: "novels.json の形が正しくありません" },
  });
  await page.goto("/");
  await page.getByRole("link", { name: "settings" }).click();
  const row = page.getByRole("row").filter({ hasText: "narou-n0002bb" });
  await row.getByRole("button", { name: "remove" }).click();
  await row.getByRole("alertdialog").getByRole("button", { name: "実行する" }).click();
  await expect(row.getByRole("alert")).toHaveText("novels.json の形が正しくありません");
  await expect(page.getByRole("link", { name: "narou-n0002bb" })).toBeVisible();
  expect(await calledCommands(page)).toContain("remove_subscription");
});

test("取得が続けて終わっても、読み直しは 1 回にまとめる", async ({ page }) => {
  await installBackend(page);
  await page.goto("/");
  await expect(page.getByText("(1 / 2)")).toBeVisible();
  const listNovelsCalls = async () => (await calledCommands(page)).filter((c) => c === "list_novels").length;
  // 読み込みが落ち着くのを待つ
  await page.waitForTimeout(700);
  const before = await listNovelsCalls();

  for (let i = 0; i < 5; i++) {
    await emit(page, "fetch-done", { command: "episode", url: `https://example.com/${i}`, error: null });
  }
  await page.waitForTimeout(1000);
  // トップは作品の一覧と未取得の一覧で、それぞれ list_novels を 1 回ずつ呼ぶ
  expect((await listNovelsCalls()) - before).toBe(2);
});
