import { Fragment } from "react";
import { useParams } from "react-router";
import { DownloadEpubButton, DownloadZipButton, SendToKindleButton } from "../components/ExportButtons";
import { ExternalLink } from "../components/ExternalLink";
import { AuthorLink, EpisodeTitle } from "../components/links";
import { NormalizeOptionsForm, useNormalizeOptions } from "../components/NormalizeOptionsForm";
import { ActionButton, ButtonGroup, SentMark } from "../components/ui";
import { fetchNovel, novelDetail, removeEpisodes } from "../data";
import { useLoad } from "../hooks";
import { formatDate, formatDateTime } from "../lib/format";
import { groupChapters } from "../lib/library";
import type { NovelDetail as NovelDetailData } from "../models";
import { LoadError } from "./Root";

export function NovelDetail() {
  const novelId = Number(useParams().novelId);
  const { data, error, reload } = useLoad(() => novelDetail(novelId), [novelId]);
  if (error) {
    return <LoadError error={error} />;
  }
  if (data === undefined) {
    return null;
  }
  if (data === null) {
    return <p>作品が見つかりません。</p>;
  }
  return <Novel novel={data} reload={reload} />;
}

function Novel({ novel, reload }: { novel: NovelDetailData; reload: () => void }) {
  const normalize = useNormalizeOptions(novel.id, novel.normalizeOptions);

  return (
    <div className="space-y-5">
      <div className="space-y-2">
        <h2 className="text-xl font-bold">
          <ExternalLink href={novel.url}>{novel.title}</ExternalLink>
        </h2>
        <h3 className="font-bold">
          <AuthorLink name={novel.authorName} url={novel.authorUrl} />
        </h3>
      </div>
      <p className="max-w-4xl leading-relaxed whitespace-pre-line">{novel.description}</p>
      <p className="flex gap-4 text-sm text-neutral-700 dark:text-neutral-300">
        {novel.latestPublishedAt && <span>updated at: {formatDate(novel.latestPublishedAt)}</span>}
        <span>{novel.charCount.toLocaleString()}文字</span>
        {novel.isConcluded && <span>完結済み</span>}
      </p>
      <ButtonGroup>
        <ActionButton action={() => fetchNovel(novel.id)}>fetch</ActionButton>
        <ActionButton
          action={async () => {
            await removeEpisodes(novel.id);
            reload();
          }}
          confirm="この作品の話を全て消します。取得した本文も消えます。よろしいですか？"
        >
          remove episodes
        </ActionButton>
        <DownloadZipButton scope={{ kind: "novel", id: novel.id }} />
        <DownloadEpubButton scope={{ kind: "novel", id: novel.id }} />
        <SendToKindleButton
          scope={{ kind: "novel", id: novel.id }}
          confirm={`本文を取得済みの ${fetchedCount(novel)} 話を 1 冊にして Kindle に送ります。よろしいですか？`}
          onSent={reload}
        />
      </ButtonGroup>
      <NormalizeOptionsForm novelId={novel.id} {...normalize} />
      <Episodes novel={novel} reload={reload} />
    </div>
  );
}

function fetchedCount(novel: NovelDetailData): number {
  return novel.episodes.filter((episode) => episode.bodyFetchedAt != null).length;
}

function Episodes({ novel, reload }: { novel: NovelDetailData; reload: () => void }) {
  return (
    <table className="border-collapse">
      <tbody>
        <tr>
          <th colSpan={4} className="pb-1 text-left font-bold">
            {novel.title}
          </th>
        </tr>
        {groupChapters(novel.episodes).map((chapter) => (
          <Fragment key={chapter.episodes[0]?.id ?? chapter.title}>
            {chapter.title != null && (
              <tr>
                <th colSpan={4} className="pt-4 pb-1 pl-2 text-left font-bold">
                  {chapter.title}
                </th>
              </tr>
            )}
            {chapter.episodes.map((episode) => (
              <tr key={episode.id} className="border-t border-neutral-200 dark:border-neutral-700">
                <td className="py-1.5 pr-4 pl-2 whitespace-nowrap tabular-nums">
                  {episode.publishedAt && formatDateTime(episode.publishedAt)}
                </td>
                <td className="py-1.5 pr-4 text-right tabular-nums">{episode.no}</td>
                <td className="py-1.5">
                  <EpisodeTitle episode={episode}>{episode.title}</EpisodeTitle> {episode.sentAt && <SentMark />}
                </td>
                <td className="py-1.5 pl-4">
                  {/* 本文を取得済みの話だけ（ADR 0025） */}
                  {episode.bodyFetchedAt && (
                    <ButtonGroup>
                      <DownloadEpubButton scope={{ kind: "episode", id: episode.id }} />
                      <SendToKindleButton scope={{ kind: "episode", id: episode.id }} onSent={reload} />
                    </ButtonGroup>
                  )}
                </td>
              </tr>
            ))}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}
