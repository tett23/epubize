import type { ReactNode } from "react";
import type { EpisodeSummary } from "../models";
import { ExternalLink } from "./ExternalLink";
import { InternalLink } from "./ui";

/** 作者のページ。サイトが示さない場合は名前だけ */
export function AuthorLink({ name, url }: { name: string; url: string | null }) {
  if (url == null) {
    return <>{name}</>;
  }
  return <ExternalLink href={url}>{name}</ExternalLink>;
}

export function novelPath(novelId: number): string {
  return `/novels/${novelId}`;
}

export function episodePath(episode: Pick<EpisodeSummary, "novelId" | "id">): string {
  return `/novels/${episode.novelId}/${episode.id}`;
}

/** 話の題名。本文を取得済みなら話のページへのリンクにする */
export function EpisodeTitle({ episode, children }: { episode: EpisodeSummary; children: ReactNode }) {
  if (episode.bodyFetchedAt == null) {
    return <>{children}</>;
  }
  return <InternalLink to={episodePath(episode)}>{children}</InternalLink>;
}
