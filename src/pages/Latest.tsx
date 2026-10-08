import { Fragment } from "react";
import { EpisodeTitle, NovelSiteLink, novelPath } from "../components/links";
import { ButtonGroup, InternalLink, PendingButton, SentMark } from "../components/ui";
import { listNovels } from "../data";
import { formatDateTime } from "../lib/format";
import { latestEpisodes } from "../lib/library";
import { uniqueId } from "../models";

const LIMIT = 100;

export function Latest() {
  const groups = latestEpisodes(listNovels(), LIMIT);

  return (
    <table className="w-full border-collapse">
      <tbody>
        {groups.map((group) => (
          <Fragment key={group.date}>
            <tr>
              <th colSpan={5} className="pt-5 pb-1 text-left text-base font-bold first:pt-0">
                {group.date}
              </th>
            </tr>
            {group.items.map(({ novel, episode }) => (
              <tr
                key={episode.id}
                className="border-t border-neutral-200 align-top hover:bg-neutral-50 dark:border-neutral-700 dark:hover:bg-neutral-800"
              >
                <td className="py-2 pr-4 whitespace-nowrap tabular-nums">{formatDateTime(episode.originCreatedAt)}</td>
                <td className="py-2 pr-4 whitespace-nowrap">
                  <NovelSiteLink novel={novel}>{uniqueId(novel)}</NovelSiteLink>
                </td>
                <td className="max-w-[16rem] py-2 pr-4">
                  <InternalLink to={novelPath(novel)}>{novel.title.slice(0, 20)}</InternalLink>
                </td>
                <td className="py-2 pr-4">
                  <EpisodeTitle episode={episode}>
                    {episode.part}　{episode.title}
                  </EpisodeTitle>
                </td>
                <td className="py-2">
                  <div className="flex items-center justify-end gap-2">
                    {episode.isSent && <SentMark />}
                    <ButtonGroup>
                      <PendingButton>refetch</PendingButton>
                      {episode.bodyFetched && <PendingButton>send to Kindle</PendingButton>}
                    </ButtonGroup>
                  </div>
                </td>
              </tr>
            ))}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}
