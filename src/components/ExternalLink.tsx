import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { MouseEvent, ReactNode } from "react";
import { linkClass } from "./ui";

/** サイトのページを、アプリのウィンドウではなく既定のブラウザで開くリンク */
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    if (!isTauri()) {
      return;
    }
    e.preventDefault();
    void openUrl(href);
  };

  return (
    <a href={href} target="_blank" rel="noreferrer" onClick={onClick} className={linkClass}>
      {children}
    </a>
  );
}
