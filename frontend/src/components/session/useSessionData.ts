/* SPDX-License-Identifier: Apache-2.0 */

import { useCallback, useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";

import {
  api,
  ApiError,
  type Graph,
  type Partition,
} from "@/lib/api";
import { formatError } from "@/lib/errors";
import {
  useAllPartitionLifecycles,
  useConstructSubscription,
  useHydratePartition,
} from "@/components/SessionEventsProvider";

import {
  canonicalLayout,
  candidateLayout,
  computeChain,
  originalLayout,
  partitionGlanceByNode,
  type SessionLayout,
  type View,
} from "./layout";

export type SessionData = {
  graph: Graph | null;
  notFound: boolean;
  error: string | null;
  partitions: Partition[];
  view: View;
  setView: Dispatch<SetStateAction<View>>;
  candidatePartition: Partition | null;
  layout: SessionLayout | null;
  chain: ReturnType<typeof computeChain> | null;
  refresh: () => Promise<void>;
  refreshPartitions: () => Promise<void>;
  registerStartedPartition: (p: Partition) => void;
  setNodeReviewed: (nodeId: string, reviewed: boolean) => void;
};

export function useSessionData(sessionId: string): SessionData {
  const [graph, setGraph] = useState<Graph | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [partitions, setPartitions] = useState<Partition[]>([]);
  const [view, setView] = useState<View>({ kind: "canonical" });

  const refresh = useCallback(async () => {
    try {
      const g = await api.getGraph(sessionId);
      setGraph(g);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        setNotFound(true);
        return;
      }
      setError(formatError(e, "Failed to load graph"));
    }
  }, [sessionId]);

  const refreshPartitions = useCallback(async () => {
    try {
      const list = await api.listPartitions(sessionId);
      setPartitions(list);
    } catch {
      setPartitions([]);
    }
  }, [sessionId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    void refreshPartitions();
  }, [refreshPartitions]);

  useConstructSubscription(
    useCallback(() => {
      void refreshPartitions();
      void refresh();
    }, [refresh, refreshPartitions]),
  );

  const hydratePartition = useHydratePartition();
  const lifecycles = useAllPartitionLifecycles();

  useEffect(() => {
    if (view.kind !== "candidate") return;
    if (!partitions.some((p) => p.id === view.partitionId)) {
      setView({ kind: "canonical" });
    }
  }, [partitions, view]);

  const chain = useMemo(() => (graph ? computeChain(graph) : null), [graph]);

  const candidatePartition = useMemo(
    () =>
      view.kind === "candidate"
        ? partitions.find((p) => p.id === view.partitionId) ?? null
        : null,
    [partitions, view],
  );

  const partitionGlanceByNodeId = useMemo(
    () => partitionGlanceByNode(partitions, lifecycles),
    [partitions, lifecycles],
  );

  const layout = useMemo<SessionLayout | null>(() => {
    if (!chain || !graph) return null;
    if (view.kind === "candidate" && candidatePartition) {
      const lay = candidateLayout(chain, candidatePartition, graph);
      if (lay) return { kind: "candidate" as const, ...lay };
      return null;
    }
    if (view.kind === "original") {
      const lay = originalLayout(chain);
      if (lay) return { kind: "original" as const, ...lay };
    }
    const lay = canonicalLayout(chain, partitionGlanceByNodeId);
    return { kind: "canonical" as const, ...lay };
  }, [chain, graph, view, candidatePartition, partitionGlanceByNodeId]);

  const registerStartedPartition = useCallback(
    (p: Partition) => {
      hydratePartition(p);
      setPartitions((prev) =>
        prev.some((x) => x.id === p.id) ? prev : [...prev, p],
      );
    },
    [hydratePartition],
  );

  const setNodeReviewed = useCallback(
    (nodeId: string, reviewed: boolean) => {
      setGraph((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          nodes: prev.nodes.map((n) =>
            n.nodeId === nodeId ? { ...n, reviewed } : n,
          ),
        };
      });
      api.setNodeReviewed(sessionId, nodeId, reviewed).catch(() => {
        setGraph((prev) => {
          if (!prev) return prev;
          return {
            ...prev,
            nodes: prev.nodes.map((n) =>
              n.nodeId === nodeId ? { ...n, reviewed: !reviewed } : n,
            ),
          };
        });
      });
    },
    [sessionId],
  );

  return {
    graph,
    notFound,
    error,
    partitions,
    view,
    setView,
    candidatePartition,
    layout,
    chain,
    refresh,
    refreshPartitions,
    registerStartedPartition,
    setNodeReviewed,
  };
}
