import type { EpisodeSummary, LatestEpisode, NovelSummary } from "../models";
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

export function libraryStats(novels: NovelSummary[]): LibraryStats {
  const total = novels.reduce((acc, n) => acc + n.total, 0);
  const fetched = novels.reduce((acc, n) => acc + n.fetched, 0);
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

export type LatestGroup = { date: string; items: LatestEpisode[] };

/** 公開日時の新しい順に並んだ話を、公開日（ローカル時刻）ごとにまとめる */
export function groupByDate(items: LatestEpisode[]): LatestGroup[] {
  const groups: LatestGroup[] = [];
  for (const item of items) {
    if (item.episode.publishedAt == null) {
      continue;
    }
    const date = formatDate(item.episode.publishedAt);
    const last = groups.at(-1);
    if (last?.date === date) {
      last.items.push(item);
    } else {
      groups.push({ date, items: [item] });
    }
  }
  return groups;
}

export type Chapter = { title: string | null; episodes: EpisodeSummary[] };

/** 目次の順に並んだ話を、続く話で章の名前が同じものごとにまとめる */
export function groupChapters(episodes: EpisodeSummary[]): Chapter[] {
  const chapters: Chapter[] = [];
  for (const episode of episodes) {
    const last = chapters.at(-1);
    if (last != null && last.title === episode.chapter) {
      last.episodes.push(episode);
    } else {
      chapters.push({ title: episode.chapter, episodes: [episode] });
    }
  }
  return chapters;
}

/** 前後の話。目次の位置で探し、ない場合は null */
export function adjacentEpisodes(
  episodes: EpisodeSummary[],
  no: number,
): { prev: EpisodeSummary | null; next: EpisodeSummary | null } {
  return {
    prev: episodes.find((episode) => episode.no === no - 1) ?? null,
    next: episodes.find((episode) => episode.no === no + 1) ?? null,
  };
}
