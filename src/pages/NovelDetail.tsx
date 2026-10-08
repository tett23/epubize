import { Fragment } from "react";
import { useParams } from "react-router";
import { NormalizeOptionsForm } from "../components/NormalizeOptionsForm";
import { AuthorLink, EpisodeTitle, NovelSiteLink } from "../components/links";
import { ButtonGroup, PendingButton, SentMark } from "../components/ui";
import { findNovel } from "../data";
import { formatDate, formatDateTime } from "../lib/format";
import type { Novel } from "../models";

export function NovelDetail() {
  const { novelId } = useParams();
  const novel = findNovel(Number(novelId));
  if (novel == null) {
    return <p>作品が見つかりません。</p>;
  }

  return (
    <div className="space-y-5">
      <div className="space-y-2">
        <h2 className="text-xl font-bold">
          <NovelSiteLink novel={novel}>{novel.title}</NovelSiteLink>
        </h2>
        <h3 className="font-bold">
          <AuthorLink author={novel.author} />
        </h3>
      </div>
      <p className="max-w-4xl leading-relaxed whitespace-pre-line">{novel.description}</p>
      <p className="flex gap-4 text-sm text-neutral-700 dark:text-neutral-300">
        <span>updated at: {formatDate(novel.episodeUpdatedAt)}</span>
        <span>{novel.characterCount.toLocaleString()}文字</span>
        {novel.isConcluded && <span>完結済み</span>}
      </p>
      <ButtonGroup>
        <PendingButton>fetch</PendingButton>
        <PendingButton>remove episodes</PendingButton>
        <PendingButton>download zip</PendingButton>
        <PendingButton>download epub</PendingButton>
        <PendingButton>download mobi</PendingButton>
        <PendingButton>send to Kindle</PendingButton>
      </ButtonGroup>
      <NormalizeOptionsForm novelId={novel.id} />
      <Episodes novel={novel} />
    </div>
  );
}

function Episodes({ novel }: { novel: Novel }) {
  return (
    <table className="border-collapse">
      <tbody>
        <tr>
          <th colSpan={3} className="pb-1 text-left font-bold">
            {novel.title}
          </th>
        </tr>
        {novel.chapters.map((chapter, i) => (
          <Fragment key={i}>
            {chapter.title != null && (
              <tr>
                <th colSpan={3} className="pt-4 pb-1 pl-2 text-left font-bold">
                  {chapter.title}
                </th>
              </tr>
            )}
            {chapter.episodes.map((episode) => (
              <tr key={episode.id} className="border-t border-neutral-200 dark:border-neutral-700">
                <td className="py-1.5 pr-4 pl-2 whitespace-nowrap tabular-nums">
                  {formatDateTime(episode.originCreatedAt)}
                </td>
                <td className="py-1.5 pr-4 text-right tabular-nums">{episode.part}</td>
                <td className="py-1.5">
                  <EpisodeTitle episode={episode}>{episode.title}</EpisodeTitle> {episode.isSent && <SentMark />}
                </td>
              </tr>
            ))}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}
