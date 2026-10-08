import { Fragment } from "react";
import { ExternalLink } from "../components/ExternalLink";
import { EpisodeTitle, novelPath } from "../components/links";
import { ActionButton, ButtonGroup, InternalLink, PendingButton, SentMark } from "../components/ui";
import { latestEpisodes, refetchEpisode } from "../data";
import { useLoad } from "../hooks";
import { formatDateTime } from "../lib/format";
import { groupByDate } from "../lib/library";
import { uniqueId } from "../models";
import { LoadError } from "./Root";

const LIMIT = 100;

export function Latest() {
  const { data, error } = useLoad(() => latestEpisodes(LIMIT), []);
  if (error) {
    return <LoadError error={error} />;
  }
  if (data == null) {
    return null;
  }
  const groups = groupByDate(data);
  if (groups.length === 0) {
    return <p className="text-sm text-neutral-600 dark:text-neutral-400">話はまだありません。</p>;
  }

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
            {group.items.map((item) => (
              <tr
                key={item.episode.id}
                className="border-t border-neutral-200 align-top hover:bg-neutral-50 dark:border-neutral-700 dark:hover:bg-neutral-800"
              >
                <td className="py-2 pr-4 whitespace-nowrap tabular-nums">
                  {item.episode.publishedAt && formatDateTime(item.episode.publishedAt)}
                </td>
                <td className="py-2 pr-4 whitespace-nowrap">
                  <ExternalLink href={item.novelUrl}>{uniqueId(item)}</ExternalLink>
                </td>
                <td className="max-w-[16rem] py-2 pr-4">
                  <InternalLink to={novelPath(item.novelId)}>{item.novelTitle.slice(0, 20)}</InternalLink>
                </td>
                <td className="py-2 pr-4">
                  <EpisodeTitle episode={item.episode}>
                    {item.episode.no}　{item.episode.title}
                  </EpisodeTitle>
                </td>
                <td className="py-2">
                  <div className="flex items-center justify-end gap-2">
                    {item.episode.sentAt && <SentMark />}
                    <ButtonGroup>
                      <ActionButton action={() => refetchEpisode(item.episode.id)}>refetch</ActionButton>
                      {item.episode.bodyFetchedAt && <PendingButton>send to Kindle</PendingButton>}
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
