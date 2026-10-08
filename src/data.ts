// 画面が読み書きするデータの入口。全て Tauri のコマンドを呼ぶ（ADR 0014、ADR 0018）。

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  type EpisodeDetail,
  type LatestEpisode,
  type NovelDetail,
  type NovelSummary,
  type SourceName,
  type UnaddedNovel,
  uniqueId,
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

export type FetchSchedule = {
  enabled: boolean;
  /** 毎日の時刻（ローカル時刻、`HH:MM`） */
  at: string;
};

/** 定期取得の設定（ADR 0019） */
export function fetchSchedule(): Promise<FetchSchedule> {
  return invoke<FetchSchedule>("fetch_schedule");
}

/** 定期取得で fetch all を行ったときに呼ばれる。積んだ作品の数か、失敗の理由を受け取る */
export function onScheduledFetch(listener: (result: { Ok: number } | { Err: string }) => void): Promise<() => void> {
  if (!isTauri()) {
    return Promise.resolve(() => undefined);
  }
  return listen<{ Ok: number } | { Err: string }>("scheduled-fetch", (event) => listener(event.payload));
}

/** 管理画面で変える設定（ADR 0020） */
export type Settings = {
  /** クローラーの実行ファイル。null なら自動で探す */
  crawlerPath: string | null;
  /** epub-builder の実行ファイル。null なら自動で探す（ADR 0024） */
  epubBuilderPath: string | null;
  /** send-to-kindle の実行ファイル。null なら自動で探す（ADR 0027） */
  sendToKindlePath: string | null;
  /** send-to-kindle に -e で渡す .env。null ならアプリ用データ領域の .env（ADR 0028） */
  sendToKindleEnvPath: string | null;
  /** .env に要るキーを並べた .env.example（ADR 0028） */
  sendToKindleEnvExamplePath: string | null;
  schedule: { enabled: boolean; at: string };
};

/** .env.example と比べた .env の状態。値は含まない */
export type EnvCheck =
  | { kind: "complete" }
  | { kind: "missing"; keys: string[] }
  | { kind: "unreadable"; error: string };

export type SettingsView = {
  settings: Settings;
  /** 設定のファイルを読めなかったときの理由 */
  loadError: string | null;
  /** いま使っているクローラーの実行ファイル */
  crawlerInUse: string | null;
  /** 実行ファイルを指定しなかったときに自動で見つかるもの */
  crawlerFound: string | null;
  /** いま使う epub-builder の実行ファイル */
  epubBuilderInUse: string | null;
  /** epub-builder を指定しなかったときに自動で見つかるもの */
  epubBuilderFound: string | null;
  /** いま使う send-to-kindle の実行ファイル */
  sendToKindleInUse: string | null;
  /** send-to-kindle を指定しなかったときに自動で見つかるもの */
  sendToKindleFound: string | null;
  /** .env を指定しなかったときに使うもの。環境ごとのアプリ用データ領域の .env（ADR 0028） */
  sendToKindleEnvDefault: string;
  /** .env.example を指定しなかったときに使うもの */
  sendToKindleEnvExampleDefault: string;
  /** .env.example と比べた .env の状態。.env.example がなければ null */
  sendToKindleEnvCheck: EnvCheck | null;
  environment: string;
  subscriptionsPath: string;
  settingsPath: string;
  databasePath: string;
};

export function getSettings(): Promise<SettingsView> {
  return invoke<SettingsView>("get_settings");
}

/** 設定を確かめて保存し、すぐに反映する。値が正しくなければ理由の文字列で reject する */
export function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

/** 作品を購読から外す。取得済みの作品のデータは消さない。外したら true */
export function removeSubscription(site: SourceName, siteId: string): Promise<boolean> {
  return invoke<boolean>("remove_subscription", { siteKey: site, siteId });
}

/** 購読している作品の一覧 */
export async function listSubscriptions(): Promise<UnaddedNovel[]> {
  const items = await invoke<SubscriptionItem[]>("list_subscriptions");
  return items.map((item) => ({ site: item.site, siteId: item.id, url: item.url }));
}
