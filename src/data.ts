// 画面が読むデータの入口。いまは合成データを返す。
// SQLite（ADR 0003）を実装したら、ここを Tauri のコマンドの呼び出しに置き換える。

import type { Novel, UnaddedNovel } from "./models";
import { sampleNovels, sampleUnadded } from "./sampleData";

export function listNovels(): Novel[] {
  return sampleNovels;
}

export function findNovel(id: number): Novel | null {
  return sampleNovels.find((novel) => novel.id === id) ?? null;
}

export function listUnadded(): UnaddedNovel[] {
  return sampleUnadded;
}
