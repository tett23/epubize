// Tauri のコマンドの偽物（ADR 0022）。ページの読み込みの前に入れ、`window.__TAURI_INTERNALS__` を作る。
// 状態はページの中に持ち、テストからは `window.__epubize` で呼ばれたコマンドを調べ、イベントを流す。
// データは全て合成したもの。

import type { Page } from "@playwright/test";

type Novel = {
  id: number;
  site: string;
  siteId: string;
  url: string;
  title: string;
  authorName: string;
  authorUrl: string | null;
  isConcluded: boolean | null;
  latestPublishedAt: string | null;
  fetched: number;
  total: number;
};

type Episode = {
  id: number;
  novelId: number;
  no: number;
  url: string;
  title: string;
  chapter: string | null;
  publishedAt: string | null;
  revisedAt: string | null;
  bodyFetchedAt: string | null;
  sentAt: string | null;
};

export type BackendState = {
  crawlerFound: string | null;
  settings: { crawlerPath: string | null; schedule: { enabled: boolean; at: string } };
  subscriptions: { site: string; id: string; url: string }[];
  novels: (Novel & { description: string; episodes: Episode[] })[];
  bodies: Record<number, { body: string; images: Record<string, string> }>;
  queued: number;
  /** コマンドの名前から、そのコマンドが失敗したときの理由へ */
  failures?: Record<string, string>;
  /** 設定のファイルを読めなかったときの理由 */
  settingsLoadError?: string | null;
};

export function defaultState(): BackendState {
  const episodes: Episode[] = [1, 2].map((no) => ({
    id: no,
    novelId: 1,
    no,
    url: `https://ncode.syosetu.com/n0001aa/${no}/`,
    title: `第${no}話　合成データの話`,
    chapter: "第一章",
    publishedAt: `2026-01-0${no}T12:00:00Z`,
    revisedAt: null,
    bodyFetchedAt: no === 1 ? "2026-01-03T00:00:00Z" : null,
    sentAt: null,
  }));
  return {
    crawlerFound: "/usr/local/bin/novel-crawler",
    settings: { crawlerPath: null, schedule: { enabled: true, at: "03:00" } },
    subscriptions: [
      { site: "narou", id: "n0001aa", url: "https://ncode.syosetu.com/n0001aa/" },
      { site: "narou", id: "n0002bb", url: "https://ncode.syosetu.com/n0002bb/" },
    ],
    novels: [
      {
        id: 1,
        site: "narou",
        siteId: "n0001aa",
        url: "https://ncode.syosetu.com/n0001aa/",
        title: "合成データの作品",
        authorName: "見本 太郎",
        authorUrl: null,
        isConcluded: true,
        latestPublishedAt: "2026-01-02T12:00:00Z",
        fetched: 1,
        total: 2,
        description: "合成データのあらすじ。",
        episodes,
      },
    ],
    bodies: {
      1: {
        body: "　本文の合成データ。\n\n![地図](https://example.com/a.png)\n\n![図](https://example.com/b.png)",
        images: { "https://example.com/a.png": "data:image/png;base64,AA==" },
      },
    },
    queued: 0,
  };
}

