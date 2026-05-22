import { useState } from "react";
import { RepoIcon } from "@primer/octicons-react";
import { useMatch } from "react-router-dom";

import { Skeleton } from "@/components/ui/skeleton";
import { api } from "@/lib/api";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

export default function RepoBreadcrumb() {
  const [repo, setRepo] = useState<{
    name: string;
    repoRoot: string;
    owner?: string;
    currentBranch?: string;
  } | null>(null);
  const onSessionRoute = useMatch("/sessions/:id");

  useAbortableEffect(async (signal) => {
    try {
      const info = await api.getRepoInfo();
      if (!signal.aborted) {
        setRepo({
          name: info.name,
          repoRoot: info.repoRoot,
          owner: info.owner,
          currentBranch: info.currentBranch,
        });
      }
    } catch {
      // Non-fatal; header stays without a repo label.
    }
  }, []);

  if (repo === null) {
    return <Skeleton className="h-4 w-40" aria-hidden="true" />;
  }

  const showBranch = !onSessionRoute && repo.currentBranch;

  return (
    <nav aria-label="Repository" className="flex min-w-0 items-center gap-1.5 text-sm">
      <RepoIcon className="h-4 w-4 shrink-0 text-muted-foreground" />
      {repo.owner && (
        <>
          <span className="truncate text-link">{repo.owner}</span>
          <span className="text-muted-foreground">/</span>
        </>
      )}
      <span className="truncate font-semibold text-foreground" title={repo.repoRoot}>
        {repo.name}
      </span>
      {showBranch && (
        <span className="hidden truncate font-mono text-muted-foreground sm:inline">
          · {repo.currentBranch}
        </span>
      )}
    </nav>
  );
}
