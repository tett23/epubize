// 画面が扱うデータの形。Tauri のコマンドが返す JSON と同じ（src-tauri/src/library/query.rs、ADR 0018）

export type SourceName = "narou" | "novel18" | "hameln" | "kakuyomu";

export type NovelSummary = {
  id: number;
  site: SourceName;
  siteId: string;
  url: string;
  title: string;
  authorName: string;
  /** 作者のページ。サイトが示さない場合は null */
  authorUrl: string | null;
  /** 完結済みか。分からなければ null */
  isConcluded: boolean | null;
  /** 最新話の公開日時（ISO 8601） */
  latestPublishedAt: string | null;
  /** 本文を取得済みの話数 */
  fetched: number;
  /** 目次の話数 */
  total: number;
};

export type EpisodeSummary = {
  id: number;
  novelId: number;
  /** 目次の上での 1 始まりの位置 */
  no: number;
  url: string;
  title: string;
  /** 章の名前。続く話で同じ名前なら同じ章 */
  chapter: string | null;
  publishedAt: string | null;
  revisedAt: string | null;
  /** 本文を取得した日時。未取得なら null */
  bodyFetchedAt: string | null;
  /** Kindle に送った日時 */
  sentAt: string | null;
};

export type NovelDetail = NovelSummary & {
  description: string;
  /** 取得済みの話の文字数の合計 */
  charCount: number;
  metadataFetchedAt: string;
  /** 整形の設定。null なら既定 */
  normalizeOptions: Record<string, unknown> | null;
  episodes: EpisodeSummary[];
};

export type EpisodeDetail = EpisodeSummary & {
  preface: string | null;
  body: string | null;
  afterword: string | null;
  charCount: number | null;
  /** 取得済みの挿絵。URL から data URL へ */
  images: Record<string, string>;
};

export type LatestEpisode = {
  novelId: number;
  site: SourceName;
  siteId: string;
  novelUrl: string;
  novelTitle: string;
  episode: EpisodeSummary;
};

/** 購読しているが、まだ作品として追加していない作品 */
export type UnaddedNovel = {
  site: SourceName;
  siteId: string;
  url: string;
};

export function uniqueId(item: { site: SourceName; siteId: string }): string {
  return `${item.site}-${item.siteId}`;
}
