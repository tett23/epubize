import { episodesOf, partsOf, type Episode, type Novel } from "../models";
import { formatDate } from "./format";

/** 1 回の取得にかかる秒数の目安。取得の間隔（ADR 0015）に揃える */
const SECONDS_PER_FETCH = 5;

export type LibraryStats = {
  novels: number;
  /** 全作品の目次を取り直すのにかかる分数 */
  novelsMinutes: number;
  parts: number;
  /** 全話を取り直すのにかかる時間数 */
  partsHours: number;
  unfetched: number;
  /** 未取得の話を取るのにかかる分数 */
  unfetchedMinutes: number;
};

export function libraryStats(novels: Novel[]): LibraryStats {
  const parts = novels.map(partsOf);
  const total = parts.reduce((acc, p) => acc + p.total, 0);
  const fetched = parts.reduce((acc, p) => acc + p.fetched, 0);
  const unfetched = total - fetched;

  return {
    novels: novels.length,
    novelsMinutes: Math.ceil((novels.length * SECONDS_PER_FETCH) / 60),
    parts: total,
    partsHours: Math.ceil((total * SECONDS_PER_FETCH) / 3600),
    unfetched,
    unfetchedMinutes: Math.ceil((unfetched * SECONDS_PER_FETCH) / 60),
  };
}

/** 最新話の新しい順 */
export function sortNovels(novels: Novel[]): Novel[] {
  return [...novels].sort((a, b) => b.episodeUpdatedAt.localeCompare(a.episodeUpdatedAt));
}

export type LatestEpisode = { novel: Novel; episode: Episode };

export type LatestGroup = { date: string; items: LatestEpisode[] };

/** 全作品の話を公開日時の新しい順に並べ、公開日ごとにまとめる */
export function latestEpisodes(novels: Novel[], limit: number): LatestGroup[] {
  const items = novels
    .flatMap((novel) => episodesOf(novel).map((episode) => ({ novel, episode })))
    .sort((a, b) => b.episode.originCreatedAt.localeCompare(a.episode.originCreatedAt))
    .slice(0, limit);

  const groups: LatestGroup[] = [];
  for (const item of items) {
    const date = formatDate(item.episode.originCreatedAt);
    const last = groups.at(-1);
    if (last?.date === date) {
      last.items.push(item);
    } else {
      groups.push({ date, items: [item] });
    }
  }
  return groups;
}

/** 前後の話。話数で探し、ない場合は null */
export function adjacentEpisodes(
  novel: Novel,
  part: number,
): { prev: Episode | null; next: Episode | null } {
  const episodes = episodesOf(novel);
  return {
    prev: episodes.find((episode) => episode.part === part - 1) ?? null,
    next: episodes.find((episode) => episode.part === part + 1) ?? null,
  };
}
