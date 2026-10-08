import DOMPurify from "dompurify";
import { marked } from "marked";
import type { EpisodeDetail } from "../models";

/** 話の題名、前書き、本文、後書きを 1 つの Markdown にする。前書きと後書きは区切り線で分ける */
export function episodeMarkdown(
  episode: Pick<EpisodeDetail, "title" | "preface" | "body" | "afterword">,
): string {
  const sections = [episode.preface, episode.body, episode.afterword].filter((part): part is string => part != null);
  return [`## ${episode.title}`, sections.join("\n\n---\n\n")].join("\n\n");
}

/**
 * 本文の Markdown（ADR 0005・ADR 0009）を HTML にする。
 * ルビ（`<ruby>`）や傍点（`<span style>`）の HTML を含むため、DOMPurify で安全な要素と属性だけを残す。
 * 挿絵は、取得済みのもの（`images` の URL から data URL へ）に差し替える。
 * 取得していない挿絵は外部へ読みに行かないよう、`src` を外して代替テキストだけを残す（ADR 0018）
 */
export function renderMarkdown(markdown: string, images: Record<string, string> = {}): string {
  const html = marked.parse(markdown, { async: false, gfm: false });
  const fragment = DOMPurify.sanitize(html, {
    ALLOWED_TAGS: ["p", "br", "hr", "em", "strong", "ruby", "rb", "rt", "rp", "span", "img", "h1", "h2", "h3"],
    ALLOWED_ATTR: ["style", "src", "alt"],
    RETURN_DOM_FRAGMENT: true,
  });
  for (const img of fragment.querySelectorAll("img")) {
    const stored = images[img.getAttribute("src") ?? ""];
    if (stored != null) {
      img.setAttribute("src", stored);
    } else {
      img.removeAttribute("src");
    }
  }
  const container = document.createElement("div");
  container.append(fragment);
  return container.innerHTML;
}
