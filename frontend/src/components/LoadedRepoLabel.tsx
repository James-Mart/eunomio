import { useState } from "react";

import { Skeleton } from "@/components/ui/skeleton";
import { api } from "@/lib/api";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

export default function LoadedRepoLabel() {
  const [repo, setRepo] = useState<{ name: string; root: string } | null>(null);

  useAbortableEffect(async (signal) => {
    try {
      const info = await api.getRepoInfo();
      if (!signal.aborted) setRepo({ name: info.name, root: info.repoRoot });
    } catch {
      // Non-fatal; header stays without a repo label.
    }
  }, []);

  if (repo === null) {
    return <Skeleton className="h-4 w-32" aria-hidden="true" />;
  }

  return (
    <p
      className="max-w-[min(50vw,18rem)] truncate text-center text-sm text-muted-foreground justify-self-center"
      title={repo.root}
    >
      {repo.name}
    </p>
  );
}
