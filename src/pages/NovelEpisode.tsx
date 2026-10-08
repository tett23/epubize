import { useParams } from "react-router";
import { ExternalLink } from "../components/ExternalLink";
import { NormalizeOptionsForm, useNormalizeOptions } from "../components/NormalizeOptionsForm";
import { episodePath, novelPath } from "../components/links";
import { ActionButton, ButtonGroup, InternalLink, PendingButton, SentMark } from "../components/ui";
import { episodeDetail, novelDetail, refetchEpisode } from "../data";
import { useLoad } from "../hooks";
import { formatDate } from "../lib/format";
import { adjacentEpisodes } from "../lib/library";
import { episodeMarkdown, renderMarkdown } from "../lib/markdown";
import { uniqueId, type EpisodeDetail, type EpisodeSummary, type NovelDetail } from "../models";
import type { NormalizeOptions } from "../normalizeOptions";
import { LoadError } from "./Root";

export function NovelEpisode() {
  const params = useParams();
  const novelId = Number(params.novelId);
  const episodeId = Number(params.episodeId);
  const { data, error } = useLoad(
    () => Promise.all([novelDetail(novelId), episodeDetail(episodeId)]),
    [novelId, episodeId],
  );
  if (error) {
    return <LoadError error={error} />;
  }
  if (data === undefined) {
    return null;
  }
  const [novel, episode] = data;
  if (novel == null || episode == null || episode.novelId !== novel.id) {
    return <p>話が見つかりません。</p>;
  }
  return <Episode novel={novel} episode={episode} />;
}

function Episode({ novel, episode }: { novel: NovelDetail; episode: EpisodeDetail }) {
  const normalize = useNormalizeOptions(novel.id, novel.normalizeOptions);

  return (
    <div className="space-y-5">
      <h2 className="text-xl font-bold">
        <InternalLink to={novelPath(novel.id)}>{novel.title}</InternalLink>{" "}
        <ExternalLink href={novel.url}>{uniqueId(novel)}</ExternalLink>
      </h2>
      <h3 className="text-lg font-bold">
        <ExternalLink href={episode.url}>
          {episode.no}: {episode.title}
        </ExternalLink>{" "}
        {episode.sentAt && <SentMark />}
      </h3>
      <ButtonGroup>
        <ActionButton action={() => refetchEpisode(episode.id)}>refetch</ActionButton>
        <PendingButton>download zip</PendingButton>
        <PendingButton>download epub</PendingButton>
        <PendingButton>download mobi</PendingButton>
        <PendingButton>send to Kindle</PendingButton>
      </ButtonGroup>
      <p className="text-sm text-neutral-700 dark:text-neutral-300">
        {episode.revisedAt ?? episode.publishedAt
          ? `updated at: ${formatDate((episode.revisedAt ?? episode.publishedAt) as string)}`
          : null}
      </p>
      <EpisodeNav episodes={novel.episodes} no={episode.no} />
      <section className="space-y-2">
        <h4 className="text-sm text-neutral-700 dark:text-neutral-300">options:</h4>
        <NormalizeOptionsForm novelId={novel.id} {...normalize} />
      </section>
      {episode.body == null ? (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">本文はまだ取得していません。</p>
      ) : (
        <>
          <Preview episode={episode} options={normalize.options} />
          <pre className="overflow-x-auto rounded bg-neutral-100 p-4 text-sm whitespace-pre-wrap dark:bg-neutral-800">
            {episodeMarkdown(episode)}
          </pre>
        </>
      )}
    </div>
  );
}

function EpisodeNav({ episodes, no }: { episodes: EpisodeSummary[]; no: number }) {
  const { prev, next } = adjacentEpisodes(episodes, no);
  const link = (target: EpisodeSummary | null, label: string) =>
    target?.bodyFetchedAt ? (
      <InternalLink to={episodePath(target)}>{label}</InternalLink>
    ) : (
      <span className="text-neutral-400">{label}</span>
    );

  return (
    <nav className="flex gap-3">
      {link(prev, "Prev")}
      <span className="text-neutral-400">|</span>
      {link(next, "Next")}
    </nav>
  );
}

/** 本文の見た目の確認。整形の設定のうち、組方向だけを反映する */
function Preview({ episode, options }: { episode: EpisodeDetail; options: NormalizeOptions }) {
  const html = renderMarkdown(episodeMarkdown(episode), episode.images);
  const layout =
    options.direction === "vertical"
      ? "h-[40rem] w-full overflow-x-auto [writing-mode:vertical-rl]"
      : "max-w-[40rem] [writing-mode:horizontal-tb]";

  return (
    <div
      className={`novel-body rounded border border-neutral-200 p-6 font-serif leading-loose dark:border-neutral-700 ${layout}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
