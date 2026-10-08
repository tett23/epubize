import { useState, type FormEvent } from "react";
import { ExternalLink } from "../components/ExternalLink";
import { FetchPanel } from "../components/FetchPanel";
import { AuthorLink, novelPath } from "../components/links";
import { ActionButton, Button, InternalLink, PartsCount } from "../components/ui";
import { addNovel, addSubscription, listNovels, listUnadded } from "../data";
import { useLoad } from "../hooks";
import { formatDateTime } from "../lib/format";
import { libraryStats } from "../lib/library";
import { uniqueId, type NovelSummary, type UnaddedNovel } from "../models";

export function Root() {
  const novels = useLoad(listNovels, []);
  const unadded = useLoad(listUnadded, []);

  return (
    <div className="space-y-6">
      <AddNovelForm onAdded={unadded.reload} />
      <FetchPanel />
      {novels.error && <LoadError error={novels.error} />}
      {novels.data && (
        <>
          <Stats novels={novels.data} />
          <Novels novels={novels.data} />
        </>
      )}
      <Unadded items={unadded.data ?? []} error={unadded.error} />
    </div>
  );
}

export function LoadError({ error }: { error: string }) {
  return (
    <p role="alert" className="text-sm text-red-600 dark:text-red-400">
      読み込めません: {error}
    </p>
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

function Stats({ novels }: { novels: NovelSummary[] }) {
  const s = libraryStats(novels);
  return (
    <p className="text-sm text-neutral-700 dark:text-neutral-300">
      本: {s.novels}（{s.novelsMinutes}分）　部分: {s.parts}（{s.partsHours}時間）　未取得: {s.unfetched}（
      {s.unfetchedMinutes}分）
    </p>
  );
}

function Novels({ novels }: { novels: NovelSummary[] }) {
  if (novels.length === 0) {
    return (
      <p className="text-sm text-neutral-600 dark:text-neutral-400">
        作品はまだありません。未取得の作品の add か、fetch all で取得します。
      </p>
    );
  }
  return (
    <table className="w-full border-collapse">
      <tbody className="divide-y divide-neutral-200 dark:divide-neutral-700">
        {novels.map((novel) => (
          <tr key={novel.id} className="align-top hover:bg-neutral-50 dark:hover:bg-neutral-800">
            <td className="py-2 pr-4 whitespace-nowrap tabular-nums">
              {novel.latestPublishedAt ? formatDateTime(novel.latestPublishedAt) : ""}
            </td>
            <td className="py-2 pr-4 whitespace-nowrap">
              <ExternalLink href={novel.url}>{uniqueId(novel)}</ExternalLink>
            </td>
            <td className="w-[20vw] py-2 pr-4">
              <AuthorLink name={novel.authorName} url={novel.authorUrl} />
            </td>
            <td className="py-2">
              <InternalLink to={novelPath(novel.id)}>{novel.title}</InternalLink>{" "}
              <PartsCount fetched={novel.fetched} total={novel.total} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Unadded({ items, error }: { items: UnaddedNovel[]; error: string | null }) {
  if (error == null && items.length === 0) {
    return null;
  }

  return (
    <section className="space-y-2">
      <h2 className="text-lg font-bold">未取得</h2>
      {error != null && <LoadError error={error} />}
      <table className="border-collapse">
        <tbody>
          {items.map((item) => (
            <tr key={uniqueId(item)}>
              <td className="py-1 pr-4">
                <ExternalLink href={item.url}>{uniqueId(item)}</ExternalLink>
              </td>
              <td className="py-1">
                <ActionButton action={() => addNovel(item.site, item.siteId)}>add</ActionButton>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
