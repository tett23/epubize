import type { ReactNode } from "react";
import type { Author, Episode, Novel } from "../models";
import { ExternalLink } from "./ExternalLink";
import { InternalLink } from "./ui";

/** 作品のサイト上のページ */
export function NovelSiteLink({ novel, children }: { novel: Novel; children: ReactNode }) {
  return <ExternalLink href={novel.url}>{children}</ExternalLink>;
}

/** 作者のページ。サイトが示さない場合は名前だけ */
export function AuthorLink({ author }: { author: Author }) {
  if (author.url == null) {
    return <>{author.name}</>;
  }
  return <ExternalLink href={author.url}>{author.name}</ExternalLink>;
}

export function novelPath(novel: Novel): string {
  return `/novels/${novel.id}`;
}

export function episodePath(episode: Episode): string {
  return `/novels/${episode.novelId}/${episode.id}`;
}

/** 話の題名。本文を取得済みなら話のページへのリンクにする */
export function EpisodeTitle({ episode, children }: { episode: Episode; children: ReactNode }) {
  if (!episode.bodyFetched) {
    return <>{children}</>;
  }
  return <InternalLink to={episodePath(episode)}>{children}</InternalLink>;
}
