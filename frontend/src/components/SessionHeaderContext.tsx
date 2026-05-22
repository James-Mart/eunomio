import { useState } from "react";
import { useMatch } from "react-router-dom";

import { Skeleton } from "@/components/ui/skeleton";
import { api, type Session } from "@/lib/api";
import { useAbortableEffect } from "@/lib/useAbortableEffect";

export default function SessionHeaderContext() {
  const onSettings = useMatch("/settings");
  const match = useMatch("/sessions/:id");
  const sessionId = match?.params.id;
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(false);

  useAbortableEffect(
    async (signal) => {
      if (!sessionId) {
        setSession(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      try {
        const row = await api.getSession(sessionId);
        if (!signal.aborted) setSession(row);
      } catch {
        if (!signal.aborted) setSession(null);
      } finally {
        if (!signal.aborted) setLoading(false);
      }
    },
    [sessionId],
  );

  if (onSettings || !sessionId) {
    return null;
  }

  if (loading && session === null) {
    return <Skeleton className="hidden h-4 w-48 sm:block" aria-hidden="true" />;
  }

  if (!session) {
    return null;
  }

  return (
    <span className="hidden min-w-0 items-center gap-1.5 text-sm text-muted-foreground sm:flex">
      <span className="truncate font-mono">{session.baseRef}</span>
      <span aria-hidden>←</span>
      <span className="truncate font-mono text-foreground">{session.sourceRef}</span>
    </span>
  );
}
