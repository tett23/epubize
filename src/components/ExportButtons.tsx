import { downloadEpub, downloadZip, type ExportScope, revealPath, sendToKindle } from "../data";
import { ActionButton } from "./ui";

/** 書き出したファイルの名前と、Finder で表示するボタン */
function Saved({ path }: { path: string }) {
  const name = path.split("/").pop() ?? path;
  return (
    <>
      <span title={path}>保存しました: {name}</span>{" "}
      <button type="button" className="cursor-pointer underline" onClick={() => void revealPath(path)}>
        Finder で表示
      </button>
    </>
  );
}

/** EPUB 3.0 を作り、ダウンロードのディレクトリに書く（ADR 0029） */
export function DownloadEpubButton({ scope }: { scope: ExportScope }) {
  return (
    <ActionButton action={() => downloadEpub(scope)} done={(path) => <Saved path={path} />}>
      download epub
    </ActionButton>
  );
}

/** epub-builder のプロジェクト（Markdown と挿絵）を zip にして、ダウンロードのディレクトリに書く */
export function DownloadZipButton({ scope }: { scope: ExportScope }) {
  return (
    <ActionButton action={() => downloadZip(scope)} done={(path) => <Saved path={path} />}>
      download zip
    </ActionButton>
  );
}

/** EPUB 3.0 を作って Kindle に送る。送った後に `onSent` で送った印を読み直す */
export function SendToKindleButton({
  scope,
  confirm,
  onSent,
}: {
  scope: ExportScope;
  confirm?: string;
  onSent: () => void;
}) {
  return (
    <ActionButton
      action={async () => {
        const count = await sendToKindle(scope);
        onSent();
        return count;
      }}
      confirm={confirm}
      done={(count) => `送信しました（${count} 話）`}
    >
      send to Kindle
    </ActionButton>
  );
}
