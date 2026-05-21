import { useEffect, useMemo, useRef, useState } from "react";

import type { Graph, GraphNode, Partition } from "@/lib/api";

import {
  CANDIDATE_SLICE_ID,
  CANDIDATE_TARGET_PREFIX,
  findLeafNodeId,
  type SessionLayout,
  type View,
} from "./layout";

export type SessionSelection = {
  selectedNodeId: string | null;
  setSelectedNodeId: (id: string | null) => void;
  selectedCanonicalNode: GraphNode | null;
  isCandidateSliceSelected: boolean;
};

export function useSessionSelection(
  graph: Graph | null,
  layout: SessionLayout | null,
  view: View,
  candidatePartition: Partition | null,
): SessionSelection {
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  const didInitRef = useRef(false);
  useEffect(() => {
    if (!graph || didInitRef.current) return;
    didInitRef.current = true;
    setSelectedNodeId(findLeafNodeId(graph));
  }, [graph]);

  useEffect(() => {
    if (!layout || !selectedNodeId) return;
    if (layout.nodes.some((n) => n.id === selectedNodeId)) return;
    if (selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)) {
      const stripped = selectedNodeId.slice(CANDIDATE_TARGET_PREFIX.length);
      if (layout.nodes.some((n) => n.id === stripped)) {
        setSelectedNodeId(stripped);
        return;
      }
    }
    if (layout.kind === "candidate" && !selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)) {
      const prefixed = CANDIDATE_TARGET_PREFIX + selectedNodeId;
      if (layout.nodes.some((n) => n.id === prefixed)) {
        setSelectedNodeId(prefixed);
        return;
      }
    }
    setSelectedNodeId(null);
  }, [layout, selectedNodeId]);

  const prevStageRef = useRef<"pending" | "proposed" | null>(null);
  const prevPartitionIdRef = useRef<number | null>(null);

  useEffect(() => {
    if (view.kind !== "candidate" || !candidatePartition || layout?.kind !== "candidate") {
      prevStageRef.current =
        layout?.kind === "candidate" ? layout.stage : null;
      prevPartitionIdRef.current =
        view.kind === "candidate" ? view.partitionId : null;
      return;
    }

    const stage = layout.stage;
    const partitionId = view.partitionId;
    const prevStage = prevStageRef.current;
    const prevPartitionId = prevPartitionIdRef.current;

    if (prevPartitionId === partitionId && prevStage !== null) {
      if (prevStage === "pending" && stage === "proposed") {
        setSelectedNodeId(CANDIDATE_SLICE_ID);
      } else if (prevStage === "proposed" && stage === "pending") {
        setSelectedNodeId(candidatePartition.targetNodeId);
      }
    }

    prevStageRef.current = stage;
    prevPartitionIdRef.current = partitionId;
  }, [view, candidatePartition, layout]);

  const selectedCanonicalNode = useMemo<GraphNode | null>(() => {
    if (!graph || !selectedNodeId) return null;
    const resolved = selectedNodeId.startsWith(CANDIDATE_TARGET_PREFIX)
      ? selectedNodeId.slice(CANDIDATE_TARGET_PREFIX.length)
      : selectedNodeId;
    return graph.nodes.find((n) => n.nodeId === resolved) ?? null;
  }, [graph, selectedNodeId]);

  const isCandidateSliceSelected =
    layout?.kind === "candidate" &&
    layout.stage === "proposed" &&
    selectedNodeId === CANDIDATE_SLICE_ID;

  return {
    selectedNodeId,
    setSelectedNodeId,
    selectedCanonicalNode,
    isCandidateSliceSelected,
  };
}
