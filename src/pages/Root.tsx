import { useCallback, useEffect, useState, type FormEvent } from "react";
import { ExternalLink } from "../components/ExternalLink";
import { FetchPanel } from "../components/FetchPanel";
import { AuthorLink, NovelSiteLink, novelPath } from "../components/links";
import { Button, InternalLink, PartsCount, PendingButton } from "../components/ui";
import { addSubscription, listNovels, listUnadded } from "../data";
import { formatDateTime } from "../lib/format";
import { libraryStats, sortNovels } from "../lib/library";
import { partsOf, uniqueId, type Novel, type UnaddedNovel } from "../models";

export function Root() {
  const novels = sortNovels(listNovels());
  const unadded = useUnadded();

  return (
    <div className="space-y-6">
      <AddNovelForm onAdded={unadded.reload} />
      <FetchPanel />
      <Stats novels={novels} />
      <Novels novels={novels} />
      <Unadded {...unadded} />
    </div>
  );
}

type Message = { kind: "info" | "error"; text: string };

/** 作品の URL を購読に加える（ADR 0014） */
function AddNovelForm({ onAdded }: { onAdded: () => void }) {
  const [url, setUrl] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<Message | null>(null);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      const { subscription, added } = await addSubscription(url);
      const id = `${subscription.site}-${subscription.id}`;
      setMessage({ kind: "info", text: added ? `${id} を購読に加えました` : `${id} はすでに購読しています` });
      setUrl("");
      onAdded();
    } catch (err) {
      setMessage({ kind: "error", text: String(err) });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-1">
      <form className="flex gap-2" onSubmit={onSubmit}>
        <input
          type="url"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="作品の URL"
          className="w-96 max-w-full rounded border border-neutral-300 px-2 py-1 dark:border-neutral-600 dark:bg-neutral-800"
        />
        <Button type="submit" disabled={url.trim() === "" || submitting}>
          add
        </Button>
      </form>
      {message && (
        <p
          role={message.kind === "error" ? "alert" : "status"}
          className={`text-sm ${message.kind === "error" ? "text-red-600 dark:text-red-400" : "text-neutral-600 dark:text-neutral-400"}`}
        >
          {message.text}
        </p>
      )}
    </div>
  );
}

function Stats({ novels }: { novels: Novel[] }) {
  const s = libraryStats(novels);
  return (
    <p className="text-sm text-neutral-700 dark:text-neutral-300">
      本: {s.novels}（{s.novelsMinutes}分）　部分: {s.parts}（{s.partsHours}時間）　未取得: {s.unfetched}（
      {s.unfetchedMinutes}分）
    </p>
  );
}

function Novels({ novels }: { novels: Novel[] }) {
  return (
    <table className="w-full border-collapse">
      <tbody className="divide-y divide-neutral-200 dark:divide-neutral-700">
        {novels.map((novel) => (
          <tr key={novel.id} className="align-top hover:bg-neutral-50 dark:hover:bg-neutral-800">
            <td className="py-2 pr-4 whitespace-nowrap tabular-nums">{formatDateTime(novel.episodeUpdatedAt)}</td>
            <td className="py-2 pr-4 whitespace-nowrap">
              <NovelSiteLink novel={novel}>{uniqueId(novel)}</NovelSiteLink>
            </td>
            <td className="w-[20vw] py-2 pr-4">
              <AuthorLink author={novel.author} />
            </td>
            <td className="py-2">
              <InternalLink to={novelPath(novel)}>{novel.title}</InternalLink> <PartsCount {...partsOf(novel)} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

type UnaddedState = {
  items: UnaddedNovel[];
  error: string | null;
  reload: () => void;
};

function useUnadded(): UnaddedState {
  const [items, setItems] = useState<UnaddedNovel[]>([]);
  const [error, setError] = useState<string | null>(null);
  const reload = useCallback(() => {
    listUnadded().then(
      (value) => {
        setItems(value);
        setError(null);
      },
      (err: unknown) => setError(String(err)),
    );
  }, []);
  useEffect(reload, [reload]);

  return { items, error, reload };
}

function Unadded({ items, error }: UnaddedState) {
  if (error == null && items.length === 0) {
    return null;
  }

  return (
    <section className="space-y-2">
      <h2 className="text-lg font-bold">未取得</h2>
      {error != null && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          購読の一覧を読めません: {error}
        </p>
      )}
      <table className="border-collapse">
        <tbody>
          {items.map((item) => (
            <tr key={uniqueId(item)}>
              <td className="py-1 pr-4">
                <ExternalLink href={item.url}>{uniqueId(item)}</ExternalLink>
              </td>
              <td className="py-1">
                <PendingButton>add</PendingButton>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
