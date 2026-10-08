import { useState, type ReactNode } from "react";
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
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      title={title}
      className="rounded border border-neutral-300 bg-neutral-50 px-2.5 py-1 text-sm text-neutral-700 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-neutral-50 dark:border-neutral-600 dark:bg-neutral-800 dark:text-neutral-200 dark:hover:bg-neutral-700"
    >
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
 * `confirm` があれば、押したときに確かめてから呼ぶ
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
  const [error, setError] = useState<string | null>(null);
  const onClick = async () => {
    if (confirm != null && !window.confirm(confirm)) {
      return;
    }
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

  return (
    <span className="inline-flex items-center gap-2">
      <Button onClick={onClick} disabled={running}>
        {children}
      </Button>
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
