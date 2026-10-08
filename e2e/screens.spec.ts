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

test("外部のリンクは、アプリの中では開かず既定のブラウザで開く", async ({ page }) => {
  await installBackend(page);
  await page.goto("/");
  await page.getByRole("link", { name: "narou-n0001aa" }).first().click();
  await expect
    .poll(
      async () =>
        (
          await page.evaluate(
            () =>
              (window as unknown as { __epubize: { calls: { cmd: string; args: Record<string, unknown> }[] } })
                .__epubize.calls,
          )
        ).find((c) => c.cmd === "plugin:opener|open_url")?.args,
    )
    .toMatchObject({ url: "https://ncode.syosetu.com/n0001aa/" });
  await expect(page).toHaveURL("http://localhost:1420/");
});

test.describe("管理画面の epub-builder", () => {
  test("見つからなければそう出し、指定して保存すると使用中に出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    const section = page.getByRole("region").or(page.locator("section")).filter({ hasText: "EPUB を作るときに" });
    await expect(section.getByText("見つかりません")).toHaveCount(2);

    await page.getByLabel("epub-builder の実行ファイル").fill("/usr/local/bin/epub-builder");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("status")).toHaveText("保存しました");
    await expect(section.getByText("/usr/local/bin/epub-builder")).toBeVisible();
    const saved = await page.evaluate(
      () =>
        (window as unknown as { __epubize: { state: { settings: { epubBuilderPath: string | null } } } }).__epubize
          .state.settings.epubBuilderPath,
    );
    expect(saved).toBe("/usr/local/bin/epub-builder");
  });

  test("実行できないパスは保存しない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await page.getByLabel("epub-builder の実行ファイル").fill("/nonexistent/epub-builder");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("alert")).toContainText("epub-builder の実行ファイルが見つからないか、実行できません");
  });

  test("自動で見つかるものがあれば、欄の案内と使用中に出す", async ({ page }) => {
    await installBackend(page, { ...defaultState(), epubBuilderFound: "/usr/local/bin/epub-builder" });
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await expect(page.getByLabel("epub-builder の実行ファイル")).toHaveAttribute(
      "placeholder",
      "/usr/local/bin/epub-builder",
    );
  });
});

test.describe("話ごとの EPUB と送信", () => {
  test("本文を取得済みの話だけに、話の題名の後ろに download epub と send to Kindle を出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();

    const fetched = page.getByRole("row").filter({ hasText: "第1話　合成データの話" });
    await expect(fetched.getByRole("button", { name: "download epub" })).toBeDisabled();
    await expect(fetched.getByRole("button", { name: "send to Kindle" })).toBeDisabled();
    // 話の題名の列の後ろに並ぶ
    const cells = fetched.getByRole("cell");
    await expect(cells.nth(2)).toContainText("第1話　合成データの話");
    await expect(cells.nth(3).getByRole("button", { name: "download epub" })).toBeVisible();

    const unfetched = page.getByRole("row").filter({ hasText: "第2話　合成データの話" });
    await expect(unfetched.getByRole("button")).toHaveCount(0);
  });

  test("mobi のボタンはどこにも出さない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "合成データの作品" }).click();
    await expect(page.getByRole("button", { name: /mobi/ })).toHaveCount(0);
    await page.getByRole("link", { name: "第1話　合成データの話" }).click();
    await expect(page.getByRole("heading", { name: "1: 第1話　合成データの話" })).toBeVisible();
    await expect(page.getByRole("button", { name: /mobi/ })).toHaveCount(0);
  });
});

