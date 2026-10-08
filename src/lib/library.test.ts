import { describe, expect, it } from "vitest";
import type { Episode, Novel } from "../models";
import { adjacentEpisodes, latestEpisodes, libraryStats, sortNovels } from "./library";

function episode(id: number, part: number, originCreatedAt: string, bodyFetched = true): Episode {
  return {
    id,
    novelId: 0,
    part,
    url: `https://example.com/${id}`,
    title: `話 ${part}`,
    originCreatedAt,
    updatedAt: originCreatedAt,
    bodyFetched,
    isSent: false,
    preface: null,
    body: null,
    afterword: null,
  };
}

function novel(id: number, episodeUpdatedAt: string, episodes: Episode[]): Novel {
  return {
    id,
    sourceName: "narou",
    sourceId: `n${id}`,
    url: `https://example.com/n${id}`,
    title: `作品 ${id}`,
    author: { name: "作者", url: null },
    description: "",
    episodeUpdatedAt,
    characterCount: 0,
    isConcluded: false,
    chapters: [{ title: null, episodes: episodes.map((e) => ({ ...e, novelId: id })) }],
  };
}

describe("libraryStats", () => {
  it("作品数・話数・未取得の話数と、取得にかかる時間の目安を数える", () => {
    const novels = [
      novel(1, "2026-01-01T00:00:00Z", [
        episode(1, 1, "2026-01-01T00:00:00Z"),
        episode(2, 2, "2026-01-01T00:00:00Z", false),
      ]),
      novel(2, "2026-01-01T00:00:00Z", [episode(3, 1, "2026-01-01T00:00:00Z", false)]),
    ];

    expect(libraryStats(novels)).toEqual({
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

describe("sortNovels", () => {
  it("最新話の新しい順に並べる", () => {
    const novels = [novel(1, "2026-01-01T00:00:00Z", []), novel(2, "2026-02-01T00:00:00Z", [])];
    expect(sortNovels(novels).map((n) => n.id)).toEqual([2, 1]);
  });
});

describe("latestEpisodes", () => {
  // 日付の区切りはローカル時刻で決まるため、時差の影響を受けない正午で作る
  const novels = [
    novel(1, "2026-03-02T12:00:00", [episode(1, 1, "2026-03-01T12:00:00"), episode(2, 2, "2026-03-02T12:00:00")]),
    novel(2, "2026-02-01T12:00:00", [episode(3, 1, "2026-03-02T11:00:00"), episode(4, 2, "2026-02-01T12:00:00")]),
  ].map((n) => ({
    ...n,
    chapters: n.chapters.map((c) => ({
      ...c,
      episodes: c.episodes.map((e) => ({ ...e, originCreatedAt: new Date(e.originCreatedAt).toISOString() })),
    })),
  }));

  it("全作品の話を新しい順に並べ、公開日ごとにまとめる", () => {
    const groups = latestEpisodes(novels, 10);
    expect(groups.map((g) => [g.date, g.items.map((i) => i.episode.id)])).toEqual([
      ["2026-03-02", [2, 3]],
      ["2026-03-01", [1]],
      ["2026-02-01", [4]],
    ]);
  });

  it("件数を絞る", () => {
    const groups = latestEpisodes(novels, 2);
    expect(groups.flatMap((g) => g.items.map((i) => i.episode.id))).toEqual([2, 3]);
  });
});

describe("adjacentEpisodes", () => {
  const n = novel(1, "2026-01-01T00:00:00Z", [
    episode(1, 1, "2026-01-01T00:00:00Z"),
    episode(2, 2, "2026-01-02T00:00:00Z"),
    episode(3, 3, "2026-01-03T00:00:00Z"),
  ]);

  it("前後の話を話数で探す", () => {
    const { prev, next } = adjacentEpisodes(n, 2);
    expect([prev?.id, next?.id]).toEqual([1, 3]);
  });

  it("最初と最後の話では、ない側を null にする", () => {
    expect(adjacentEpisodes(n, 1).prev).toBeNull();
    expect(adjacentEpisodes(n, 3).next).toBeNull();
  });
});
