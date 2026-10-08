import { type ReactNode, useState } from "react";
import { Link } from "react-router";

export const linkClass =
  "text-blue-700 underline underline-offset-2 hover:text-blue-900 dark:text-blue-400 dark:hover:text-blue-300";

export function InternalLink({ to, children }: { to: string; children: ReactNode }) {
  return (
    <Link to={to} className={linkClass}>
      {children}
    </Link>
  );
}

/**
 * ボタンの見た目。ホバーで背景と枠をはっきり濃くし、押した瞬間はさらに濃くする。
 * キーボードで選んだときは枠を出す。押せないときはホバーしても変わらない
 */
const buttonClass = [
  "cursor-pointer rounded border px-2.5 py-1 text-sm transition-colors duration-100",
  "border-neutral-300 bg-white text-neutral-800 shadow-sm",
  "hover:border-neutral-500 hover:bg-neutral-200 hover:text-neutral-950",
  "active:bg-neutral-300",
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600",
  "dark:border-neutral-600 dark:bg-neutral-800 dark:text-neutral-100",
  "dark:hover:border-neutral-400 dark:hover:bg-neutral-600 dark:hover:text-white dark:active:bg-neutral-500",
  "disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none",
  "disabled:hover:border-neutral-300 disabled:hover:bg-white disabled:hover:text-neutral-800",
  "dark:disabled:hover:border-neutral-600 dark:disabled:hover:bg-neutral-800 dark:disabled:hover:text-neutral-100",
].join(" ");

export function Button({
  type = "button",
  disabled = false,
  title,
  onClick,
  children,
}: {
  type?: "button" | "submit";
  disabled?: boolean;
  title?: string;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button type={type} onClick={onClick} disabled={disabled} title={title} className={buttonClass}>
      {children}
    </button>
  );
}

/** 裏の機能がまだないボタン。押せない状態で置く */
export function PendingButton({ children }: { children: ReactNode }) {
  return (
    <Button disabled title="未実装">
      {children}
    </Button>
  );
}

/**
 * 押すと `action` を呼ぶボタン。終わるまで押せなくし、失敗したら理由をボタンの横に出す。
 * `confirm` があれば、押したときにボタンの横に確認の文と「実行する」「やめる」を出し、実行するを押したときに呼ぶ。
 * Tauri の WebView では `window.confirm` のダイアログが出ないため、画面の中で確かめる
 */
export function ActionButton({
  action,
  confirm,
  children,
}: {
  action: () => Promise<unknown>;
  confirm?: string;
  children: ReactNode;
}) {
  const [running, setRunning] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async () => {
    setConfirming(false);
    setRunning(true);
    try {
      await action();
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  };
  const onClick = () => {
    if (confirm != null) {
      setConfirming(true);
      return;
    }
    void run();
  };

  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <Button onClick={onClick} disabled={running || confirming}>
        {children}
      </Button>
      {confirming && (
        <span role="alertdialog" aria-label={confirm} className="inline-flex flex-wrap items-center gap-2 text-sm">
          <span className="text-red-700 dark:text-red-400">{confirm}</span>
          <Button onClick={() => void run()}>実行する</Button>
          <Button onClick={() => setConfirming(false)}>やめる</Button>
        </span>
      )}
      {error && (
        <span role="alert" className="text-sm text-red-600 dark:text-red-400">
          {error}
        </span>
      )}
    </span>
  );
}

export function ButtonGroup({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap gap-2">{children}</div>;
}

/** 取得済み / 全話。全話を取得していなければ赤くする */
export function PartsCount({ fetched, total }: { fetched: number; total: number }) {
  const color = fetched === total ? "" : "text-red-600 dark:text-red-400";
  return (
    <span className={`whitespace-nowrap ${color}`}>
      ({fetched} / {total})
    </span>
  );
}

export function SentMark() {
  return <span className="text-sm text-neutral-500 dark:text-neutral-400">(sent)</span>;
}