test.describe("管理画面の send-to-kindle", () => {
  test("見つからなければそう出し、指定して保存すると使用中に出す", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    const section = page.locator("section").filter({ hasText: "Kindle に EPUB を送るときに" });
    await expect(section.getByText("見つかりません")).toHaveCount(2);

    await page.getByLabel("send-to-kindle の実行ファイル").fill("/usr/local/bin/send-to-kindle");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("status")).toHaveText("保存しました");
    await expect(section.getByText("/usr/local/bin/send-to-kindle")).toBeVisible();
    const saved = await page.evaluate(
      () =>
        (window as unknown as { __epubize: { state: { settings: { sendToKindlePath: string | null } } } }).__epubize
          .state.settings.sendToKindlePath,
    );
    expect(saved).toBe("/usr/local/bin/send-to-kindle");
  });

  test("実行できないパスは保存しない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await page.getByLabel("send-to-kindle の実行ファイル").fill("/nonexistent/send-to-kindle");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("alert")).toContainText(
      "send-to-kindle の実行ファイルが見つからないか、実行できません",
    );
  });

  test("既定ではデータの置き場所の .env を使い、足りないキーを出す", async ({ page }) => {
    await installBackend(page, { ...defaultState(), missingEnvKeys: ["SMTP_PASSWORD", "SEND_TO_KINDLE_EMAIL"] });
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await expect(page.getByLabel("send-to-kindle の .env", { exact: true })).toHaveAttribute(
      "placeholder",
      "/tmp/epubize/.env",
    );
    await expect(page.getByLabel("send-to-kindle の .env.example")).toHaveAttribute(
      "placeholder",
      "/tmp/epubize/.env.example",
    );
    await expect(page.getByTestId("env-check")).toHaveText(".env に足りないキー: SMTP_PASSWORD, SEND_TO_KINDLE_EMAIL");
  });

  test(".env.example がなければ検査の結果を出さない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await expect(page.getByLabel("send-to-kindle の .env.example")).toBeVisible();
    await expect(page.getByTestId("env-check")).toHaveCount(0);
  });

  test(".env と .env.example を指定して保存すると、使用中に出す", async ({ page }) => {
    await installBackend(page, { ...defaultState(), missingEnvKeys: [] });
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    const section = page.locator("section").filter({ hasText: "Kindle に EPUB を送るときに" });
    await expect(section.getByText("/tmp/epubize/.env", { exact: true })).toBeVisible();

    await page.getByLabel("send-to-kindle の .env", { exact: true }).fill("/usr/local/etc/send-to-kindle/.env");
    await page.getByLabel("send-to-kindle の .env.example").fill("/usr/local/etc/send-to-kindle/.env.example");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("status")).toHaveText("保存しました");
    await expect(section.getByText("/usr/local/etc/send-to-kindle/.env", { exact: true })).toBeVisible();
    await expect(section.getByText("/usr/local/etc/send-to-kindle/.env.example", { exact: true })).toBeVisible();
    await expect(page.getByTestId("env-check")).toHaveText(".env に .env.example のキーがすべてあります");
    const saved = await page.evaluate(
      () =>
        (
          window as unknown as {
            __epubize: {
              state: { settings: { sendToKindleEnvPath: string | null; sendToKindleEnvExamplePath: string | null } };
            };
          }
        ).__epubize.state.settings,
    );
    expect(saved.sendToKindleEnvPath).toBe("/usr/local/etc/send-to-kindle/.env");
    expect(saved.sendToKindleEnvExamplePath).toBe("/usr/local/etc/send-to-kindle/.env.example");
  });

  test("見つからない .env は保存しない", async ({ page }) => {
    await installBackend(page);
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await page.getByLabel("send-to-kindle の .env", { exact: true }).fill("/nonexistent/.env");
    await page.getByRole("button", { name: "保存" }).click();
    await expect(page.getByRole("alert")).toContainText("send-to-kindle の .env が見つかりません");
  });

  test("自動で見つかるものがあれば、欄の案内に出す", async ({ page }) => {
    await installBackend(page, { ...defaultState(), sendToKindleFound: "/usr/local/bin/send-to-kindle" });
    await page.goto("/");
    await page.getByRole("link", { name: "settings" }).click();
    await expect(page.getByLabel("send-to-kindle の実行ファイル")).toHaveAttribute(
      "placeholder",
      "/usr/local/bin/send-to-kindle",
    );
  });
});
