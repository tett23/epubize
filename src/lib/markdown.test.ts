// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { episodeMarkdown, renderMarkdown } from "./markdown";

describe("episodeMarkdown", () => {
  it("題名の後に、前書き・本文・後書きを区切り線でつなぐ", () => {
    expect(episodeMarkdown({ title: "題", preface: "前", body: "本", afterword: "後" })).toBe(
      "## 題\n\n前\n\n---\n\n本\n\n---\n\n後",
    );
  });

  it("ない部分は飛ばす", () => {
    expect(episodeMarkdown({ title: "題", preface: null, body: "本", afterword: null })).toBe("## 題\n\n本");
  });
});

describe("renderMarkdown", () => {
  it("ルビと傍点の HTML を残す", () => {
    const html = renderMarkdown(
      '<ruby>霧<rp>(</rp><rt>きり</rt><rp>)</rp></ruby>と<span style="text-emphasis: sesame;">点</span>',
    );
    expect(html).toContain("<ruby>霧<rp>(</rp><rt>きり</rt><rp>)</rp></ruby>");
    expect(html).toContain('<span style="text-emphasis: sesame;">点</span>');
  });

  it("強調と太字を変換する", () => {
    expect(renderMarkdown("*強調*と**太字**")).toBe("<p><em>強調</em>と<strong>太字</strong></p>\n");
  });

  it("スクリプトやイベント属性を取り除く", () => {
    const html = renderMarkdown('<script>alert(1)</script><img src="x" onerror="alert(1)">');
    expect(html).not.toContain("<script");
    expect(html).not.toContain("onerror");
  });

  it("取得済みの挿絵を data URL に差し替え、取得していない挿絵は読みに行かない", () => {
    const html = renderMarkdown("![地図](https://example.com/a.png)\n\n![図](https://example.com/b.png)", {
      "https://example.com/a.png": "data:image/png;base64,AA==",
    });
    expect(html).toContain('<img src="data:image/png;base64,AA==" alt="地図">');
    expect(html).toContain('<img alt="図">');
    expect(html).not.toContain("https://example.com/b.png");
  });
});
