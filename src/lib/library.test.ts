import { describe, expect, it } from "vitest";
import type { EpisodeSummary, LatestEpisode, NovelSummary } from "../models";
import { adjacentEpisodes, groupByDate, groupChapters, libraryStats } from "./library";

function episode(
  id: number,
  no: number,
  chapter: string | null = null,
  publishedAt: string | null = null,
): EpisodeSummary {
  return {
    id,
    novelId: 1,
    no,
    url: `https://example.com/${id}`,
    title: `話 ${no}`,
    chapter,
    publishedAt,
    revisedAt: null,
    bodyFetchedAt: null,
    sentAt: null,
  };
}

function novel(fetched: number, total: number): NovelSummary {
  return {
    id: 1,
    site: "narou",
    siteId: "n0001aa",
    url: "https://example.com/n0001aa/",
    title: "作品",
    authorName: "作者",
    authorUrl: null,
    isConcluded: null,
    latestPublishedAt: null,
    fetched,
    total,
  };
}

function latest(id: number, publishedAt: string | null): LatestEpisode {
  return {
    novelId: 1,
    site: "narou",
    siteId: "n0001aa",
    novelUrl: "https://example.com/n0001aa/",
    novelTitle: "作品",
    episode: episode(id, id, null, publishedAt),
  };
}

describe("libraryStats", () => {
  it("作品数・話数・未取得の話数と、取得にかかる時間の目安を数える", () => {
    expect(libraryStats([novel(1, 2), novel(0, 1)])).toEqual({
      novels: 2,
      novelsMinutes: 1,
      parts: 3,
      partsHours: 1,
      unfetched: 2,
      unfetchedMinutes: 1,
    });
  });

  it("作品がなければ全て 0", () => {
    expect(libraryStats([])).toEqual({
      novels: 0,
      novelsMinutes: 0,
      parts: 0,
      partsHours: 0,
      unfetched: 0,
      unfetchedMinutes: 0,
    });
  });
});

describe("groupByDate", () => {
  // 日付の区切りはローカル時刻で決まるため、時差の影響を受けない正午で作る
  const at = (local: string) => new Date(local).toISOString();

  it("公開日ごとにまとめ、並びを保つ", () => {
    const groups = groupByDate([
      latest(1, at("2026-03-02T12:00:00")),
      latest(2, at("2026-03-02T11:00:00")),
      latest(3, at("2026-03-01T12:00:00")),
    ]);
    expect(groups.map((g) => [g.date, g.items.map((i) => i.episode.id)])).toEqual([
      ["2026-03-02", [1, 2]],
      ["2026-03-01", [3]],
    ]);
  });

  it("公開日時のない話は飛ばす", () => {
    expect(groupByDate([latest(1, null)])).toEqual([]);
  });
});

describe("groupChapters", () => {
  it("続く話で章の名前が同じものごとにまとめる", () => {
    const chapters = groupChapters([
      episode(1, 1, null),
      episode(2, 2, "第一章"),
      episode(3, 3, "第一章"),
      episode(4, 4, "第二章"),
      episode(5, 5, "第一章"),
    ]);
    expect(chapters.map((c) => [c.title, c.episodes.map((e) => e.id)])).toEqual([
      [null, [1]],
      ["第一章", [2, 3]],
      ["第二章", [4]],
      ["第一章", [5]],
    ]);
  });
});

describe("adjacentEpisodes", () => {
  const episodes = [episode(1, 1), episode(2, 2), episode(3, 3)];

  it("前後の話を目次の位置で探す", () => {
    const { prev, next } = adjacentEpisodes(episodes, 2);
    expect([prev?.id, next?.id]).toEqual([1, 3]);
  });

  it("最初と最後の話では、ない側を null にする", () => {
    expect(adjacentEpisodes(episodes, 1).prev).toBeNull();
    expect(adjacentEpisodes(episodes, 3).next).toBeNull();
  });
});
