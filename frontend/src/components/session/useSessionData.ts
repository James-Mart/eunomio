import { useCallback, useEffect, useMemo, useState } from "react";

import {
  api,
  type Graph,
  type Partition,
  type PhaseState,
} from "@/lib/api";
import { formatError } from "@/lib/errors";
import {
  useConstructSubscription,
  useHydratePartition,
} from "@/components/SessionEventsProvider";

import {
  canonicalLayout,
  candidateLayout,
  computeChain,
  type PhaseStatus,
  type SessionLayout,
} from "./layout";

export type SessionData = {
  graph: Graph | null;
  error: string | null;
  partitions: Partition[];
  candidatePartitionId: number | null;
  setCandidatePartitionId: (id: number | null) => void;
  candidatePartition: Partition | null;
  layout: SessionLayout | null;
  chain: ReturnType<typeof computeChain> | null;
  refresh: () => Promise<void>;
  refreshPartitions: () => Promise<void>;
  registerStartedPartition: (p: Partition) => void;
};

export function useSessionData(sessionId: string): SessionData {
  const [graph, setGraph] = useState<Graph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [partitions, setPartitions] = useState<Partition[]>([]);
  const [candidatePartitionId, setCandidatePartitionId] = useState<
    number | null
  >(null);

  const refresh = useCallback(async () => {
    try {
      const g = await api.getGraph(sessionId);
      setGraph(g);
    } catch (e) {
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

  useEffect(() => {
    if (candidatePartitionId === null) return;
    if (!partitions.some((p) => p.id === candidatePartitionId)) {
      setCandidatePartitionId(null);
    }
  }, [partitions, candidatePartitionId]);

  const chain = useMemo(() => (graph ? computeChain(graph) : null), [graph]);

  const candidatePartition = useMemo(
    () => partitions.find((p) => p.id === candidatePartitionId) ?? null,
    [partitions, candidatePartitionId],
  );

  const phaseStatusByNode = useMemo(() => {
    const m = new Map<string, PhaseStatus>();
    for (const p of partitions) {
      const existing = m.get(p.targetNodeId);
      const urgent = (s: PhaseState) => s !== "running";
      if (!existing || (!urgent(existing.phaseState) && urgent(p.phaseState))) {
        m.set(p.targetNodeId, { phase: p.phase, phaseState: p.phaseState });
      }
    }
    return m;
  }, [partitions]);

  const layout = useMemo<SessionLayout | null>(() => {
    if (!chain || !graph) return null;
    if (candidatePartition) {
      const lay = candidateLayout(chain, candidatePartition, graph);
      if (lay) return { kind: "candidate" as const, ...lay };
    }
    const lay = canonicalLayout(chain, phaseStatusByNode);
    return { kind: "canonical" as const, ...lay };
  }, [chain, graph, candidatePartition, phaseStatusByNode]);

  const registerStartedPartition = useCallback(
    (p: Partition) => {
      hydratePartition(p);
      setPartitions((prev) =>
        prev.some((x) => x.id === p.id) ? prev : [...prev, p],
      );
    },
    [hydratePartition],
  );

  return {
    graph,
    error,
    partitions,
    candidatePartitionId,
    setCandidatePartitionId,
    candidatePartition,
    layout,
    chain,
    refresh,
    refreshPartitions,
    registerStartedPartition,
  };
}
