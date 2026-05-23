/* SPDX-License-Identifier: Apache-2.0 */

import { useState } from "react";
import { RepoKindIcon } from "@/components/RepoKindIcon";
import { useMatch } from "react-router-dom";

import { Skeleton } from "@/components/ui/skeleton";
import { api } from "@/lib/api";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

export default function RepoBreadcrumb() {
  const match = useMatch("/sessions/:id");
  const sessionId = match?.params.id;
  const [repo, setRepo] = useState<{
    owner?: string;
    name: string;
    title?: string;
    isLocal: boolean;
    literalRemote: string;
  } | null | undefined>(undefined);

  useAbortableEffect(
    async (signal) => {
      if (!sessionId) {
        setRepo(undefined);
        return;
      }
      try {
        const session = await api.getSession(sessionId);
        if (signal.aborted) return;
        setRepo({
          owner: session.repoOwner,
          name: session.repoName,
          title: session.isLocal ? session.literalRemote : undefined,
          isLocal: session.isLocal,
          literalRemote: session.literalRemote,
        });
      } catch {
        if (!signal.aborted) setRepo(undefined);
      }
    },
    [sessionId],
  );

  if (repo === undefined || !sessionId) {
    return null;
  }

  if (repo === null) {
    return <Skeleton className="h-4 w-40" aria-hidden="true" />;
  }

  return (
    <nav aria-label="Repository" className="flex min-w-0 items-center gap-1.5 text-sm">
      <RepoKindIcon
        isLocal={repo.isLocal}
        remoteUrl={repo.literalRemote}
        className="h-4 w-4 shrink-0 text-muted-foreground"
      />
      {repo.owner && (
        <>
          <span className="truncate text-link">{repo.owner}</span>
          <span className="text-muted-foreground">/</span>
        </>
      )}
      <span className="truncate font-semibold text-foreground" title={repo.title}>
        {repo.name}
      </span>
    </nav>
  );
}
