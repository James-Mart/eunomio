/* SPDX-License-Identifier: Apache-2.0 */

import { useCallback, useEffect, useMemo, useState } from "react";

import { api } from "@/lib/api";

export function useEdgeFileViewed(
  sessionId: string | null,
  targetNodeId: string | null,
) {
  const [paths, setPaths] = useState<string[]>([]);

  useEffect(() => {
    if (!sessionId || !targetNodeId) {
      setPaths([]);
      return;
    }
    let cancelled = false;
    api
      .getEdgeViewedFiles(sessionId, targetNodeId)
      .then((r) => {
        if (!cancelled) setPaths(r.paths);
      })
      .catch(() => {
        if (!cancelled) setPaths([]);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, targetNodeId]);

  const viewedPaths = useMemo(() => new Set(paths), [paths]);

  const toggleViewed = useCallback(
    (filePath: string, viewed: boolean) => {
      if (!sessionId || !targetNodeId) return;
      setPaths((prev) => {
        const next = new Set(prev);
        if (viewed) next.add(filePath);
        else next.delete(filePath);
        return [...next].sort();
      });
      api
        .setEdgeFileViewed(sessionId, targetNodeId, filePath, viewed)
        .catch(() => {
          setPaths((prev) => {
            const next = new Set(prev);
            if (viewed) next.delete(filePath);
            else next.add(filePath);
            return [...next].sort();
          });
        });
    },
    [sessionId, targetNodeId],
  );

  return { viewedPaths, toggleViewed };
}