/** 偽物を入れる。`page.goto` の前に呼ぶ */
export async function installBackend(page: Page, state: BackendState = defaultState()) {
  await page.addInitScript((initial: BackendState) => {
    const state = structuredClone(initial);
    const calls: { cmd: string; args: Record<string, unknown> }[] = [];
    const callbacks = new Map<number, (payload: unknown) => void>();
    const listeners = new Map<string, number[]>();
    let nextId = 1;

    const summary = (n: BackendState["novels"][number]) => {
      const { description: _d, episodes: _e, ...rest } = n;
      return {
        ...rest,
        fetched: n.episodes.filter((e) => e.bodyFetchedAt != null).length,
        total: n.episodes.length,
      };
    };
    const fail = (message: string) => {
      throw message;
    };

    const commands: Record<string, (args: Record<string, unknown>) => unknown> = {
      environment: () => "development",
      list_subscriptions: () => state.subscriptions,
      add_subscription: ({ url }) => {
        const m = String(url).match(/^https:\/\/ncode\.syosetu\.com\/(n[0-9a-z]+)/);
        if (!m) fail(`対応していない URL です: ${url}`);
        const id = (m as RegExpMatchArray)[1];
        const added = !state.subscriptions.some((s) => s.id === id);
        if (added) state.subscriptions.push({ site: "narou", id, url: `https://ncode.syosetu.com/${id}/` });
        return { subscription: { site: "narou", id }, added };
      },
      remove_subscription: ({ siteKey, siteId }) => {
        const before = state.subscriptions.length;
        state.subscriptions = state.subscriptions.filter((s) => !(s.site === siteKey && s.id === siteId));
        return state.subscriptions.length < before;
      },
      get_settings: () => ({
        settings: state.settings,
        loadError: state.settingsLoadError ?? null,
        crawlerInUse: state.settings.crawlerPath ?? state.crawlerFound,
        crawlerFound: state.crawlerFound,
        environment: "development",
        subscriptionsPath: "/tmp/epubize/novels.json",
        settingsPath: "/tmp/epubize/settings.json",
        databasePath: "/tmp/epubize/epubize.sqlite3",
      }),
      save_settings: ({ settings }) => {
        const next = settings as BackendState["settings"];
        if (!/^\d{2}:\d{2}$/.test(next.schedule.at)) fail(`時刻は HH:MM で指定してください: "${next.schedule.at}"`);
        if (next.crawlerPath != null && !next.crawlerPath.startsWith("/usr/")) {
          fail(`クローラーの実行ファイルが見つからないか、実行できません: ${next.crawlerPath}`);
        }
        state.settings = next;
        return null;
      },
      fetch_status: () => ({
        configured: (state.settings.crawlerPath ?? state.crawlerFound) != null,
        queued: state.queued,
      }),
      fetch_schedule: () => state.settings.schedule,
      fetch_all: () => state.subscriptions.length,
      fetch_all_metadata: () => state.subscriptions.length,
      add_novel: () => null,
      fetch_novel: () => null,
      refetch_episode: () => null,
      remove_episodes: ({ novelId }) => {
        const novel = state.novels.find((n) => n.id === novelId);
        const removed = novel?.episodes.length ?? 0;
        if (novel) novel.episodes = [];
        return removed;
      },
      list_novels: () => state.novels.map(summary),
      novel_detail: ({ novelId }) => {
        const n = state.novels.find((x) => x.id === novelId);
        if (!n) return null;
        return {
          ...summary(n),
          description: n.description,
          charCount: 10,
          metadataFetchedAt: "2026-01-03T00:00:00Z",
          normalizeOptions: null,
          episodes: n.episodes,
        };
      },
      episode_detail: ({ episodeId }) => {
        for (const n of state.novels) {
          const e = n.episodes.find((x) => x.id === episodeId);
          if (e) {
            const body = state.bodies[e.id];
            return {
              ...e,
              preface: null,
              body: body?.body ?? null,
              afterword: null,
              charCount: 10,
              images: body?.images ?? {},
            };
          }
        }
        return null;
      },
      latest_episodes: () =>
        state.novels.flatMap((n) =>
          n.episodes.map((e) => ({
            novelId: n.id,
            site: n.site,
            siteId: n.siteId,
            novelUrl: n.url,
            novelTitle: n.title,
            episode: e,
          })),
        ),
      set_normalize_options: () => null,
      "plugin:event|listen": ({ event, handler }) => {
        const list = listeners.get(String(event)) ?? [];
        list.push(Number(handler));
        listeners.set(String(event), list);
        return Number(handler);
      },
      "plugin:event|unlisten": ({ event, eventId }) => {
        const list = listeners.get(String(event)) ?? [];
        listeners.set(
          String(event),
          list.filter((id) => id !== Number(eventId)),
        );
        return null;
      },
      "plugin:opener|open_url": () => null,
    };

    const w = window as unknown as Record<string, unknown>;
    w.isTauri = true;
    w.__TAURI_INTERNALS__ = {
      transformCallback(callback: (payload: unknown) => void) {
        const id = nextId++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
      async invoke(cmd: string, args: Record<string, unknown> = {}) {
        calls.push({ cmd, args });
        const failure = state.failures?.[cmd];
        if (failure != null) throw failure;
        const command = commands[cmd];
        if (!command) throw `unknown command: ${cmd}`;
        return command(args);
      },
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
    w.__epubize = {
      calls,
      state,
      emit(event: string, payload: unknown) {
        for (const id of listeners.get(event) ?? []) {
          callbacks.get(id)?.({ event, id, payload });
        }
      },
    };
  }, state);
}

/** 呼ばれたコマンドの名前の一覧 */
export function calledCommands(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (window as unknown as { __epubize: { calls: { cmd: string }[] } }).__epubize.calls.map((c) => c.cmd),
  );
}

/** イベントを流す */
export function emit(page: Page, event: string, payload: unknown): Promise<void> {
  return page.evaluate(
    ([e, p]) => (window as unknown as { __epubize: { emit: (e: string, p: unknown) => void } }).__epubize.emit(e, p),
    [event, payload] as const,
  );
}

/** 呼ばれたコマンドと引数の一覧。イベントの購読などの内部のコマンドは除く */
export function calls(page: Page): Promise<{ cmd: string; args: Record<string, unknown> }[]> {
  return page.evaluate(() =>
    (
      window as unknown as { __epubize: { calls: { cmd: string; args: Record<string, unknown> }[] } }
    ).__epubize.calls.filter((c) => !c.cmd.startsWith("plugin:")),
  );
}
