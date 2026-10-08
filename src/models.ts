export type SourceName = "narou" | "novel18" | "hameln" | "kakuyomu";

export type Author = {
  name: string;
  /** 作者のページ。サイトが示さない場合は null */
  url: string | null;
};

export type Episode = {
  id: number;
  novelId: number;
  /** 目次の上での話数（1 始まり） */
  part: number;
  url: string;
  title: string;
  /** 公開日時（ISO 8601） */
  originCreatedAt: string;
  /** 改稿日時（ISO 8601） */
  updatedAt: string;
  /** 本文を取得済みか */
  bodyFetched: boolean;
  /** Kindle に送信済みか */
  isSent: boolean;
  /** 本文・前書き・後書き（CommonMark、ADR 0005・ADR 0009）。未取得なら null */
  preface: string | null;
  body: string | null;
  afterword: string | null;
};

export type Chapter = {
  /** 章の名前。章に分かれていない話は null */
  title: string | null;
  episodes: Episode[];
};

export type Novel = {
  id: number;
  sourceName: SourceName;
  /** サイトの上での作品 ID */
  sourceId: string;
  url: string;
  title: string;
  author: Author;
  description: string;
  /** 最新話の公開日時（ISO 8601） */
  episodeUpdatedAt: string;
  characterCount: number;
  isConcluded: boolean;
  chapters: Chapter[];
};

/** 購読しているが、まだ追加していない作品 */
export type UnaddedNovel = {
  sourceName: SourceName;
  sourceId: string;
  url: string;
};

export function uniqueId(item: { sourceName: SourceName; sourceId: string }): string {
  return `${item.sourceName}-${item.sourceId}`;
}

export function episodesOf(novel: Novel): Episode[] {
  return novel.chapters.flatMap((chapter) => chapter.episodes);
}

export type Parts = { fetched: number; total: number };

export function partsOf(novel: Novel): Parts {
  const episodes = episodesOf(novel);
  return {
    fetched: episodes.filter((episode) => episode.bodyFetched).length,
    total: episodes.length,
  };
}
