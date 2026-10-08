// 画面が読み書きするデータの入口。全て Tauri のコマンドを呼ぶ（ADR 0014、ADR 0018）。

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  uniqueId,
  type EpisodeDetail,
  type LatestEpisode,
  type NovelDetail,
  type NovelSummary,
  type SourceName,
  type UnaddedNovel,
} from "./models";

export function listNovels(): Promise<NovelSummary[]> {
  return invoke<NovelSummary[]>("list_novels");
}

export function novelDetail(novelId: number): Promise<NovelDetail | null> {
  return invoke<NovelDetail | null>("novel_detail", { novelId });
}

export function episodeDetail(episodeId: number): Promise<EpisodeDetail | null> {
  return invoke<EpisodeDetail | null>("episode_detail", { episodeId });
}

export function latestEpisodes(limit: number): Promise<LatestEpisode[]> {
  return invoke<LatestEpisode[]>("latest_episodes", { limit });
}

/** 整形の設定を保存する。null なら既定に戻す */
export function setNormalizeOptions(novelId: number, options: Record<string, unknown> | null): Promise<void> {
  return invoke("set_normalize_options", { novelId, options });
}

type SubscriptionItem = { site: SourceName; id: string; url: string };

/** 購読しているが、まだ作品として追加していないもの */
export async function listUnadded(): Promise<UnaddedNovel[]> {
  const [items, novels] = await Promise.all([invoke<SubscriptionItem[]>("list_subscriptions"), listNovels()]);
  const added = new Set(novels.map(uniqueId));
  return items
    .map((item) => ({ site: item.site, siteId: item.id, url: item.url }))
    .filter((item) => !added.has(uniqueId(item)));
}

export type AddSubscriptionResult = {
  subscription: { site: SourceName; id: string };
  /** 新しく加えたか。すでに購読していれば false */
  added: boolean;
};

/** 作品の URL を購読に加える。対応していない URL や novels.json が壊れているときは、理由の文字列で reject する */
export function addSubscription(url: string): Promise<AddSubscriptionResult> {
  return invoke<AddSubscriptionResult>("add_subscription", { url });
}

/** データの置き場所を決めている環境（ADR 0014）。Tauri の外では、フロントエンドのビルドの種類を返す */
export async function getEnvironment(): Promise<string> {
  return isTauri() ? invoke<string>("environment") : import.meta.env.MODE;
}

export type FetchStatus = {
  /** クローラーが見つかったか */
  configured: boolean;
  /** キューに残っているタスクの数（取得と待ちの両方を数える） */
  queued: number;
};

export function fetchStatus(): Promise<FetchStatus> {
  return invoke<FetchStatus>("fetch_status");
}

/** 全てのキューを破棄し、購読している全作品の目次、本文、挿絵の取得を積み直す（ADR 0018）。作品の数を返す */
export function fetchAll(): Promise<number> {
  return invoke<number>("fetch_all");
}

/** 全てのキューを破棄し、購読している全作品の目次だけの取得を積み直す（ADR 0018）。作品の数を返す */
export function fetchAllMetadata(): Promise<number> {
  return invoke<number>("fetch_all_metadata");
}

/** 購読している作品の目次を取得して、作品として加える */
export function addNovel(site: SourceName, siteId: string): Promise<void> {
  return invoke("add_novel", { siteKey: site, siteId });
}

/** 作品の目次を取り直し、未取得と改稿された話の本文と挿絵を取得する */
export function fetchNovel(novelId: number): Promise<void> {
  return invoke("fetch_novel", { novelId });
}

/** 話の本文を取り直す */
export function refetchEpisode(episodeId: number): Promise<void> {
  return invoke("refetch_episode", { episodeId });
}

/** 作品の話を全て消す。消した数を返す */
export function removeEpisodes(novelId: number): Promise<number> {
  return invoke<number>("remove_episodes", { novelId });
}

export type FetchDone = {
  command: "toc" | "episode" | "image";
  url: string;
  /** 取得または保存に失敗したときの理由。成功なら null */
  error: string | null;
};

/** 取得が終わるたびに呼ばれる。戻り値の関数で購読をやめる */
export function onFetchDone(listener: (done: FetchDone) => void): Promise<() => void> {
  if (!isTauri()) {
    return Promise.resolve(() => undefined);
  }
  return listen<FetchDone>("fetch-done", (event) => listener(event.payload));
}
