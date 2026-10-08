import { useEffect, useState } from "react";
import { fetchAll, fetchStatus, onFetchDone, type FetchDone, type FetchStatus } from "../data";
import { Button, ButtonGroup, PendingButton } from "./ui";

/** 画面に残す取得の結果の数 */
const MAX_RESULTS = 20;

/** キューの残りを読み直す間隔 */
const POLL_MS = 1000;

type Message = { kind: "info" | "error"; text: string };

/** fetch all と、取得のキューの状態（ADR 0015、ADR 0016） */
export function FetchPanel() {
  const [status, setStatus] = useState<FetchStatus | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [results, setResults] = useState<FetchDone[]>([]);

  useEffect(() => {
    const load = () => fetchStatus().then(setStatus, () => setStatus(null));
    load();
    const timer = setInterval(load, POLL_MS);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    const unlisten = onFetchDone((done) => setResults((prev) => [done, ...prev].slice(0, MAX_RESULTS)));
    return () => {
      void unlisten.then((stop) => stop(), () => undefined);
    };
  }, []);

  const onClick = async () => {
    try {
      const count = await fetchAll();
      setMessage({ kind: "info", text: `キューを空にして、${count} 作品の目次の取得を積みました` });
      setStatus(await fetchStatus());
    } catch (err) {
      setMessage({ kind: "error", text: String(err) });
    }
  };

  return (
    <section className="space-y-2">
      <div className="flex flex-wrap items-center gap-4">
        <ButtonGroup>
          <Button onClick={onClick} disabled={status?.configured === false}>
            fetch all
          </Button>
          <PendingButton>fetch all metadata</PendingButton>
        </ButtonGroup>
        {status && (
          <span className="text-sm text-neutral-600 dark:text-neutral-400">
            {status.configured ? `キュー: ${status.queued} 件` : "クローラーが指定されていません（EPUBIZE_CRAWLER）"}
          </span>
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
          {results.map((result, i) => (
            <li key={i} className={result.error ? "text-red-600 dark:text-red-400" : ""}>
              {result.command} {result.url} {result.error ? `: ${result.error}` : "ok"}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
