import { useState } from "react";
import { ExternalLink } from "../components/ExternalLink";
import { AuthorLink, NovelSiteLink, novelPath } from "../components/links";
import { ButtonGroup, InternalLink, PartsCount, PendingButton } from "../components/ui";
import { listNovels, listUnadded } from "../data";
import { formatDateTime } from "../lib/format";
import { libraryStats, sortNovels } from "../lib/library";
import { partsOf, uniqueId, type Novel } from "../models";

export function Root() {
  const novels = sortNovels(listNovels());

  return (
    <div className="space-y-6">
      <AddNovelForm />
      <ButtonGroup>
        <PendingButton>fetch all</PendingButton>
        <PendingButton>fetch all metadata</PendingButton>
      </ButtonGroup>
      <Stats novels={novels} />
      <Novels novels={novels} />
      <Unadded />
    </div>
  );
}

function AddNovelForm() {
  const [url, setUrl] = useState("");

  return (
    <form className="flex gap-2" onSubmit={(e) => e.preventDefault()}>
      <input
        type="url"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="作品の URL"
        className="w-96 max-w-full rounded border border-neutral-300 px-2 py-1 dark:border-neutral-600 dark:bg-neutral-800"
      />
      <PendingButton>add</PendingButton>
    </form>
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

function Unadded() {
  const items = listUnadded();
  if (items.length === 0) {
    return null;
  }

  return (
    <section className="space-y-2">
      <h2 className="text-lg font-bold">未取得</h2>
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
