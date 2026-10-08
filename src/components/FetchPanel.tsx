import { useEffect, useState } from "react";
import {
  type FetchDone,
  type FetchSchedule,
  type FetchStatus,
  fetchAll,
  fetchAllMetadata,
  fetchSchedule,
  fetchStatus,
  onFetchDone,
  onScheduledFetch,
} from "../data";
import { Button, ButtonGroup } from "./ui";

/** 画面に残す取得の結果の数 */
const MAX_RESULTS = 20;

/** キューの残りを読み直す間隔 */
const POLL_MS = 1000;

type Message = { kind: "info" | "error"; text: string };

/** fetch all と fetch all metadata、取得のキューの状態、定期取得（ADR 0015、ADR 0016、ADR 0018、ADR 0019） */
export function FetchPanel() {
  const [status, setStatus] = useState<FetchStatus | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  /** 取得の結果。`seq` は表示の key に使う通し番号 */
  const [results, setResults] = useState<(FetchDone & { seq: number })[]>([]);
  const [schedule, setSchedule] = useState<FetchSchedule | null>(null);

  useEffect(() => {
    fetchSchedule().then(setSchedule, () => setSchedule(null));
    const unlisten = onScheduledFetch((result) =>
      setMessage(
        "Ok" in result
          ? { kind: "info", text: `定期取得: キューを空にして、${result.Ok} 作品の目次、本文、挿絵の取得を積みました` }
          : { kind: "error", text: `定期取得: ${result.Err}` },
      ),
    );
    return () => {
      void unlisten.then(
        (stop) => stop(),
        () => undefined,
      );
    };
  }, []);

  useEffect(() => {
    const load = () => fetchStatus().then(setStatus, () => setStatus(null));
    load();
    const timer = setInterval(load, POLL_MS);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    let seq = 0;
    const unlisten = onFetchDone((done) => {
      seq += 1;
      const item = { ...done, seq };
      setResults((prev) => [item, ...prev].slice(0, MAX_RESULTS));
    });
    return () => {
      void unlisten.then(
        (stop) => stop(),
        () => undefined,
      );
    };
  }, []);

  const run = (start: () => Promise<number>, what: string) => async () => {
    try {
      const count = await start();
      setMessage({ kind: "info", text: `キューを空にして、${count} 作品の${what}の取得を積みました` });
      setStatus(await fetchStatus());
    } catch (err) {
      setMessage({ kind: "error", text: String(err) });
    }
  };

  return (
    <section className="space-y-2">
      <div className="flex flex-wrap items-center gap-4">
        <ButtonGroup>
          <Button onClick={run(fetchAll, "目次、本文、挿絵")} disabled={status?.configured === false}>
            fetch all
          </Button>
          <Button onClick={run(fetchAllMetadata, "目次")} disabled={status?.configured === false}>
            fetch all metadata
          </Button>
        </ButtonGroup>
        {status && (
          <span className="text-sm text-neutral-600 dark:text-neutral-400">
            {status.configured ? `キュー: ${status.queued} 件` : "クローラーが見つかりません（PATH の novel-crawler）"}
          </span>
        )}
        {schedule?.enabled && (
          <span className="text-sm text-neutral-600 dark:text-neutral-400">毎日 {schedule.at} に fetch all</span>
        )}
      </div>
      {message && (
        <p
          role={message.kind === "error" ? "alert" : "status"}
          className={`text-sm ${message.kind === "error" ? "text-red-600 dark:text-red-400" : "text-neutral-600 dark:text-neutral-400"}`}
        >
          {message.text}
        </p>
      )}
      {results.length > 0 && (
        <ul className="max-h-48 space-y-0.5 overflow-y-auto rounded border border-neutral-200 p-2 text-sm dark:border-neutral-700">
          {results.map((result) => (
            <li key={result.seq} className={result.error ? "text-red-600 dark:text-red-400" : ""}>
              {result.command} {result.url} {result.error ? `: ${result.error}` : "ok"}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
