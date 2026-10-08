// 画面が読むデータの入口。
// 作品と話はまだ合成データを返す。SQLite（ADR 0003）を実装したら Tauri のコマンドの呼び出しに置き換える。
// 購読している作品は、Tauri のコマンドで novels.json から読み書きする（ADR 0014）。

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { uniqueId, type Novel, type SourceName, type UnaddedNovel } from "./models";
import { sampleNovels } from "./sampleData";

export function listNovels(): Novel[] {
  return sampleNovels;
}

export function findNovel(id: number): Novel | null {
  return sampleNovels.find((novel) => novel.id === id) ?? null;
}

type SubscriptionItem = { site: SourceName; id: string; url: string };

/** 購読しているが、まだ作品として追加していないもの */
export async function listUnadded(): Promise<UnaddedNovel[]> {
  const items = await invoke<SubscriptionItem[]>("list_subscriptions");
  const added = new Set(listNovels().map(uniqueId));
  return items
    .map((item) => ({ sourceName: item.site, sourceId: item.id, url: item.url }))
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
  /** クローラーが指定されているか */
  configured: boolean;
  /** キューに残っているタスクの数（取得と待ちの両方を数える） */
  queued: number;
};

export function fetchStatus(): Promise<FetchStatus> {
  return invoke<FetchStatus>("fetch_status");
}

/** 全てのキューを破棄し、購読している全作品の目次の取得を積み直す（ADR 0015、ADR 0016）。積んだ数を返す */
export function fetchAll(): Promise<number> {
  return invoke<number>("fetch_all");
}

export type FetchDone = {
  command: "toc" | "episode" | "image";
  url: string;
  /** 失敗したときの理由。成功なら null */
  error: string | null;
};

/** 取得が終わるたびに呼ばれる。戻り値の関数で購読をやめる */
export function onFetchDone(listener: (done: FetchDone) => void): Promise<() => void> {
  return listen<FetchDone>("fetch-done", (event) => listener(event.payload));
}
