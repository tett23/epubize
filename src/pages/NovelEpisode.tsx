import { useParams } from "react-router";
import { ExternalLink } from "../components/ExternalLink";
import { NormalizeOptionsForm } from "../components/NormalizeOptionsForm";
import { NovelSiteLink, episodePath, novelPath } from "../components/links";
import { ButtonGroup, InternalLink, PendingButton, SentMark } from "../components/ui";
import { findNovel } from "../data";
import { formatDate } from "../lib/format";
import { adjacentEpisodes } from "../lib/library";
import { episodeMarkdown, renderMarkdown } from "../lib/markdown";
import { episodesOf, uniqueId, type Episode, type Novel } from "../models";
import { useNormalizeOptions } from "../normalizeOptions";

export function NovelEpisode() {
  const { novelId, episodeId } = useParams();
  const novel = findNovel(Number(novelId));
  const episode = novel && episodesOf(novel).find((item) => item.id === Number(episodeId));
  if (novel == null || episode == null) {
    return <p>話が見つかりません。</p>;
  }

  return (
    <div className="space-y-5">
      <h2 className="text-xl font-bold">
        <InternalLink to={novelPath(novel)}>{novel.title}</InternalLink>{" "}
        <NovelSiteLink novel={novel}>{uniqueId(novel)}</NovelSiteLink>
      </h2>
      <h3 className="text-lg font-bold">
        <ExternalLink href={episode.url}>
          {episode.part}: {episode.title}
        </ExternalLink>{" "}
        {episode.isSent && <SentMark />}
      </h3>
      <ButtonGroup>
        <PendingButton>refetch</PendingButton>
        <PendingButton>download zip</PendingButton>
        <PendingButton>download epub</PendingButton>
        <PendingButton>download mobi</PendingButton>
        <PendingButton>send to Kindle</PendingButton>
      </ButtonGroup>
      <p className="text-sm text-neutral-700 dark:text-neutral-300">updated at: {formatDate(episode.updatedAt)}</p>
      <EpisodeNav novel={novel} episode={episode} />
      <section className="space-y-2">
        <h4 className="text-sm text-neutral-700 dark:text-neutral-300">options:</h4>
        <NormalizeOptionsForm novelId={novel.id} />
      </section>
      <Preview episode={episode} />
      <pre className="overflow-x-auto rounded bg-neutral-100 p-4 text-sm whitespace-pre-wrap dark:bg-neutral-800">
        {episodeMarkdown(episode)}
      </pre>
    </div>
  );
}

function EpisodeNav({ novel, episode }: { novel: Novel; episode: Episode }) {
  const { prev, next } = adjacentEpisodes(novel, episode.part);
  const link = (target: Episode | null, label: string) =>
    target?.bodyFetched ? (
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
function Preview({ episode }: { episode: Episode }) {
  const [options] = useNormalizeOptions(episode.novelId);
  const html = renderMarkdown(episodeMarkdown(episode));
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
