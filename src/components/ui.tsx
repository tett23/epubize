import type { ReactNode } from "react";
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

/** 裏の機能がまだないボタン。押せない状態で置く */
export function PendingButton({ children }: { children: ReactNode }) {
  return (
    <button
      type="button"
      disabled
      title="未実装"
      className="rounded border border-neutral-300 bg-neutral-50 px-2.5 py-1 text-sm text-neutral-700 disabled:cursor-not-allowed disabled:opacity-60 dark:border-neutral-600 dark:bg-neutral-800 dark:text-neutral-200"
    >
      {children}
    </button>
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
