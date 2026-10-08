// 画面を作るための合成データ。実在の作品とは関係がない（CLAUDE.md の境界）。
// 取り込み（ADR 0005）を実装したら、SQLite から読むものに置き換える。

import type { Chapter, Episode, Novel, SourceName } from "./models";

type NovelSeed = {
  sourceName: SourceName;
  sourceId: string;
  title: string;
  author: string;
  hasAuthorPage: boolean;
  description: string;
  isConcluded: boolean;
  /** 章ごとの話数。章名が null の章は、章に分かれていない話 */
  chapters: { title: string | null; count: number }[];
  /** 本文を取得済みの話数（先頭から） */
  fetched: number;
  /** 送信済みの話数（先頭から） */
  sent: number;
  /** 最新話の公開日時 */
  latest: string;
  /** 話の間隔（時間） */
  intervalHours: number;
};

const seeds: NovelSeed[] = [
  {
    sourceName: "narou",
    sourceId: "n0001aa",
    title: "合成データ：灯台守と七つの鍵",
    author: "見本 太郎",
    hasAuthorPage: true,
    description:
      "海辺の町で灯台を守る少年が、七つの鍵を探して旅に出る。\nこれは画面の確認のための合成データであり、実在の作品とは関係がない。",
    isConcluded: false,
    chapters: [
      { title: "第一章　潮騒の町", count: 6 },
      { title: "第二章　霧の向こう", count: 6 },
    ],
    fetched: 10,
    sent: 4,
    latest: "2026-10-07T21:00:00+09:00",
    intervalHours: 24,
  },
  {
    sourceName: "kakuyomu",
    sourceId: "1000000000000000001",
    title: "合成データ：雨宿りの喫茶店",
    author: "試験 花子",
    hasAuthorPage: true,
    description: "雨の日だけ開く喫茶店の、店主と常連客の話。",
    isConcluded: true,
    chapters: [{ title: null, count: 8 }],
    fetched: 8,
    sent: 8,
    latest: "2026-10-07T07:00:00+09:00",
    intervalHours: 48,
  },
  {
    sourceName: "hameln",
    sourceId: "100001",
    title: "合成データ：時計塔の修理人",
    author: "仮名 次郎",
    hasAuthorPage: false,
    description: "止まった時計塔を直すため、修理人が街の歯車を集める。",
    isConcluded: false,
    chapters: [
      { title: "序章", count: 2 },
      { title: "本編", count: 9 },
    ],
    fetched: 7,
    sent: 0,
    latest: "2026-10-06T12:00:00+09:00",
    intervalHours: 72,
  },
  {
    sourceName: "narou",
    sourceId: "n0002bb",
    title: "合成データ：とても長い題名の作品が一覧でどのように折り返されるかを確かめるための見本",
    author: "長い名前の作者による合成データの見本",
    hasAuthorPage: true,
    description: "長い題名と長い作者名が、一覧と詳細でどう表示されるかを確かめるための合成データ。",
    isConcluded: false,
    chapters: [{ title: null, count: 5 }],
    fetched: 5,
    sent: 2,
    latest: "2026-10-05T18:30:00+09:00",
    intervalHours: 24,
  },
  {
    sourceName: "novel18",
    sourceId: "n0003cc",
    title: "合成データ：星図を描く人",
    author: "例示 三郎",
    hasAuthorPage: true,
    description: "夜ごとに星図を描き足す天文台の記録。",
    isConcluded: false,
    chapters: [{ title: null, count: 4 }],
    fetched: 0,
    sent: 0,
    latest: "2026-09-30T09:15:00+09:00",
    intervalHours: 96,
  },
];

const prefaceSample = "前書きの合成データ。";

const bodySample = [
  "　朝の<ruby>霧<rp>(</rp><rt>きり</rt><rp>)</rp></ruby>が晴れてゆく。",
  "　灯台の窓から、港に並ぶ船が見えた。",
  "<br>",
  "「今日は風が強いね」",
  "　少年はそう言って、<span style=\"text-emphasis: sesame; -webkit-text-emphasis: sesame;\">七つ目</span>の鍵を握りしめた。",
  "　これは*強調*と**太字**を含む、画面の確認のための合成データである。",
  "<br>",
  "<br>",
  "　波の音が、少しずつ遠ざかってゆく。",
  "　ＡＢＣ　１２３　全角の英数字も確かめる‼",
].join("\n\n");

const afterwordSample = "後書きの合成データ。";

function buildNovel(seed: NovelSeed, novelId: number, firstEpisodeId: number): Novel {
  const url = `https://example.com/${seed.sourceName}/${seed.sourceId}/`;
  const total = seed.chapters.reduce((acc, chapter) => acc + chapter.count, 0);
  const latest = new Date(seed.latest).getTime();
  let part = 0;

  const chapters: Chapter[] = seed.chapters.map((chapter) => ({
    title: chapter.title,
    episodes: Array.from({ length: chapter.count }, (): Episode => {
      part += 1;
      const published = new Date(latest - (total - part) * seed.intervalHours * 3600 * 1000);
      const fetched = part <= seed.fetched;
      return {
        id: firstEpisodeId + part - 1,
        novelId,
        part,
        url: `${url}${part}/`,
        title: `第${part}話　合成データの話`,
        originCreatedAt: published.toISOString(),
        updatedAt: published.toISOString(),
        bodyFetched: fetched,
        isSent: fetched && part <= seed.sent,
        preface: fetched && part % 3 === 1 ? prefaceSample : null,
        body: fetched ? bodySample : null,
        afterword: fetched && part % 2 === 0 ? afterwordSample : null,
      };
    }),
  }));

  return {
    id: novelId,
    sourceName: seed.sourceName,
    sourceId: seed.sourceId,
    url,
    title: seed.title,
    author: {
      name: seed.author,
      url: seed.hasAuthorPage ? `https://example.com/${seed.sourceName}/authors/${novelId}/` : null,
    },
    description: seed.description,
    episodeUpdatedAt: new Date(latest).toISOString(),
    characterCount: total * 3000,
    isConcluded: seed.isConcluded,
    chapters,
  };
}

function buildNovels(): Novel[] {
  let nextEpisodeId = 1;
  return seeds.map((seed, i) => {
    const novel = buildNovel(seed, i + 1, nextEpisodeId);
    nextEpisodeId += novel.chapters.reduce((acc, chapter) => acc + chapter.episodes.length, 0);
    return novel;
  });
}

export const sampleNovels: Novel[] = buildNovels();
